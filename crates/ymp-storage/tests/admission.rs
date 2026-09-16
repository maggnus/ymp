//! Admission integration through actual services, without starting a backend.
mod support;
use support::Directory;
use ymp_kernel as kernel;
#[allow(dead_code)]
#[path = "../../ymp-kernel/tests/support/workspace_fixture.rs"]
mod fixture;
use ymp_storage as storage;
#[path = "support/admission_fixture.rs"]
mod admission_fixture;
use admission_fixture::Setup;
use std::sync::Arc;
use ymp_domain::{Id, assignment::*, coordination::*, workspace::WorkspacePath};
use ymp_kernel::{
    gatekeeper::AdmissionStatus,
    journal::{Journal, ParameterSchemas},
};
use ymp_runtime::memory_journal::MemoryJournal;
use ymp_storage::journal::SqliteJournal;
fn id<T>(s: &str) -> Id<T> {
    Id::new(s).unwrap()
}
fn exercise<J: Journal>(s: Setup<J>, root: &support::Directory) {
    let award = s.award("work");
    let (expected, at, request) = s.request("assignment", award.clone());
    let before = s.gate.view(&s.session).unwrap();
    let mut prepared = s.gate.prepare(&s.session, expected, at, request).unwrap();
    assert_eq!(prepared.status(), AdmissionStatus::Prepared);
    assert!(prepared.committed_assignment().is_none());
    assert_eq!(s.gate.view(&s.session).unwrap(), before);
    let admitted = s.gate.admit(&mut prepared).unwrap();
    assert_eq!(prepared.status(), AdmissionStatus::Delivered);
    assert_eq!(admitted.assignment.state, AssignmentState::Admitted);
    let view = s.gate.view(&s.session).unwrap();
    assert_eq!(view.admission().assignments().len(), 1);
    let stored = s.journal.read(&s.session).unwrap();
    let packet = &stored.events[expected as usize..];
    let replica = MemoryJournal::new();
    replica
        .append(&s.session, 0, &stored.events[..expected as usize])
        .unwrap();
    for length in 1..packet.len() {
        assert_eq!(
            replica
                .append(&s.session, expected, &packet[..length])
                .unwrap_err()
                .code,
            "admission_incomplete"
        );
        assert_eq!(replica.read(&s.session).unwrap().revision, expected);
    }
    let mut changed = packet.to_vec();
    for event in &mut changed {
        if let Event::GrantIssued { nonce, .. } = &mut event.payload {
            *nonce = ymp_domain::Digest::of(b"different attempt");
        }
    }
    assert!(replica.append(&s.session, expected, &changed).is_err());
    replica.append(&s.session, expected, packet).unwrap();
    for length in 1..packet.len() {
        assert!(
            replica
                .resolve_append(&s.session, expected, &packet[..length])
                .is_err()
        );
    }
    let standalone = MemoryJournal::new();
    standalone
        .append(&s.session, 0, &stored.events[..expected as usize])
        .unwrap();
    let mut funding = packet[0].clone();
    if let Event::ReservationChanged {
        change: ReservationChange::Reserved(data),
        ..
    } = &mut funding.payload
    {
        data.admission = None;
    }
    let (policy, input, refs) = ymp_kernel::treasury::attribution(&funding.payload).unwrap();
    funding.policy = policy;
    funding.input = input;
    funding.refs = refs;
    standalone.append(&s.session, expected, &[funding]).unwrap();
    assert!(
        standalone
            .append(&s.session, expected + 1, &packet[1..])
            .is_err()
    );

    assert_eq!(
        view.coordination().commitments()[&id("work")].state,
        CommitmentState::Active
    );
    assert_eq!(view.treasury().unwrap().accounts.len(), 1);
    assert_eq!(view.path_locks().len(), usize::from(s.files));
    assert_eq!(
        s.gate
            .authorize(&admitted.grant, TeamOperation::BoardRead, at)
            .unwrap(),
        admitted.assignment
    );
    assert!(
        s.gate
            .authorize(&admitted.grant, TeamOperation::NoticePost, at)
            .is_err()
    );
    assert!(
        s.gate
            .authorize(
                &admitted.grant,
                TeamOperation::BoardRead,
                at + admitted.assignment.allowance.timeout
            )
            .is_err()
    );
    assert!(s.gate.admit(&mut prepared).is_err());
    assert_eq!(s.gate.view(&s.session).unwrap(), view);
    if let Some(access) = admitted.files {
        assert!(
            access
                .write(&WorkspacePath::new("file").unwrap(), b"unstarted", at)
                .is_err()
        );
        s.gate
            .workspace()
            .authorize_access(
                &s.session,
                view.revision(),
                at,
                id("assignment"),
                id("invocation"),
                s.provider.as_ref(),
            )
            .unwrap();
        access
            .write(&WorkspacePath::new("file").unwrap(), b"admitted access", at)
            .unwrap();
        assert_eq!(
            std::fs::read(root.0.join("file")).unwrap(),
            b"admitted access"
        );
    }
    if !s.files {
        let (expected, at, request) = s.request("duplicate", award);
        assert!(s.gate.prepare(&s.session, expected, at, request).is_err());
    }
    assert_eq!(
        s.gate
            .view(&s.session)
            .unwrap()
            .admission()
            .assignments()
            .len(),
        1
    );
}
#[test]
fn memory_admission_commits_one_funded_no_files_assignment() {
    let root = support::Directory::new();
    let database = support::Directory::new();
    let store = SqliteJournal::open(database.database(), ParameterSchemas::default()).unwrap();
    exercise(
        Setup::new(Arc::new(MemoryJournal::new()), &store, &root, false),
        &root,
    );
}
#[test]
fn sqlite_admission_commits_resources_and_real_mediated_path_ownership() {
    let root = support::Directory::new();
    let database = support::Directory::new();
    let store =
        Arc::new(SqliteJournal::open(database.database(), ParameterSchemas::default()).unwrap());
    exercise(Setup::new(store.clone(), &store, &root, true), &root);
    let reopened = SqliteJournal::open(database.database(), ParameterSchemas::default()).unwrap();
    assert_eq!(
        reopened
            .read(&id("admission"))
            .unwrap()
            .view_with_schemas(&id("admission"), None, reopened.schemas())
            .unwrap()
            .admission()
            .assignments()
            .len(),
        1
    );
}
#[test]
fn denial_cancels_only_the_proposed_commitment_without_partial_resources() {
    let root = support::Directory::new();
    let database = support::Directory::new();
    let store = SqliteJournal::open(database.database(), ParameterSchemas::default()).unwrap();
    let s = Setup::new(Arc::new(MemoryJournal::new()), &store, &root, false);
    let award = s.award("work");
    let (expected, at, mut request) = s.request("bad", award);
    request.role = RoleKind::Producer;
    let mut prepared = s.gate.prepare(&s.session, expected, at, request).unwrap();
    assert_eq!(s.gate.view(&s.session).unwrap().revision(), expected);
    assert!(s.gate.admit(&mut prepared).is_err());
    let view = s.gate.view(&s.session).unwrap();
    assert!(view.admission().assignments().is_empty());
    assert!(view.treasury().unwrap().accounts.is_empty());
    assert!(view.path_locks().is_empty());
    assert!(matches!(
        view.coordination().commitments()[&id("work")].state,
        CommitmentState::Cancelled(_)
    ));
    assert_eq!(
        view.coordination().solicitations()[&id("work")].value.state,
        SolicitationState::Withdrawn
    );
}

use std::{
    collections::BTreeMap,
    sync::atomic::{AtomicUsize, Ordering},
};
use ymp_domain::{Denial, Ref, Result, journal::Envelope};
use ymp_kernel::{
    events::Event,
    journal::{JournalRead, WorkspaceInventory},
    treasury::ReservationChange,
};
/// 1: consistent grant alteration; 2: lost ACK and unreadable journal;
/// 3/4: exact commit proved, then local capability delivery cannot read state.
struct AdmissionJournal<J: Journal> {
    inner: Arc<J>,
    mode: AtomicUsize,
    remaining_reads: AtomicUsize,
    admission_appends: AtomicUsize,
    cancellation_mode: AtomicUsize,
    late_packet: std::sync::Mutex<Option<Vec<Envelope<Event>>>>,
}
impl<J: Journal> AdmissionJournal<J> {
    fn new(inner: Arc<J>) -> Self {
        Self {
            inner,
            mode: AtomicUsize::new(0),
            remaining_reads: AtomicUsize::new(usize::MAX),
            admission_appends: AtomicUsize::new(0),
            cancellation_mode: AtomicUsize::new(0),
            late_packet: std::sync::Mutex::new(None),
        }
    }
}
impl<J: Journal> Journal for AdmissionJournal<J> {
    fn schemas(&self) -> &ParameterSchemas {
        self.inner.schemas()
    }
    fn binding_identity(&self) -> Result<ymp_domain::workspace::JournalIdentity> {
        self.inner.binding_identity()
    }
    fn workspace_binding(
        &self,
        root: &ymp_domain::workspace::WorkspaceLocation,
    ) -> Result<Option<ymp_domain::workspace::WorkspaceBinding>> {
        self.inner.workspace_binding(root)
    }
    fn read(&self, session: &Id) -> Result<JournalRead> {
        let remaining = self.remaining_reads.load(Ordering::SeqCst);
        if remaining == 0 {
            return Err(Denial::new(
                "fixture_read",
                "Journal temporarily unavailable",
            ));
        }
        if remaining != usize::MAX {
            self.remaining_reads.fetch_sub(1, Ordering::SeqCst);
        }
        self.inner.read(session)
    }
    fn workspace_inventory(&self, session: &Id) -> Result<WorkspaceInventory> {
        self.inner.workspace_inventory(session)
    }
    fn append(&self, session: &Id, expected: u64, events: &[Envelope<Event>]) -> Result<u64> {
        if events.iter().any(|event| {
            matches!(
                event.payload,
                Event::CommitmentChanged {
                    change: ymp_kernel::arbiter::CommitmentChange::Cancelled { .. },
                    ..
                }
            )
        }) {
            match self.cancellation_mode.swap(0, Ordering::SeqCst) {
                5 => {
                    let packet = self.late_packet.lock().unwrap().take().unwrap();
                    self.inner.append(session, expected, &packet)?;
                    return Err(Denial::new(
                        "stale_revision",
                        "Original admission committed before cancellation",
                    ));
                }
                6 => {
                    return Err(Denial::new(
                        "fixture_cancel",
                        "Cancellation temporarily unavailable",
                    ));
                }
                _ => {}
            }
        }
        if !events
            .iter()
            .any(|event| matches!(event.payload, Event::AssignmentAdmitted { .. }))
        {
            return self.inner.append(session, expected, events);
        }
        self.admission_appends.fetch_add(1, Ordering::SeqCst);
        let mode = self.mode.swap(0, Ordering::SeqCst);
        if mode == 5 || mode == 6 {
            *self.late_packet.lock().unwrap() = Some(events.to_vec());
            self.cancellation_mode.store(mode, Ordering::SeqCst);
            return Err(Denial::new("path_conflict", "Injected append-time refusal"));
        }
        if mode == 1 {
            let mut changed = vec![];
            let mut refs = BTreeMap::<Ref, Ref>::new();
            for source in events {
                let mut event = source.clone();
                for reference in &mut event.refs {
                    if let Some(replacement) = refs.get(reference) {
                        *reference = replacement.clone();
                    }
                }
                match &mut event.payload {
                    Event::ReservationChanged {
                        change: ReservationChange::Reserved(data),
                        ..
                    } => {
                        data.admission
                            .as_mut()
                            .unwrap()
                            .grant
                            .operations
                            .insert(TeamOperation::NoticePost);
                    }
                    Event::GrantIssued { grant, .. } => {
                        grant.operations.insert(TeamOperation::NoticePost);
                    }
                    _ => {}
                }
                event.refs.sort();
                event.refs.dedup();
                refs.insert(source.reference()?, event.reference()?);
                changed.push(event);
            }
            return self.inner.append(session, expected, &changed);
        }
        let result = self.inner.append(session, expected, events)?;
        if mode == 2 {
            self.remaining_reads.store(0, Ordering::SeqCst);
            return Err(Denial::new(
                "fixture_ack",
                "Committed admission acknowledgement lost",
            ));
        }
        if mode == 3 || mode == 4 {
            self.remaining_reads.store(mode - 2, Ordering::SeqCst);
        }
        Ok(result)
    }
}
#[test]
fn a_self_consistent_adapter_rewrite_cannot_expand_the_requested_grant() {
    let root = Directory::new();
    let database = Directory::new();
    let store = SqliteJournal::open(database.database(), ParameterSchemas::default()).unwrap();
    let journal = Arc::new(AdmissionJournal::new(Arc::new(MemoryJournal::new())));
    let s = Setup::new(journal.clone(), &store, &root, false);
    let award = s.award("work");
    let (expected, at, request) = s.request("assignment", award);
    let mut prepared = s.gate.prepare(&s.session, expected, at, request).unwrap();
    journal.mode.store(1, Ordering::SeqCst);
    assert!(s.gate.admit(&mut prepared).is_err());
    assert_eq!(prepared.status(), AdmissionStatus::Prepared);
    assert!(prepared.committed_assignment().is_none());
    let view = s.gate.view(&s.session).unwrap();
    assert!(
        view.admission().assignments()[&id("assignment")]
            .intent
            .grant
            .operations
            .contains(&TeamOperation::NoticePost)
    );
    assert!(s.gate.admit(&mut prepared).is_err());
    assert_eq!(journal.admission_appends.load(Ordering::SeqCst), 1);
}
#[test]
fn lost_ack_and_post_commit_delivery_failure_do_not_recommit_or_duplicate_capabilities() {
    for mode in [2, 3, 4] {
        let root = Directory::new();
        let database = Directory::new();
        let store = Arc::new(
            SqliteJournal::open(database.database(), ParameterSchemas::default()).unwrap(),
        );
        let journal = Arc::new(AdmissionJournal::new(store.clone()));
        let s = Setup::new(journal.clone(), &store, &root, mode == 4);
        let award = s.award("work");
        let (expected, at, request) = s.request("assignment", award);
        let mut prepared = s.gate.prepare(&s.session, expected, at, request).unwrap();
        journal.mode.store(mode, Ordering::SeqCst);
        assert!(s.gate.admit(&mut prepared).is_err());
        assert_eq!(
            prepared.status(),
            if mode == 2 {
                AdmissionStatus::Prepared
            } else {
                AdmissionStatus::Committed
            }
        );
        journal.remaining_reads.store(usize::MAX, Ordering::SeqCst);
        let before = s.gate.view(&s.session).unwrap();
        assert_eq!(before.admission().assignments().len(), 1);
        let admitted = s.gate.admit(&mut prepared).unwrap();
        assert_eq!(s.gate.view(&s.session).unwrap(), before);
        assert_eq!(journal.admission_appends.load(Ordering::SeqCst), 1);
        assert!(
            s.gate
                .authorize(&admitted.grant, TeamOperation::BoardRead, at)
                .is_ok()
        );
        assert!(s.gate.admit(&mut prepared).is_err());
    }
}

#[test]
fn a_competing_admission_or_rejection_cannot_cancel_the_winning_assignment() {
    let root = Directory::new();
    let database = Directory::new();
    let store = SqliteJournal::open(database.database(), ParameterSchemas::default()).unwrap();
    let s = Setup::new(Arc::new(MemoryJournal::new()), &store, &root, false);
    let award = s.award("work");
    let (expected, at, request) = s.request("winner", award.clone());
    let mut winner = s.gate.prepare(&s.session, expected, at, request).unwrap();
    let (_, _, request) = s.request("loser", award.clone());
    let mut loser = s.gate.prepare(&s.session, expected, at, request).unwrap();
    let (_, _, mut request) = s.request("rejected", award);
    request.role = RoleKind::Producer;
    let mut rejected = s.gate.prepare(&s.session, expected, at, request).unwrap();
    let result = s.gate.admit(&mut winner).unwrap();
    let before = s.gate.view(&s.session).unwrap();
    assert!(s.gate.admit(&mut loser).is_err());
    assert!(s.gate.admit(&mut rejected).is_err());
    assert_eq!(s.gate.view(&s.session).unwrap(), before);
    assert!(
        s.gate
            .authorize(&result.grant, TeamOperation::BoardRead, at)
            .is_ok()
    );
    assert_eq!(
        before.coordination().commitments()[&id("work")].state,
        CommitmentState::Active
    );
}
#[test]
fn declaring_no_file_needs_does_not_prove_absence_of_actual_file_capabilities() {
    let root = Directory::new();
    let database = Directory::new();
    let store = SqliteJournal::open(database.database(), ParameterSchemas::default()).unwrap();
    let s = Setup::new(Arc::new(MemoryJournal::new()), &store, &root, false);
    let award = s.award("work");
    let view = s.gate.view(&s.session).unwrap();
    let prior = view.registry().unwrap().clone();
    let mut facts = prior.input.facts;
    facts.discoveries[0].provider.capabilities = Some(std::collections::BTreeSet::from([
        ymp_domain::journal::Capability::ReadFiles,
        ymp_domain::journal::Capability::WriteFiles,
    ]));
    let registry = ymp_kernel::registry::Registry::new(s.journal.clone());
    let at = view.latest_at() + 1;
    let input = registry.prepare(&s.session, facts, at).unwrap();
    let responses = ymp_kernel::registry::readiness_views(&input)
        .iter()
        .map(|view| ymp_kernel::registry::ReadinessResponse {
            profile: view.profile.clone(),
            input: ymp_domain::Digest::of_value(view).unwrap(),
            proposal: ymp_domain::Proposal {
                value: ymp_domain::identity::Readiness::Ready,
                rationale: "Broader actual capabilities".into(),
                basis: vec![],
                policy: prior.effective.policy.clone(),
            },
        })
        .collect();
    registry
        .record(
            &s.session,
            view.revision(),
            at,
            input,
            prior.effective,
            responses,
        )
        .unwrap();
    let (expected, at, request) = s.request("assignment", award);
    let mut plan = s.gate.prepare(&s.session, expected, at, request).unwrap();
    let denial = s.gate.admit(&mut plan).err().unwrap();
    assert_eq!(denial.code, "execution_boundary");
    assert!(!denial.refs.is_empty());
    assert!(
        s.gate
            .view(&s.session)
            .unwrap()
            .admission()
            .assignments()
            .is_empty()
    );
}
#[test]
fn revoke_invalidates_the_grant_without_settling_or_releasing_held_resources() {
    let root = Directory::new();
    let database = Directory::new();
    let journal =
        Arc::new(SqliteJournal::open(database.database(), ParameterSchemas::default()).unwrap());
    let s = Setup::new(journal.clone(), &journal, &root, true);
    let award = s.award("work");
    let (expected, at, request) = s.request("assignment", award);
    let mut plan = s.gate.prepare(&s.session, expected, at, request).unwrap();
    let admitted = s.gate.admit(&mut plan).unwrap();
    let view = s.gate.view(&s.session).unwrap();
    s.gate
        .revoke(
            &s.session,
            view.revision(),
            at + 1,
            &admitted.assignment.id,
            "User stop".into(),
        )
        .unwrap();
    let stopped = s.gate.view(&s.session).unwrap();
    assert_eq!(
        stopped.admission().assignments()[&id("assignment")]
            .intent
            .assignment
            .state,
        AssignmentState::Revoked
    );
    let financial = &stopped.treasury().unwrap().accounts[&id("assignment")];
    assert!(financial.revoked);
    assert_eq!(
        financial.reservation.state,
        ymp_domain::resources::ReservationState::Held
    );
    assert!(financial.settlement.is_none());
    let paths = &stopped.path_locks()[&id("assignment")];
    assert!(paths.revoked);
    assert!(paths.released.is_none());
    assert!(
        s.gate
            .authorize(&admitted.grant, TeamOperation::BoardRead, at + 1)
            .is_err()
    );
    assert!(
        admitted
            .files
            .unwrap()
            .write(&WorkspacePath::new("file").unwrap(), b"revoked", at + 1)
            .is_err()
    );
    assert!(!root.0.join("file").exists());
}

#[test]
fn independent_producers_share_the_role_with_disjoint_enforced_paths() {
    let root = Directory::new();
    let database = Directory::new();
    let journal =
        Arc::new(SqliteJournal::open(database.database(), ParameterSchemas::default()).unwrap());
    let mut s = Setup::new(journal.clone(), &journal, &root, true);
    let award = s.award("left-work");
    let (expected, at, request) = s.request_path("left-assignment", award, "left");
    let mut plan = s.gate.prepare(&s.session, expected, at, request).unwrap();
    let left = s.gate.admit(&mut plan).unwrap();
    s.switch_to_new_agent("second");
    let award = s.award("right-work");
    let (expected, at, request) = s.request_path("right-assignment", award, "right");
    let mut plan = s.gate.prepare(&s.session, expected, at, request).unwrap();
    let right = s.gate.admit(&mut plan).unwrap();
    assert_eq!(left.assignment.role, RoleKind::Producer);
    assert_eq!(right.assignment.role, RoleKind::Producer);
    assert_ne!(left.assignment.agent, right.assignment.agent);
    assert_eq!(
        s.gate
            .view(&s.session)
            .unwrap()
            .admission()
            .assignments()
            .len(),
        2
    );
}
#[test]
fn cross_session_file_conflict_cancels_only_the_losing_proposed_commitment() {
    let root = Directory::new();
    let database = Directory::new();
    let journal =
        Arc::new(SqliteJournal::open(database.database(), ParameterSchemas::default()).unwrap());
    let first = Setup::new_named(journal.clone(), &journal, &root, true, "first");
    let second = Setup::new_named(journal.clone(), &journal, &root, true, "second");
    let award = first.award("work");
    let (expected, at, request) = first.request("assignment", award);
    let mut a = first
        .gate
        .prepare(&first.session, expected, at, request)
        .unwrap();
    let award = second.award("work");
    let (expected, at, request) = second.request("assignment", award);
    let mut b = second
        .gate
        .prepare(&second.session, expected, at, request)
        .unwrap();
    let (a, b) = std::thread::scope(|scope| {
        let a = scope.spawn(|| first.gate.admit(&mut a));
        let b = scope.spawn(|| second.gate.admit(&mut b));
        (a.join().unwrap(), b.join().unwrap())
    });
    assert_eq!(usize::from(a.is_ok()) + usize::from(b.is_ok()), 1);
    let loser = if a.is_err() { &first } else { &second };
    let winner = if a.is_ok() { &first } else { &second };
    let lost = loser.gate.view(&loser.session).unwrap();
    assert!(matches!(
        lost.coordination().commitments()[&id("work")].state,
        CommitmentState::Cancelled(_)
    ));
    assert!(lost.admission().assignments().is_empty());
    assert!(lost.treasury().unwrap().accounts.is_empty());
    assert!(lost.path_locks().is_empty());
    assert_eq!(
        winner
            .gate
            .view(&winner.session)
            .unwrap()
            .admission()
            .assignments()
            .len(),
        1
    );
}

#[test]
fn admission_counts_admitted_work_and_does_not_reset_attempts_with_new_contribution_ids() {
    for (parallel, members, attempts, expected_code) in [
        (2, 4, 4, "parallel_limit"),
        (4, 2, 4, "member_limit"),
        (4, 4, 2, "attempt_limit"),
    ] {
        let root = Directory::new();
        let database = Directory::new();
        let store = SqliteJournal::open(database.database(), ParameterSchemas::default()).unwrap();
        let mut constraints = fixture::default_constraints(Default::default());
        constraints.parallel_limit = parallel;
        constraints.max_members = members;
        constraints.attempt_limit = attempts;
        let mut s = Setup::new_config(
            Arc::new(MemoryJournal::new()),
            &store,
            &root,
            false,
            "limits",
            Some(constraints),
        );
        let first = s.profile.clone();
        s.switch_to_new_agent("second");
        let second = s.profile.clone();
        s.switch_to_new_agent("third");
        let third = s.profile.clone();
        for (name, profile) in [("first", first), ("second", second)] {
            s.profile = profile;
            let award = s.award(name);
            let (expected, at, request) = s.request(name, award);
            let mut plan = s.gate.prepare(&s.session, expected, at, request).unwrap();
            s.gate.admit(&mut plan).unwrap();
        }
        s.profile = third;
        let award = s.award("third");
        let (expected, at, request) = s.request("third", award);
        let mut plan = s.gate.prepare(&s.session, expected, at, request).unwrap();
        let denial = s.gate.admit(&mut plan).err().unwrap();
        assert_eq!(denial.code, expected_code);
        assert!(!denial.refs.is_empty());
        let view = s.gate.view(&s.session).unwrap();
        assert_eq!(view.admission().assignments().len(), 2);
        assert_eq!(view.treasury().unwrap().accounts.len(), 2);
    }
}
#[test]
fn current_owner_pins_override_an_older_award_without_granting_partial_authority() {
    let root = Directory::new();
    let database = Directory::new();
    let store = SqliteJournal::open(database.database(), ParameterSchemas::default()).unwrap();
    let s = Setup::new(Arc::new(MemoryJournal::new()), &store, &root, false);
    let award = s.award("work");
    let (_, _, request) = s.request("assignment", award);
    let before = s.gate.view(&s.session).unwrap();
    let mut constraints = before.task().unwrap().constraints.clone();
    constraints.pins.models = Some(std::collections::BTreeSet::from([
        "another-model".to_owned()
    ]));
    s.intake
        .refine(
            &s.control,
            ymp_kernel::intake::IntakeRefinement {
                expected_revision: before.revision(),
                at: before.latest_at() + 1,
                constraints,
                criteria: before.criteria().to_vec(),
                reason: "Owner changed the permitted model".into(),
                note: ymp_kernel::intake::IntakeNote::Clarification(
                    ymp_domain::task::Clarification {
                        question: "Which model is now permitted?".into(),
                        answer: "another-model".into(),
                        at: before.latest_at() + 1,
                    },
                ),
            },
        )
        .unwrap();
    let view = s.gate.view(&s.session).unwrap();
    let mut plan = s
        .gate
        .prepare(&s.session, view.revision(), view.latest_at() + 1, request)
        .unwrap();
    assert!(s.gate.admit(&mut plan).is_err());
    let view = s.gate.view(&s.session).unwrap();
    assert!(view.admission().assignments().is_empty());
    assert!(view.treasury().unwrap().accounts.is_empty());
    assert!(view.path_locks().is_empty());
}
#[test]
fn budget_and_capability_denials_are_recorded_without_partial_reservation() {
    for over_budget in [false, true] {
        let root = Directory::new();
        let database = Directory::new();
        let store = SqliteJournal::open(database.database(), ParameterSchemas::default()).unwrap();
        let s = Setup::new(Arc::new(MemoryJournal::new()), &store, &root, false);
        let award = s.award("work");
        let (expected, at, mut request) = s.request("assignment", award);
        if over_budget {
            request.allowance.proposal.value.cost = ymp_domain::task::Real::new(100.0).unwrap();
        } else {
            request
                .access
                .insert(ymp_domain::journal::Capability::Network);
        }
        let mut plan = s.gate.prepare(&s.session, expected, at, request).unwrap();
        let denial = s.gate.admit(&mut plan).err().unwrap();
        assert!(!denial.refs.is_empty());
        let view = s.gate.view(&s.session).unwrap();
        assert!(view.admission().assignments().is_empty());
        assert!(view.treasury().unwrap().accounts.is_empty());
        assert_eq!(view.treasury().unwrap().budget.held.get(), 0.0);
    }
}

#[test]
fn cancellation_retains_the_original_plan_until_the_winning_packet_is_known() {
    for mode in [5, 6] {
        let root = Directory::new();
        let database = Directory::new();
        let store = SqliteJournal::open(database.database(), ParameterSchemas::default()).unwrap();
        let journal = Arc::new(AdmissionJournal::new(Arc::new(MemoryJournal::new())));
        let s = Setup::new(journal.clone(), &store, &root, false);
        let award = s.award("work");
        let (expected, at, request) = s.request("assignment", award);
        let mut prepared = s.gate.prepare(&s.session, expected, at, request).unwrap();
        journal.mode.store(mode, Ordering::SeqCst);
        let result = s.gate.admit(&mut prepared);
        if mode == 5 {
            let admitted = result.unwrap();
            assert_eq!(prepared.status(), AdmissionStatus::Delivered);
            assert_eq!(
                s.gate
                    .authorize(&admitted.grant, TeamOperation::BoardRead, at)
                    .unwrap()
                    .id,
                id("assignment")
            );
            assert_eq!(
                s.gate
                    .view(&s.session)
                    .unwrap()
                    .coordination()
                    .commitments()[&id("work")]
                    .state,
                CommitmentState::Active
            );
        } else {
            assert_eq!(
                result.err().unwrap().code,
                "admission_cancellation_uncertain"
            );
            assert_eq!(prepared.status(), AdmissionStatus::Prepared);
            assert_eq!(
                s.gate
                    .view(&s.session)
                    .unwrap()
                    .coordination()
                    .commitments()[&id("work")]
                    .state,
                CommitmentState::Proposed
            );
            assert_eq!(
                s.gate.admit(&mut prepared).err().unwrap().code,
                "path_conflict"
            );
            assert_eq!(prepared.status(), AdmissionStatus::Delivered);
            let view = s.gate.view(&s.session).unwrap();
            assert!(view.admission().assignments().is_empty());
            assert!(matches!(
                view.coordination().commitments()[&id("work")].state,
                CommitmentState::Cancelled(_)
            ));
        }
        assert_eq!(journal.admission_appends.load(Ordering::SeqCst), 1);
    }
}

#[test]
fn application_runtime_connects_policies_scope_preparation_and_atomic_admission() {
    for (files, missing_provider) in [(false, false), (true, false), (true, true)] {
        let root = Directory::new();
        let database = Directory::new();
        let journal = Arc::new(
            SqliteJournal::open(database.database(), ParameterSchemas::default()).unwrap(),
        );
        let s = Setup::new(journal.clone(), &journal, &root, files);
        let award = s.award("work");
        let app = ymp_runtime::application::Application::new(journal.clone());
        let runtime = app.admission(Arc::new(journal.content_store()));
        let view = app.view(&s.session, None).unwrap();
        let at = view.latest_at() + 1;
        let command = ymp_runtime::admission::AdmissionCommand {
            assignment: id("assignment"),
            award,
            role: if files {
                RoleKind::Producer
            } else {
                RoleKind::Planner
            },
            workspace: id("workspace"),
            access: if files {
                std::collections::BTreeSet::from([
                    ymp_domain::journal::Capability::ReadFiles,
                    ymp_domain::journal::Capability::WriteFiles,
                ])
            } else {
                Default::default()
            },
            paths: if files {
                vec![
                    (
                        WorkspacePath::new("file").unwrap(),
                        ymp_domain::workspace::LockMode::Read,
                    ),
                    (
                        WorkspacePath::new("file").unwrap(),
                        ymp_domain::workspace::LockMode::Write,
                    ),
                ]
            } else {
                vec![]
            },
            reservation: id("assignment"),
            grant: id("assignment"),
            operations: std::collections::BTreeSet::from([TeamOperation::BoardRead]),
            lease: Lease {
                expires: at + 20,
                renew_on: Default::default(),
                renewals_left: 0,
            },
        };
        let provider: Option<Arc<dyn ymp_kernel::ports::execution::WorkspaceProvider>> =
            if files && !missing_provider {
                Some(s.provider.clone())
            } else {
                None
            };
        let mut prepared = runtime
            .prepare(
                &s.session,
                view.revision(),
                at,
                command,
                ymp_runtime::admission::AdmissionPolicies {
                    cost: &s.cost,
                    resources: &s.resource,
                },
                provider,
            )
            .unwrap();
        let admitted = runtime.admit(&mut prepared);
        if missing_provider {
            assert_eq!(admitted.err().unwrap().code, "workspace_provider");
            assert!(
                app.view(&s.session, None)
                    .unwrap()
                    .admission()
                    .assignments()
                    .is_empty()
            );
        } else {
            let admitted = admitted.unwrap();
            assert!(
                runtime
                    .authorize(&admitted.grant, TeamOperation::BoardRead, at)
                    .is_ok()
            );
            if let Some(files) = admitted.files {
                runtime.release_files(&s.session, at, &files).unwrap();
            }
        }
    }
}
