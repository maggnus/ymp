#![forbid(unsafe_code)]

//! Acceptance: a running participant recruits another one through the kernel's mechanical gate,
//! and nothing else about that request is decided anywhere.
//!
//! * The positive half: one request for a frozen, live entry under a budget that still holds a
//!   start admits exactly one participant, records it in the journal beside the facts that paid for
//!   it, charges the authority, and hands the participant to the one managed start path.
//! * Each of the five gates refuses in its own words, and **a refusal writes nothing**: the journal
//!   file is required to be byte-for-byte what it was, which is what makes a refusal something a
//!   proposer may retry rather than something it has already been charged for.
//! * Competing requests are serialized by the run's single writer: the last participant-start and
//!   the last place under the ceiling are each taken once, and a repeat of one request admits
//!   nobody a second time.
//! * The record is what a restart rebuilds from: reopening the store gives back a run holding the
//!   admitted participant and a ledger in which the authority it cost has been spent.
//!
//! Nothing here starts a process or reaches a network. The measurement that says whether an entry
//! is live and the path that starts a participant are the two boundaries recruitment stands on, and
//! both are stated by fixtures, so what this file measures is the gate and nothing around it.

use std::fs;
use std::path::Path;

use tempfile::{TempDir, tempdir};
use ymp_application::{Application, ApplicationConfig, ApplicationError, ParticipantAdmission};
use ymp_domain::commitment::{
    BudgetVector, CommitmentEvent, CommitmentLedger, Dimension, StopReason, StopRun,
};
use ymp_domain::commitment::{CommitmentCommand, RegisterParticipant};
use ymp_domain::pool::{EntryIdentity, FrozenEntry};
use ymp_domain::recruitment::{
    Gate, ParticipantStartPath, ProposerState, RecruitmentPolicy, RecruitmentRefusal,
    RequestParticipant,
};
use ymp_domain::{Budget, Command, EventKind};
use ymp_testkit::recruitment::{MeasuredHost, RecordedStarts, route};

const ROOT: &str = "participant-root";
const ROOT_PRINCIPAL: &str = "anthropic";
const ROOT_OBLIGATION: &str = "obligation-root";

fn opus() -> EntryIdentity {
    EntryIdentity::new("anthropic", "claude-code", "claude-opus-5")
}

fn sonnet() -> EntryIdentity {
    EntryIdentity::new("anthropic", "claude-code", "claude-sonnet-5")
}

/// An entry the pool permits and admission did not find live when the run was created.
fn held_back() -> EntryIdentity {
    EntryIdentity::new("openai", "codex", "gpt-5")
}

/// An entry no pool of this run ever held.
fn foreign() -> EntryIdentity {
    EntryIdentity::new("openai", "codex", "gpt-5-mini")
}

/// A run whose pool is frozen to two live Anthropic entries and one the catalog held back, with a
/// commitment kernel whose root participant holds `starts` participant-starts and as many slices.
fn run(starts: u64, policy: RecruitmentPolicy) -> (TempDir, Application) {
    let temporary = tempdir().expect("temporary directory");
    let mut application = Application::create_with_config(
        temporary.path(),
        "run-1",
        Budget::new(2, 1),
        ApplicationConfig {
            recruitment: policy,
            ..ApplicationConfig::default()
        },
    )
    .expect("create the run");
    application
        .execute(
            "ymp.pool.freeze",
            Command::FreezePool {
                pool: "default".to_owned(),
                entries: vec![
                    FrozenEntry::admissible("anthropic", "claude-code", "claude-opus-5"),
                    FrozenEntry::admissible("anthropic", "claude-code", "claude-sonnet-5"),
                    FrozenEntry::unavailable(
                        "openai",
                        "codex",
                        "gpt-5",
                        "the account states no credential",
                    ),
                ],
                digest: "b".repeat(64),
            },
        )
        .expect("freeze the pool");
    application
        .open_commitment_kernel(
            "kernel",
            ROOT,
            ROOT_PRINCIPAL,
            ROOT_OBLIGATION,
            BudgetVector::ZERO
                .with(Dimension::ParticipantStarts, starts)
                .with(Dimension::InvocationStarts, starts),
        )
        .expect("open the commitment kernel");
    (temporary, application)
}

fn request(request_id: &str, entry: EntryIdentity) -> RequestParticipant {
    RequestParticipant::new(request_id, ROOT, entry)
}

/// The bytes of the journal as they stand on disk. A refusal is required to leave them untouched.
fn journal(store: &Path) -> Vec<u8> {
    fs::read(store.join("events.jsonl")).expect("read the journal")
}

fn admissions_in_journal(application: &Application) -> Vec<EventKind> {
    application
        .events_after(0)
        .expect("committed records")
        .into_iter()
        .map(|envelope| envelope.event)
        .filter(|event| matches!(event, EventKind::ParticipantAdmitted { .. }))
        .collect()
}

fn balance(ledger: &CommitmentLedger, participant: &str, dimension: Dimension) -> u64 {
    ledger
        .participants()
        .get(participant)
        .map_or(0, |record| record.balance.get(dimension))
}

/// The whole positive half of the card, measured on one request.
#[test]
fn one_request_admits_one_participant_records_it_charges_it_and_starts_it() {
    let (temporary, mut application) = run(2, RecruitmentPolicy::WORKING);
    let host = MeasuredHost::serving(&[opus(), sonnet()]);
    let mut starts = RecordedStarts::new();

    let admission = application
        .request_participant(&request("request-1", sonnet()), &host, &mut starts)
        .expect("the request is admitted");

    // Admitted on the entry the proposer named. The pool's first live entry is the Opus one, and
    // nothing here reaches for it: a request that names the second live entry gets the second.
    assert_eq!(admission.admitted.entry, sonnet());
    assert_eq!(admission.admitted.proposer, ROOT);
    assert_eq!(admission.admitted.profile, "claude-code");
    assert_eq!(admission.admitted.route, route(&sonnet()));
    assert_eq!(
        admission.admitted.workspace,
        format!("participants/{}", admission.admitted.participant_id)
    );
    assert!(admission.start_failure.is_none());

    // Exactly one participant was handed to the one managed start path, and it is the one the
    // record names.
    assert_eq!(starts.started().len(), 1);
    assert_eq!(starts.started()[0], admission.admitted);

    // The journal carries one admission, and it carries the facts that paid for it in the same
    // record: a reader either holds the whole admission or has never heard of it.
    let recorded = admissions_in_journal(&application);
    assert_eq!(recorded.len(), 1);
    let EventKind::ParticipantAdmitted { admitted, facts } = &recorded[0] else {
        panic!("the record is not an admission");
    };
    assert_eq!(admitted, &admission.admitted);
    assert_eq!(facts, &admission.facts);
    assert!(facts.iter().any(|fact| matches!(
        fact,
        CommitmentEvent::ParticipantRegistered { participant_id, .. }
            if participant_id == &admission.admitted.participant_id
    )));

    // The start was charged: one unit of the authority to start a participant left the accounts for
    // good, and the offer-stage allowance moved to the participant it admitted.
    let ledger = application.commitments().expect("the run holds a kernel");
    assert_eq!(ledger.consumed().get(Dimension::ParticipantStarts), 1);
    assert_eq!(balance(ledger, ROOT, Dimension::ParticipantStarts), 1);
    assert_eq!(
        balance(
            ledger,
            &admission.admitted.participant_id,
            Dimension::InvocationStarts
        ),
        1
    );

    // The record is what a restart rebuilds from, not the memory of this process.
    drop(application);
    let reopened = Application::open(temporary.path()).expect("reopen the store");
    assert_eq!(
        reopened.admissions(),
        std::slice::from_ref(&admission.admitted)
    );
    let ledger = reopened.commitments().expect("the rebuilt kernel");
    assert_eq!(ledger.consumed().get(Dimension::ParticipantStarts), 1);
    assert!(
        ledger
            .participants()
            .contains_key(&admission.admitted.participant_id)
    );
}

/// Each gate refuses in its own words, and a refusal leaves the store byte for byte as it was.
#[test]
fn every_gate_refuses_in_plain_words_and_writes_nothing() {
    // Gate 1: an entry the run's frozen pool never held, and one it held but admission did not
    // find live. Both are outside what this run may recruit on, and the refusal says which pool.
    for entry in [foreign(), held_back()] {
        let (temporary, mut application) = run(2, RecruitmentPolicy::WORKING);
        let before = journal(temporary.path());
        let refusal = refused(
            &mut application,
            &request("request-1", entry.clone()),
            &MeasuredHost::serving(std::slice::from_ref(&entry)),
        );
        assert_eq!(refusal.gate(), Some(Gate::FrozenMembership));
        assert!(
            refusal.to_string().starts_with(&format!(
                "the default pool this run was frozen to does not permit {entry}"
            )),
            "{refusal}"
        );
        assert_eq!(journal(temporary.path()), before);
        assert!(application.admissions().is_empty());
    }

    // Gate 2: a proposer holding no further authority to start a participant.
    let (temporary, mut application) = run(0, RecruitmentPolicy::WORKING);
    let before = journal(temporary.path());
    let refusal = refused(
        &mut application,
        &request("request-1", opus()),
        &MeasuredHost::serving(&[opus()]),
    );
    assert_eq!(refusal.gate(), Some(Gate::ParticipantStarts));
    assert_eq!(
        refusal.to_string(),
        "participant-root holds no further permission to start a participant"
    );
    assert_eq!(journal(temporary.path()), before);

    // Gate 3: a run already holding as many participants as its ceiling.
    let (temporary, mut application) = run(
        4,
        RecruitmentPolicy {
            participants: 2,
            offer_allowance: RecruitmentPolicy::WORKING.offer_allowance,
        },
    );
    let host = MeasuredHost::serving(&[opus()]);
    application
        .request_participant(
            &request("request-1", opus()),
            &host,
            &mut RecordedStarts::new(),
        )
        .expect("the run has room for a second participant");
    let before = journal(temporary.path());
    let refusal = refused(&mut application, &request("request-2", opus()), &host);
    assert_eq!(refusal.gate(), Some(Gate::Concurrency));
    assert_eq!(
        refusal.to_string(),
        "this run already holds 2 participants, which is the ceiling it was created under"
    );
    assert_eq!(journal(temporary.path()), before);
    assert_eq!(application.admissions().len(), 1);

    // Gate 4: an entry no runtime on this host answers for at this moment.
    let (temporary, mut application) = run(2, RecruitmentPolicy::WORKING);
    let before = journal(temporary.path());
    let refusal = refused(
        &mut application,
        &request("request-1", opus()),
        &MeasuredHost::serving_nothing("the engine did not answer the probe"),
    );
    assert_eq!(refusal.gate(), Some(Gate::RuntimeAdmission));
    assert_eq!(
        refusal.to_string(),
        "anthropic · claude-code · claude-opus-5 is not admitted on this host at the moment: \
         the engine did not answer the probe"
    );
    assert_eq!(journal(temporary.path()), before);

    // Gate 5: a proposer that cannot fund the offer-stage allowance the newcomer is given.
    let (temporary, mut application) = run(
        1,
        RecruitmentPolicy {
            participants: 6,
            offer_allowance: BudgetVector::units(Dimension::InvocationStarts, 4),
        },
    );
    let before = journal(temporary.path());
    let refusal = refused(
        &mut application,
        &request("request-1", opus()),
        &MeasuredHost::serving(&[opus()]),
    );
    assert_eq!(refusal.gate(), Some(Gate::OfferStageCharge));
    assert_eq!(
        refusal.to_string(),
        "participant-root cannot fund the offer-stage allowance a newly admitted participant is \
         given: it holds no further invocation_starts"
    );
    assert_eq!(journal(temporary.path()), before);
}

/// A proposer that is not a participant of this run, and one whose slice yielded, are refused with
/// no effect at all — no participant, no charge, and not one byte written.
#[test]
fn a_request_from_a_proposer_that_is_not_running_is_refused_without_an_effect() {
    let (temporary, mut application) = run(2, RecruitmentPolicy::WORKING);
    let host = MeasuredHost::serving(&[opus()]);
    let before = journal(temporary.path());

    let stranger = RequestParticipant::new("request-1", "participant-stranger", opus());
    let refusal = refused(&mut application, &stranger, &host);
    assert_eq!(
        refusal,
        RecruitmentRefusal::ProposerUnknown {
            proposer: "participant-stranger".to_owned(),
        }
    );
    assert!(refusal.gate().is_none());

    // A participant the ledger holds but that is not the proposer of anything either.
    application
        .execute_commitment(
            "register",
            &CommitmentCommand::RegisterParticipant(RegisterParticipant {
                participant_id: "participant-second".to_owned(),
                principal_id: "openai".to_owned(),
                sponsor: ROOT.to_owned(),
                endowment: BudgetVector::ZERO,
            }),
        )
        .expect("a second participant is registered");
    let after_register = journal(temporary.path());
    assert_ne!(after_register, before);

    // Stopping the run's kernel is the state in which nothing further is started at all.
    application
        .execute_commitment(
            "stop",
            &CommitmentCommand::StopRun(StopRun {
                authority: ROOT.to_owned(),
                reason: StopReason::Cancelled,
            }),
        )
        .expect("the run is stopped");
    let stopped = journal(temporary.path());
    let refusal = refused(&mut application, &request("request-2", opus()), &host);
    assert_eq!(refusal, RecruitmentRefusal::RunStopped);
    assert_eq!(journal(temporary.path()), stopped);
    assert!(application.admissions().is_empty());
    let ledger = application.commitments().expect("the run holds a kernel");
    assert_eq!(ledger.consumed().get(Dimension::ParticipantStarts), 1);
}

/// Competing requests are serialized by the run's single writer, and a repeat admits nobody twice.
#[test]
fn competing_and_repeated_requests_admit_exactly_one_participant() {
    // Room for the root participant and two more, so two requests compete for the last place.
    let (temporary, mut application) = run(
        4,
        RecruitmentPolicy {
            participants: 3,
            offer_allowance: RecruitmentPolicy::WORKING.offer_allowance,
        },
    );
    let host = MeasuredHost::serving(&[opus(), sonnet()]);
    let mut starts = RecordedStarts::new();

    let first = application
        .request_participant(&request("request-1", opus()), &host, &mut starts)
        .expect("the first request is admitted");

    // The same request delivered again is refused as a duplicate and names the participant the
    // first delivery admitted. Nothing is written and nothing is started.
    let before = journal(temporary.path());
    let refusal = refused(&mut application, &request("request-1", opus()), &host);
    assert_eq!(
        refusal,
        RecruitmentRefusal::DuplicateRequest {
            request_id: "request-1".to_owned(),
            participant_id: first.admitted.participant_id.clone(),
        }
    );
    assert_eq!(journal(temporary.path()), before);

    // A repeat that names a different entry under the same request identity is the same repeat:
    // one request admits one participant, whatever a later delivery of it claims to want.
    assert_eq!(
        refused(&mut application, &request("request-1", sonnet()), &host),
        RecruitmentRefusal::DuplicateRequest {
            request_id: "request-1".to_owned(),
            participant_id: first.admitted.participant_id.clone(),
        }
    );

    // Two further requests compete for the one place the ceiling still leaves. The writer decides
    // them in the order it received them, so the first takes the place and the second is told which
    // constraint stopped it.
    let second = application
        .request_participant(&request("request-2", sonnet()), &host, &mut starts)
        .expect("the second request takes the last place");
    let refusal = refused(&mut application, &request("request-3", opus()), &host);
    assert_eq!(refusal.gate(), Some(Gate::Concurrency));

    // The serialization is not an in-process convention: a second writer on the same store is
    // refused, so there is no second path a competing request could have been decided on.
    assert!(matches!(
        Application::open(temporary.path()),
        Err(ApplicationError::WriterAlreadyActive)
    ));

    assert_eq!(application.admissions().len(), 2);
    assert_eq!(admissions_in_journal(&application).len(), 2);
    assert_eq!(starts.started().len(), 2);
    assert_ne!(
        first.admitted.participant_id,
        second.admitted.participant_id
    );
    let ledger = application.commitments().expect("the run holds a kernel");
    assert_eq!(ledger.consumed().get(Dimension::ParticipantStarts), 2);
    assert_eq!(ledger.participants().len(), 3);
}

/// A start that fails does not take the admission back: the authority was spent when the permission
/// was granted, so the run states plainly that it holds a participant with no process rather than
/// reporting a refusal that never happened.
#[test]
fn a_failed_start_leaves_the_admission_it_was_given_standing() {
    let (temporary, mut application) = run(2, RecruitmentPolicy::WORKING);
    let mut refusing = RecordedStarts::failing("this host has no sandbox left");

    let admission = application
        .request_participant(
            &request("request-1", opus()),
            &MeasuredHost::serving(&[opus()]),
            &mut refusing,
        )
        .expect("the request is admitted");

    let failure = admission.start_failure.expect("the start failed");
    assert_eq!(failure.participant_id, admission.admitted.participant_id);
    assert_eq!(
        failure.to_string(),
        format!(
            "{} was admitted and could not be started: this host has no sandbox left",
            admission.admitted.participant_id
        )
    );
    assert!(refusing.started().is_empty());

    // And it recruits nobody. A participant that holds no process is not a live participant of
    // this run, whatever the accounts say it holds, so a request arriving in its name is refused
    // without an effect — the journal is what it was and no second participant exists.
    let before = journal(temporary.path());
    let onward = RequestParticipant::new("request-2", &admission.admitted.participant_id, opus());
    let refusal = refused(&mut application, &onward, &MeasuredHost::serving(&[opus()]));
    assert_eq!(
        refusal,
        RecruitmentRefusal::ProposerNotRunning {
            proposer: admission.admitted.participant_id.clone(),
            state: ProposerState::NotStarted,
        }
    );
    assert!(refusal.gate().is_none());
    assert_eq!(journal(temporary.path()), before);
    assert_eq!(
        application
            .commitments()
            .expect("the run holds a kernel")
            .participants()
            .len(),
        2
    );

    // The admission and its charge stand, and a restart reads them back.
    assert_eq!(admissions_in_journal(&application).len(), 1);
    drop(application);
    let reopened = Application::open(temporary.path()).expect("reopen the store");
    assert_eq!(reopened.admissions().len(), 1);
    assert_eq!(
        reopened
            .commitments()
            .expect("the rebuilt kernel")
            .consumed()
            .get(Dimension::ParticipantStarts),
        1
    );
}

/// What the run recruits on is the boundary it was frozen to, and the host's answer about that one
/// entry — never an ordering over the pool, and never a substitute for what was refused.
#[test]
fn the_gate_names_no_entry_the_request_did_not() {
    let (_temporary, mut application) = run(4, RecruitmentPolicy::WORKING);
    // The host serves the second live entry of the pool and not the first.
    let host = MeasuredHost::serving(&[sonnet()]);

    let refusal = refused(&mut application, &request("request-1", opus()), &host);
    assert_eq!(refusal.gate(), Some(Gate::RuntimeAdmission));
    assert!(
        !refusal.to_string().contains("claude-sonnet-5"),
        "the refusal offers a substitute: {refusal}"
    );

    let admission = application
        .request_participant(
            &request("request-2", sonnet()),
            &host,
            &mut RecordedStarts::new(),
        )
        .expect("the entry the request named is served");
    assert_eq!(admission.admitted.entry, sonnet());
}

/// Run one request that is expected to be refused, and give back the refusal.
fn refused(
    application: &mut Application,
    request: &RequestParticipant,
    host: &MeasuredHost,
) -> RecruitmentRefusal {
    let mut starts = RecordedStarts::new();
    match application.request_participant(request, host, &mut starts) {
        Err(ApplicationError::RecruitmentRefused(refusal)) => {
            assert!(
                starts.started().is_empty(),
                "a refused request started a participant"
            );
            refusal
        }
        Err(other) => panic!("the request failed instead of being refused: {other}"),
        Ok(admission) => panic!(
            "the request was admitted: {}",
            admission.admitted.participant_id
        ),
    }
}

/// The type of one admission is carried out of the crate whole, so a caller reads what was admitted
/// without opening the journal itself.
#[allow(dead_code)]
fn admission_is_public(admission: ParticipantAdmission) -> String {
    admission.admitted.participant_id
}

/// The start path is a trait the product implements once, so a check can stand in for it.
#[allow(dead_code)]
fn start_path_is_a_trait(path: &mut dyn ParticipantStartPath) -> &mut dyn ParticipantStartPath {
    path
}
