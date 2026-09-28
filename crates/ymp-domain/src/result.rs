//! Immutable candidate content; submission is not acceptance.
pub use crate::workspace::Artifact;
use crate::{
    Denial, Digest, Id, Ref, Result,
    identity::{Agent, ExecutionProfile},
    plan::WorkItem,
    workspace::{Snapshot, WorkspacePath},
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResultVersion {
    pub id: Id<ResultVersion>,
    pub item: Id<WorkItem>,
    pub producer: Id<Agent>,
    pub profile: ExecutionProfile,
    pub before: Id<Snapshot>,
    pub after: Id<Snapshot>,
    pub artifacts: BTreeSet<Artifact>,
    pub summary: String,
}
impl ResultVersion {
    pub fn validate(&self) -> Result<()> {
        self.profile.validate()?;
        crate::require_text(&self.summary, 4096)?;
        if self.before == self.after
            || self.artifacts.is_empty()
            || self.artifacts.len() > 4096
            || self
                .artifacts
                .iter()
                .map(|a| &a.path)
                .collect::<BTreeSet<_>>()
                .len()
                != self.artifacts.len()
            || self
                .artifacts
                .iter()
                .any(|a| a.path == WorkspacePath::root())
        {
            return Err(Denial::new(
                "result_version",
                "A candidate needs distinct snapshots and unique bounded artifact paths",
            ));
        }
        Ok(())
    }
    pub fn reference(&self) -> Result<Ref> {
        Ok(Ref {
            id: self.id.erased(),
            version: Digest::of_value(self)?,
        })
    }
}
