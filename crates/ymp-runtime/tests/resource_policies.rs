use std::collections::{BTreeMap, BTreeSet};
use ymp_domain::{
    Digest, Id,
    identity::{ExecutionProfile, Pool},
    journal::{decode, encode},
    resources::*,
    task::*,
};
use ymp_kernel::ports::resources::*;
use ymp_runtime::policies::resources::*;

fn amount(value: f64) -> Real {
    Real::new(value).unwrap()
}
fn demand() -> ResourceDemand {
    ResourceDemand {
        contribution: Id::new("contribution").unwrap(),
        kind: ContributionKind::Produce,
        difficulty: Difficulty::Simple,
        provider: Id::new("provider").unwrap(),
        profile: ExecutionProfile {
            agent: Id::new("agent").unwrap(),
            provider_version: Some("fixture".into()),
            model: "fixture-model".into(),
            family: None,
            effort: Some("low".into()),
        },
    }
}
fn book() -> PriceBook {
    PriceBook {
        version: "relative-v1".into(),
        rates: vec![],
        fallback: Rates::fallback(),
    }
}
fn model() -> PriceWeighted {
    PriceWeighted::new(PriceWeightedParameters {
        expected_input: 10,
        expected_output: 10,
        p90_factor: amount(1.5),
    })
    .unwrap()
}
fn policy() -> PurposeBounded {
    PurposeBounded::new(PurposeBoundedParameters {
        max_cost: amount(100.0),
        timeout: 1000,
        native_turns: 2,
        output_chars: 2000,
        report_call_cost: amount(5.0),
    })
    .unwrap()
}
fn cost_view(coverage: Coverage) -> CostView {
    CostView {
        journal: Digest::of(b"view"),
        demand: demand(),
        receipt: Receipt {
            id: Id::new("receipt").unwrap(),
            invocation: Id::new("invocation").unwrap(),
            usage: Usage {
                input: 100,
                cache_read: 80,
                cache_write: 10,
                output: 20,
                reasoning: Some(5),
            },
            coverage,
            cost: None,
        },
        pricebook: book(),
        allowance: Allowance {
            cost: amount(200.0),
            timeout: 1000,
            native_turns: 2,
            output_chars: 1000,
        },
        unknown_usage: UnknownUsage::Stop,
        known_complete_cost: None,
    }
}
fn reporting_view() -> ReportingView {
    ReportingView {
        journal: Digest::of(b"view"),
        task: Task {
            id: Id::new("task").unwrap(),
            goal: Goal {
                request: "Produce a result".into(),
                assumptions: vec![],
                clarifications: vec![],
            },
            contract: Id::new("contract").unwrap(),
            constraints: Constraints {
                budget: amount(100.0),
                verification_reserve: amount(20.0),
                deadline: Some(10_000),
                pins: Pins::unrestricted(),
                allowed: BTreeSet::new(),
                parallel_limit: 1,
                attempt_limit: 2,
                max_members: 2,
            },
        },
        pool: Pool {
            eligible: BTreeSet::from([Id::new("agent").unwrap()]),
            offerings: BTreeMap::new(),
            excluded: BTreeMap::new(),
        },
        at: 1,
    }
}
#[test]
fn cache_and_reasoning_are_not_double_priced_and_policy_parameters_round_trip() {
    let model = model();
    let view = cost_view(Coverage::Complete);
    let response = cost_response(&model, &view).unwrap();
    let ReceiptPrice::Known(cost) = response.proposal.value else {
        panic!("Known complete cost expected");
    };
    assert!(cost.get() >= 110.5 && cost.get() - 110.5 < 1e-10);
    assert_eq!(response.input, Digest::of_value(&view).unwrap());
    let selection = decode(&encode(model.selection()).unwrap()).unwrap();
    let restored = PriceWeighted::from_selection(&selection).unwrap();
    assert_eq!(restored.cost(&view).unwrap(), model.cost(&view).unwrap());
}
#[test]
fn partial_cost_claims_do_not_become_complete_without_a_validated_cost_input() {
    let model = model();
    for coverage in [Coverage::Partial, Coverage::Unknown] {
        let mut view = cost_view(coverage);
        view.receipt.cost = Some(amount(0.0));
        assert_eq!(model.cost(&view).unwrap().value, ReceiptPrice::Unknown);
        view.unknown_usage = UnknownUsage::Estimate;
        assert_eq!(
            model.cost(&view).unwrap().value,
            ReceiptPrice::Estimated(view.allowance.cost)
        );
        view.known_complete_cost = Some(amount(123.0));
        assert_eq!(
            model.cost(&view).unwrap().value,
            ReceiptPrice::Known(amount(123.0))
        );
    }
}
#[test]
fn cumulative_usage_requires_an_established_nonresetting_baseline() {
    let prior = Usage {
        input: 1000,
        cache_read: 600,
        cache_write: 0,
        output: 100,
        reasoning: Some(20),
    };
    let total = Usage {
        input: 1300,
        cache_read: 700,
        cache_write: 0,
        output: 150,
        reasoning: Some(30),
    };
    let delta = total.since(&prior).unwrap();
    assert_eq!(
        delta,
        Usage {
            input: 300,
            cache_read: 100,
            cache_write: 0,
            output: 50,
            reasoning: Some(10)
        }
    );
    assert_eq!(total.since(&prior).unwrap(), delta);
    assert!(prior.since(&total).is_err());
    let cost = weighted_cost(&delta, &Rates::fallback()).unwrap();
    assert!(cost.get() >= 410.0 && cost.get() - 410.0 < 1e-10);
}
#[test]
fn resource_policy_bounds_allowances_and_chooses_deterministic_reporting_when_needed() {
    let policy = policy();
    let estimate = EstimateView {
        journal: Digest::of(b"view"),
        demand: demand(),
        pricebook: book(),
        history: vec![],
    };
    let estimate = model().estimate(&estimate).unwrap().value;
    let mut view = AllowanceView {
        journal: Digest::of(b"view"),
        demand: demand(),
        estimate,
        remaining: amount(60.0),
        deadline: Some(500),
        at: 100,
    };
    let allocation = allowance_response(&policy, &view).unwrap();
    assert_eq!(allocation.proposal.value.cost, amount(60.0));
    assert_eq!(allocation.proposal.value.timeout, 400);
    view.remaining = amount(1.0);
    assert!(policy.allowance(&view).is_err());
    let mut report = reporting_view();
    let plan = reporting_response(&policy, &report).unwrap().proposal.value;
    assert_eq!(plan.reserve().unwrap(), amount(10.0));
    report.task.constraints.verification_reserve = amount(95.0);
    assert_eq!(
        policy.reporting_reserve(&report).unwrap().value,
        ReportingPlan::deterministic()
    );
    report.task.constraints.verification_reserve = amount(20.0);
    report.pool.eligible.clear();
    assert_eq!(
        policy.reporting_reserve(&report).unwrap().value,
        ReportingPlan::deterministic()
    );
}
#[test]
fn conservative_arithmetic_and_invalid_counters_cannot_create_capacity() {
    let tiny = amount(2_f64.powi(-54));
    assert!(add(amount(1.0), tiny).unwrap().get() > 1.0);
    assert!(remaining(amount(1.0), tiny).unwrap().get() < 1.0);
    assert!(add(amount(f64::MAX), amount(f64::MAX)).is_err());
    assert!(multiply(amount(f64::MAX), amount(2.0)).is_err());
    assert!(remaining(amount(-1.0), amount(0.0)).is_err());
    assert!(
        multiply(amount(f64::from_bits(1)), amount(1.1))
            .unwrap()
            .get()
            >= f64::from_bits(2)
    );
    let invalid = Usage {
        input: 1,
        cache_read: u64::MAX,
        cache_write: 1,
        output: 0,
        reasoning: Some(1),
    };
    assert!(invalid.validate().is_err());
    let mut duplicate = book();
    let rate = PriceRate {
        provider: Id::new("provider").unwrap(),
        model: "model".into(),
        rates: Rates::fallback(),
    };
    duplicate.rates = vec![rate.clone(), rate];
    assert!(duplicate.validate().is_err());
}

#[test]
fn restoring_valid_integer_parameters_keeps_the_original_policy_identity() {
    use ymp_domain::journal::PolicySelection;
    let cost = PolicySelection::new(
        "CostModel",
        "PriceWeighted",
        "1",
        serde_json::json!({"expected_input":1,"expected_output":2,"p90_factor":2}),
    )
    .unwrap();
    let restored = PriceWeighted::from_selection(&cost).unwrap();
    assert_eq!(restored.selection(), &cost);
    assert_eq!(
        restored
            .cost(&cost_view(Coverage::Complete))
            .unwrap()
            .policy,
        cost.policy
    );
    let resources=PolicySelection::new("ResourcePolicy","PurposeBounded","1",serde_json::json!({"max_cost":100,"timeout":1000,"native_turns":2,"output_chars":2000,"report_call_cost":5})).unwrap();
    let restored = PurposeBounded::from_selection(&resources).unwrap();
    assert_eq!(restored.selection(), &resources);
    assert_eq!(
        restored
            .reporting_reserve(&reporting_view())
            .unwrap()
            .policy,
        resources.policy
    );
    let defaults = PriceWeighted::new(PriceWeightedParameters::default()).unwrap();
    assert!(
        defaults
            .selection()
            .parameters
            .get("expected_input")
            .is_some()
    );
    assert!(defaults.selection().parameters.get("p90_factor").is_some());
    PurposeBounded::new(PurposeBoundedParameters::default()).unwrap();
}
