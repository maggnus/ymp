//! Executable checks and attributed observations; no acceptance or evidence decisions.
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
