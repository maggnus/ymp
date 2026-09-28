//! Replaceable A9 strategies use immutable, attributable work-boundary inputs.
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use ymp_domain::{
    Digest, Id, Prob, Proposal, Ref, Result,
    journal::{Capability, EscalationStep, Method, PolicySelection},
    plan::WorkItem,
    task::{Criterion, Real},
    verification::*,
};
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MonitorParameters {
    pub epsilon: Real,
    pub stall_limit: u32,
}
impl MonitorParameters {
    pub fn validate(&self) -> Result<()> {
        if self.epsilon.get() < 0.0 || self.stall_limit == 0 || self.stall_limit > 128 {
            return Err(ymp_domain::Denial::new(
                "monitor_parameters",
                "Use nonnegative epsilon and a finite stall limit",
            ));
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DiagnosisParameters {
    pub p_min: Prob,
    pub mutation_threshold: Prob,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EscalationParameters {
    pub max_uses: u32,
    pub max_cost: Real,
    pub timeout_ms: u64,
}
impl EscalationParameters {
    pub fn validate(&self) -> Result<()> {
        if self.max_uses == 0
            || self.max_uses > 16
            || self.max_cost.get() <= 0.0
            || self.timeout_ms == 0
            || self.timeout_ms > 120_000
        {
            return Err(ymp_domain::Denial::new(
                "escalation_parameters",
                "Escalation needs finite use, cost and time limits",
            ));
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProgressInput {
    pub journal: Digest,
    pub boundary: Digest,
    pub at: u64,
    pub criteria: Vec<Criterion>,
    pub ledger: CriteriaLedger,
    pub previous: BTreeMap<Id<Criterion>, Prob>,
    pub stall_count: u32,
    pub history_start: u64,
    pub rejection_since: u64,
    pub work_sequence: u64,
    pub looping: bool,
    pub new_acceptance: bool,
    pub basis: Vec<Ref>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProgressAssessment {
    pub record: ProgressRecord,
    pub stall_count: u32,
    pub stall: bool,
}
pub trait ProgressMonitor {
    fn selection(&self) -> &PolicySelection;
    fn assess(&self, input: &ProgressInput) -> Result<Proposal<ProgressAssessment>>;
}
pub fn assessment(
    input: &ProgressInput,
    parameters: &MonitorParameters,
    accepted_only: bool,
) -> Result<ProgressAssessment> {
    parameters.validate()?;
    let progress = Real::new(
        input
            .criteria
            .iter()
            .map(|c| {
                c.weight.get()
                    * (input.ledger.entries[&c.id].belief.get()
                        - input
                            .previous
                            .get(&c.id)
                            .copied()
                            .unwrap_or(Prob::new(0.5).unwrap())
                            .get())
            })
            .sum(),
    )?;
    let unproductive = if accepted_only {
        !input.new_acceptance
    } else {
        progress < parameters.epsilon
    };
    let count = if unproductive || input.looping {
        input
            .stall_count
            .checked_add(1)
            .ok_or_else(|| ymp_domain::Denial::new("stall_limit", "Stall counter exhausted"))?
    } else {
        0
    };
    Ok(ProgressAssessment {
        record: ProgressRecord {
            at: input.at,
            satisfied: input
                .criteria
                .iter()
                .filter(|c| c.required)
                .all(|c| input.ledger.entries[&c.id].status == LedgerStatus::Satisfied),
            looping: input.looping,
            progress,
            diagnosis: None,
            rationale: if accepted_only {
                "Experimental monitor requires a newly accepted version to reset a stall"
            } else {
                "Weighted criterion belief delta and repeated work determine the stall"
            }
            .into(),
        },
        stall_count: count,
        stall: count >= parameters.stall_limit,
    })
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DiagnosisInput {
    pub journal: Digest,
    pub boundary: Digest,
    pub item: Option<Id<WorkItem>>,
    pub environment: Vec<Ref>,
    pub check_defect: Vec<Ref>,
    pub capability_mismatch: Vec<Ref>,
    pub artifact_defect: Vec<Ref>,
    pub artifact_profiles: usize,
    pub attempts: usize,
    pub attempt_limit: u32,
    pub calibrated_success: Option<Prob>,
    pub ambiguity: Vec<Ref>,
    pub plan_defect: Vec<Ref>,
    pub verification_remaining: Real,
    pub minimum_verification: Option<Real>,
    pub needs: BTreeSet<Capability>,
    pub basis: Vec<Ref>,
    pub limitations: Vec<String>,
}
pub trait FailureDiagnoser {
    fn selection(&self) -> &PolicySelection;
    fn diagnose(&self, input: &DiagnosisInput) -> Result<Proposal<Diagnosis>>;
}
pub fn diagnose(input: &DiagnosisInput, p: &DiagnosisParameters, direct_only: bool) -> Diagnosis {
    if !input.environment.is_empty() {
        Diagnosis::Environment
    } else if !input.check_defect.is_empty() {
        Diagnosis::CheckDefect
    } else if direct_only {
        Diagnosis::Unknown
    } else if !input.capability_mismatch.is_empty() {
        Diagnosis::CapabilityMismatch
    } else if !input.artifact_defect.is_empty() && input.attempts < (input.attempt_limit as usize) {
        Diagnosis::ArtifactDefect
    } else if input.artifact_profiles >= 2 || input.calibrated_success.is_some_and(|s| s < p.p_min)
    {
        Diagnosis::CapabilityLimit
    } else if !input.ambiguity.is_empty() {
        Diagnosis::Ambiguity
    } else if !input.plan_defect.is_empty() {
        Diagnosis::PlanDefect
    } else if input
        .minimum_verification
        .is_none_or(|minimum| input.verification_remaining < minimum)
    {
        Diagnosis::BudgetExhausted
    } else {
        Diagnosis::Unknown
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EscalationInput {
    pub at_revision: u64,
    pub diagnosis: Ref,
    pub kind: Diagnosis,
    pub method: Method,
    pub needs: BTreeSet<Capability>,
    pub item: Option<Id<WorkItem>>,
    pub used: Vec<EscalationStep>,
    pub environment_failed: bool,
    pub basis: Vec<Ref>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EscalationPlan {
    pub step: EscalationStep,
    pub expected_effect: String,
    pub limit: EscalationParameters,
    pub success_condition: String,
    pub limitation: Option<String>,
}
pub trait EscalationPolicy {
    fn selection(&self) -> &PolicySelection;
    fn next(&self, input: &EscalationInput) -> Result<Proposal<EscalationPlan>>;
}
pub fn escalation(
    input: &EscalationInput,
    p: &EscalationParameters,
    stop_uncertain: bool,
) -> Result<EscalationPlan> {
    p.validate()?;
    let mut step = match input.kind {
        Diagnosis::Environment => {
            if input.environment_failed {
                EscalationStep::ReplaceCheck
            } else {
                EscalationStep::FixEnvironment
            }
        }
        Diagnosis::CheckDefect => EscalationStep::ReplaceCheck,
        Diagnosis::CapabilityMismatch => EscalationStep::Reassign(input.needs.clone()),
        Diagnosis::ArtifactDefect => EscalationStep::Retry,
        Diagnosis::CapabilityLimit => [
            input
                .item
                .as_ref()
                .map(|id| EscalationStep::Decompose(id.erased())),
            Some(EscalationStep::AlternativeAttempts(2)),
            Some(EscalationStep::StrongerProfile),
        ]
        .into_iter()
        .flatten()
        .find(|s| !input.used.contains(s))
        .unwrap_or(EscalationStep::StopPreserving),
        Diagnosis::Ambiguity => EscalationStep::Clarify,
        Diagnosis::PlanDefect => EscalationStep::Replan,
        Diagnosis::BudgetExhausted => EscalationStep::StopPreserving,
        Diagnosis::Unknown => EscalationStep::AddVerifier,
    };
    if stop_uncertain
        && !matches!(
            input.kind,
            Diagnosis::Environment | Diagnosis::CheckDefect | Diagnosis::ArtifactDefect
        )
    {
        step = EscalationStep::StopPreserving;
    }
    let mut limitation = (!input.method.ladder.contains(&step))
        .then(|| "The selected method does not authorize this escalation step".into());
    if input.used.iter().filter(|s| **s == step).count() >= p.max_uses as usize {
        step = EscalationStep::StopPreserving;
        limitation = (!input.method.ladder.contains(&step))
            .then(|| "No unused authorized step remains; owner action is required".into());
    }
    if matches!(
        step,
        EscalationStep::Reassign(_)
            | EscalationStep::Decompose(_)
            | EscalationStep::AlternativeAttempts(_)
            | EscalationStep::StrongerProfile
            | EscalationStep::Clarify
            | EscalationStep::Replan
    ) {
        limitation=Some("This team, profile, plan or interactive handler belongs to a later task; no authority is issued".into());
    }
    Ok(EscalationPlan {
        expected_effect: format!("Address {:?} through {:?}", input.kind, step),
        success_condition: match step {
            EscalationStep::FixEnvironment => {
                "The exact check completes without an execution Error"
            }
            EscalationStep::ReplaceCheck => {
                "An independent paid reviewer approves the exact replacement and it becomes active"
            }
            EscalationStep::Retry => {
                "A new bounded attempt uses the failing evidence and improves the criteria"
            }
            EscalationStep::AddVerifier => "An independent Researcher returns a paid diagnosis",
            EscalationStep::StopPreserving => {
                "Reporting mode denies further work funding while preserving state"
            }
            _ => "The owning future handler supplies its actual outcome",
        }
        .into(),
        step,
        limit: p.clone(),
        limitation,
    })
}
