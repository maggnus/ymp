//! Acceptance: authorizing a run in the product starts the participant that run ignites on.
//!
//! Everything the domain and the application settled in P9 is reached here through the surface an
//! operator actually uses: the contract is typed, the authorization is taken, and what follows is
//! measured on the store, on the private copy and on the transcript rather than on any value this
//! file passed in. The programs that serve a route are the one thing substituted — the in-process
//! fixture runtime stands in for the managed engines — so nothing here starts a process of the
//! machine it runs on. The private baseline is *not* substituted: it is the product's own, which is
//! why the copy is measured for a repository of its own.
//!
//! The half that must fail is stated on each check.

mod support;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use support::{SIZES, screen};
use ymp_application::{Application, ORIGIN_ATTEMPT, ORIGIN_PARTICIPANT, PreparedContract};
use ymp_domain::commitment::{CommitmentEvent, Dimension};
use ymp_domain::participant::{ParticipantOutcome, ParticipantState};
use ymp_domain::pool::EntryIdentity;
use ymp_domain::{Budget, Command as DomainCommand, EventKind, RunStatus};
use ymp_testkit::origin_start::{FakeRuntimes, OriginHost};
use ymp_tui::Session;
use ymp_tui::state::App;

/// The entry a root measured by the fixture ignites on.
fn origin_entry() -> EntryIdentity {
    EntryIdentity::new("anthropic", "claude-code", "claude-opus-5")
}

/// A product root the interface addresses as one exact store, with the account measured on it. The
/// store is the root here, which is what an invocation naming one exact store reaches.
struct Surface {
    host: OriginHost,
    store: PathBuf,
}

impl Surface {
    fn measured() -> Self {
        let host = OriginHost::empty();
        let store = host.store("surface");
        std::fs::create_dir_all(&store).expect("the store directory");
        ymp_testkit::ready_root::measured(&store);
        Self { host, store }
    }

    /// A session over this store, serving every frozen route with the fixture runtime.
    fn session(&self, contract: &PreparedContract) -> (Session, Arc<FakeRuntimes>) {
        let runtimes = Arc::new(FakeRuntimes::default());
        let mut session = Session::open(&self.store, std::slice::from_ref(contract));
        session.set_participant_runtimes(Arc::clone(&runtimes) as Arc<_>);
        (session, runtimes)
    }
}

/// Every event this store committed, read back after the session that wrote it has gone.
fn committed(store: &Path) -> Vec<EventKind> {
    Application::open(store)
        .expect("reopen the store")
        .events_after(0)
        .expect("committed events")
        .into_iter()
        .map(|event| event.event)
        .collect()
}

/// How many participant starts this run's own accounting has consumed.
fn participant_starts_consumed(store: &Path) -> u64 {
    let mut application = Application::open(store).expect("reopen the store");
    application
        .recovered_commitments()
        .expect("the ledger")
        .map(|ledger| {
            ledger
                .facts()
                .iter()
                .filter_map(|fact| match fact {
                    CommitmentEvent::BudgetConsumed { amount, .. } => {
                        Some(amount.get(Dimension::ParticipantStarts))
                    }
                    _ => None,
                })
                .sum()
        })
        .unwrap_or(0)
}

/// The transcript this session states, at both accepted sizes.
fn transcript(session: &mut Session) -> Vec<String> {
    let app = App::new(session.projection(None));
    SIZES
        .into_iter()
        .map(|(width, height)| screen(&app, width, height))
        .collect()
}

/// The authorization starts one participant, on the entry the run froze, in a private copy that
/// carries a baseline of its own — and pays for it once.
///
/// The check that must fail: leave the authorization where this card found it, doing nothing after
/// the run is created. No start fact, no charge, no private copy, and the transcript states only
/// that a run exists.
#[test]
fn authorizing_a_run_starts_the_participant_it_ignites_on() {
    let surface = Surface::measured();
    let contract = surface.host.contract();
    let (mut session, runtimes) = surface.session(&contract);
    session.start_run(contract.contract_id());
    session.await_origin();

    // The host was asked for the frozen route, and for nothing else.
    assert_eq!(
        runtimes.only_route(),
        origin_entry(),
        "the surface asked this host for a route the run did not freeze"
    );

    // The transcript states the ignition in the operator's own reading of the run.
    let rendered = transcript(&mut session);
    for screen in &rendered {
        assert!(
            screen.contains("ignited on"),
            "the transcript does not state that the run ignited:\n{screen}"
        );
        assert!(screen.contains(ORIGIN_PARTICIPANT), "{screen}");
    }

    drop(session);
    let events = committed(&surface.store);
    let starts: Vec<_> = events
        .iter()
        .filter_map(|event| match event {
            EventKind::ParticipantStarted(start) => Some(start),
            _ => None,
        })
        .collect();
    assert_eq!(
        starts.len(),
        1,
        "one authorization produced {} starts",
        starts.len()
    );
    assert_eq!(starts[0].entry, origin_entry());
    assert_eq!(starts[0].participant_id, ORIGIN_PARTICIPANT);
    assert_eq!(starts[0].attempt_id, ORIGIN_ATTEMPT);

    // The private copy the participant worked in stands, and it carries a baseline of its own: the
    // product's own Git initialization, not a fixture's.
    let workspace = Path::new(&starts[0].workspace);
    assert!(
        workspace.is_dir(),
        "the start named a copy nothing materialized: {}",
        workspace.display()
    );
    assert!(
        workspace.join(".git").is_dir(),
        "the private copy has no repository of its own: {}",
        workspace.display()
    );
    assert!(
        workspace.join("README.md").is_file(),
        "the private copy does not carry the source it was made from"
    );

    // And the run paid for exactly one participant start.
    assert_eq!(participant_starts_consumed(&surface.store), 1);
}

/// A second authorization in the same store starts no second participant.
///
/// A store holds one run, and this session names one exact store rather than a root, so the second
/// authorization has nowhere to put a run and says so. What is measured is the store: one start
/// fact and one charge after both authorizations, not merely a refusal on the screen.
///
/// The check that must fail: let the second authorization reach the start path with the run that
/// already ignited. The store then holds a second start fact for one run.
#[test]
fn a_second_authorization_starts_no_second_participant() {
    let surface = Surface::measured();
    let contract = surface.host.contract();
    let (mut session, runtimes) = surface.session(&contract);
    session.start_run(contract.contract_id());
    session.await_origin();
    session.start_run(contract.contract_id());
    session.await_origin();

    assert_eq!(
        runtimes.requested().len(),
        1,
        "a second authorization asked this host for a runtime again: {:?}",
        runtimes.requested()
    );
    drop(session);

    let starts = committed(&surface.store)
        .into_iter()
        .filter(|event| matches!(event, EventKind::ParticipantStarted(_)))
        .count();
    assert_eq!(starts, 1, "one run holds {starts} starts");
    assert_eq!(participant_starts_consumed(&surface.store), 1);
}

/// A run that holds no attempt to spend starts nothing through the surface, and pays for nothing.
///
/// The half that must fail is the build this card started from: the private copy was made, the
/// commitment kernel opened and the participant registered — one participant start charged — and
/// the run then ended itself with `RunExhausted`. Measured here on the store: no commitment record
/// of any kind, no copy, and a run still running.
#[test]
fn a_run_with_no_attempt_left_is_refused_before_the_copy_and_the_charge() {
    let surface = Surface::measured();
    let contract = surface.host.contract_budgeted(Some(Budget::new(0, 1)));
    let (mut session, runtimes) = surface.session(&contract);
    session.start_run(contract.contract_id());
    session.await_origin();

    let rendered = transcript(&mut session);
    for screen in &rendered {
        assert!(
            screen.contains("your goal is held"),
            "the refusal is not stated in the transcript:\n{screen}"
        );
    }
    assert!(
        runtimes.requested().is_empty(),
        "a refused start asked the host for a runtime"
    );
    assert!(
        runtimes.established().is_empty(),
        "a refused start had a private copy established for it"
    );
    drop(session);

    assert!(
        !surface.store.join("workspaces").exists(),
        "a refused start left a private copy behind"
    );
    let events = committed(&surface.store);
    assert!(
        !events.iter().any(|event| matches!(
            event,
            EventKind::CommitmentKernelOpened { .. }
                | EventKind::CommitmentFactsRecorded { .. }
                | EventKind::RunExhausted { .. }
                | EventKind::ParticipantStarted(_)
        )),
        "a refused start wrote to the journal: {events:?}"
    );
    assert_eq!(participant_starts_consumed(&surface.store), 0);
    assert_eq!(
        Application::open(&surface.store)
            .expect("reopen the store")
            .state()
            .status,
        RunStatus::Running,
        "a refused start ended the run"
    );
}

/// Two runs authorized under one root, with the pool edited between them, each ignite on the entry
/// their own record froze.
///
/// One authorization freezes the pool and ignites the participant in the same act, so the pool as
/// it stands and the record the run carries name the same entry at that moment: a start reading the
/// live pool cannot be told from one reading the record by a single authorization, and this check
/// does not claim to. What it does tell apart is a surface answering with anything fixed — a
/// constant, the first run's route, the route this session was left holding — because two
/// authorizations under one root demand two different answers from one host in one process.
///
/// The check that must fail: answer the route from anything but each run's own record. The second
/// authorization then asks this host for the first run's model, which its own frozen pool does not
/// permit, and the run ignites on nothing.
///
/// What no check here reaches, stated rather than implied: a build that resolved the route from the
/// live pool passes this, because the surface never lets the two diverge. The reading is held to the
/// record by `ymp-testkit/tests/origin_participant.rs`, where a run created earlier is started after
/// the pool has moved.
#[test]
fn two_runs_authorized_under_one_root_ignite_on_what_each_froze() {
    let other = EntryIdentity::new("anthropic", "claude-code", "claude-sonnet-5");
    let host = OriginHost::measured();
    let contract = host.contract();
    let runtimes: Arc<FakeRuntimes> = Arc::new(FakeRuntimes::default());

    let first_store = host.store("first");
    let mut first =
        Session::open_under_root(&host.root, &first_store, std::slice::from_ref(&contract));
    first.set_participant_runtimes(Arc::clone(&runtimes) as Arc<_>);
    first.start_run(contract.contract_id());
    first.await_origin();
    drop(first);

    // The pool this root offers is edited after the first run was authorized and before the second
    // one is. What each run may draw on was fixed when it was created; the edit reaches the next.
    ymp_runtime_registry::Pools::under(&host.root)
        .edit(
            &ymp_runtime_registry::PoolName::default_pool(),
            &ymp_runtime_registry::Providers::under(&host.root),
            &ymp_runtime_registry::Registry::under(&host.root),
            |declared| {
                declared.models =
                    ymp_runtime_registry::PoolModels::explicit([ymp_runtime_registry::PoolEntry {
                        provider: other.provider.clone(),
                        engine: other.engine.clone(),
                        model: other.model.clone(),
                    }]);
            },
        )
        .expect("edit the pool this root offers");

    let second_store = host.store("second");
    let mut second =
        Session::open_under_root(&host.root, &second_store, std::slice::from_ref(&contract));
    second.set_participant_runtimes(Arc::clone(&runtimes) as Arc<_>);
    second.start_run(contract.contract_id());
    second.await_origin();
    drop(second);

    assert_eq!(
        runtimes.requested(),
        vec![origin_entry(), other.clone()],
        "the two authorizations asked this host for routes other than the two they froze"
    );
    assert_eq!(
        committed(&first_store)
            .into_iter()
            .find_map(|event| match event {
                EventKind::ParticipantStarted(start) => Some(start.entry),
                _ => None,
            })
            .expect("the first run states its start"),
        origin_entry(),
        "the first run's record states a route it was not created under"
    );
    assert_eq!(
        committed(&second_store)
            .into_iter()
            .find_map(|event| match event {
                EventKind::ParticipantStarted(start) => Some(start.entry),
                _ => None,
            })
            .expect("the second run states its start"),
        other,
        "the second run ran on the first run's model"
    );
}

/// The four facts a participant's life produces are read at both accepted sizes.
///
/// The run is driven through its whole life — started, yielded, resumed, finished — and the
/// transcript is then read the way an operator reads it. Nothing is projected for this check: the
/// lines come from the projections the accepted work already ships, which is what makes this a
/// reading of the product rather than of a fixture.
///
/// The check that must fail: project any one of the four as no line, and the size it is missing
/// from names it.
#[test]
fn the_four_participant_facts_are_read_at_both_sizes() {
    let surface = Surface::measured();
    let contract = surface.host.contract();
    let (mut session, _runtimes) = surface.session(&contract);
    session.start_run(contract.contract_id());
    session.await_origin();
    drop(session);

    // The participant of this run stopped for an instruction. Giving it one and letting it finish
    // is the operator's own act; it is taken here directly on the store, because the surface that
    // carries an instruction to a working participant is not part of this card.
    {
        let mut application = Application::open(&surface.store).expect("reopen the store");
        assert_eq!(
            application
                .origin_participant()
                .expect("the run states its participant")
                .state,
            ParticipantState::Yielded,
            "the fixture participant did not stop for an instruction"
        );
        application
            .execute(
                "ymp.check.resume",
                DomainCommand::ResumeParticipant {
                    participant_id: ORIGIN_PARTICIPANT.to_owned(),
                    cursor: "continue".to_owned(),
                },
            )
            .expect("the participant resumes");
        application
            .execute(
                "ymp.check.finish",
                DomainCommand::FinishParticipant {
                    participant_id: ORIGIN_PARTICIPANT.to_owned(),
                    outcome: ParticipantOutcome::Completed,
                },
            )
            .expect("the participant finishes");
    }

    let mut session = Session::open(&surface.store, &[]);
    for screen in transcript(&mut session) {
        for stated in [
            "this run ignited on",
            "stopped and is waiting to be resumed",
            "was resumed on the attempt it was running",
            "ended · completed",
        ] {
            assert!(
                screen.contains(stated),
                "the transcript does not state {stated:?}:\n{screen}"
            );
        }
    }
}
