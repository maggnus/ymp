mod support;
use std::{collections::BTreeSet, sync::Arc};
use ymp_domain::{
    Denial, Digest, Id, Proposal, Result, identity::*, journal::PolicySelection, resources::*,
    task::*,
};
use ymp_kernel::{
    journal::{Journal, ParameterSchemas},
    ports::{checks::ReadinessProbe, resources::*},
    registry::readiness_views,
    treasury::*,
};
use ymp_runtime::{
    application::{Application, IntakeRequest},
    memory_journal::MemoryJournal,
    policies::resources::*,
    readiness::{StaticDependencyProbe, readiness_response},
};
use ymp_storage::journal::SqliteJournal;

fn n(value: f64) -> CostUnits {
    CostUnits::new(value).unwrap()
}
fn model() -> PriceWeighted {
    PriceWeighted::new(PriceWeightedParameters {
        expected_input: 10,
        expected_output: 0,
        p90_factor: n(1.0),
    })
    .unwrap()
}
fn policy() -> PurposeBounded {
    PurposeBounded::new(PurposeBoundedParameters {
        max_cost: n(100.0),
        timeout: 1000,
        native_turns: 2,
        output_chars: 1000,
        report_call_cost: n(10.0),
    })
    .unwrap()
}
fn setup<J: Journal>(
    journal: Arc<J>,
    cost: &dyn CostModel,
    resource: &dyn ResourcePolicy,
    unknown: UnknownUsage,
) -> (Treasury<J>, Id, BudgetControl, ResourceDemand) {
    let session = Id::new("financial-session").unwrap();
    let app = Application::new(journal.clone());
    let probe = StaticDependencyProbe::new().unwrap();
    let task = Task {
        id: Id::new("task").unwrap(),
        goal: Goal {
            request: "Exercise financial rules at the invocation seam".into(),
            assumptions: vec![],
            clarifications: vec![],
        },
        contract: Id::new("contract").unwrap(),
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
    };
    let criterion = Criterion {
        id: Id::new("criterion").unwrap(),
        text: "Account for every funded invocation".into(),
        kind: CriterionKind::Constraint,
        weight: n(1.0),
        required: true,
        origin: CriterionOrigin::User,
        needs_class: BTreeSet::new(),
    };
    app.open(
        session.clone(),
        1,
        IntakeRequest {
            task,
            criteria: vec![criterion],
        },
        vec![
            support::policy("SoloWithVerifier"),
            probe.selection().clone(),
            cost.selection().clone(),
            resource.selection().clone(),
        ],
    )
    .unwrap();
    let provider = Provider {
        id: Id::new("provider").unwrap(),
        kind: ProviderKind::Scripted,
        version: Some("fixture".into()),
        capabilities: Some(BTreeSet::new()),
    };
    let agent = Agent {
        id: Id::new("agent").unwrap(),
        name: "fixture agent".into(),
        provider: provider.id.clone(),
        defaults: ProfileSettings::default(),
        instructions: String::new(),
        enabled: true,
    };
    let facts = RegistryFacts {
        agents: vec![agent.clone()],
        discoveries: vec![Discovery {
            provider: provider.clone(),
            offerings: vec![ModelOffering {
                provider: provider.id.clone(),
                model: Some("fixture-model".into()),
                family: None,
                efforts: None,
                default_effort: None,
            }],
            default_model: Some("fixture-model".into()),
            adapter_available: true,
            source: DiscoverySource::ScriptedFixture,
            method: "accounting fixture".into(),
            observed_at: 1,
        }],
        dependencies: vec![],
    };
    let input = app.registry_input(&session, facts, 2).unwrap();
    let responses = readiness_views(&input)
        .iter()
        .map(|v| readiness_response(&probe, v).unwrap())
        .collect();
    app.record_pool(&session, 2, 2, input, probe.selection().clone(), responses)
        .unwrap();
    let profile = app
        .profile(&session, &agent.id, &ProfileSettings::default())
        .unwrap();
    let treasury = Treasury::new(journal);
    let view = treasury.view(&session).unwrap();
    let report = reporting_response(resource, &reporting_view(&view, 3).unwrap()).unwrap();
    let control = treasury
        .open(
            &session,
            3,
            3,
            BudgetRequest {
                id: Id::new("budget").unwrap(),
                pricebook: PriceBook {
                    version: "relative-v1".into(),
                    rates: vec![],
                    fallback: Rates::fallback(),
                },
                unknown_usage: unknown,
                reporting: report,
            },
        )
        .unwrap();
    (
        treasury,
        session,
        control,
        ResourceDemand {
            contribution: Id::new("contribution").unwrap(),
            kind: ContributionKind::Produce,
            difficulty: Difficulty::Simple,
            provider: provider.id,
            profile,
        },
    )
}
fn reserve<J: Journal>(
    treasury: &Treasury<J>,
    session: &Id,
    id: &str,
    demand: &ResourceDemand,
    cost: &dyn CostModel,
    resource: &dyn ResourcePolicy,
) -> Result<Id<Reservation>> {
    let view = treasury.view(session)?;
    let estimated = estimate_response(cost, &estimate_view(&view, demand)?)?;
    let allowance = allowance_response(
        resource,
        &allowance_view(&view, demand, estimated.proposal.value.clone(), 10)?,
    )?;
    let id = Id::new(id)?;
    treasury.reserve(
        session,
        view.revision(),
        10,
        ReserveRequest {
            id: id.clone(),
            assignment: Id::new(format!("assignment-{id}"))?,
            demand: demand.clone(),
            estimate: estimated,
            allowance,
        },
    )?;
    Ok(id)
}
fn receipt(invocation: &str, input: u64, coverage: Coverage) -> Receipt {
    Receipt {
        id: Id::new(format!("receipt-{invocation}")).unwrap(),
        invocation: Id::new(invocation).unwrap(),
        usage: Usage {
            input,
            cache_read: 0,
            cache_write: 0,
            output: 0,
            reasoning: None,
        },
        coverage,
        cost: None,
    }
}
fn authorize<J: Journal>(
    treasury: &Treasury<J>,
    session: &Id,
    reservation: &Id<Reservation>,
    invocation: &str,
) {
    treasury
        .authorize(
            session,
            treasury.view(session).unwrap().revision(),
            11,
            reservation.clone(),
            Id::new(invocation).unwrap(),
        )
        .unwrap();
}
fn observe<J: Journal>(
    treasury: &Treasury<J>,
    session: &Id,
    reservation: &Id<Reservation>,
    receipt: Receipt,
) -> Result<u64> {
    treasury.observe(
        session,
        treasury.view(session)?.revision(),
        12,
        reservation.clone(),
        receipt,
        &[],
    )
}
fn settle<J: Journal>(
    treasury: &Treasury<J>,
    session: &Id,
    reservation: &Id<Reservation>,
    cost: &dyn CostModel,
) -> Result<ResourceResponse<ReceiptPrice>> {
    let view = treasury.view(session)?;
    let response = cost_response(cost, &cost_view(&view, reservation)?)?;
    treasury.settle(
        session,
        view.revision(),
        13,
        reservation.clone(),
        response.clone(),
    )?;
    Ok(response)
}

#[test]
fn reserve_boundaries_keep_verification_and_reporting_protected() {
    let cost = model();
    let resource = policy();
    let (treasury, session, control, demand) = setup(
        Arc::new(MemoryJournal::new()),
        &cost,
        &resource,
        UnknownUsage::Stop,
    );
    for index in 0..6 {
        reserve(
            &treasury,
            &session,
            &format!("produce-{index}"),
            &demand,
            &cost,
            &resource,
        )
        .unwrap();
    }
    assert!(
        reserve(
            &treasury,
            &session,
            "excess-production",
            &demand,
            &cost,
            &resource
        )
        .is_err()
    );
    let mut coordination = demand.clone();
    coordination.kind = ContributionKind::Plan;
    assert!(
        reserve(
            &treasury,
            &session,
            "excess-coordination",
            &coordination,
            &cost,
            &resource
        )
        .is_err()
    );
    let mut verify = demand.clone();
    verify.kind = ContributionKind::Verify;
    for index in 0..2 {
        reserve(
            &treasury,
            &session,
            &format!("verify-{index}"),
            &verify,
            &cost,
            &resource,
        )
        .unwrap();
    }
    assert!(
        reserve(
            &treasury,
            &session,
            "excess-verification",
            &verify,
            &cost,
            &resource
        )
        .is_err()
    );
    let mut report = demand.clone();
    report.kind = ContributionKind::Narrate;
    assert!(
        reserve(
            &treasury,
            &session,
            "early-report",
            &report,
            &cost,
            &resource
        )
        .is_err()
    );
    treasury
        .start_reporting(
            &control,
            treasury.view(&session).unwrap().revision(),
            14,
            ReportingMode::Narrated,
        )
        .unwrap();
    for index in 0..2 {
        reserve(
            &treasury,
            &session,
            &format!("report-{index}"),
            &report,
            &cost,
            &resource,
        )
        .unwrap();
    }
    assert!(
        reserve(
            &treasury,
            &session,
            "third-report",
            &report,
            &cost,
            &resource
        )
        .is_err()
    );
    let view = treasury.view(&session).unwrap();
    let ledger = view.treasury().unwrap();
    assert_eq!(ledger.budget.held, n(100.0));
    assert_eq!(ledger.remaining(Purpose::Reporting).unwrap(), n(0.0));
    assert!(
        treasury
            .authorize(
                &session,
                view.revision(),
                15,
                Id::new("produce-0").unwrap(),
                Id::new("late-work").unwrap()
            )
            .is_err()
    );
}

#[test]
fn observations_idempotence_revocation_and_never_started_release_are_distinct() {
    let cost = model();
    let resource = policy();
    let (treasury, session, control, demand) = setup(
        Arc::new(MemoryJournal::new()),
        &cost,
        &resource,
        UnknownUsage::Stop,
    );
    let unused = reserve(&treasury, &session, "unused", &demand, &cost, &resource).unwrap();
    let proof = treasury.never_started(&session, &unused).unwrap();
    treasury
        .release_unstarted(
            &session,
            treasury.view(&session).unwrap().revision(),
            11,
            &proof,
        )
        .unwrap();
    let active = reserve(&treasury, &session, "active", &demand, &cost, &resource).unwrap();
    authorize(&treasury, &session, &active, "call");
    assert!(treasury.never_started(&session, &active).is_err());
    treasury
        .revoke(
            &session,
            treasury.view(&session).unwrap().revision(),
            12,
            active.clone(),
            "User interrupted the work".into(),
        )
        .unwrap();
    assert_eq!(
        treasury
            .view(&session)
            .unwrap()
            .treasury()
            .unwrap()
            .budget
            .held,
        n(10.0)
    );
    let unknown = receipt("call", 0, Coverage::Unknown);
    observe(&treasury, &session, &active, unknown.clone()).unwrap();
    assert!(settle(&treasury, &session, &active, &cost).is_err());
    assert!(
        reserve(
            &treasury,
            &session,
            "after-unknown",
            &demand,
            &cost,
            &resource
        )
        .is_err()
    );
    let known = receipt("call", 7, Coverage::Complete);
    observe(&treasury, &session, &active, known.clone()).unwrap();
    let response = settle(&treasury, &session, &active, &cost).unwrap();
    let revision = treasury.view(&session).unwrap().revision();
    assert_eq!(
        observe(&treasury, &session, &active, known).unwrap(),
        revision
    );
    assert_eq!(
        treasury
            .settle(&session, revision, 14, active.clone(), response)
            .unwrap(),
        revision
    );
    assert!(
        observe(
            &treasury,
            &session,
            &active,
            receipt("call", 8, Coverage::Complete)
        )
        .is_err()
    );
    assert_eq!(
        treasury
            .view(&session)
            .unwrap()
            .treasury()
            .unwrap()
            .budget
            .spent,
        n(7.0)
    );
    assert_eq!(
        treasury
            .view(&session)
            .unwrap()
            .treasury()
            .unwrap()
            .budget
            .held,
        n(0.0)
    );
    treasury
        .start_reporting(&control, revision, 15, ReportingMode::Deterministic)
        .unwrap();
    let mut report = demand.clone();
    report.kind = ContributionKind::Narrate;
    assert!(
        reserve(
            &treasury,
            &session,
            "stopped-narration",
            &report,
            &cost,
            &resource
        )
        .is_err()
    );
}

#[test]
fn unbounded_estimates_and_recorded_overruns_are_retained_across_sqlite_restart() {
    let directory = support::Directory::new();
    let cost = model();
    let resource = policy();
    let journal =
        Arc::new(SqliteJournal::open(directory.database(), ParameterSchemas::default()).unwrap());
    let (treasury, session, _, demand) =
        setup(journal.clone(), &cost, &resource, UnknownUsage::Estimate);
    let reservation = reserve(&treasury, &session, "estimated", &demand, &cost, &resource).unwrap();
    authorize(&treasury, &session, &reservation, "unknown-call");
    observe(
        &treasury,
        &session,
        &reservation,
        receipt("unknown-call", 0, Coverage::Partial),
    )
    .unwrap();
    settle(&treasury, &session, &reservation, &cost).unwrap();
    let before = treasury.view(&session).unwrap();
    assert_eq!(before.treasury().unwrap().budget.spent, n(10.0));
    assert!(before.treasury().unwrap().unbounded);
    assert!(
        before
            .treasury()
            .unwrap()
            .remaining(Purpose::Verification)
            .is_err()
    );
    drop(treasury);
    drop(journal);
    let reopened = Treasury::new(Arc::new(
        SqliteJournal::open(directory.database(), ParameterSchemas::default()).unwrap(),
    ));
    assert_eq!(reopened.view(&session).unwrap(), before);
    assert!(
        reserve(
            &reopened,
            &session,
            "after-restart",
            &demand,
            &cost,
            &resource
        )
        .is_err()
    );
    let (treasury, session, _, demand) = setup(
        Arc::new(MemoryJournal::new()),
        &cost,
        &resource,
        UnknownUsage::Stop,
    );
    let reservation = reserve(&treasury, &session, "overrun", &demand, &cost, &resource).unwrap();
    authorize(&treasury, &session, &reservation, "expensive");
    observe(
        &treasury,
        &session,
        &reservation,
        receipt("expensive", 120, Coverage::Complete),
    )
    .unwrap();
    settle(&treasury, &session, &reservation, &cost).unwrap();
    assert_eq!(
        treasury
            .view(&session)
            .unwrap()
            .treasury()
            .unwrap()
            .budget
            .spent,
        n(120.0)
    );
    assert!(
        reserve(
            &treasury,
            &session,
            "after-overrun",
            &demand,
            &cost,
            &resource
        )
        .is_err()
    );
}

struct FlatCost {
    selection: PolicySelection,
}
impl CostModel for FlatCost {
    fn selection(&self) -> &PolicySelection {
        &self.selection
    }
    fn estimate(&self, _: &EstimateView) -> Result<Proposal<CostEstimate>> {
        let price: CostUnits =
            serde_json::from_value(self.selection.parameters["price"].clone()).unwrap();
        Ok(Proposal {
            value: CostEstimate {
                expected: price,
                p90: price,
            },
            rationale: "Fixed per-invocation price independent of token counts".into(),
            basis: vec![],
            policy: self.selection.policy.clone(),
        })
    }
    fn cost(&self, view: &CostView) -> Result<Proposal<ReceiptPrice>> {
        let price: CostUnits =
            serde_json::from_value(self.selection.parameters["price"].clone()).unwrap();
        let value = if let Some(cost) = view.known_complete_cost {
            ReceiptPrice::Known(cost)
        } else if view.receipt.coverage == Coverage::Complete {
            ReceiptPrice::Known(price)
        } else if view.unknown_usage == UnknownUsage::Estimate {
            ReceiptPrice::Estimated(view.allowance.cost)
        } else {
            ReceiptPrice::Unknown
        };
        Ok(Proposal {
            value,
            rationale: "Charge the selected flat price once".into(),
            basis: vec![],
            policy: self.selection.policy.clone(),
        })
    }
}
struct FixedAllocation {
    selection: PolicySelection,
}
impl FixedAllocation {
    fn allowance_value(&self, key: &str) -> Allowance {
        Allowance {
            cost: serde_json::from_value(self.selection.parameters[key].clone()).unwrap(),
            timeout: 1000,
            native_turns: 1,
            output_chars: 500,
        }
    }
}
impl ResourcePolicy for FixedAllocation {
    fn selection(&self) -> &PolicySelection {
        &self.selection
    }
    fn allowance(&self, _: &AllowanceView) -> Result<Proposal<Allowance>> {
        Ok(Proposal {
            value: self.allowance_value("allocation"),
            rationale: "Allocate a fixed envelope rather than the estimate's p90".into(),
            basis: vec![],
            policy: self.selection.policy.clone(),
        })
    }
    fn reporting_reserve(&self, view: &ReportingView) -> Result<Proposal<ReportingPlan>> {
        let value = if view.pool.eligible.is_empty() {
            ReportingPlan::deterministic()
        } else {
            ReportingPlan {
                narration: Some(self.allowance_value("reporting")),
                correction: Some(self.allowance_value("reporting")),
            }
        };
        Ok(Proposal {
            value,
            rationale: "Protect two fixed reporting envelopes".into(),
            basis: vec![],
            policy: self.selection.policy.clone(),
        })
    }
}
fn alternate_schemas() -> ParameterSchemas {
    let mut schemas = ParameterSchemas::default();
    schemas
        .register("CostModel", "FlatTest", "1", |selected| {
            if selected
                .parameters
                .as_object()
                .is_none_or(|object| object.len() != 1)
            {
                return Err(Denial::new(
                    "parameters",
                    "Expected one flat-price parameter",
                ));
            }
            let price: CostUnits = serde_json::from_value(selected.parameters["price"].clone())
                .map_err(|_| Denial::new("parameters", "Invalid flat price"))?;
            nonnegative(price)
        })
        .unwrap();
    schemas
        .register("ResourcePolicy", "FixedAllocationTest", "1", |selected| {
            if selected
                .parameters
                .as_object()
                .is_none_or(|object| object.len() != 2)
            {
                return Err(Denial::new(
                    "parameters",
                    "Expected two envelope parameters",
                ));
            }
            for key in ["allocation", "reporting"] {
                let cost: CostUnits = serde_json::from_value(selected.parameters[key].clone())
                    .map_err(|_| Denial::new("parameters", "Invalid envelope cost"))?;
                nonnegative(cost)?;
            }
            Ok(())
        })
        .unwrap();
    schemas
}
#[test]
fn materially_different_policies_use_the_same_treasury_consumer_and_keep_parameters() {
    let alternate_cost = FlatCost {
        selection: PolicySelection::new(
            "CostModel",
            "FlatTest",
            "1",
            serde_json::json!({"price":3}),
        )
        .unwrap(),
    };
    let alternate_resource = FixedAllocation {
        selection: PolicySelection::new(
            "ResourcePolicy",
            "FixedAllocationTest",
            "1",
            serde_json::json!({"allocation":4,"reporting":4}),
        )
        .unwrap(),
    };
    let journal = Arc::new(MemoryJournal::with_schemas(alternate_schemas()));
    let (treasury, session, _, demand) = setup(
        journal.clone(),
        &alternate_cost,
        &alternate_resource,
        UnknownUsage::Stop,
    );
    let reserved = reserve(
        &treasury,
        &session,
        "flat",
        &demand,
        &alternate_cost,
        &alternate_resource,
    )
    .unwrap();
    let initial = treasury.view(&session).unwrap();
    assert_eq!(initial.treasury().unwrap().budget.reporting_reserve, n(8.0));
    assert_eq!(initial.treasury().unwrap().budget.held, n(4.0));
    authorize(&treasury, &session, &reserved, "flat-call");
    observe(
        &treasury,
        &session,
        &reserved,
        receipt("flat-call", 50, Coverage::Complete),
    )
    .unwrap();
    settle(&treasury, &session, &reserved, &alternate_cost).unwrap();
    let view = treasury.view(&session).unwrap();
    assert_eq!(view.treasury().unwrap().budget.spent, n(3.0));
    let settlement = view.treasury().unwrap().accounts[&reserved]
        .settlement
        .as_ref()
        .unwrap();
    assert_eq!(settlement.decision.effective, alternate_cost.selection);
    assert_eq!(
        journal.view(&session, Some(initial.revision())).unwrap(),
        initial
    );
    let cost = model();
    let resource = policy();
    let (treasury, session, _, demand) = setup(
        Arc::new(MemoryJournal::new()),
        &cost,
        &resource,
        UnknownUsage::Stop,
    );
    let reserved = reserve(&treasury, &session, "weighted", &demand, &cost, &resource).unwrap();
    authorize(&treasury, &session, &reserved, "weighted-call");
    observe(
        &treasury,
        &session,
        &reserved,
        receipt("weighted-call", 50, Coverage::Complete),
    )
    .unwrap();
    settle(&treasury, &session, &reserved, &cost).unwrap();
    assert_eq!(
        treasury
            .view(&session)
            .unwrap()
            .treasury()
            .unwrap()
            .budget
            .spent,
        n(50.0)
    );
}

#[test]
fn a_raw_receipt_cost_does_not_certify_partial_usage_and_forged_input_is_refused() {
    let cost = model();
    let resource = policy();
    let journal = Arc::new(MemoryJournal::new());
    let (treasury, session, _, demand) =
        setup(journal.clone(), &cost, &resource, UnknownUsage::Stop);
    let reserved = reserve(&treasury, &session, "partial", &demand, &cost, &resource).unwrap();
    authorize(&treasury, &session, &reserved, "partial-call");
    let mut partial = receipt("partial-call", 1, Coverage::Partial);
    partial.cost = Some(n(0.0));
    observe(&treasury, &session, &reserved, partial).unwrap();
    let view = treasury.view(&session).unwrap();
    let mut response = cost_response(&cost, &cost_view(&view, &reserved).unwrap()).unwrap();
    response.proposal.value = ReceiptPrice::Known(n(0.0));
    let before = journal.read(&session).unwrap();
    assert!(
        treasury
            .settle(&session, view.revision(), 20, reserved.clone(), response)
            .is_err()
    );
    assert_eq!(journal.read(&session).unwrap(), before);
    let mut response = cost_response(&cost, &cost_view(&view, &reserved).unwrap()).unwrap();
    response.input = Digest::of(b"a different receipt view");
    assert!(
        treasury
            .settle(&session, view.revision(), 20, reserved, response)
            .is_err()
    );
    assert_eq!(journal.read(&session).unwrap(), before);
}

#[test]
fn authorization_rechecks_protected_capacity_after_another_invocation_overruns() {
    let cost = model();
    let resource = policy();
    // Both unprotected purposes and Verification must recheck their current cap.
    for (kind, charged) in [
        (ContributionKind::Produce, 70),
        (ContributionKind::Plan, 70),
        (ContributionKind::Verify, 85),
    ] {
        let journal = Arc::new(MemoryJournal::new());
        let (treasury, session, _, mut demand) =
            setup(journal.clone(), &cost, &resource, UnknownUsage::Stop);
        demand.kind = kind;
        let pending = reserve(&treasury, &session, "pending", &demand, &cost, &resource).unwrap();
        demand.kind = ContributionKind::Verify;
        let active = reserve(&treasury, &session, "active", &demand, &cost, &resource).unwrap();
        authorize(&treasury, &session, &active, "overrun");
        observe(
            &treasury,
            &session,
            &active,
            receipt("overrun", charged, Coverage::Complete),
        )
        .unwrap();
        settle(&treasury, &session, &active, &cost).unwrap();
        let before = journal.read(&session).unwrap();
        let error = treasury
            .authorize(
                &session,
                before.revision,
                20,
                pending,
                Id::new("pending-call").unwrap(),
            )
            .unwrap_err();
        assert_eq!(error.code, "budget");
        assert_eq!(journal.read(&session).unwrap(), before);
    }
}

#[test]
fn resumed_cache_usage_known_charges_and_unstarted_holds_survive_sqlite_restart() {
    let directory = support::Directory::new();
    let cost = model();
    let resource = policy();
    let journal =
        Arc::new(SqliteJournal::open(directory.database(), ParameterSchemas::default()).unwrap());
    let (treasury, session, _, demand) =
        setup(journal.clone(), &cost, &resource, UnknownUsage::Stop);
    let reserved = reserve(&treasury, &session, "resumed", &demand, &cost, &resource).unwrap();
    let unused = reserve(&treasury, &session, "unstarted", &demand, &cost, &resource).unwrap();
    let old_proof = treasury.never_started(&session, &unused).unwrap();
    authorize(&treasury, &session, &reserved, "resumed-call");
    let baseline = Usage {
        input: 100,
        cache_read: 60,
        cache_write: 20,
        output: 10,
        reasoning: Some(4),
    };
    let cumulative = Usage {
        input: 120,
        cache_read: 70,
        cache_write: 24,
        output: 12,
        reasoning: Some(5),
    };
    let mut normalized = receipt("resumed-call", 0, Coverage::Complete);
    normalized.usage = cumulative.since(&baseline).unwrap();
    observe(&treasury, &session, &reserved, normalized.clone()).unwrap();
    let response = settle(&treasury, &session, &reserved, &cost).unwrap();
    let before = treasury.view(&session).unwrap();
    let book = before.treasury().unwrap();
    // 6 uncached + 10 cached reads * .1 + 4 cached writes * 1.25 + 2 output * 4.
    // Reasoning is part of output. Directed rounding may add a few ulps.
    assert!(book.budget.spent.get() >= 20.0 && book.budget.spent.get() < 20.00000000001);
    assert_eq!(book.budget.held, n(10.0));
    drop(treasury);
    drop(journal);
    let journal =
        Arc::new(SqliteJournal::open(directory.database(), ParameterSchemas::default()).unwrap());
    let reopened = Treasury::new(journal.clone());
    assert_eq!(reopened.view(&session).unwrap(), before);
    assert_eq!(
        observe(&reopened, &session, &reserved, normalized).unwrap(),
        before.revision()
    );
    assert_eq!(
        reopened
            .settle(&session, before.revision(), 20, reserved, response)
            .unwrap(),
        before.revision()
    );
    // Stored data does not recreate a capability from another Treasury instance.
    assert!(
        reopened
            .release_unstarted(&session, before.revision(), 20, &old_proof)
            .is_err()
    );
    let proof = reopened.never_started(&session, &unused).unwrap();
    reopened
        .release_unstarted(&session, before.revision(), 20, &proof)
        .unwrap();
    assert_eq!(
        reopened
            .view(&session)
            .unwrap()
            .treasury()
            .unwrap()
            .budget
            .held,
        n(0.0)
    );
    let view = reopened.view(&session).unwrap();
    let reset = reopened.open(
        &session,
        view.revision(),
        21,
        BudgetRequest {
            id: Id::new("replacement").unwrap(),
            pricebook: book.pricebook.clone(),
            unknown_usage: UnknownUsage::Stop,
            reporting: reporting_response(&resource, &reporting_view(&view, 21).unwrap()).unwrap(),
        },
    );
    assert_eq!(reset.err().unwrap().code, "budget_exists");
    assert_eq!(reopened.view(&session).unwrap(), view);
}
