//! Effective access is a trusted backend guarantee, never a planner's path claim.
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum WorkspaceAccess {
    ReadAll,
    WriteAll,
    /// Relative, non-symlink paths enforced by the execution implementation.
    Scoped {
        reads: Vec<PathBuf>,
        writes: Vec<PathBuf>,
    },
}
impl WorkspaceAccess {
    pub fn conflicts(&self, other: &Self) -> bool {
        use WorkspaceAccess::*;
        let overlaps = |a: &PathBuf, b: &PathBuf| a.starts_with(b) || b.starts_with(a);
        match (self, other) {
            (WriteAll, _) | (_, WriteAll) => true,
            (ReadAll, ReadAll) => false,
            (ReadAll, Scoped { writes, .. }) | (Scoped { writes, .. }, ReadAll) => {
                !writes.is_empty()
            }
            (
                Scoped {
                    reads: ar,
                    writes: aw,
                },
                Scoped {
                    reads: br,
                    writes: bw,
                },
            ) => {
                aw.iter()
                    .any(|a| br.iter().chain(bw).any(|b| overlaps(a, b)))
                    || bw.iter().any(|b| ar.iter().any(|a| overlaps(a, b)))
            }
        }
    }
    pub fn is_read_only(&self) -> bool {
        matches!(self, Self::ReadAll)
            || matches!(self, Self::Scoped { writes, .. } if writes.is_empty())
    }
    pub fn covers(&self, actual: &Self) -> bool {
        if self == actual || matches!(self, Self::WriteAll) {
            return true;
        }
        match (self, actual) {
            (Self::ReadAll, access) => access.is_read_only(),
            (
                Self::Scoped { reads, writes },
                Self::Scoped {
                    reads: ar,
                    writes: aw,
                },
            ) => {
                ar.iter()
                    .all(|p| reads.iter().chain(writes).any(|r| p.starts_with(r)))
                    && aw.iter().all(|p| writes.iter().any(|w| p.starts_with(w)))
            }
            _ => false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceAccessDecision {
    pub reservation_id: String,
    pub policy: crate::ExecutionBackendIdentity,
    pub backend: crate::ExecutionBackendIdentity,
    pub directory: PathBuf,
    pub backend_access: WorkspaceAccess,
    pub effective_access: WorkspaceAccess,
    pub rationale: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceWait {
    pub code: String,
    pub holder: Option<String>,
    pub detail: String,
}
