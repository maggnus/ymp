//! Numeric policy evidence; these values do not create Journal evidence or grades.
use std::collections::BTreeSet;
use ymp_domain::{Digest, Id, Prob, Ref, task::*, verification::*};
use ymp_kernel::ports::{
    experience::CreditPolicy,
    planning::{BeliefModel, BeliefView},
};
use ymp_runtime::policies::{
    belief::{LikelihoodRatioTable, StrongestSupport},
    credit::{ConfirmedOnly, IncludeDiscriminated},
};
fn id<T>(s: &str) -> Id<T> {
    Id::new(s).unwrap()
}
fn evidence(name: &str, class: EvidenceClass, independence: Independence) -> Evidence {
    Evidence {
        id: id(name),
        criterion: id("criterion"),
        result: Some(id("result")),
        class,
        independence,
        runs: vec![],
        discrimination: Discrimination {
            baseline_fails: None,
            candidate_passes: true,
            mutation_score: None,
        },
        author: None,
        polarity: Polarity::Supports,
    }
}
#[test]
fn neutral_correlated_support_matches_a8_and_credit_policies_remain_distinct() {
    let mut input = BeliefView {
        journal: Digest::of(b"input"),
        entry: LedgerEntry::initial(0).unwrap(),
        evidence: vec![
            evidence(
                "producer",
                EvidenceClass::Executed,
                Independence::ProducerAuthored,
            ),
            evidence(
                "inspection",
                EvidenceClass::Inspection,
                Independence::IndependentVisible,
            ),
            evidence(
                "static",
                EvidenceClass::StaticRead,
                Independence::ProducerAuthored,
            ),
        ],
        criterion: Criterion {
            id: id("criterion"),
            text: "A new behavior".into(),
            kind: CriterionKind::NewBehavior,
            weight: Real::new(1.0).unwrap(),
            required: true,
            origin: CriterionOrigin::User,
            needs_class: BTreeSet::from([EvidenceClass::Executed]),
        },
        subject: Ref {
            id: id("result"),
            version: Digest::of(b"version"),
        },
        prior_basis: vec![],
        rules: AssessmentRules {
            mutation_threshold: Prob::new(0.8).unwrap(),
        },
        at: 1,
    };
    let model = LikelihoodRatioTable::standard().unwrap();
    let first = model.update(&input).unwrap().value.entry;
    assert!((first.belief.get() - 2.475 / 3.475).abs() < 1e-12);
    assert_eq!(first.status, LedgerStatus::Supported);
    let mut duplicate = input.evidence[0].clone();
    duplicate.id = id("duplicate");
    input.evidence.push(duplicate);
    assert_eq!(
        model.update(&input).unwrap().value.entry.belief,
        first.belief
    );
    assert_eq!(
        StrongestSupport::standard()
            .unwrap()
            .update(&input)
            .unwrap()
            .value
            .entry
            .belief
            .get(),
        0.6
    );
    input.evidence[0].polarity = Polarity::Contradicts;
    assert_eq!(
        model.update(&input).unwrap().value.entry.status,
        LedgerStatus::Contradicted
    );
    assert_eq!(model.update(&input).unwrap().value.entry.belief.get(), 0.0);
    let strict = ConfirmedOnly::new().unwrap();
    let experimental = IncludeDiscriminated::new().unwrap();
    assert!(
        !strict
            .creditable(ConfirmationGrade::Discriminated)
            .unwrap()
            .value
    );
    assert!(
        experimental
            .creditable(ConfirmationGrade::Discriminated)
            .unwrap()
            .value
    );
    assert_ne!(strict.selection(), experimental.selection());
    assert!(
        !experimental
            .creditable(ConfirmationGrade::Unconfirmed)
            .unwrap()
            .value
    );
    let mut parameters: StrongestSupportParameters = serde_json::from_value(
        StrongestSupport::standard()
            .unwrap()
            .selection()
            .parameters
            .clone(),
    )
    .unwrap();
    parameters.prior = Prob::new(0.99).unwrap();
    let high_prior = StrongestSupport::new(parameters).unwrap();
    input.prior_basis = vec![ymp_kernel::ports::planning::PriorBasis {
        source: Ref {
            id: id("review"),
            version: Digest::of(b"review"),
        },
        polarity: Polarity::Supports,
    }];
    input.evidence.clear();
    input.criterion.needs_class.clear();
    assert_eq!(
        high_prior.update(&input).unwrap().value.entry.status,
        LedgerStatus::Supported
    );
    input.criterion.needs_class.insert(EvidenceClass::Executed);
    input.evidence.push(evidence(
        "only-inspection",
        EvidenceClass::Inspection,
        Independence::IndependentVisible,
    ));
    assert_eq!(
        high_prior.update(&input).unwrap().value.entry.status,
        LedgerStatus::Supported
    );
    let mut contradiction = evidence(
        "failed-execution",
        EvidenceClass::Executed,
        Independence::Trusted,
    );
    contradiction.polarity = Polarity::Contradicts;
    input.evidence.push(contradiction);
    assert_eq!(
        high_prior.update(&input).unwrap().value.entry.status,
        LedgerStatus::Contradicted
    );
}
