//! Pure A9 precedence evidence; these fixtures do not create kernel observations.
use std::collections::BTreeSet;
use ymp_domain::{Digest, Id, Prob, Ref, task::Real, verification::Diagnosis};
use ymp_kernel::ports::progress::{DiagnosisInput, DiagnosisParameters, FailureDiagnoser};
use ymp_runtime::policies::diagnosis::RuleBasedDiagnoser;
#[test]
fn literal_a9_first_match_keeps_remaining_attempts_before_profile_history() {
    let source = Ref {
        id: Id::new("fact").unwrap(),
        version: Digest::of(b"fixture"),
    };
    let mut input = DiagnosisInput {
        journal: Digest::of(b"fixture"),
        boundary: Digest::of(b"boundary"),
        item: None,
        environment: vec![source.clone()],
        check_defect: vec![source.clone()],
        capability_mismatch: vec![source.clone()],
        artifact_defect: vec![source.clone()],
        artifact_profiles: 2,
        attempts: 1,
        attempt_limit: 2,
        calibrated_success: None,
        ambiguity: vec![source.clone()],
        plan_defect: vec![source],
        verification_remaining: Real::new(0.0).unwrap(),
        minimum_verification: Some(Real::new(1.0).unwrap()),
        needs: BTreeSet::new(),
        basis: vec![],
        limitations: vec![],
    };
    let policy = RuleBasedDiagnoser::new(DiagnosisParameters {
        p_min: Prob::new(0.2).unwrap(),
        mutation_threshold: Prob::new(0.8).unwrap(),
    })
    .unwrap();
    assert_eq!(
        policy.diagnose(&input).unwrap().value,
        Diagnosis::Environment
    );
    input.environment.clear();
    assert_eq!(
        policy.diagnose(&input).unwrap().value,
        Diagnosis::CheckDefect
    );
    input.check_defect.clear();
    assert_eq!(
        policy.diagnose(&input).unwrap().value,
        Diagnosis::CapabilityMismatch
    );
    input.capability_mismatch.clear();
    assert_eq!(
        policy.diagnose(&input).unwrap().value,
        Diagnosis::ArtifactDefect
    );
    input.attempts = 2;
    assert_eq!(
        policy.diagnose(&input).unwrap().value,
        Diagnosis::CapabilityLimit
    );
    input.artifact_profiles = 1;
    assert_eq!(policy.diagnose(&input).unwrap().value, Diagnosis::Ambiguity);
    input.ambiguity.clear();
    assert_eq!(
        policy.diagnose(&input).unwrap().value,
        Diagnosis::PlanDefect
    );
    input.plan_defect.clear();
    assert_eq!(
        policy.diagnose(&input).unwrap().value,
        Diagnosis::BudgetExhausted
    );
    input.verification_remaining = Real::new(2.0).unwrap();
    assert_eq!(policy.diagnose(&input).unwrap().value, Diagnosis::Unknown);
}
