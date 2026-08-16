#![forbid(unsafe_code)]

//! Acceptance: a run ignites on the entry its frozen record names, once, and pays for it.
//!
//! Everything here runs against a product root of this check's own — a temporary directory, never
//! the operator's `~/.ymp` — and against the in-process fixture runtime. No engine on the host
//! running this is discovered, started or probed, and no model list is measured from one, so what
//! the checks state is a property of the build rather than of the machine.
//!
//! What each check is measured against is stated where it stands, and each has a half that must
//! fail: a start reading the live pool, a start with no guard against a second authorization, a
//! start with no host admission, and a resumption that begins a second attempt.

use ymp_application::{
    Application, ORIGIN_ATTEMPT, ORIGIN_PARTICIPANT, OriginStartRefused, OriginStartRequest,
};
use ymp_domain::commitment::{CommitmentEvent, Dimension};
use ymp_domain::participant::{ParticipantOutcome, ParticipantState};
use ymp_domain::pool::EntryIdentity;
use ymp_domain::{EventKind, RunState};
use ymp_runtime_api::RuntimeEventKind;
use ymp_runtime_registry::{Engine, PoolEntry, PoolModels, PoolName, Pools, Providers, Registry};
use ymp_testkit::origin_start::{FakeRuntimes, OriginHost};

/// The entry a root measured by the fixture ignites on: the first model of the measured list, which
/// is what position and measured readiness alone reach.
fn origin_entry() -> EntryIdentity {
    EntryIdentity::new("anthropic", "claude-code", "claude-opus-5")
}

/// The other model that root serves. A start that resolved its route anywhere but the frozen record
/// lands here once the pool has been edited, which is what makes the two tell each other apart.
fn other_entry() -> EntryIdentity {
    EntryIdentity::new("anthropic", "claude-code", "claude-sonnet-5")
}

fn request(host: &OriginHost) -> OriginStartRequest {
    OriginStartRequest::under(&host.root)
}

/// Every start fact one run's journal holds, read from the record rather than from the process that
/// wrote it.
fn started_in_journal(store: &std::path::Path) -> Vec<ymp_domain::participant::ParticipantStart> {
    Application::open(store)
        .expect("reopen the store")
        .events_after(0)
        .expect("committed events")
        .into_iter()
        .filter_map(|event| match event.event {
            EventKind::ParticipantStarted(start) => Some(start),
            _ => None,
        })
        .collect()
}

/// The projection a restart rebuilds, which is what a later reader holds.
fn reopened(store: &std::path::Path) -> RunState {
    Application::open(store)
        .expect("reopen the store")
        .state()
        .clone()
}

/// How many participant starts the run's own accounting has consumed.
fn participant_starts_consumed(application: &mut Application) -> u64 {
    application
        .recovered_commitments()
        .expect("the ledger")
        .expect("this run carries a commitment kernel")
        .facts()
        .iter()
        .filter_map(|fact| match fact {
            CommitmentEvent::BudgetConsumed { amount, .. } => {
                Some(amount.get(Dimension::ParticipantStarts))
            }
            _ => None,
        })
        .sum()
}

/// A run ignites on the entry its record names, once, and the start is a journalled, charged fact.
///
/// The check that must fail: drop the guard that refuses a second authorization and let the
/// recorded result of the first answer it. The second call then returns without refusing and starts
/// a second runtime session for one authorization — measured below as a second start fact, a second
/// charge, or a session handed back at all.
#[test]
fn a_run_ignites_once_on_the_entry_its_record_names() {
    let host = OriginHost::measured();
    let store = host.store("ignites");
    let mut application = host.start_run(&store);
    let runtimes = FakeRuntimes::default();

    let attempt = application
        .start_origin_participant(&request(&host), &runtimes)
        .expect("the run ignites");
    assert_eq!(
        *attempt.route(),
        origin_entry(),
        "the participant started on a route the frozen record does not name"
    );
    assert_eq!(attempt.start().participant_id, ORIGIN_PARTICIPANT);
    assert_eq!(attempt.start().attempt_id, ORIGIN_ATTEMPT);
    assert!(
        std::path::Path::new(&attempt.start().workspace).is_dir(),
        "the start named a workspace nothing materialized: {}",
        attempt.start().workspace
    );
    drop(attempt);

    // The host was asked for exactly the frozen route, and for nothing else.
    assert_eq!(runtimes.only_route(), origin_entry());

    // One participant start is charged, and the accounting holds no second one.
    assert_eq!(
        participant_starts_consumed(&mut application),
        1,
        "the run paid for a number of participant starts other than one"
    );

    // A second authorization of the same run is refused rather than served from the record.
    let refusal = application
        .start_origin_participant(&request(&host), &runtimes)
        .expect_err("a second authorization started a second participant");
    let stated = refusal.to_string();
    assert!(
        matches!(refusal, OriginStartRefused::AlreadyStarted { .. }),
        "{stated}"
    );
    assert!(stated.contains(ORIGIN_PARTICIPANT), "{stated}");
    assert_eq!(
        runtimes.requested().len(),
        1,
        "a refused second authorization still asked the host for a runtime"
    );
    assert_eq!(
        participant_starts_consumed(&mut application),
        1,
        "a refused second authorization was charged for a participant start"
    );
    drop(application);

    // The record states one start, and the projection a restart rebuilds states one participant on
    // the frozen route.
    let started = started_in_journal(&store);
    assert_eq!(
        started.len(),
        1,
        "the journal holds {} starts",
        started.len()
    );
    assert_eq!(started[0].entry, origin_entry());
    let participant = reopened(&store)
        .origin_participant
        .expect("the reopened run states its participant");
    assert_eq!(*participant.entry(), origin_entry());
    assert_eq!(participant.start.attempt_id, ORIGIN_ATTEMPT);
}

/// A pool edited and an account held back after the freeze reach the next run and not this one.
///
/// This is the discriminating scenario. The root serves two models and the run ignites on the
/// first; the pool is then edited down to the second alone and the account is held back, so a start
/// that resolved its route from the live pool would either run on the second model or refuse
/// outright. What the host is asked for and what the journal records are both required to be the
/// entry frozen at creation.
///
/// The check that must fail: resolve the route with `freeze_under(root, None)` — the live pool —
/// instead of the run's own frozen record.
#[test]
fn what_changed_after_the_freeze_does_not_move_the_participant() {
    let host = OriginHost::measured();
    let store = host.store("frozen-route");
    let mut application = host.start_run(&store);

    // The pool stops following the catalog and keeps only the second model. The live digest moves.
    Pools::under(&host.root)
        .edit(
            &PoolName::default_pool(),
            &Providers::under(&host.root),
            &Registry::under(&host.root),
            |declared| {
                declared.models = PoolModels::explicit([PoolEntry {
                    provider: other_entry().provider,
                    engine: other_entry().engine,
                    model: other_entry().model,
                }]);
            },
        )
        .expect("edit the pool");
    // And the account is held back, which is what leaves the live pool with nothing live at all.
    let providers = Providers::under(&host.root);
    providers
        .set_enabled(Engine::ClaudeCode.provider(), false, Some("held back"))
        .expect("hold the account back");
    providers
        .observe(&Registry::under(&host.root))
        .expect("observe the accounts");
    Pools::under(&host.root)
        .reconcile(&providers, &Registry::under(&host.root))
        .expect("resolve the pools again");
    assert!(
        ymp_application::freeze_under(&host.root, None).is_err(),
        "the live pool still offers a run something, so this scenario discriminates nothing"
    );

    let runtimes = FakeRuntimes::default();
    let attempt = application
        .start_origin_participant(&request(&host), &runtimes)
        .expect("a run already created ignites on what it froze");
    assert_eq!(
        *attempt.route(),
        origin_entry(),
        "a pool edited after the freeze moved the participant onto another route"
    );
    drop(attempt);
    assert_eq!(
        runtimes.only_route(),
        origin_entry(),
        "the host was asked for a route the live pool names rather than the frozen one"
    );
    drop(application);

    let started = started_in_journal(&store);
    assert_eq!(started.len(), 1);
    assert_eq!(
        started[0].entry,
        origin_entry(),
        "the record states a route the run was not created under"
    );
}

/// The sharper half of the same rule: a live pool that still resolves, and resolves to something
/// else.
///
/// The account stays enabled and only the pool is edited, so the pool as it stands now ignites on
/// the second model rather than on nothing. A start that consulted it would therefore run — quietly,
/// and on the wrong model — where the check above would only see a run that failed to start. What
/// the host is asked for and what the record states are both required to be the first model still.
#[test]
fn a_pool_that_still_resolves_after_the_freeze_does_not_move_the_participant() {
    let host = OriginHost::measured();
    let store = host.store("edited-pool");
    let mut application = host.start_run(&store);

    Pools::under(&host.root)
        .edit(
            &PoolName::default_pool(),
            &Providers::under(&host.root),
            &Registry::under(&host.root),
            |declared| {
                declared.models = PoolModels::explicit([PoolEntry {
                    provider: other_entry().provider,
                    engine: other_entry().engine,
                    model: other_entry().model,
                }]);
            },
        )
        .expect("edit the pool");
    assert_eq!(
        ymp_application::freeze_under(&host.root, None)
            .expect("the edited pool still resolves")
            .origin,
        other_entry(),
        "the edited pool ignites where it did, so this scenario discriminates nothing"
    );

    let runtimes = FakeRuntimes::default();
    let attempt = application
        .start_origin_participant(&request(&host), &runtimes)
        .expect("the run ignites");
    assert_eq!(
        *attempt.route(),
        origin_entry(),
        "the participant started on the model the live pool names rather than the frozen one"
    );
    drop(attempt);
    assert_eq!(runtimes.only_route(), origin_entry());
    drop(application);
    assert_eq!(started_in_journal(&store)[0].entry, origin_entry());
}

/// An engine this host no longer admits refuses the start in plain words, and the run is left
/// exactly as it was.
///
/// The check that must fail: drop the admission the engine record decides and go straight to the
/// driver. The start then proceeds on a held-back engine — measured here as a session handed back,
/// a start fact in the journal, or a participant start charged.
#[test]
fn an_engine_this_host_holds_back_refuses_the_start_and_leaves_the_record_whole() {
    let host = OriginHost::measured();
    let store = host.store("unadmitted");
    let mut application = host.start_run(&store);
    let before = application.events_after(0).expect("committed events").len();

    Registry::under(&host.root)
        .update(Engine::ClaudeCode, |record| {
            record.enabled = false;
            record.disabled_reason = Some("the operator held this engine back".to_owned());
        })
        .expect("hold the engine back");

    let runtimes = FakeRuntimes::default();
    let refusal = application
        .start_origin_participant(&request(&host), &runtimes)
        .expect_err("a held-back engine started a participant");
    let stated = refusal.to_string();
    assert!(
        matches!(refusal, OriginStartRefused::RuntimeNotAdmitted { .. }),
        "{stated}"
    );
    assert!(stated.contains("/runtimes"), "{stated}");
    assert!(stated.contains("held this engine back"), "{stated}");
    assert!(stated.contains("your goal is held"), "{stated}");
    assert!(
        stated.contains("nothing has left this host"),
        "the refusal does not state that nothing happened: {stated}"
    );

    // The refusal is honest: no runtime was asked for, no record was written, nothing was charged.
    assert!(
        runtimes.requested().is_empty(),
        "a refused start asked the host for a runtime anyway"
    );
    assert_eq!(
        application.events_after(0).expect("committed events").len(),
        before,
        "a refused start wrote to the journal"
    );
    assert!(
        application.origin_participant().is_none(),
        "a refused start left a participant in the projection"
    );
    assert!(
        application
            .recovered_commitments()
            .expect("the ledger")
            .is_none(),
        "a refused start opened a commitment kernel"
    );
    drop(application);

    assert!(
        started_in_journal(&store).is_empty(),
        "a refused start left a start fact in the record"
    );
    // The frozen record itself is untouched: the run still names the entry it was created under.
    assert_eq!(
        reopened(&store)
            .frozen_pool
            .expect("the run carries its frozen pool")
            .origin,
        origin_entry()
    );

    // Admitting the engine again is all it takes; the run ignites on the entry it always named.
    let mut application = Application::open(&store).expect("reopen the store");
    Registry::under(&host.root)
        .update(Engine::ClaudeCode, |record| {
            record.enabled = true;
            record.disabled_reason = None;
        })
        .expect("admit the engine again");
    let attempt = application
        .start_origin_participant(&request(&host), &runtimes)
        .expect("an admitted engine ignites the run");
    assert_eq!(*attempt.route(), origin_entry());
}

/// The participant yields, is resumed and finishes, and the attempt it runs as is the one it
/// started with throughout.
///
/// The check that must fail: let a resumption go through the start path — or drop the domain's
/// requirement that a resumption names a yielded participant — and a second attempt appears in the
/// record for one authorization.
#[test]
fn the_participant_yields_resumes_and_finishes_without_a_second_attempt() {
    let host = OriginHost::measured();
    let store = host.store("yield-resume");
    let mut application = host.start_run(&store);
    let runtimes = FakeRuntimes::default();
    let mut attempt = application
        .start_origin_participant(&request(&host), &runtimes)
        .expect("the run ignites");

    // Up to the yield: the record states the slice stopped, and the participant is waiting.
    let mut yielded = false;
    while let Some(event) = attempt.next_event().expect("the participant runs") {
        if matches!(event.event, RuntimeEventKind::Yielded { .. }) {
            yielded = true;
            break;
        }
    }
    assert!(yielded, "the fixture participant never yielded");
    assert_eq!(attempt.state(), Some(ParticipantState::Yielded));

    // A resumption resumes the attempt that exists.
    attempt.resume("continue").expect("the participant resumes");
    assert_eq!(attempt.state(), Some(ParticipantState::Running));
    while attempt
        .next_event()
        .expect("the participant runs")
        .is_some()
    {}
    assert_eq!(attempt.state(), Some(ParticipantState::Finished));
    drop(attempt);
    drop(application);

    // One start, one attempt, one yield and one resumption, in that order, and the ending states
    // what the runtime did.
    let events: Vec<EventKind> = Application::open(&store)
        .expect("reopen the store")
        .events_after(0)
        .expect("committed events")
        .into_iter()
        .map(|event| event.event)
        .collect();
    let attempts: Vec<&str> = events
        .iter()
        .filter_map(|event| match event {
            EventKind::ParticipantStarted(start) => Some(start.attempt_id.as_str()),
            EventKind::AttemptStarted { attempt_id } => Some(attempt_id.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(
        attempts,
        vec![ORIGIN_ATTEMPT],
        "one start authorization produced attempts {attempts:?}"
    );
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event, EventKind::ParticipantYielded { .. }))
            .count(),
        1
    );
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event, EventKind::ParticipantResumed { .. }))
            .count(),
        1
    );
    assert!(
        events.iter().any(|event| matches!(
            event,
            EventKind::ParticipantFinished { outcome, .. } if *outcome == ParticipantOutcome::Completed
        )),
        "the record does not state how the attempt ended"
    );

    // And the run's own attempt budget was spent once, by the one attempt that ran.
    let state = reopened(&store);
    assert_eq!(state.budget.attempts_remaining, 0);
    assert_eq!(state.active_attempts, vec![ORIGIN_ATTEMPT.to_owned()]);
    assert_eq!(
        state
            .origin_participant
            .expect("the reopened run states its participant")
            .state,
        ParticipantState::Finished
    );
}

/// A host that serves no runtime for the frozen route refuses in plain words and writes nothing.
///
/// It is the other half of the admission: the engine may be admitted on this host and still serve
/// no such model, and a run that cannot be started is told so rather than started on something else.
#[test]
fn a_route_this_host_cannot_serve_refuses_rather_than_starting_on_another() {
    let host = OriginHost::measured();
    let store = host.store("unserved");
    let mut application = host.start_run(&store);
    let runtimes = FakeRuntimes::default().serving_nothing_for(origin_entry());

    let refusal = application
        .start_origin_participant(&request(&host), &runtimes)
        .expect_err("a route this host cannot serve started something");
    let stated = refusal.to_string();
    assert!(
        matches!(refusal, OriginStartRefused::RouteUnavailable { .. }),
        "{stated}"
    );
    assert!(stated.contains(&origin_entry().to_string()), "{stated}");
    assert!(stated.contains("/runtimes"), "{stated}");
    assert!(
        application.origin_participant().is_none(),
        "a refused start left a participant in the projection"
    );
    drop(application);
    assert!(
        started_in_journal(&store).is_empty(),
        "a refused start left a start fact in the record"
    );
}
