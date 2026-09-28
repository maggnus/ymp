//! P2 through real admission/Arbiter consumers; no backend or acceptance producer.
mod support;
use support::Directory;
use ymp_kernel as kernel;
use ymp_storage as storage;
#[path = "support/admission_fixture.rs"]
#[allow(dead_code)]
mod admission_fixture;
#[allow(dead_code)]
#[path = "../../ymp-kernel/tests/support/workspace_fixture.rs"]
mod fixture;
use admission_fixture::Setup;
use std::{collections::BTreeSet, sync::Arc};
use ymp_domain::{Digest, Id, Ref, assignment::*, coordination::*, task::Real};
use ymp_kernel::{
    arbiter::{Arbiter, award_view},
    gatekeeper::{AdmissionStatus, AdmittedAssignment},
    journal::{Journal, ParameterSchemas},
    ports::organization::AwardPolicy,
};
struct StaleReadJournal {
    inner: MemoryJournal,
    stale: std::sync::Mutex<Option<ymp_kernel::journal::JournalRead>>,
}
impl Journal for StaleReadJournal {
    fn schemas(&self) -> &ParameterSchemas {
        self.inner.schemas()
    }
    fn read(&self, session: &Id) -> ymp_domain::Result<ymp_kernel::journal::JournalRead> {
        if let Some(stale) = self.stale.lock().unwrap().take() {
            return Ok(stale);
        }
        self.inner.read(session)
    }
    fn workspace_inventory(
        &self,
        session: &Id,
    ) -> ymp_domain::Result<ymp_kernel::journal::WorkspaceInventory> {
        self.inner.workspace_inventory(session)
    }
    fn append(
        &self,
        session: &Id,
        expected: u64,
        events: &[ymp_domain::journal::Envelope<ymp_kernel::events::Event>],
    ) -> ymp_domain::Result<u64> {
        self.inner.append(session, expected, events)
    }
}
use ymp_runtime::{
    clock::{Clock, ManualClock},
    memory_journal::MemoryJournal,
    policies::award::FirstOffer,
};
use ymp_storage::journal::SqliteJournal;
fn id<T>(value: &str) -> Id<T> {
    Id::new(value).unwrap()
}
fn terms(duration: u64) -> CommitmentTerms {
    CommitmentTerms {
        lease_duration: duration,
        renewal_duration: duration / 2,
        renew_on: BTreeSet::from([ProgressSignal::Heartbeat]),
        renewals: 2,
        release_delta: Real::new(2.0).unwrap(),
    }
}
fn setup<J: Journal>(
    journal: Arc<J>,
    store: &SqliteJournal,
    root: &Directory,
    files: bool,
    duration: u64,
) -> Setup<J> {
    Setup::new_with_award(
        journal,
        store,
        root,
        files,
        "lifecycle",
        None,
        FirstOffer::with_commitment_terms(terms(duration)).unwrap(),
    )
}
fn admit<J: Journal>(s: &Setup<J>) -> AdmittedAssignment<J> {
    let award = s.award("work");
    let (expected, at, mut request) = s.request("old", award);
    request.operations.extend([
        TeamOperation::CommitmentRelease,
        TeamOperation::CommitmentDelegate,
    ]);
    let mut prepared = s.gate.prepare(&s.session, expected, at, request).unwrap();
    s.gate.admit(&mut prepared).unwrap()
}
fn solicitation<J: Journal>(s: &Setup<J>, name: &str, stimulus: f64) -> Solicitation {
    let view = s.gate.view(&s.session).unwrap();
    Solicitation {
        id: id(name),
        contribution: id("work"),
        stimulus: Real::new(stimulus).unwrap(),
        deadline: view.latest_at() + 3,
        eligible: BTreeSet::from([s.profile.agent.clone()]),
        visibility: SolicitationVisibility::Open,
        reopened: 1,
        state: SolicitationState::Open,
    }
}
fn award_open<J: Journal>(s: &Setup<J>, name: &str) -> Ref {
    let arbiter = Arbiter::new(s.journal.clone());
    let view = arbiter.view(&s.session).unwrap();
    let source = &view.coordination().solicitations()[&id(name)].value;
    let contribution = &view.coordination().contributions()[&source.contribution].value;
    let at = source.deadline;
    arbiter
        .submit(
            &s.session,
            view.revision(),
            at,
            Offer {
                id: id(name),
                solicitation: id(name),
                agent: s.profile.agent.clone(),
                profile: s.profile.clone(),
                forecast: contribution.forecast.clone(),
                cost: contribution.cost.clone(),
                approach: "Bounded P2 consumer scenario".into(),
                source: OfferSource::RuntimeProxy,
                at,
            },
        )
        .unwrap();
    let view = arbiter.view(&s.session).unwrap();
    let input = award_view(&view, &id(name), at).unwrap();
    arbiter
        .award_with_terms(
            &s.session,
            view.revision(),
            at,
            s.award.award(&input).unwrap(),
            Digest::of_value(&input).unwrap(),
            Commitment {
                id: id(name),
                debtor: s.profile.agent.clone(),
                creditor: Creditor::Runtime,
                subject: id("work"),
                condition: None,
                lease: Lease {
                    expires: at + 100,
                    renew_on: BTreeSet::new(),
                    renewals_left: 0,
                },
                state: CommitmentState::Proposed,
                history: vec![],
            },
            Some(s.award.commitment_terms(&input).unwrap()),
        )
        .unwrap();
    arbiter.view(&s.session).unwrap().coordination().awards()[&id(name)]
        .reference
        .clone()
}
#[test]
fn leases_renew_at_the_boundary_exhaust_and_expire_without_releasing_holds() {
    for duration in [20, 30] {
        let root = Directory::new();
        let database = Directory::new();
        let store = SqliteJournal::open(database.database(), ParameterSchemas::default()).unwrap();
        let s = setup(
            Arc::new(MemoryJournal::new()),
            &store,
            &root,
            false,
            duration,
        );
        let admitted = admit(&s);
        let arbiter = Arbiter::new(s.journal.clone());
        let view = arbiter.view(&s.session).unwrap();
        let initial = view.coordination().commitments()[&id("work")].lease.expires;
        assert_eq!(initial, view.latest_at() + duration);
        let pending = solicitation(&s, "pending-renewal", 1.0);
        arbiter
            .open_delegation(
                &s.gate,
                &admitted.grant,
                view.revision(),
                view.latest_at() + 1,
                pending,
            )
            .unwrap();
        award_open(&s, "pending-renewal");
        let clock = ManualClock::new(initial);
        assert!(
            arbiter
                .tick(&s.gate, &s.session, clock.now().unwrap())
                .unwrap()
                .is_empty()
        );
        for remaining in [1, 0] {
            let view = arbiter.view(&s.session).unwrap();
            arbiter
                .renew(
                    &s.gate,
                    &admitted.grant,
                    view.revision(),
                    clock.now().unwrap(),
                    ProgressSignal::Heartbeat,
                )
                .unwrap();
            let current = arbiter.view(&s.session).unwrap();
            assert_eq!(
                current.coordination().commitments()[&id("work")]
                    .lease
                    .renewals_left,
                remaining
            );
            assert_eq!(
                current.coordination().commitments()[&id("work")]
                    .lease
                    .expires,
                view.coordination().commitments()[&id("work")].lease.expires + duration / 2
            );
            clock
                .advance_to(
                    current.coordination().commitments()[&id("work")]
                        .lease
                        .expires,
                )
                .unwrap();
        }
        let cleaned = arbiter.view(&s.session).unwrap();
        assert_eq!(
            cleaned.coordination().solicitations()[&id("pending-renewal")]
                .value
                .state,
            SolicitationState::Withdrawn
        );
        assert!(matches!(
            cleaned.coordination().commitments()[&id("pending-renewal")].state,
            CommitmentState::Cancelled(_)
        ));
        let before = arbiter.view(&s.session).unwrap();
        assert!(
            arbiter
                .renew(
                    &s.gate,
                    &admitted.grant,
                    before.revision(),
                    clock.now().unwrap(),
                    ProgressSignal::Heartbeat
                )
                .is_err()
        );
        assert_eq!(arbiter.view(&s.session).unwrap(), before);
        clock.advance_to(clock.now().unwrap() + 1).unwrap();
        assert!(
            s.gate
                .authorize(
                    &admitted.grant,
                    TeamOperation::BoardRead,
                    clock.now().unwrap()
                )
                .is_err(),
            "An expired lease must deny the grant before tick"
        );
        assert_eq!(
            arbiter
                .tick(&s.gate, &s.session, clock.now().unwrap())
                .unwrap(),
            vec![id("work")]
        );
        assert!(
            arbiter
                .tick(&s.gate, &s.session, clock.now().unwrap())
                .unwrap()
                .is_empty()
        );
        let stopped = arbiter.view(&s.session).unwrap();
        assert_eq!(
            stopped.coordination().commitments()[&id("work")].state,
            CommitmentState::Expired
        );
        assert_eq!(
            stopped.treasury().unwrap().budget.held,
            before.treasury().unwrap().budget.held
        );
        assert!(stopped.treasury().unwrap().accounts[&id("old")].revoked);
        let next = solicitation(&s, "expired-reopen", 1.0);
        arbiter
            .reopen(
                &s.session,
                stopped.revision(),
                clock.now().unwrap(),
                next,
                id("work"),
            )
            .unwrap();
    }
}
#[test]
fn release_reopens_atomically_and_retains_unknown_cost_and_file_ownership() {
    let root = Directory::new();
    let database = Directory::new();
    let journal =
        Arc::new(SqliteJournal::open(database.database(), ParameterSchemas::default()).unwrap());
    let s = setup(journal.clone(), &journal, &root, true, 30);
    let admitted = admit(&s);
    let arbiter = Arbiter::new(journal.clone());
    let view = arbiter.view(&s.session).unwrap();
    let pending = solicitation(&s, "pending-release", 1.0);
    arbiter
        .open_delegation(
            &s.gate,
            &admitted.grant,
            view.revision(),
            view.latest_at() + 1,
            pending,
        )
        .unwrap();
    let treasury = ymp_kernel::treasury::Treasury::new(journal.clone());
    let view = arbiter.view(&s.session).unwrap();
    treasury
        .authorize(
            &s.session,
            view.revision(),
            view.latest_at(),
            id("old"),
            id("invocation"),
        )
        .unwrap();
    let before = arbiter.view(&s.session).unwrap();
    let next = solicitation(&s, "released-reopen", 3.0);
    arbiter
        .release(
            &s.gate,
            &admitted.grant,
            before.revision(),
            before.latest_at() + 1,
            "Offer a different approach".into(),
            next,
        )
        .unwrap();
    let after = arbiter.view(&s.session).unwrap();
    assert_eq!(
        after.coordination().solicitations()[&id("pending-release")]
            .value
            .state,
        SolicitationState::Withdrawn
    );

    assert!(matches!(
        after.coordination().commitments()[&id("work")].state,
        CommitmentState::Released(_)
    ));
    assert_eq!(
        after.coordination().solicitations()[&id("released-reopen")]
            .value
            .stimulus
            .get(),
        3.0
    );
    assert_eq!(
        after.coordination().solicitations()[&id("released-reopen")]
            .value
            .state,
        SolicitationState::Open
    );
    assert!(after.path_locks()[&id("old")].revoked);
    assert!(after.path_locks()[&id("old")].released.is_none());
    let account = &after.treasury().unwrap().accounts[&id("old")];
    assert!(account.revoked && account.settlement.is_none());
    assert_eq!(
        account.reservation.state,
        ymp_domain::resources::ReservationState::Held
    );
    assert_eq!(
        after.treasury().unwrap().budget.held,
        before.treasury().unwrap().budget.held
    );
    let stored = journal.read(&s.session).unwrap();
    let replica = MemoryJournal::new();
    replica
        .append(&s.session, 0, &stored.events[..before.revision() as usize])
        .unwrap();
    let packet = &stored.events[before.revision() as usize..];
    assert_eq!(
        replica
            .append(&s.session, before.revision(), &packet[..packet.len() - 1])
            .unwrap_err()
            .code,
        "release_incomplete"
    );
    let reopened = SqliteJournal::open(database.database(), ParameterSchemas::default()).unwrap();
    assert_eq!(reopened.view(&s.session, None).unwrap(), after);
    assert!(
        s.gate
            .authorize(&admitted.grant, TeamOperation::BoardRead, after.latest_at())
            .is_err()
    );
    assert!(
        admitted
            .files
            .unwrap()
            .write(
                &ymp_domain::workspace::WorkspacePath::new("file").unwrap(),
                b"after release",
                after.latest_at()
            )
            .is_err()
    );
}
fn delegation<J: Journal>(mut s: Setup<J>, mode: u8) {
    let admitted = admit(&s);
    let arbiter = Arbiter::new(s.journal.clone());
    if mode != 2 {
        s.switch_to_new_agent("next-agent");
    }
    let view = arbiter.view(&s.session).unwrap();
    let next = solicitation(&s, "successor", 1.0);
    arbiter
        .open_delegation(
            &s.gate,
            &admitted.grant,
            view.revision(),
            view.latest_at() + 1,
            next,
        )
        .unwrap();
    let award = award_open(&s, "successor");
    let (expected, at, mut request) = s.request("next", award);
    if mode == 1 {
        request.allowance.proposal.value.cost = Real::new(100.0).unwrap();
    }
    let before = arbiter.view(&s.session).unwrap();
    let mut plan = arbiter
        .prepare_delegation(&s.gate, &admitted.grant, expected, at, request)
        .unwrap();
    let result = s.gate.admit(&mut plan);
    let after = arbiter.view(&s.session).unwrap();
    if mode != 0 {
        assert_eq!(
            result.err().unwrap().code,
            if mode == 1 {
                "budget"
            } else {
                "delegation_basis"
            }
        );
        assert_eq!(
            after.coordination().commitments()[&id("work")],
            before.coordination().commitments()[&id("work")]
        );
        assert_eq!(after.treasury(), before.treasury());
        assert_eq!(after.admission().assignments().len(), 1);
        assert!(
            s.gate
                .authorize(&admitted.grant, TeamOperation::BoardRead, at)
                .is_ok()
        );
        return;
    }
    let successor = result.unwrap();
    assert_eq!(plan.status(), AdmissionStatus::Delivered);
    assert_eq!(
        after.coordination().commitments()[&id("work")].state,
        CommitmentState::Delegated(id("next-agent"))
    );
    assert_eq!(
        after.coordination().commitments()[&id("successor")].state,
        CommitmentState::Active
    );
    assert!(
        s.gate
            .authorize(&admitted.grant, TeamOperation::BoardRead, at)
            .is_err()
    );
    assert!(
        s.gate
            .authorize(&successor.grant, TeamOperation::BoardRead, at)
            .is_ok()
    );
    assert!(s.gate.admit(&mut plan).is_err());
    let stored = s.journal.read(&s.session).unwrap();
    let replica = MemoryJournal::new();
    replica
        .append(&s.session, 0, &stored.events[..expected as usize])
        .unwrap();
    let packet = &stored.events[expected as usize..];
    for count in 1..packet.len() {
        assert!(
            replica
                .append(&s.session, expected, &packet[..count])
                .is_err()
        );
        assert_eq!(replica.read(&s.session).unwrap().revision, expected);
    }
    replica.append(&s.session, expected, packet).unwrap();
    assert_eq!(replica.view(&s.session, None).unwrap(), after);
}
#[test]
fn delegation_admits_once_and_refuses_budget_exhaustion_and_self_responsibility() {
    for mode in 0..3 {
        let root = Directory::new();
        let database = Directory::new();
        let store = SqliteJournal::open(database.database(), ParameterSchemas::default()).unwrap();
        delegation(
            setup(Arc::new(MemoryJournal::new()), &store, &root, false, 30),
            mode,
        );
    }
    let root = Directory::new();
    let database = Directory::new();
    let store =
        Arc::new(SqliteJournal::open(database.database(), ParameterSchemas::default()).unwrap());
    delegation(setup(store.clone(), &store, &root, false, 30), 0);
}
#[test]
fn generic_or_foreign_references_do_not_become_completion_or_cancellation() {
    let root = Directory::new();
    let database = Directory::new();
    let store = SqliteJournal::open(database.database(), ParameterSchemas::default()).unwrap();
    let s = setup(
        Arc::new(StaleReadJournal {
            inner: MemoryJournal::new(),
            stale: std::sync::Mutex::new(None),
        }),
        &store,
        &root,
        false,
        30,
    );
    let legacy = Setup::new_with_award(
        Arc::new(MemoryJournal::new()),
        &store,
        &root,
        false,
        "legacy-source",
        None,
        FirstOffer::new().unwrap(),
    );
    let award = legacy.award("legacy-work");
    let (expected, at, request) = legacy.request("legacy-assignment", award);
    let mut plan = legacy
        .gate
        .prepare(&legacy.session, expected, at, request)
        .unwrap();
    assert_eq!(
        legacy.gate.admit(&mut plan).err().unwrap().code,
        "commitment_terms"
    );
    assert!(
        legacy
            .gate
            .view(&legacy.session)
            .unwrap()
            .admission()
            .assignments()
            .is_empty()
    );
    let admitted = admit(&s);
    let arbiter = Arbiter::new(s.journal.clone());
    let before = arbiter.view(&s.session).unwrap();
    let known = before.coordination().commitments()[&id("work")]
        .history
        .last()
        .unwrap();
    for basis in [
        known.clone(),
        Ref {
            id: id("foreign"),
            version: Digest::of(b"foreign"),
        },
    ] {
        assert!(
            arbiter
                .discharge(
                    &s.gate,
                    &s.session,
                    before.revision(),
                    before.latest_at(),
                    &id("work"),
                    &basis
                )
                .is_err()
        );
        assert!(
            arbiter
                .cancel(
                    &s.gate,
                    &s.session,
                    before.revision(),
                    before.latest_at(),
                    &id("work"),
                    &basis
                )
                .is_err()
        );
        assert_eq!(arbiter.view(&s.session).unwrap(), before);
    }
    let old_read = s.journal.read(&s.session).unwrap();
    s.gate
        .revoke(
            &s.session,
            before.revision(),
            before.latest_at() + 1,
            &admitted.assignment.id,
            "Concurrent revocation".into(),
        )
        .unwrap();
    let revoked = arbiter.view(&s.session).unwrap();
    let next = solicitation(&s, "stale-release", 3.0);
    *s.journal.stale.lock().unwrap() = Some(old_read);
    assert_eq!(
        arbiter
            .release(
                &s.gate,
                &admitted.grant,
                revoked.revision(),
                revoked.latest_at(),
                "Stale holder".into(),
                next
            )
            .unwrap_err()
            .code,
        "stale_revision"
    );
    assert_eq!(arbiter.view(&s.session).unwrap(), revoked);
}
