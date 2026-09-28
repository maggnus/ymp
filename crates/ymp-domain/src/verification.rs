//! Checks, evidence, reviews and separately graded acceptance and criterion assessment.
use crate::{
    Denial, Digest, Id, Ref, Result,
    assignment::{Assignment, ErrorClass},
    journal::{Capability, PolicySelection},
    task::Criterion,
    workspace::{Snapshot, WorkspacePath},
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CheckSpec {
    /// Program and inputs are pinned to the check's verifier snapshot, not the target.
    Command {
        program: WorkspacePath,
        args: Vec<String>,
        inputs: BTreeSet<WorkspacePath>,
    },
    ExactBytes {
        path: WorkspacePath,
        digest: Digest,
    },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CheckAuthor {
    User,
    Agent(Id<Assignment>),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Independence {
    ProducerAuthored,
    IndependentVisible,
    IndependentHidden,
    Trusted,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CheckVisibility {
    Visible,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Check {
    pub id: Id<Check>,
    pub criterion: Id<Criterion>,
    pub criterion_version: Digest,
    pub spec: CheckSpec,
    pub author: CheckAuthor,
    pub independence: Independence,
    pub visibility: CheckVisibility,
    pub needs: BTreeSet<Capability>,
    pub verifier: Option<Ref>,
    pub version: Digest,
}
impl Check {
    pub fn content_version(&self) -> Result<Digest> {
        Digest::of_value(&(
            &self.id,
            &self.criterion,
            &self.criterion_version,
            &self.spec,
            &self.author,
            self.independence,
            self.visibility,
            &self.needs,
            &self.verifier,
        ))
    }
    pub fn reference(&self) -> Ref {
        Ref {
            id: self.id.erased(),
            version: self.version.clone(),
        }
    }
    pub fn validate(&self) -> Result<()> {
        let required = match &self.spec {
            CheckSpec::Command {
                program,
                args,
                inputs,
            } => {
                if program == &WorkspacePath::root()
                    || self.verifier.is_none()
                    || inputs.contains(&WorkspacePath::root())
                    || inputs.len() > 1024
                    || args.len() > 256
                    || args.iter().any(|a| a.len() > 16_384 || a.contains('\0'))
                {
                    return Err(Denial::new(
                        "check_spec",
                        "Command requires a pinned verifier, file paths and bounded explicit arguments",
                    ));
                }
                BTreeSet::from([Capability::ReadFiles, Capability::RunProcess])
            }
            CheckSpec::ExactBytes { path, .. } => {
                if path == &WorkspacePath::root() || self.verifier.is_some() {
                    return Err(Denial::new(
                        "check_spec",
                        "ExactBytes requires one target file and no executable verifier",
                    ));
                }
                BTreeSet::from([Capability::ReadFiles])
            }
        };
        if !required.is_subset(&self.needs) || self.version != self.content_version()? {
            return Err(Denial::new(
                "check_version",
                "Check content version or required capabilities disagree",
            ));
        }
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CheckRunRole {
    Baseline,
    Candidate,
    Control,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CheckOutcome {
    Pass,
    Fail,
    Error(ErrorClass),
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CheckObservationKind {
    Exited(i32),
    ExactBytes(Option<Digest>),
    Error { class: ErrorClass, reason: String },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CheckLimits {
    pub timeout_ms: u64,
    pub output_bytes: usize,
}
impl CheckLimits {
    pub fn validate(&self) -> Result<()> {
        if self.timeout_ms == 0
            || self.timeout_ms > 120_000
            || self.output_bytes == 0
            || self.output_bytes > 4 * 1024 * 1024
        {
            return Err(Denial::new(
                "check_limits",
                "Checks require a finite timeout of at most 120 seconds and at most 4 MiB per output stream",
            ));
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CheckEnvironment {
    pub runner: PolicySelection,
    pub platform: String,
    pub identity: BTreeMap<String, String>,
    pub limits: CheckLimits,
}
impl CheckEnvironment {
    pub fn validate(&self) -> Result<()> {
        self.runner.validate()?;
        self.limits.validate()?;
        crate::require_text(&self.platform, 4096)?;
        if self.runner.policy.port != "CheckRunner" || self.identity.len() > 128 {
            return Err(Denial::new(
                "check_environment",
                "Invalid runner environment identity",
            ));
        }
        for (key, value) in &self.identity {
            crate::require_text(key, 256)?;
            crate::require_text(value, 16_384)?;
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CheckRun {
    pub id: Id<CheckRun>,
    pub check: Id<Check>,
    pub check_version: Digest,
    pub target: Id<Snapshot>,
    pub target_version: Digest,
    pub role: CheckRunRole,
    /// None means no normal process exit was observed; never invent an exit code.
    pub exit: Option<i32>,
    pub stdout: Digest,
    pub stderr: Digest,
    pub env: Digest,
    pub observation: CheckObservationKind,
    pub outcome: CheckOutcome,
    pub at: u64,
}
impl CheckRun {
    pub fn reference(&self) -> Result<Ref> {
        Ok(Ref {
            id: self.id.erased(),
            version: Digest::of_value(self)?,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Polarity {
    Supports,
    Contradicts,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Discrimination {
    pub baseline_fails: Option<bool>,
    pub candidate_passes: bool,
    pub mutation_score: Option<crate::task::Real>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Evidence {
    pub id: Id<Evidence>,
    pub criterion: Id<Criterion>,
    pub result: Option<Id<crate::result::ResultVersion>>,
    pub class: crate::task::EvidenceClass,
    pub independence: Independence,
    pub runs: Vec<Id<CheckRun>>,
    pub discrimination: Discrimination,
    pub author: Option<Id<crate::identity::Agent>>,
    pub polarity: Polarity,
}
impl Evidence {
    pub fn validate(&self) -> Result<()> {
        if self.runs.len() > 128
            || self.runs.iter().collect::<BTreeSet<_>>().len() != self.runs.len()
            || self
                .discrimination
                .mutation_score
                .is_some_and(|score| !(0.0..=1.0).contains(&score.get()))
        {
            return Err(Denial::new(
                "evidence",
                "Evidence has duplicate runs or invalid discrimination",
            ));
        }
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReviewVerdict {
    Approve,
    Reject,
    NeedsEvidence,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FindingSeverity {
    Blocking,
    Advisory,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Finding {
    pub criterion: Option<Id<Criterion>>,
    pub text: String,
    pub severity: FindingSeverity,
    pub proposed_check: Option<CheckSpec>,
}
impl Finding {
    pub fn validate(&self) -> Result<()> {
        crate::require_text(&self.text, 4096)?;
        if let Some(spec) = &self.proposed_check {
            match spec {
                CheckSpec::Command {
                    program,
                    args,
                    inputs,
                } if program == &WorkspacePath::root()
                    || inputs.contains(&WorkspacePath::root())
                    || inputs.len() > 1024
                    || args.len() > 256
                    || args.iter().any(|a| a.len() > 16_384 || a.contains('\0')) =>
                {
                    return Err(Denial::new(
                        "finding_check",
                        "Suggested command exceeds bounded check syntax",
                    ));
                }
                CheckSpec::ExactBytes { path, .. } if path == &WorkspacePath::root() => {
                    return Err(Denial::new(
                        "finding_check",
                        "Suggested byte check must name a file",
                    ));
                }
                _ => {}
            }
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Review {
    pub id: Id<Review>,
    pub result: Id<crate::result::ResultVersion>,
    pub reviewer: Id<crate::identity::Agent>,
    pub profile: crate::identity::ExecutionProfile,
    pub verdict: ReviewVerdict,
    pub findings: Vec<Finding>,
    pub basis: Vec<Id<Evidence>>,
}
impl Review {
    pub fn validate(&self) -> Result<()> {
        self.profile.validate()?;
        if self.reviewer != self.profile.agent
            || self.findings.len() > 256
            || self.basis.len() > 128
            || self.basis.iter().collect::<BTreeSet<_>>().len() != self.basis.len()
        {
            return Err(Denial::new(
                "review",
                "Invalid reviewer, findings or repeated evidence",
            ));
        }
        for finding in &self.findings {
            finding.validate()?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConfirmationBasis {
    TrustedCheck,
    ExternalData,
    Consequences,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConfirmationGrade {
    Refuted,
    Unconfirmed,
    Discriminated,
    Confirmed(ConfirmationBasis),
}
impl ConfirmationGrade {
    pub fn rank(self) -> u8 {
        match self {
            Self::Refuted => 0,
            Self::Unconfirmed => 1,
            Self::Discriminated => 2,
            Self::Confirmed(_) => 3,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AcceptanceDecision {
    Accepted,
    Rejected(String),
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AcceptanceSubject {
    ResultVersion(Id<crate::result::ResultVersion>),
    FinalAggregate(Id),
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Acceptance {
    pub id: Id<Acceptance>,
    pub subject: AcceptanceSubject,
    pub decision: AcceptanceDecision,
    pub grades: BTreeMap<Id<Criterion>, ConfirmationGrade>,
    pub grade: ConfirmationGrade,
    pub basis: Vec<Ref>,
    pub at: u64,
}
impl Acceptance {
    pub fn reference(&self) -> Result<Ref> {
        Ok(Ref {
            id: self.id.erased(),
            version: Digest::of_value(self)?,
        })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum LedgerStatus {
    Unmet,
    Supported,
    Satisfied,
    Contradicted,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LedgerEntry {
    pub status: LedgerStatus,
    pub belief: crate::Prob,
    pub evidence: Vec<Id<Evidence>>,
    pub stimulus: crate::task::Real,
    pub unmet_since: u64,
    pub changed: u64,
}
impl LedgerEntry {
    pub fn initial(at: u64) -> Result<Self> {
        Ok(Self {
            status: LedgerStatus::Unmet,
            belief: crate::Prob::new(0.5)?,
            evidence: vec![],
            stimulus: crate::task::Real::new(1.0)?,
            unmet_since: at,
            changed: at,
        })
    }
    pub fn updated(
        &self,
        status: LedgerStatus,
        belief: crate::Prob,
        mut evidence: Vec<Id<Evidence>>,
        at: u64,
    ) -> Self {
        evidence.sort();
        evidence.dedup();
        Self {
            status,
            belief,
            changed: if status != self.status || belief != self.belief || evidence != self.evidence
            {
                at
            } else {
                self.changed
            },
            unmet_since: if self.status == LedgerStatus::Satisfied
                && status != LedgerStatus::Satisfied
            {
                at
            } else {
                self.unmet_since
            },
            evidence,
            stimulus: self.stimulus,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CriteriaLedger {
    pub session: Id,
    pub entries: BTreeMap<Id<Criterion>, LedgerEntry>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssessmentRules {
    pub mutation_threshold: crate::Prob,
}
pub fn discriminating(evidence: &Evidence, criterion: &Criterion, rules: &AssessmentRules) -> bool {
    evidence.discrimination.candidate_passes
        && (criterion.kind != crate::task::CriterionKind::NewBehavior
            || evidence.discrimination.baseline_fails == Some(true))
        && evidence
            .discrimination
            .mutation_score
            .is_none_or(|score| score.get() >= rules.mutation_threshold.get())
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BeliefThresholds {
    pub behavior: crate::Prob,
    pub quality: crate::Prob,
    pub support: crate::Prob,
}
impl BeliefThresholds {
    pub fn validate(&self) -> Result<()> {
        if self.support.get() <= 0.0 || self.support > self.behavior || self.support > self.quality
        {
            return Err(Denial::new(
                "belief_thresholds",
                "Support must be positive and no greater than satisfaction thresholds",
            ));
        }
        Ok(())
    }
    pub fn for_criterion(&self, criterion: &Criterion) -> crate::Prob {
        if criterion.kind == crate::task::CriterionKind::Quality {
            self.quality
        } else {
            self.behavior
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LikelihoodRatioParameters {
    pub trusted: crate::task::Real,
    pub external: crate::task::Real,
    pub hidden: crate::task::Real,
    pub independent: crate::task::Real,
    pub inspection: crate::task::Real,
    pub producer: crate::task::Real,
    pub static_read: crate::task::Real,
    pub thresholds: BeliefThresholds,
}
impl LikelihoodRatioParameters {
    pub fn validate(&self) -> Result<()> {
        self.thresholds.validate()?;
        if [
            self.trusted,
            self.external,
            self.hidden,
            self.independent,
            self.inspection,
            self.producer,
            self.static_read,
        ]
        .iter()
        .any(|value| value.get() <= 0.0 || value.get() > 1_000_000.0)
        {
            return Err(Denial::new(
                "likelihood_ratio",
                "Likelihood ratios must be positive and bounded",
            ));
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StrongestSupportParameters {
    pub prior: crate::Prob,
    pub trusted: crate::Prob,
    pub independent: crate::Prob,
    pub inspection: crate::Prob,
    pub producer: crate::Prob,
    pub static_read: crate::Prob,
    pub thresholds: BeliefThresholds,
}
impl StrongestSupportParameters {
    pub fn validate(&self) -> Result<()> {
        self.thresholds.validate()?;
        if self.prior.get() <= 0.0 || self.prior.get() >= 1.0 {
            return Err(Denial::new(
                "belief_prior",
                "A prior must lie strictly between zero and one",
            ));
        }
        Ok(())
    }
}
