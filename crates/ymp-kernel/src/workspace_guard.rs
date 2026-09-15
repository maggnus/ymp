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
}
impl<J: Journal, C: ContentStore> WorkspaceGuard<J, C> {
    pub fn new(journal: Arc<J>, content: Arc<C>) -> Self {
        Self { journal, content }
    }
    pub fn view(&self, session: &Id) -> Result<SessionView> {
        self.journal
            .read(session)?
            .view_with_schemas(session, None, self.journal.schemas())
    }
    fn commit(&self, session: &Id, expected: u64, at: u64, payload: Event) -> Result<u64> {
        let current = self.journal.read(session)?;
        let (policy, refs) = match &payload {
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
