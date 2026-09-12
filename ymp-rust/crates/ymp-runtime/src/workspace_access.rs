//! Direct MVP resource coordination. No source copying or publication framework.
use anyhow::{ensure, Result};
use std::{
    path::{Component, Path, PathBuf},
    sync::{Arc, Mutex, OnceLock},
};
use tokio::sync::Notify;
use ymp_core::*;

pub struct WorkspaceAccessInput<'a> {
    pub directory: &'a Path,
    pub purpose: &'a str,
    pub task: Option<&'a Task>,
    pub backend_access: &'a WorkspaceAccess,
}
pub trait WorkspaceAccessPolicy: Send + Sync {
    fn identity(&self) -> ExecutionBackendIdentity;
    /// Read-only contributions produce their result as text in the session record.
    /// Native adapters receive the actual read_only permission request.
    fn execution_read_only(&self, task: &Task) -> bool {
        task.access == TaskAccess::ReadOnly
    }
    /// A proposal may broaden actual access or reject it, never invent protection.
    fn resolve(&self, input: &WorkspaceAccessInput<'_>) -> Result<WorkspaceAccess>;
}
pub struct DirectWorkspaceAccessPolicy;
impl WorkspaceAccessPolicy for DirectWorkspaceAccessPolicy {
    fn identity(&self) -> ExecutionBackendIdentity {
        ExecutionBackendIdentity {
            id: "ymp.direct-mvp".into(),
            version: "1".into(),
        }
    }
    fn resolve(&self, input: &WorkspaceAccessInput<'_>) -> Result<WorkspaceAccess> {
        Ok(input.backend_access.clone())
    }
}

pub(crate) fn validate_access(directory: &Path, access: &WorkspaceAccess) -> Result<()> {
    if let WorkspaceAccess::Scoped { reads, writes } = access {
        for path in reads.iter().chain(writes) {
            ensure!(
                !path.as_os_str().is_empty()
                    && path.components().all(|p| matches!(p, Component::Normal(_))),
                "unsupported_workspace_scope: paths must be normalized relative paths"
            );
            let mut current = directory.to_path_buf();
            for component in path.components() {
                current.push(component);
                match std::fs::symlink_metadata(&current) {
                    Ok(metadata) => {
                        ensure!(!metadata.file_type().is_symlink(), "unsupported_workspace_scope: symlink paths cannot establish disjoint access");
                        #[cfg(unix)]
                        {
                            use std::os::unix::fs::MetadataExt;
                            ensure!(!metadata.is_file() || metadata.nlink()==1, "unsupported_workspace_scope: hard-linked files cannot establish disjoint access");
                        }
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                    Err(e) => return Err(e.into()),
                }
            }
        }
    }
    Ok(())
}
#[derive(Clone)]
struct Held {
    id: String,
    session: String,
    agent: Option<String>,
    directory: PathBuf,
    access: WorkspaceAccess,
    invocation: bool,
}
#[derive(Default)]
pub(crate) struct AccessCoordinator {
    held: Mutex<Vec<Held>>,
    pub changed: Notify,
}
pub(crate) struct AccessLease {
    coordinator: Arc<AccessCoordinator>,
    pub id: String,
    pub record: Option<(ymp_storage::Store, String, WorkspaceAccessDecision)>,
}
impl Drop for AccessLease {
    fn drop(&mut self) {
        if let Some((store, session, access)) = &self.record {
            let _ = store.record_decision(&DecisionRecord {
                id: new_id(),
                session_id: session.clone(),
                kind: "workspace_access_released".into(),
                actor: None,
                reason: "Runtime resource ownership ended; this is not result acceptance".into(),
                outcome: None,
                links: RecordLinks {
                    workspace_access: Some(access.clone()),
                    ..Default::default()
                },
                created_at: now(),
            });
        }
        self.coordinator
            .held
            .lock()
            .unwrap()
            .retain(|h| h.id != self.id);
        self.coordinator.changed.notify_waiters();
    }
}
pub(crate) fn coordinator() -> Arc<AccessCoordinator> {
    static SHARED: OnceLock<Arc<AccessCoordinator>> = OnceLock::new();
    SHARED
        .get_or_init(|| Arc::new(AccessCoordinator::default()))
        .clone()
}
impl AccessCoordinator {
    #[allow(clippy::too_many_arguments)]
    pub fn acquire(
        self: &Arc<Self>,
        id: &str,
        session: &str,
        agent: Option<&str>,
        directory: &Path,
        access: &WorkspaceAccess,
        parent: Option<&str>,
        limit: usize,
    ) -> std::result::Result<AccessLease, WorkspaceWait> {
        let mut held = self.held.lock().unwrap();
        for h in held.iter() {
            if h.session == session && agent.is_some() && h.agent.as_deref() == agent {
                return Err(WorkspaceWait {
                    code: "agent_busy".into(),
                    holder: Some(h.id.clone()),
                    detail: "The agent has an active assignment".into(),
                });
            }
            if Some(h.id.as_str()) != parent
                && (directory.starts_with(&h.directory) || h.directory.starts_with(directory))
                && (directory != h.directory || h.access.conflicts(access))
            {
                return Err(WorkspaceWait {
                    code: "resource_conflict".into(),
                    holder: Some(h.id.clone()),
                    detail: "Effective filesystem access conflicts with active work".into(),
                });
            }
        }
        if agent.is_some()
            && held
                .iter()
                .filter(|h| h.session == session && h.invocation)
                .count()
                >= limit
        {
            return Err(WorkspaceWait {
                code: "concurrency_limit".into(),
                holder: None,
                detail: "The active invocation ceiling is occupied".into(),
            });
        }
        held.push(Held {
            id: id.into(),
            session: session.into(),
            agent: agent.map(str::to_owned),
            directory: directory.into(),
            access: access.clone(),
            invocation: agent.is_some(),
        });
        Ok(AccessLease {
            coordinator: self.clone(),
            id: id.into(),
            record: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scoped_resources_reject_path_aliases_and_conflict_by_components() {
        let temp = tempfile::tempdir().unwrap();
        let scope = |path: &str| WorkspaceAccess::Scoped {
            reads: vec![],
            writes: vec![path.into()],
        };
        assert!(validate_access(temp.path(), &scope("../escape")).is_err());
        assert!(validate_access(temp.path(), &scope("/absolute")).is_err());
        assert!(scope("outputs").conflicts(&WorkspaceAccess::Scoped {
            reads: vec!["outputs/report.txt".into()],
            writes: vec![]
        }));
        assert!(!scope("outputs/a.txt").conflicts(&scope("outputs/ab.txt")));
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(temp.path(), temp.path().join("alias")).unwrap();
            assert!(validate_access(temp.path(), &scope("alias/report.txt")).is_err());
            std::fs::write(temp.path().join("one"), "data").unwrap();
            std::fs::hard_link(temp.path().join("one"), temp.path().join("two")).unwrap();
            assert!(validate_access(temp.path(), &scope("two")).is_err());
        }
    }
}
