//! Persisted award foundations; these tests do not claim assignment admission or execution.
mod support;
use ymp_kernel as kernel;
#[allow(dead_code)]
#[path = "../../ymp-kernel/tests/support/workspace_fixture.rs"]
mod fixture;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};
use ymp_domain::{
    Digest, Id, Prob, Proposal, Result,
    assignment::{
        Contribution, ContributionAuthor, ContributionKind, CostEstimate, Difficulty, Forecast,
        ForecastSource,
    },
    coordination::*,
    identity::{ExecutionProfile, ProfileSettings, Readiness},
    journal::{Capability, PolicySelection},
    task::Real,
};
use ymp_kernel::{
    arbiter::{Arbiter, AwardView, award_view},
    journal::{Journal, ParameterSchemas},
    ports::organization::AwardPolicy,
    registry::{ReadinessResponse, Registry, readiness_views},
};
use ymp_runtime::{
    memory_journal::MemoryJournal, policies::award::FirstOffer, workspace::direct::Direct,
};
use ymp_storage::journal::SqliteJournal;
fn id<T>(s: &str) -> Id<T> {
    Id::new(s).unwrap()
}
fn real(n: f64) -> Real {
    Real::new(n).unwrap()
}
struct LatestFixture {
    selection: PolicySelection,
}
impl AwardPolicy for LatestFixture {
    fn selection(&self) -> &PolicySelection {
        &self.selection
    }
    fn award(&self, view: &AwardView) -> Result<Proposal<Award>> {
        let offer = view
            .offers
            .iter()
            .max_by_key(|offer| offer.value.at)
            .unwrap();
        Ok(Proposal {
            value: Award {
                solicitation: view.solicitation.value.id.clone(),
                offer: offer.value.id.clone(),
                rationale: "Choose the latest fixture offer".into(),
            },
            rationale: "Different fixture strategy".into(),
            basis: vec![offer.reference.clone()],
            policy: self.selection.policy.clone(),
        })
    }
}
fn schemas() -> ParameterSchemas {
    let mut schemas = ParameterSchemas::default();
    schemas
        .register("AwardPolicy", "LatestFixture", "1", |selection| {
            assert_eq!(selection.parameters, serde_json::json!({}));
            Ok(())
        })
        .unwrap();
    schemas
}
fn exercise<J: Journal>(
    journal: Arc<J>,
    storage: &SqliteJournal,
    root: &support::Directory,
    policy: &dyn AwardPolicy,
    expected_agent: &str,
) {
    let provider = Direct::open(&root.0, ymp_domain::workspace::CaptureLimits::default()).unwrap();
    let session = id("awards");
    let (_, first) = fixture::open_with_policies(
        journal.clone(),
        Arc::new(storage.content_store()),
        &session,
        &provider,
        BTreeSet::from([Capability::ReadFiles, Capability::WriteFiles]),
        vec![policy.selection().clone()],
    );
    let arbiter = Arbiter::new(journal.clone());
    let prior = arbiter.view(&session).unwrap().registry().unwrap().clone();
    let mut facts = prior.input.facts.clone();
    let mut second = facts.agents[0].clone();
    second.id = id("second");
    facts.agents.push(second);
    let registry = Registry::new(journal.clone());
    let input = registry.prepare(&session, facts, 5).unwrap();
    let responses = readiness_views(&input)
        .iter()
        .map(|view| ReadinessResponse {
            profile: view.profile.clone(),
            input: Digest::of_value(view).unwrap(),
            proposal: Proposal {
                value: Readiness::Ready,
                rationale: "Explicit second Scripted fixture".into(),
                basis: vec![],
                policy: prior.effective.policy.clone(),
            },
        })
        .collect();
    registry
        .record(&session, 4, 5, input, prior.effective, responses)
        .unwrap();
    let second = registry
        .profile(&session, &id("second"), &ProfileSettings::default())
        .unwrap();
    let forecast = Forecast {
        p_success: Prob::new(0.5).unwrap(),
        delta_belief: BTreeMap::new(),
        source: ForecastSource::Model(policy.selection().policy.clone()),
    };
    let contribution = Contribution {
        id: id("contribution"),
        session: session.clone(),
        kind: ContributionKind::Produce,
        targets: BTreeSet::from([id("criterion")]),
        subject: None,
        needs: BTreeSet::from([Capability::WriteFiles]),
        forecast: forecast.clone(),
        cost: CostEstimate {
            expected: real(1.0),
            p90: real(2.0),
        },
        difficulty: Difficulty::Standard,
        proposed_by: ContributionAuthor::Runtime,
        basis: vec![],
    };
    let mut invalid_subject = contribution.clone();
    invalid_subject.subject = Some(ymp_domain::assignment::ContributionSubject::ResultVersion(
        arbiter
            .view(&session)
            .unwrap()
            .contract()
            .unwrap()
            .reference(),
    ));
    assert_eq!(
        arbiter
            .propose(&session, 5, 6, invalid_subject)
            .unwrap_err()
            .code,
        "subject_unsupported"
    );
    let mut invalid_subject = contribution.clone();
    invalid_subject.subject = Some(ymp_domain::assignment::ContributionSubject::WorkItem(
        ymp_domain::Ref {
            id: id("unknown"),
            version: Digest::of(b"missing"),
        },
    ));
    assert!(arbiter.propose(&session, 5, 6, invalid_subject).is_err());
    arbiter
        .propose(&session, 5, 6, contribution.clone())
        .unwrap();
    arbiter
        .view(&session)
        .unwrap()
        .resolve(&contribution.reference().unwrap())
        .unwrap();
    arbiter
        .open(
            &session,
            6,
            7,
            Solicitation {
                id: id("solicitation"),
                contribution: contribution.id.clone(),
                stimulus: real(1.0),
                deadline: 10,
                eligible: BTreeSet::from([first.agent.clone(), second.agent.clone()]),
                visibility: SolicitationVisibility::Open,
                reopened: 0,
                state: SolicitationState::Open,
            },
        )
        .unwrap();
    let offer = |name: &str, profile: ExecutionProfile, at: u64| Offer {
        id: id(name),
        solicitation: id("solicitation"),
        agent: profile.agent.clone(),
        profile,
        forecast: forecast.clone(),
        cost: CostEstimate {
            expected: real(1.0),
            p90: real(2.0),
        },
        approach: format!("Approach from {name}"),
        source: OfferSource::RuntimeProxy,
        at,
    };
    arbiter
        .submit(&session, 7, 8, offer("first", first, 8))
        .unwrap();
    arbiter
        .submit(&session, 8, 9, offer("second", second.clone(), 9))
        .unwrap();
    let before = arbiter.view(&session).unwrap();
    assert!(
        arbiter
            .submit(&session, 9, 1, offer("before-opening", second.clone(), 1))
            .is_err()
    );
    assert!(
        arbiter
            .submit(&session, 9, 101, offer("late", second, 101))
            .is_err()
    );
    assert_eq!(arbiter.view(&session).unwrap(), before);
    assert_eq!(
        award_view(&before, &id("solicitation"), 9)
            .unwrap_err()
            .code,
        "solicitation_open"
    );
    let input = award_view(&before, &id("solicitation"), 10).unwrap();
    let proposal = policy.award(&input).unwrap();
    let picked = &before.coordination().offers()[&proposal.value.offer].value;
    assert_eq!(picked.agent.as_str(), expected_agent);
    let commitment = Commitment {
        id: id("commitment"),
        debtor: picked.agent.clone(),
        creditor: Creditor::Runtime,
        subject: contribution.id,
        condition: None,
        lease: Lease {
            expires: 60,
            renew_on: BTreeSet::from([ProgressSignal::EvidenceAdded]),
            renewals_left: 2,
        },
        state: CommitmentState::Proposed,
        history: vec![],
    };
    assert!(
        arbiter
            .award(
                &session,
                9,
                10,
                proposal.clone(),
                Digest::of(b"wrong view"),
                commitment.clone()
            )
            .is_err()
    );
    assert_eq!(arbiter.view(&session).unwrap(), before);
    arbiter
        .award(
            &session,
            9,
            10,
            proposal,
            Digest::of_value(&input).unwrap(),
            commitment,
        )
        .unwrap();
    let complete = arbiter.view(&session).unwrap();
    assert_eq!(complete.revision(), 11);
    let commitment = &complete.coordination().commitments()[&id("commitment")];
    assert_eq!(commitment.state, CommitmentState::Proposed);
    assert_eq!(commitment.history.len(), 2);
    let awarded = &complete.coordination().awards()[&id("solicitation")].value;
    assert_eq!(&awarded.decision.effective, policy.selection());
    assert!(complete.treasury().is_none());
    assert!(complete.path_locks().is_empty());
    assert!(
        journal
            .read(&session)
            .unwrap()
            .view_with_schemas(&session, Some(10), journal.schemas())
            .is_err()
    );
    // Replaying a complete prior state cannot authorize appending either packet half.
    let events = journal.read(&session).unwrap().events;
    let replica = MemoryJournal::with_schemas(schemas());
    replica.append(&session, 0, &events[..9]).unwrap();
    assert_eq!(
        replica
            .append(&session, 9, &events[9..10])
            .unwrap_err()
            .code,
        "award_incomplete"
    );
    assert_eq!(replica.read(&session).unwrap().revision, 9);
    let mut interleaved = events[8].clone();
    interleaved.seq = 11;
    assert_eq!(
        replica
            .append(&session, 9, &[events[9].clone(), interleaved])
            .unwrap_err()
            .code,
        "award_incomplete"
    );
    let mut forged = events[10].clone();
    if let ymp_kernel::events::Event::CommitmentChanged {
        change: ymp_kernel::arbiter::CommitmentChange::Proposed(commitment),
        ..
    } = &mut forged.payload
    {
        commitment.debtor = id("another-debtor");
    }
    assert_eq!(
        replica
            .append(&session, 9, &[events[9].clone(), forged])
            .unwrap_err()
            .code,
        "commitment"
    );
    assert_eq!(replica.read(&session).unwrap().revision, 9);
    replica.append(&session, 9, &events[9..]).unwrap();
    assert!(replica.resolve_append(&session, 9, &events[9..10]).is_err());
    assert_eq!(
        replica
            .read(&session)
            .unwrap()
            .view_with_schemas(&session, None, replica.schemas())
            .unwrap(),
        complete
    );
}
#[test]
fn real_award_consumer_supports_different_strategies_and_atomic_replay() {
    for latest in [false, true] {
        let root = support::Directory::new();
        let database = support::Directory::new();
        let sqlite = SqliteJournal::open(database.database(), schemas()).unwrap();
        let policy: Box<dyn AwardPolicy> = if latest {
            Box::new(LatestFixture {
                selection: PolicySelection::new(
                    "AwardPolicy",
                    "LatestFixture",
                    "1",
                    serde_json::json!({}),
                )
                .unwrap(),
            })
        } else {
            Box::new(FirstOffer::new().unwrap())
        };
        exercise(
            Arc::new(MemoryJournal::with_schemas(schemas())),
            &sqlite,
            &root,
            policy.as_ref(),
            if latest { "second" } else { "agent" },
        );
    }
}
#[test]
fn sqlite_reopens_the_selected_award_and_proposed_commitment() {
    let root = support::Directory::new();
    let database = support::Directory::new();
    let sqlite = Arc::new(SqliteJournal::open(database.database(), schemas()).unwrap());
    exercise(
        sqlite.clone(),
        &sqlite,
        &root,
        &FirstOffer::new().unwrap(),
        "agent",
    );
    let before = sqlite.read(&id("awards")).unwrap();
    drop(sqlite);
    let reopened = SqliteJournal::open(database.database(), schemas()).unwrap();
    assert_eq!(reopened.read(&id("awards")).unwrap(), before);
}
