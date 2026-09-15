//! Synthetic evidence fixtures exercise the real Treasury, without native execution.
use super::*;
use crate::{
    intake::{Intake, IntakeRequest},
    journal::{JournalRead, ParameterSchemas},
    registry::{ReadinessResponse, Registry, readiness_views},
};
use std::{collections::BTreeSet, sync::Mutex};
use ymp_domain::{Proposal, identity::*, task::*};

struct TestJournal {
    schemas: ParameterSchemas,
    read: Mutex<JournalRead>,
}
impl Journal for TestJournal {
    fn schemas(&self) -> &ParameterSchemas {
        &self.schemas
    }
    fn read(&self, _: &Id) -> Result<JournalRead> {
        Ok(self.read.lock().unwrap().clone())
    }
    fn append(&self, session: &Id, expected: u64, events: &[Envelope<Event>]) -> Result<u64> {
        let mut read = self.read.lock().unwrap();
        let next = validate_append(&read, session, expected, events, &self.schemas)?;
        read.events.extend_from_slice(events);
        read.revision = next.revision();
        Ok(read.revision)
    }
}
fn n(value: f64) -> CostUnits {
    CostUnits::new(value).unwrap()
}
fn id<T>(value: &str) -> Id<T> {
    Id::new(value).unwrap()
}
fn response<T>(
    selection: &PolicySelection,
    input: &impl Serialize,
    value: T,
) -> ResourceResponse<T> {
    ResourceResponse {
        input: Digest::of_value(input).unwrap(),
        proposal: Proposal {
            value,
            rationale: "Synthetic accounting seam".into(),
            basis: vec![],
            policy: selection.policy.clone(),
        },
    }
}
struct Fixture {
    journal: Arc<TestJournal>,
    treasury: Treasury<TestJournal>,
    session: Id,
    demand: ResourceDemand,
}
impl Fixture {
    fn new(unknown: UnknownUsage) -> Self {
        let journal = Arc::new(TestJournal {
            schemas: ParameterSchemas::default(),
            read: Mutex::new(JournalRead {
                revision: 0,
                events: vec![],
            }),
        });
        let session = id("proof-session");
        let cost = PolicySelection::new(
            "CostModel",
            "PriceWeighted",
            "1",
            serde_json::to_value(PriceWeightedParameters::default()).unwrap(),
        )
        .unwrap();
        let resource = PolicySelection::new(
            "ResourcePolicy",
            "PurposeBounded",
            "1",
            serde_json::to_value(PurposeBoundedParameters::default()).unwrap(),
        )
        .unwrap();
        let probe = PolicySelection::new(
            "ReadinessProbe",
            "StaticDependencyProbe",
            "1",
            serde_json::json!({}),
        )
        .unwrap();
        Intake::new(journal.clone())
            .open(
                session.clone(),
                1,
                IntakeRequest {
                    task: Task {
                        id: id("task"),
                        goal: Goal {
                            request: "Check accounting evidence".into(),
                            assumptions: vec![],
                            clarifications: vec![],
                        },
                        contract: id("contract"),
                        constraints: Constraints {
                            budget: n(100.0),
                            verification_reserve: n(20.0),
                            deadline: None,
                            pins: Pins::unrestricted(),
                            allowed: BTreeSet::new(),
                            parallel_limit: 4,
                            attempt_limit: 3,
                            max_members: 2,
                        },
                    },
                    criteria: vec![Criterion {
                        id: id("criterion"),
                        text: "Keep costs attributable".into(),
                        kind: CriterionKind::Constraint,
                        weight: n(1.0),
                        required: true,
                        origin: CriterionOrigin::User,
                        needs_class: BTreeSet::new(),
                    }],
                },
                vec![cost, resource.clone(), probe.clone()],
            )
            .unwrap();
        let registry = Registry::new(journal.clone());
        let facts = RegistryFacts {
            agents: vec![Agent {
                id: id("agent"),
                name: "Synthetic agent".into(),
                provider: id("provider"),
                defaults: ProfileSettings::default(),
                instructions: String::new(),
                enabled: true,
            }],
            discoveries: vec![Discovery {
                provider: Provider {
                    id: id("provider"),
                    kind: ProviderKind::Scripted,
                    version: None,
                    capabilities: Some(BTreeSet::new()),
                },
                offerings: vec![ModelOffering {
                    provider: id("provider"),
                    model: Some("fixture".into()),
                    family: None,
                    efforts: None,
                    default_effort: None,
                }],
                default_model: Some("fixture".into()),
                adapter_available: true,
                source: DiscoverySource::ScriptedFixture,
                method: "Evidence fixture".into(),
                observed_at: 1,
            }],
            dependencies: vec![],
        };
        let input = registry.prepare(&session, facts, 2).unwrap();
        let responses = readiness_views(&input)
            .iter()
            .map(|v| ReadinessResponse {
                profile: v.profile.clone(),
                input: Digest::of_value(v).unwrap(),
                proposal: Proposal {
                    value: Readiness::Ready,
                    rationale: "Synthetic readiness".into(),
                    basis: vec![],
                    policy: probe.policy.clone(),
                },
            })
            .collect();
        registry
            .record(&session, 2, 2, input, probe, responses)
            .unwrap();
        let profile = registry
            .profile(&session, &id("agent"), &ProfileSettings::default())
            .unwrap();
        let treasury = Treasury::new(journal.clone());
        let view = treasury.view(&session).unwrap();
        let allowance = Allowance {
            cost: n(10.0),
            timeout: 100,
            native_turns: 1,
            output_chars: 100,
        };
        treasury
            .open(
                &session,
                3,
                3,
                BudgetRequest {
                    id: id("budget"),
                    pricebook: PriceBook {
                        version: "fixture".into(),
                        rates: vec![],
                        fallback: Rates::fallback(),
                    },
                    unknown_usage: unknown,
                    reporting: response(
                        &resource,
                        &reporting_view(&view, 3).unwrap(),
                        ReportingPlan {
                            narration: Some(allowance.clone()),
                            correction: Some(allowance),
                        },
                    ),
                },
            )
            .unwrap();
        Self {
            journal,
            treasury,
            session,
            demand: ResourceDemand {
                contribution: id("contribution"),
                kind: ContributionKind::Produce,
                difficulty: Difficulty::Simple,
                provider: id("provider"),
                profile,
            },
        }
    }
    fn view(&self) -> SessionView {
        self.treasury.view(&self.session).unwrap()
    }
    fn reserve(&self, name: &str, kind: ContributionKind, amount: f64) -> Result<Id<Reservation>> {
        let view = self.view();
        let mut demand = self.demand.clone();
        demand.kind = kind;
        let estimate = CostEstimate {
            expected: n(amount),
            p90: n(amount),
        };
        let estimated = response(
            selected(&view, "CostModel")?,
            &estimate_view(&view, &demand)?,
            estimate.clone(),
        );
        let allocation = response(
            selected(&view, "ResourcePolicy")?,
            &allowance_view(&view, &demand, estimate, 4)?,
            Allowance {
                cost: n(amount),
                timeout: 100,
                native_turns: 1,
                output_chars: 100,
            },
        );
        let reservation = id(name);
        self.treasury.reserve(
            &self.session,
            view.revision(),
            4,
            ReserveRequest {
                id: reservation.clone(),
                assignment: id(name),
                demand,
                estimate: estimated,
                allowance: allocation,
            },
        )?;
        Ok(reservation)
    }
    fn authorize(&self, reservation: &Id<Reservation>) -> Result<u64> {
        self.treasury.authorize(
            &self.session,
            self.view().revision(),
            5,
            reservation.clone(),
            reservation.erased(),
        )
    }
    fn receipt(&self, reservation: &Id<Reservation>) -> Receipt {
        Receipt {
            id: id(&format!("receipt-{reservation}")),
            invocation: reservation.erased(),
            usage: Usage {
                input: 0,
                cache_read: 0,
                cache_write: 0,
                output: 0,
                reasoning: None,
            },
            coverage: Coverage::Partial,
            cost: None,
        }
    }
    // Deliberately private and test-only. W1-0017 must derive these facts from
    // validated executor evidence; the fixture is not evidence of such execution.
    fn proof(
        &self,
        reservation: &Id<Reservation>,
        fact: AccountingFact,
        receipt: Option<&Receipt>,
    ) -> AccountingEvidence {
        let view = self.view();
        let entry = &view.treasury().unwrap().accounts[reservation];
        AccountingEvidence {
            session: self.session.clone(),
            issuer: self.treasury.issuer.clone(),
            record: AccountingEvidenceRecord {
                reservation: reservation.clone(),
                assignment: entry.reservation.assignment.clone(),
                invocation: entry.invocation.clone(),
                state: entry.last.clone(),
                receipt: receipt.map(|r| Digest::of_value(r).unwrap()),
                fact,
                basis: vec![entry.last.clone()],
            },
        }
    }
    fn observe(
        &self,
        reservation: &Id<Reservation>,
        receipt: Receipt,
        evidence: &[&AccountingEvidence],
    ) -> Result<u64> {
        self.treasury.observe(
            &self.session,
            self.view().revision(),
            6,
            reservation.clone(),
            receipt,
            evidence,
        )
    }
    fn settle(&self, reservation: &Id<Reservation>, price: ReceiptPrice) -> Result<u64> {
        let view = self.view();
        let response = response(
            selected(&view, "CostModel")?,
            &cost_view(&view, reservation)?,
            price,
        );
        self.treasury.settle(
            &self.session,
            view.revision(),
            7,
            reservation.clone(),
            response,
        )
    }
}

#[test]
fn bounded_unknown_stop_retains_exposure_and_only_allows_protected_work() {
    let f = Fixture::new(UnknownUsage::Stop);
    let production = f
        .reserve("production", ContributionKind::Produce, 10.0)
        .unwrap();
    let verification = f
        .reserve("verification", ContributionKind::Verify, 30.0)
        .unwrap();
    f.authorize(&production).unwrap();
    let bound = f.proof(
        &production,
        AccountingFact::EnforcedUpperBound(n(30.0)),
        None,
    );
    f.observe(&production, f.receipt(&production), &[&bound])
        .unwrap();
    let view = f.view();
    let book = view.treasury().unwrap();
    assert_eq!(book.budget.held, n(60.0));
    assert!(!book.unbounded);
    assert!(book.unsettled_usage);
    assert_eq!(book.remaining(Purpose::Production).unwrap(), n(0.0));
    assert_eq!(book.remaining(Purpose::Verification).unwrap(), n(0.0));
    // Verification was reserved before uncertainty arose and exceeds its own
    // protected reserve now; authorization must recheck without adding it twice.
    assert_eq!(f.authorize(&verification).unwrap_err().code, "budget");
    let proof = f.treasury.never_started(&f.session, &verification).unwrap();
    f.treasury
        .release_unstarted(&f.session, f.view().revision(), 8, &proof)
        .unwrap();
    let within = f
        .reserve("within-reserve", ContributionKind::Verify, 20.0)
        .unwrap();
    f.authorize(&within).unwrap();
    assert!(
        f.reserve("unprotected", ContributionKind::Produce, 1.0)
            .is_err()
    );
    assert!(f.settle(&production, ReceiptPrice::Unknown).is_err());
}

#[test]
fn estimates_retain_the_full_verified_exposure_after_settlement_and_replay() {
    let f = Fixture::new(UnknownUsage::Estimate);
    let reserved = f
        .reserve("estimated", ContributionKind::Produce, 10.0)
        .unwrap();
    f.authorize(&reserved).unwrap();
    let bound = f.proof(&reserved, AccountingFact::EnforcedUpperBound(n(30.0)), None);
    f.observe(&reserved, f.receipt(&reserved), &[&bound])
        .unwrap();
    f.settle(&reserved, ReceiptPrice::Estimated(n(10.0)))
        .unwrap();
    let before = f.view();
    let book = before.treasury().unwrap();
    assert_eq!(book.budget.spent, n(10.0));
    assert_eq!(book.budget.held, n(20.0));
    assert_eq!(book.remaining(Purpose::Production).unwrap(), n(30.0));
    assert!(book.unsettled_usage);
    assert!(!book.unbounded);
    let restored = Treasury::new(f.journal.clone());
    assert_eq!(restored.view(&f.session).unwrap(), before);
    assert!(
        f.observe(&reserved, f.receipt(&reserved), &[&bound])
            .is_err()
    );
}

#[test]
fn complete_cost_dominates_a_smaller_bound_before_and_after_settlement() {
    let f = Fixture::new(UnknownUsage::Stop);
    let reserved = f
        .reserve("complete", ContributionKind::Produce, 10.0)
        .unwrap();
    f.authorize(&reserved).unwrap();
    let receipt = f.receipt(&reserved);
    let full = f.proof(
        &reserved,
        AccountingFact::CompleteCost(n(70.0)),
        Some(&receipt),
    );
    let bound = f.proof(&reserved, AccountingFact::EnforcedUpperBound(n(10.0)), None);
    f.observe(&reserved, receipt, &[&full, &bound]).unwrap();
    let view = f.view();
    assert_eq!(view.treasury().unwrap().budget.held, n(70.0));
    assert_eq!(
        view.treasury()
            .unwrap()
            .remaining(Purpose::Verification)
            .unwrap(),
        n(10.0)
    );
    assert!(
        f.reserve("too-expensive", ContributionKind::Verify, 20.0)
            .is_err()
    );
    assert!(f.settle(&reserved, ReceiptPrice::Known(n(10.0))).is_err());
    f.settle(&reserved, ReceiptPrice::Known(n(70.0))).unwrap();
    let view = f.view();
    assert_eq!(view.treasury().unwrap().budget.spent, n(70.0));
    assert_eq!(view.treasury().unwrap().budget.held, n(0.0));
}

#[test]
fn evidence_is_bound_to_issuer_session_assignment_invocation_state_and_receipt() {
    let f = Fixture::new(UnknownUsage::Stop);
    let reserved = f.reserve("bound", ContributionKind::Produce, 10.0).unwrap();
    let stale_unstarted = f.treasury.never_started(&f.session, &reserved).unwrap();
    f.authorize(&reserved).unwrap();
    assert!(
        f.treasury
            .release_unstarted(&f.session, f.view().revision(), 6, &stale_unstarted)
            .is_err()
    );
    let receipt = f.receipt(&reserved);
    let before = f.journal.read(&f.session).unwrap();
    for field in 0..8 {
        let mut proof = f.proof(
            &reserved,
            AccountingFact::CompleteCost(n(7.0)),
            Some(&receipt),
        );
        match field {
            0 => proof.issuer = Arc::new(()),
            1 => proof.session = id("other"),
            2 => proof.record.reservation = id("other"),
            3 => proof.record.assignment = id("other"),
            4 => proof.record.invocation = Some(id("other")),
            5 => proof.record.state.version = Digest::of(b"stale"),
            6 => proof.record.receipt = Some(Digest::of(b"different")),
            7 => proof.record.basis.clear(),
            _ => unreachable!(),
        }
        assert!(
            f.observe(&reserved, receipt.clone(), &[&proof]).is_err(),
            "field {field}"
        );
        assert_eq!(f.journal.read(&f.session).unwrap(), before);
    }
    let proof = f.proof(
        &reserved,
        AccountingFact::CompleteCost(n(7.0)),
        Some(&receipt),
    );
    f.observe(&reserved, receipt, &[&proof]).unwrap();
    assert!(!f.view().treasury().unwrap().unbounded);
    f.settle(&reserved, ReceiptPrice::Known(n(7.0))).unwrap();
    assert_eq!(f.view().treasury().unwrap().budget.spent, n(7.0));
}

#[test]
fn a_later_partial_observation_cannot_erase_a_confirmed_cost_or_restore_a_disproved_bound() {
    for policy in [UnknownUsage::Stop, UnknownUsage::Estimate] {
        let f = Fixture::new(policy);
        let reserved = f
            .reserve("monotone", ContributionKind::Produce, 10.0)
            .unwrap();
        f.authorize(&reserved).unwrap();
        let first = f.receipt(&reserved);
        let full = f.proof(
            &reserved,
            AccountingFact::CompleteCost(n(70.0)),
            Some(&first),
        );
        let bound = f.proof(&reserved, AccountingFact::EnforcedUpperBound(n(10.0)), None);
        f.observe(&reserved, first.clone(), &[&full, &bound])
            .unwrap();
        let mut later = first;
        later.usage.input = 1;
        f.observe(&reserved, later, &[]).unwrap();
        let view = f.view();
        let book = view.treasury().unwrap();
        assert_eq!(book.budget.held, n(70.0));
        assert!(book.unbounded);
        assert!(book.remaining(Purpose::Verification).is_err());
        assert_eq!(book.accounts[&reserved].complete_cost, None);
        assert_eq!(book.accounts[&reserved].upper_bound, None);
        if policy == UnknownUsage::Estimate {
            f.settle(&reserved, ReceiptPrice::Estimated(n(10.0)))
                .unwrap();
            let view = f.view();
            let book = view.treasury().unwrap();
            assert_eq!(book.budget.spent, n(10.0));
            assert_eq!(book.budget.held, n(60.0));
            assert!(book.unbounded);
        }
        let reopened = Treasury::new(f.journal.clone());
        assert_eq!(reopened.view(&f.session).unwrap(), f.view());
        assert!(
            f.reserve("protected", ContributionKind::Verify, 1.0)
                .is_err()
        );
    }
}
