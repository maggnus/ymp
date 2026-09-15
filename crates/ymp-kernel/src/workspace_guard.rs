//! Trusted workspace registration and immutable capture. Path-lock admission and
//! evidence-backed cessation are the remaining part of W1-0005.
use crate::{
    events::Event,
    journal::{ContentStore, Journal, validate_append},
    ports::execution::WorkspaceProvider,
    view::SessionView,
};
use std::sync::Arc;
use ymp_domain::{
    Denial, Digest, Id, Result,
    journal::{Actor, Envelope, encode},
    workspace::*,
};

pub(crate) fn validate_tree_content(tree: &SnapshotTree, store: &dyn ContentStore) -> Result<()> {
    tree.validate()?;
    for file in tree.files.values() {
        let bytes = store.get(&file.digest, file.bytes as usize)?;
        if bytes.len() as u64 != file.bytes || Digest::of(&bytes) != file.digest {
            return Err(Denial::new(
                "snapshot_content",
                "Snapshot content is missing or does not match its manifest",
            ));
        }
    }
    Ok(())
}
fn validate_provider(workspace: &Workspace, provider: &dyn WorkspaceProvider) -> Result<()> {
    if workspace.provider != *provider.selection() || workspace.location != provider.location()? {
        return Err(Denial::new(
            "workspace_provider",
            "Provider configuration or physical workspace differs from the recorded identity",
        ));
    }
    Ok(())
}
pub struct WorkspaceGuard<J: Journal, C: ContentStore> {
    journal: Arc<J>,
    content: Arc<C>,
    issuer: Arc<()>,
}
impl<J: Journal, C: ContentStore> WorkspaceGuard<J, C> {
    pub fn new(journal: Arc<J>, content: Arc<C>) -> Self {
        Self {
            journal,
            content,
            issuer: Arc::new(()),
        }
    }
    pub fn view(&self, session: &Id) -> Result<SessionView> {
        self.journal
            .read(session)?
            .view_with_schemas(session, None, self.journal.schemas())
    }
    fn commit(&self, session: &Id, expected: u64, at: u64, payload: Event) -> Result<u64> {
        let current = self.journal.read(session)?;
        let (policy, refs) = match &payload {
            Event::LockChanged { change, .. } => {
                let view = current.view_with_schemas(session, None, self.journal.schemas())?;
                (None, crate::workspace_locks::attribution(&view, change)?)
            }
            Event::WorkspaceOpened { workspace, .. } => {
                (Some(workspace.provider.policy.clone()), vec![])
            }
            Event::SnapshotTaken { snapshot, .. } => {
                let view = current.view_with_schemas(session, None, self.journal.schemas())?;
                let workspace = view
                    .workspaces()
                    .get(&snapshot.workspace)
                    .ok_or_else(|| Denial::new("workspace_missing", "Workspace is not open"))?;
                (
                    Some(workspace.provider.policy.clone()),
                    vec![workspace.reference()?],
                )
            }
            _ => return Err(Denial::new("workspace_event", "Not a workspace event")),
        };
        let event = Envelope {
            seq: expected
                .checked_add(1)
                .ok_or_else(|| Denial::new("revision_overflow", "Journal sequence exhausted"))?,
            session: session.clone(),
            at,
            actor: Actor::Runtime,
            policy,
            input: None,
            refs,
            payload,
        };
        let events = [event];
        let next = validate_append(&current, session, expected, &events, self.journal.schemas())?;

        if crate::workspace_locks::touches_ownership(&events) {
            let inventory = self.journal.workspace_inventory(session)?;
            if inventory.current != current {
                return Err(Denial::new(
                    "stale_revision",
                    "Ownership inventory differs from the current journal",
                ));
            }
            crate::journal::validate_workspace_append(
                &inventory,
                session,
                expected,
                &events,
                self.journal.schemas(),
            )?;
        }
        let committed = self.journal.append(session, expected, &events)?;
        if committed != next.revision() {
            return Err(Denial::new(
                "journal_append",
                "Journal returned an unexpected revision",
            ));
        }
        Ok(committed)
    }
    pub fn open(
        &self,
        session: &Id,
        expected: u64,
        at: u64,
        id: Id<Workspace>,
        provider: &dyn WorkspaceProvider,
    ) -> Result<u64> {
        let location = provider.location()?;
        location.validate()?;
        self.commit(
            session,
            expected,
            at,
            Event::WorkspaceOpened {
                version: 1,
                workspace: Box::new(Workspace {
                    id,
                    kind: WorkspaceKind::Direct,
                    location,
                    provider: provider.selection().clone(),
                }),
            },
        )
    }
    pub fn snapshot(
        &self,
        session: &Id,
        expected: u64,
        at: u64,
        workspace: &Id<Workspace>,
        id: Id<Snapshot>,
        provider: &dyn WorkspaceProvider,
    ) -> Result<u64> {
        let view = self.view(session)?;
        if view.revision() != expected {
            return Err(Denial::new(
                "stale_revision",
                "Workspace state changed before capture",
            ));
        }
        let recorded = view
            .workspaces()
            .get(workspace)
            .ok_or_else(|| Denial::new("workspace_missing", "Workspace is not open"))?;
        validate_provider(recorded, provider)?;
        let tree = provider.capture(self.content.as_ref())?;
        validate_provider(recorded, provider)?;
        validate_tree_content(&tree, self.content.as_ref())?;
        let encoded = encode(&tree)?;
        let digest = Digest::of(&encoded);
        if self.content.put(&encoded)? != digest
            || self.content.get(&digest, encoded.len())? != encoded
        {
            return Err(Denial::new(
                "snapshot_content",
                "Snapshot manifest was not retained exactly",
            ));
        }
        self.commit(
            session,
            expected,
            at,
            Event::SnapshotTaken {
                version: 1,
                snapshot: Box::new(Snapshot {
                    id,
                    workspace: workspace.clone(),
                    tree,
                    taken: at,
                }),
            },
        )
    }
    /// Recover the retained manifest and validate all of its content, without consulting the live tree.
    pub fn retained(&self, session: &Id, id: &Id<Snapshot>) -> Result<Snapshot> {
        let view = self.view(session)?;
        let snapshot = view
            .snapshots()
            .get(id)
            .ok_or_else(|| Denial::new("snapshot_missing", "Snapshot was not recorded"))?;
        let expected = encode(&snapshot.tree)?;
        let bytes = self.content.get(&Digest::of(&expected), expected.len())?;
        if bytes != expected {
            return Err(Denial::new(
                "snapshot_content",
                "Stored snapshot manifest differs from the journal",
            ));
        }
        validate_tree_content(&snapshot.tree, self.content.as_ref())?;
        Ok(snapshot.clone())
    }
    pub fn read_artifact(
        &self,
        session: &Id,
        snapshot: &Id<Snapshot>,
        path: &WorkspacePath,
    ) -> Result<Vec<u8>> {
        let view = self.view(session)?;
        let snapshot = view
            .snapshots()
            .get(snapshot)
            .ok_or_else(|| Denial::new("snapshot_missing", "Snapshot was not recorded"))?;
        let file = snapshot
            .tree
            .files
            .get(path)
            .ok_or_else(|| Denial::new("artifact_missing", "File is not in the snapshot"))?;
        let bytes = self.content.get(
            &file.digest,
            file.bytes
                .try_into()
                .map_err(|_| Denial::new("snapshot_limit", "File size is not addressable"))?,
        )?;
        if bytes.len() as u64 != file.bytes || Digest::of(&bytes) != file.digest {
            return Err(Denial::new(
                "snapshot_content",
                "Artifact differs from the recorded content",
            ));
        }
        Ok(bytes)
    }
}

/// Kernel-owned evidence of an executor's actual, protected access boundary.
/// Factories for mediated and native execution must validate that boundary first.
/// Stored observations are not sufficient to recreate this capability.
/// ```compile_fail
/// let proof: ymp_kernel::workspace_guard::AccessEvidence = serde_json::from_str("{}").unwrap();
/// ```
pub struct AccessEvidence {
    session: Id,
    issuer: Arc<()>,
    workspace: ymp_domain::Ref,
    profile: ymp_domain::identity::ExecutionProfile,
    actual: Vec<(WorkspacePath, LockMode)>,
    basis: Vec<ymp_domain::Ref>,
}
/// Cessation evidence is deliberately distinct from revocation and financial settlement.
pub struct CessationEvidence {
    session: Id,
    issuer: Arc<()>,
    record: crate::workspace_locks::CessationRecord,
}
pub struct LockRequest {
    pub assignment: Id,
    pub workspace: Id<Workspace>,
    pub profile: ymp_domain::identity::ExecutionProfile,
    pub paths: Vec<(WorkspacePath, LockMode)>,
}
impl<J: Journal, C: ContentStore> WorkspaceGuard<J, C> {
    pub fn lock(
        &self,
        session: &Id,
        expected: u64,
        at: u64,
        request: LockRequest,
        provider: &dyn WorkspaceProvider,
        proof: &AccessEvidence,
    ) -> Result<u64> {
        let view = self.view(session)?;
        let workspace = view
            .workspaces()
            .get(&request.workspace)
            .ok_or_else(|| Denial::new("workspace_missing", "Workspace is not open"))?;
        if proof.session != *session
            || !Arc::ptr_eq(&proof.issuer, &self.issuer)
            || proof.workspace != workspace.reference()?
            || proof.profile != request.profile
            || proof.actual.is_empty()
            || proof.actual.len() > 256
        {
            return Err(Denial::new(
                "access_evidence",
                "Effective access lacks matching kernel evidence",
            ));
        }
        validate_provider(workspace, provider)?;
        let paths: Vec<_> = proof.actual.iter().map(|(p, _)| p.clone()).collect();
        let observations = provider.observe_paths(&paths)?;
        if observations.len() != paths.len()
            || observations.iter().zip(&paths).any(|(o, p)| o.path != *p)
        {
            return Err(Denial::new(
                "path_observation",
                "Provider did not return the requested physical observations",
            ));
        }
        let effective = proof
            .actual
            .iter()
            .zip(observations)
            .map(|((path, mode), observation)| ObservedPathLock {
                lock: PathLock {
                    path: path.clone(),
                    mode: *mode,
                    holder: request.assignment.clone(),
                },
                observation,
            })
            .collect();
        let requested = request
            .paths
            .into_iter()
            .map(|(path, mode)| PathLock {
                path,
                mode,
                holder: request.assignment.clone(),
            })
            .collect();
        validate_provider(workspace, provider)?;
        self.commit(
            session,
            expected,
            at,
            Event::LockChanged {
                version: 1,
                change: crate::workspace_locks::LockChange::Acquired(Box::new(
                    crate::workspace_locks::LockAcquisition {
                        workspace: request.workspace,
                        assignment: request.assignment,
                        profile: request.profile,
                        requested,
                        effective,
                        basis: proof.basis.clone(),
                    },
                )),
            },
        )
    }
    pub fn authorize_access(
        &self,
        session: &Id,
        expected: u64,
        at: u64,
        assignment: Id,
        invocation: Id,
        provider: &dyn WorkspaceProvider,
    ) -> Result<u64> {
        let view = self.view(session)?;
        let record = view
            .path_locks()
            .get(&assignment)
            .ok_or_else(|| Denial::new("locks_missing", "No path ownership for assignment"))?;
        let workspace = &view.workspaces()[&record.acquired.workspace];
        validate_provider(workspace, provider)?;
        let paths: Vec<_> = record
            .acquired
            .effective
            .iter()
            .map(|l| l.lock.path.clone())
            .collect();
        let observations = provider.observe_paths(&paths)?;
        if observations
            != record
                .acquired
                .effective
                .iter()
                .map(|l| l.observation.clone())
                .collect::<Vec<_>>()
        {
            return Err(Denial::new(
                "workspace_changed",
                "Path topology changed before authorization",
            ));
        }
        self.commit(
            session,
            expected,
            at,
            Event::LockChanged {
                version: 1,
                change: crate::workspace_locks::LockChange::Authorized {
                    assignment,
                    invocation,
                },
            },
        )
    }
    pub fn revoke_access(
        &self,
        session: &Id,
        expected: u64,
        at: u64,
        assignment: Id,
        reason: String,
    ) -> Result<u64> {
        self.commit(
            session,
            expected,
            at,
            Event::LockChanged {
                version: 1,
                change: crate::workspace_locks::LockChange::Revoked { assignment, reason },
            },
        )
    }
    pub fn never_authorized(&self, session: &Id, assignment: &Id) -> Result<CessationEvidence> {
        let view = self.view(session)?;
        let record = view
            .path_locks()
            .get(assignment)
            .ok_or_else(|| Denial::new("locks_missing", "No path ownership for assignment"))?;
        if record.invocation.is_some() || record.released.is_some() {
            return Err(Denial::new(
                "execution_uncertain",
                "Access has been authorized or already released",
            ));
        }
        Ok(CessationEvidence {
            session: session.clone(),
            issuer: self.issuer.clone(),
            record: crate::workspace_locks::CessationRecord {
                assignment: assignment.clone(),
                workspace: record.acquired.workspace.clone(),
                invocation: None,
                state: record.last.clone(),
                kind: crate::workspace_locks::Cessation::NeverAuthorized,
                basis: vec![record.last.clone()],
            },
        })
    }
    pub fn release(
        &self,
        session: &Id,
        expected: u64,
        at: u64,
        evidence: &CessationEvidence,
    ) -> Result<u64> {
        if evidence.session != *session || !Arc::ptr_eq(&self.issuer, &evidence.issuer) {
            return Err(Denial::new(
                "cessation_evidence",
                "Cessation evidence belongs to another authority",
            ));
        }
        self.commit(
            session,
            expected,
            at,
            Event::LockChanged {
                version: 1,
                change: crate::workspace_locks::LockChange::Released(evidence.record.clone()),
            },
        )
    }
}

#[cfg(test)]
#[path = "workspace_guard_tests.rs"]
mod tests;
