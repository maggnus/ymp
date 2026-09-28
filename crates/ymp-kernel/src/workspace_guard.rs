//! Trusted snapshots, physical path ownership and bounded mediated file access.
use crate::{
    events::Event,
    journal::{ContentStore, Journal, validate_append},
    ports::execution::WorkspaceProvider,
    view::SessionView,
};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};
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
#[derive(Clone)]
struct PendingCapture {
    workspace: Id<Workspace>,
    started: ymp_domain::Ref,
    owner: Digest,
    taken: u64,
    began: bool,
    // None means I/O has not returned; Some is a local completion capability.
    outcome: Option<Result<Digest>>,
}
fn bounded_capture_error(error: Denial) -> Denial {
    if error.code.len() > 256 || error.message.len() > 3800 || error.refs.len() > 32 {
        Denial::new(
            "capture_failed",
            "Capture failed with an oversized provider error",
        )
    } else {
        error
    }
}
pub struct WorkspaceGuard<J: Journal, C: ContentStore> {
    journal: Arc<J>,
    content: Arc<C>,
    issuer: Arc<()>,
    captures: Mutex<BTreeMap<(Id, Id<Snapshot>), PendingCapture>>,
}
impl<J: Journal, C: ContentStore> WorkspaceGuard<J, C> {
    pub(crate) fn prepare_invocation_access(
        &self,
        access: &MediatedAccess<J>,
        assignment: &Id,
        invocation: &Id,
    ) -> Result<()> {
        if !Arc::ptr_eq(&access.issuer, &self.issuer) || &access.assignment != assignment {
            return Err(Denial::new(
                "invocation_scope",
                "File capability belongs to another assignment or authority",
            ));
        }
        access.check_open()?;
        let view = self.view(&access.session)?;
        let record = view
            .path_locks()
            .get(assignment)
            .ok_or_else(|| Denial::new("locks_missing", "No ownership for this file capability"))?;
        if record.acquired.mediated_owner.as_ref() != Some(&access.owner)
            || record.revoked
            || record.released.is_some()
            || record.invocation.is_some()
            || view
                .path_locks()
                .values()
                .any(|lock| lock.invocation.as_ref() == Some(invocation))
        {
            return Err(Denial::new(
                "invocation_scope",
                "File capability has already been authorized or closed",
            ));
        }
        let paths: Vec<_> = record
            .acquired
            .effective
            .iter()
            .map(|lock| lock.lock.path.clone())
            .collect();
        let observations = access.provider.observe_paths(&paths)?;
        if observations
            != record
                .acquired
                .effective
                .iter()
                .map(|lock| lock.observation.clone())
                .collect::<Vec<_>>()
        {
            return Err(Denial::new(
                "path_changed",
                "Physical scope changed before invocation dispatch",
            ));
        }
        access
            .provider
            .verify_binding(&access.binding, self.journal.as_ref())?;
        Ok(())
    }
    pub fn new(journal: Arc<J>, content: Arc<C>) -> Self {
        Self {
            journal,
            content,
            issuer: Arc::new(()),
            captures: Mutex::new(BTreeMap::new()),
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
            Event::WorkspaceBound { workspace, .. } => {
                let view = current.view_with_schemas(session, None, self.journal.schemas())?;
                (
                    None,
                    vec![
                        view.workspaces()
                            .get(workspace)
                            .ok_or_else(|| {
                                Denial::new("workspace_missing", "Workspace is not open")
                            })?
                            .reference()?,
                    ],
                )
            }
            Event::WorkspaceOpened { workspace, .. } => {
                (Some(workspace.provider.policy.clone()), vec![])
            }
            Event::SnapshotTaken { snapshot, version } => {
                let view = current.view_with_schemas(session, None, self.journal.schemas())?;
                let workspace = view
                    .workspaces()
                    .get(&snapshot.workspace)
                    .ok_or_else(|| Denial::new("workspace_missing", "Workspace is not open"))?;
                let mut refs = vec![workspace.reference()?];
                if *version == 2 {
                    refs.push(
                        view.capture_reads()
                            .get(&snapshot.id)
                            .ok_or_else(|| {
                                Denial::new("capture_missing", "Snapshot has no capture read hold")
                            })?
                            .started
                            .clone(),
                    );
                    refs.sort();
                    refs.dedup();
                }
                (Some(workspace.provider.policy.clone()), refs)
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
    pub fn bind_workspace(
        &self,
        session: &Id,
        expected: u64,
        at: u64,
        workspace: &Id<Workspace>,
        provider: &dyn WorkspaceProvider,
    ) -> Result<u64> {
        let view = self.view(session)?;
        if view.revision() != expected {
            return Err(Denial::new(
                "stale_revision",
                "Workspace changed before binding",
            ));
        }
        let recorded = view
            .workspaces()
            .get(workspace)
            .ok_or_else(|| Denial::new("workspace_missing", "Workspace is not open"))?;
        validate_provider(recorded, provider)?;
        let journal = self.journal.binding_identity()?;
        let binding = provider.bind(self.journal.as_ref())?;
        binding.validate()?;
        if binding.root != recorded.location || binding.journal != journal {
            return Err(Denial::new(
                "workspace_binding",
                "Provider binding differs from the workspace or journal identity",
            ));
        }
        provider.verify_binding(&binding, self.journal.as_ref())?;
        if let Some(prior) = view.workspace_bindings().get(workspace) {
            if prior != &binding {
                return Err(Denial::new(
                    "workspace_binding",
                    "Workspace binding cannot be replaced",
                ));
            }
            return Ok(view.revision());
        }
        self.commit(
            session,
            expected,
            at,
            Event::WorkspaceBound {
                version: 1,
                workspace: workspace.clone(),
                binding: Box::new(binding),
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
        self.capture(session, expected, at, workspace, id, provider, None, None)
    }
    /// Observe the exact baseline while this admitted writer still has no
    /// invocation authority; its held write scope remains protected afterwards.
    pub fn snapshot_unstarted(
        &self,
        access: &MediatedAccess<J>,
        id: Id<Snapshot>,
        at: u64,
    ) -> Result<u64> {
        if !Arc::ptr_eq(&access.issuer, &self.issuer) {
            return Err(Denial::new(
                "capture_protection",
                "Foreign mediated capability",
            ));
        }
        access.check_open()?;
        let view = self.view(&access.session)?;
        let lock = view
            .path_locks()
            .get(&access.assignment)
            .ok_or_else(|| Denial::new("locks_missing", "No baseline protection"))?;
        if lock.acquired.mediated_owner.as_ref() != Some(&access.owner) {
            return Err(Denial::new(
                "capture_protection",
                "Different mediated owner",
            ));
        }
        self.capture(
            &access.session,
            view.revision(),
            at,
            &lock.acquired.workspace,
            id,
            access.provider.as_ref(),
            None,
            Some(&access.assignment),
        )
    }
    /// Transfer proved production cessation directly into the result's read hold.
    /// No competing writer can enter between release and capture admission.
    pub fn snapshot_withdrawn(
        &self,
        access: &MediatedAccess<J>,
        evidence: &CessationEvidence,
        id: Id<Snapshot>,
        at: u64,
    ) -> Result<u64> {
        if !Arc::ptr_eq(&access.issuer, &self.issuer)
            || !Arc::ptr_eq(&evidence.issuer, &self.issuer)
            || evidence.session != access.session
            || evidence.record.assignment != access.assignment
        {
            return Err(Denial::new(
                "cessation_evidence",
                "Result capture requires this guard's exact ceased capability",
            ));
        }
        let view = self.view(&access.session)?;
        self.capture(
            &access.session,
            view.revision(),
            at,
            &evidence.record.workspace,
            id,
            access.provider.as_ref(),
            Some(evidence),
            None,
        )
    }
    #[allow(clippy::too_many_arguments)]
    fn capture(
        &self,
        session: &Id,
        expected: u64,
        at: u64,
        workspace: &Id<Workspace>,
        id: Id<Snapshot>,
        provider: &dyn WorkspaceProvider,
        release: Option<&CessationEvidence>,
        protected_by: Option<&Id>,
    ) -> Result<u64> {
        let view = self.view(session)?;
        if view.revision() != expected {
            return Err(Denial::new(
                "stale_revision",
                "Workspace state changed before capture",
            ));
        }
        if view.capture_reads().contains_key(&id) || view.snapshots().contains_key(&id) {
            return Err(Denial::new(
                "capture_identity",
                "Snapshot identity has already been used",
            ));
        }
        let recorded = view
            .workspaces()
            .get(workspace)
            .ok_or_else(|| Denial::new("workspace_missing", "Workspace is not open"))?;
        validate_provider(recorded, provider)?;
        if let Some(binding) = view.workspace_bindings().get(workspace) {
            provider.verify_binding(binding, self.journal.as_ref())?;
        }
        let observations = provider.observe_paths(&[WorkspacePath::root()])?;
        if observations.len() != 1 || observations[0].path != WorkspacePath::root() {
            return Err(Denial::new(
                "path_observation",
                "Capture requires an observed workspace root",
            ));
        }
        let mut random = [0u8; 32];
        getrandom::fill(&mut random).map_err(|_| {
            Denial::new(
                "capture_owner",
                "Cannot create a unique capture attempt identity",
            )
        })?;
        let owner = Digest::of(random);
        let change = crate::workspace_locks::LockChange::CaptureStarted {
            protected_by: protected_by.cloned(),
            owner: owner.clone(),
            snapshot: id.clone(),
            workspace: workspace.clone(),
            observation: observations[0].clone(),
        };
        let payload = Event::LockChanged {
            version: 1,
            change: change.clone(),
        };
        let mut events = Vec::new();
        let mut capture_view = view.clone();
        if let Some(evidence) = release {
            let change = crate::workspace_locks::LockChange::Released(evidence.record.clone());
            let event = Envelope {
                seq: expected + 1,
                session: session.clone(),
                at,
                actor: Actor::Runtime,
                policy: None,
                input: None,
                refs: crate::workspace_locks::attribution(&capture_view, &change)?,
                payload: Event::LockChanged { version: 1, change },
            };
            capture_view.apply(&event, self.journal.schemas())?;
            events.push(event);
        }
        let capture_event = Envelope {
            seq: capture_view
                .revision()
                .checked_add(1)
                .ok_or_else(|| Denial::new("revision_overflow", "Journal sequence exhausted"))?,
            session: session.clone(),
            at,
            actor: Actor::Runtime,
            policy: None,
            input: None,
            refs: crate::workspace_locks::attribution(&capture_view, &change)?,
            payload: payload.clone(),
        };
        let started = capture_event.reference()?;
        events.push(capture_event);
        let key = (session.clone(), id.clone());
        {
            let mut pending = self.captures.lock().map_err(|_| {
                Denial::new("capture_state", "Local completion state is unavailable")
            })?;
            if pending.contains_key(&key) || pending.len() >= 64 {
                return Err(Denial::new(
                    "capture_pending",
                    "Resolve earlier capture completions before starting more work",
                ));
            }
            pending.insert(
                key.clone(),
                PendingCapture {
                    workspace: workspace.clone(),
                    started: started.clone(),
                    owner: owner.clone(),
                    taken: at,
                    began: false,
                    outcome: None,
                },
            );
        }
        let committed = self
            .journal
            .append(session, expected, &events)
            .and_then(|actual| {
                if actual == expected + events.len() as u64 {
                    Ok(actual)
                } else {
                    Err(Denial::new(
                        "journal_append",
                        "Capture packet returned an unexpected revision",
                    ))
                }
            });
        if let Err(error) = committed {
            // Our random attempt owner distinguishes this unstarted attempt from
            // another Guard's otherwise identical request. Only a matching owner
            // and start reference can authorize its abort after acknowledgement loss.
            self.captures
                .lock()
                .map_err(|_| Denial::new("capture_state", "Local completion state is unavailable"))?
                .get_mut(&key)
                .expect("registered capture")
                .outcome = Some(Err(bounded_capture_error(error.clone())));
            if let Ok(current) = self.view(session) {
                let own_begin = current.capture_reads().get(&id).is_some_and(|capture| {
                    capture.owner == owner
                        && capture.started == started
                        && capture.workspace == *workspace
                });
                if own_begin {
                    let _ = self.resolve_capture(session, current.revision(), at, &id);
                } else {
                    // We never started I/O and the immutable recorded identity
                    // belongs elsewhere (or is absent). There is nothing of ours to abort.
                    self.captures
                        .lock()
                        .map_err(|_| {
                            Denial::new("capture_state", "Local completion state is unavailable")
                        })?
                        .remove(&key);
                }
            }
            return Err(error.with_ref(started));
        }
        self.captures
            .lock()
            .map_err(|_| Denial::new("capture_state", "Local completion state is unavailable"))?
            .get_mut(&key)
            .expect("registered capture")
            .began = true;
        // The provider contract is synchronous: returning means all capture I/O ended.
        let outcome = (|| {
            validate_provider(recorded, provider)?;
            let identity = self.journal.binding_identity();
            let tree = provider.capture(&identity, self.content.as_ref())?;
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
            Ok(digest)
        })()
        .map_err(bounded_capture_error);
        self.captures
            .lock()
            .map_err(|_| Denial::new("capture_state", "Local completion state is unavailable"))?
            .get_mut(&key)
            .expect("registered capture")
            .outcome = Some(outcome.clone());
        let current = self.view(session)?.revision();
        let committed = self.resolve_capture(session, current, at, &id)?;
        match outcome {
            Ok(_) => Ok(committed),
            Err(error) => Err(error),
        }
    }
    /// Retry only publication/abort after this Guard observed synchronous I/O end.
    /// Reopening a journal does not recreate this completion capability.
    pub fn resolve_capture(
        &self,
        session: &Id,
        expected: u64,
        at: u64,
        id: &Id<Snapshot>,
    ) -> Result<u64> {
        let key = (session.clone(), id.clone());
        let mut completions = self
            .captures
            .lock()
            .map_err(|_| Denial::new("capture_state", "Local completion state is unavailable"))?;
        let pending = completions.get(&key).cloned();
        let view = self.view(session)?;
        let capture = view.capture_reads().get(id);
        let Some(pending) = pending else {
            if capture.is_some_and(|c| c.ended.is_some()) {
                return Ok(view.revision());
            }
            return Err(Denial::new(
                "capture_evidence",
                "No local completed-I/O evidence for this capture",
            ));
        };
        let outcome = pending
            .outcome
            .ok_or_else(|| Denial::new("capture_active", "Capture I/O has not returned"))?;
        let Some(capture) = capture else {
            // Beginning never committed; no I/O was started and no hold can be released.
            if !pending.began {
                completions.remove(&key);
            }
            return Err(Denial::new(
                "capture_missing",
                "Capture beginning is not recorded",
            ));
        };
        if capture.started != pending.started
            || capture.workspace != pending.workspace
            || capture.owner != pending.owner
        {
            if !pending.began {
                completions.remove(&key);
            }
            return Err(Denial::new(
                "capture_evidence",
                "Completion belongs to another capture hold",
            ));
        }
        if capture.ended.is_some() {
            let same = match &outcome {
                Ok(digest) => view.snapshots().get(id).is_some_and(|snapshot| {
                    snapshot.workspace == pending.workspace
                        && snapshot.taken == pending.taken
                        && snapshot.tree.digest().as_ref() == Ok(digest)
                }),
                Err(error) => {
                    capture.failure.as_ref() == Some(&format!("{}: {}", error.code, error.message))
                }
            };
            if !same {
                return Err(Denial::new(
                    "capture_conflict",
                    "Capture ended with a different outcome",
                ));
            }
            completions.remove(&key);
            return Ok(view.revision());
        }
        let payload = match outcome {
            Ok(digest) => {
                let bytes = self.content.get(&digest, 32 * 1024 * 1024)?;
                if Digest::of(&bytes) != digest {
                    return Err(Denial::new(
                        "snapshot_content",
                        "Completed manifest content is corrupt",
                    ));
                }
                let tree: SnapshotTree = ymp_domain::journal::decode(&bytes)?;
                validate_tree_content(&tree, self.content.as_ref())?;
                Event::SnapshotTaken {
                    version: 2,
                    snapshot: Box::new(Snapshot {
                        id: id.clone(),
                        workspace: pending.workspace,
                        tree,
                        taken: pending.taken,
                    }),
                }
            }
            Err(error) => Event::LockChanged {
                version: 1,
                change: crate::workspace_locks::LockChange::CaptureAborted {
                    snapshot: id.clone(),
                    started: pending.started,
                    reason: format!("{}: {}", error.code, error.message),
                },
            },
        };
        let committed = self.commit(session, expected, at, payload)?;
        completions.remove(&key);
        Ok(committed)
    }
    /// Discard an unpublished capture only after this Guard knows its I/O ended.
    pub fn abandon_capture(
        &self,
        session: &Id,
        expected: u64,
        at: u64,
        id: &Id<Snapshot>,
        reason: String,
    ) -> Result<u64> {
        ymp_domain::require_text(&reason, 3800)?;
        let view = self.view(session)?;
        if view.revision() != expected {
            return Err(Denial::new(
                "stale_revision",
                "Capture state changed before abandonment",
            ));
        }
        if view
            .capture_reads()
            .get(id)
            .is_none_or(|c| c.ended.is_some())
        {
            return Err(Denial::new(
                "capture_state",
                "Capture is absent or already ended",
            ));
        }
        {
            let mut pending = self.captures.lock().map_err(|_| {
                Denial::new("capture_state", "Local completion state is unavailable")
            })?;
            let entry = pending
                .get_mut(&(session.clone(), id.clone()))
                .ok_or_else(|| {
                    Denial::new("capture_evidence", "No local completed-I/O evidence")
                })?;
            if entry.outcome.is_none() {
                return Err(Denial::new(
                    "capture_active",
                    "Capture I/O has not returned",
                ));
            }
            entry.outcome = Some(Err(Denial::new("capture_abandoned", reason)));
        }
        self.resolve_capture(session, expected, at, id)
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
    mediated_owner: Option<Digest>,
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
        let acquisition = self.prepare_lock(session, request, provider, proof)?;
        self.commit(
            session,
            expected,
            at,
            Event::LockChanged {
                version: 1,
                change: crate::workspace_locks::LockChange::Acquired(Box::new(acquisition)),
            },
        )
    }
    fn prepare_lock(
        &self,
        session: &Id,
        request: LockRequest,
        provider: &dyn WorkspaceProvider,
        proof: &AccessEvidence,
    ) -> Result<crate::workspace_locks::LockAcquisition> {
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
        Ok(crate::workspace_locks::LockAcquisition {
            mediated_owner: proof.mediated_owner.clone(),
            workspace: request.workspace,
            assignment: request.assignment,
            profile: request.profile,
            requested,
            effective,
            basis: proof.basis.clone(),
        })
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

/// One borrowed kernel-validated file operation. Not a reusable or serialized grant.
/// ```compile_fail
/// let op: ymp_kernel::workspace_guard::FileAccess = serde_json::from_str("{}").unwrap();
/// ```
pub struct FileAccess {
    binding: WorkspaceBinding,
    scope: PathObservation,
    target: PathObservation,
    limit: usize,
    created_file: Option<FileIdentity>,
    path: WorkspacePath,
    mode: LockMode,
}
impl FileAccess {
    pub fn limit(&self) -> usize {
        self.limit
    }
    pub fn target(&self) -> &PathObservation {
        &self.target
    }
    pub fn created_file(&self) -> Option<&FileIdentity> {
        self.created_file.as_ref()
    }
    pub fn scope(&self) -> &PathObservation {
        &self.scope
    }
    pub fn binding(&self) -> &WorkspaceBinding {
        &self.binding
    }
    pub fn journal(&self) -> &JournalIdentity {
        &self.binding.journal
    }
    pub fn path(&self) -> &WorkspacePath {
        &self.path
    }
    pub fn mode(&self) -> LockMode {
        self.mode
    }
}
/// Files-only capability. The host exposes these methods, never the provider,
/// root descriptor or an arbitrary process API, to the Scripted participant.
enum CreationOutcome {
    NotAttempted,
    Uncertain,
    Observed(FileIdentity),
}
struct LocalCreation {
    owner: Digest,
    path: WorkspacePath,
    outcome: CreationOutcome,
}
pub struct MediatedAccess<J: Journal> {
    journal: Arc<J>,
    provider: Arc<dyn WorkspaceProvider>,
    session: Id,
    assignment: Id,
    owner: Digest,
    issuer: Arc<()>,
    binding: WorkspaceBinding,
    acquisition: ymp_domain::Ref,
    closed: std::sync::atomic::AtomicBool,
    operation: Mutex<Option<LocalCreation>>,
}
impl<J: Journal> MediatedAccess<J> {
    pub fn request_withdrawal(&self) {
        self.closed.store(true, std::sync::atomic::Ordering::SeqCst);
    }
    fn check_open(&self) -> Result<()> {
        if self.closed.load(std::sync::atomic::Ordering::SeqCst) {
            return Err(Denial::new(
                "access_closed",
                "Mediated access has been withdrawn",
            ));
        }
        Ok(())
    }
    fn operation(&self, path: &WorkspacePath, mode: LockMode, limit: usize) -> Result<FileAccess> {
        self.check_open()?;
        let inventory = self.journal.workspace_inventory(&self.session)?;
        let view =
            inventory
                .current
                .view_with_schemas(&self.session, None, self.journal.schemas())?;
        let record = view
            .path_locks()
            .get(&self.assignment)
            .ok_or_else(|| Denial::new("locks_missing", "No recorded access for this handle"))?;
        if record.acquired.mediated_owner.as_ref() != Some(&self.owner)
            || record.released.is_some()
            || record.revoked
            || record.invocation.is_none()
        {
            return Err(Denial::new(
                "invocation_authority",
                "This mediated handle has no active authorized invocation",
            ));
        }
        view.resolve(&self.acquisition)?;
        crate::workspace_locks::validate_profile(&view, &record.acquired)?;
        if view.workspace_bindings().get(&record.acquired.workspace) != Some(&self.binding) {
            return Err(Denial::new(
                "workspace_binding",
                "Mediated access lost its recorded root binding",
            ));
        }
        // Exclusivity of a Write lock is not permission to perform Read.
        let scope =
            record
                .acquired
                .effective
                .iter()
                .find(|claim| {
                    claim.lock.mode == mode
                        && claim.lock.path.contains(path)
                        && record.acquired.requested.iter().any(|requested| {
                            requested.mode == mode && requested.path.contains(path)
                        })
                })
                .ok_or_else(|| {
                    Denial::new(
                        "access_scope",
                        "Operation is outside this assignment's requested permission",
                    )
                })?
                .observation
                .clone();
        validate_provider(
            &view.workspaces()[&record.acquired.workspace],
            self.provider.as_ref(),
        )?;
        self.provider
            .verify_binding(&self.binding, self.journal.as_ref())?;
        let journal = self.journal.binding_identity()?;
        if journal != self.binding.journal {
            return Err(Denial::new(
                "workspace_binding",
                "The journal identity changed",
            ));
        }
        let mut observations = self.provider.observe_paths(std::slice::from_ref(path))?;
        if observations.len() != 1 || observations[0].path != *path {
            return Err(Denial::new(
                "path_observation",
                "Provider returned a different operation path",
            ));
        }
        let target = observations.remove(0);
        target.validate(&self.binding.root)?;
        let actual = ObservedPathLock {
            lock: PathLock {
                path: path.clone(),
                mode,
                holder: self.assignment.clone(),
            },
            observation: target.clone(),
        };
        let current = crate::workspace_locks::WorkspaceOwnership::from_view(&view);
        for owner in inventory.other.iter().chain(std::iter::once(&current)) {
            owner.validate_operation(&self.session, &self.assignment, &actual)?;
        }
        Ok(FileAccess {
            binding: self.binding.clone(),
            scope,
            target,
            limit,
            created_file: record
                .file_holds
                .values()
                .find(|hold| hold.lock.path == *path && hold.lock.mode == LockMode::Write)
                .and_then(|hold| hold.observation.existing.last())
                .map(|part| part.identity.clone()),
            path: path.clone(),
            mode,
        })
    }
    fn commit_creation(&self, change: crate::workspace_locks::LockChange, at: u64) -> Result<()> {
        let inventory = self.journal.workspace_inventory(&self.session)?;
        let view =
            inventory
                .current
                .view_with_schemas(&self.session, None, self.journal.schemas())?;
        let expected = view.revision();
        let event = Envelope {
            seq: expected
                .checked_add(1)
                .ok_or_else(|| Denial::new("revision_overflow", "Journal sequence exhausted"))?,
            session: self.session.clone(),
            at,
            actor: Actor::Runtime,
            policy: None,
            input: None,
            refs: crate::workspace_locks::attribution(&view, &change)?,
            payload: Event::LockChanged { version: 1, change },
        };
        crate::journal::validate_workspace_append(
            &inventory,
            &self.session,
            expected,
            std::slice::from_ref(&event),
            self.journal.schemas(),
        )?;
        if self.journal.append(&self.session, expected, &[event])? != expected + 1 {
            return Err(Denial::new(
                "journal_append",
                "Unexpected creation commit revision",
            ));
        }
        Ok(())
    }
    fn prepare(
        &self,
        path: &WorkspacePath,
        mode: LockMode,
        limit: usize,
        pending: &mut Option<LocalCreation>,
        at: u64,
    ) -> Result<Box<dyn crate::ports::execution::WorkspaceFile>> {
        let coordinator = self.provider.coordinate(&self.binding)?;
        if coordinator.binding() != &self.binding {
            return Err(Denial::new(
                "binding_conflict",
                "I/O coordinator differs from this handle",
            ));
        }
        if pending.is_some() {
            return Err(Denial::new(
                "file_creation_pending",
                "Resolve this handle's earlier creation before more I/O",
            ));
        }
        let access = self.operation(path, mode, limit)?;
        self.provider.validate_file_request(&access)?;
        if mode != LockMode::Write || access.target.missing.is_empty() {
            let file = self.provider.prepare_file(&access)?;
            let identity = file.identity();
            if !access.target.missing.is_empty()
                || access
                    .target
                    .existing
                    .last()
                    .is_none_or(|part| part.identity != identity)
            {
                return Err(Denial::new(
                    "file_identity",
                    "Prepared file differs from the checked physical target",
                ));
            }
            let view = self.journal.read(&self.session)?.view_with_schemas(
                &self.session,
                None,
                self.journal.schemas(),
            )?;
            let key = Digest::of_value(&identity)?;
            let held = view.path_locks()[&self.assignment].file_holds.get(&key);
            if held.is_none_or(|held| held.lock.mode != LockMode::Write && mode == LockMode::Write)
            {
                self.commit_creation(
                    crate::workspace_locks::LockChange::FileAccessPrepared {
                        assignment: self.assignment.clone(),
                        target: access.target.clone(),
                        mode,
                    },
                    at,
                )?;
            }
            return Ok(file);
        }
        use crate::workspace_locks::LockChange;
        let mut random = [0u8; 32];
        getrandom::fill(&mut random)
            .map_err(|_| Denial::new("creation_owner", "Cannot generate a creation owner"))?;
        let owner = Digest::of(random);
        *pending = Some(LocalCreation {
            owner: owner.clone(),
            path: path.clone(),
            outcome: CreationOutcome::NotAttempted,
        });
        self.commit_creation(
            LockChange::FileCreationStarted {
                assignment: self.assignment.clone(),
                owner: owner.clone(),
                target: access.target.clone(),
            },
            at,
        )?;
        pending.as_mut().expect("local creation").outcome = CreationOutcome::Uncertain;
        let file = self.provider.prepare_file(&access)?;
        let identity = file.identity();
        pending.as_mut().expect("local creation").outcome =
            CreationOutcome::Observed(identity.clone());
        self.commit_creation(
            LockChange::FileCreated {
                assignment: self.assignment.clone(),
                owner,
                identity,
            },
            at,
        )?;
        *pending = None;
        // The physical object is durably owned before the coordinator drops and
        // before any data write. The returned file retains that same descriptor.
        Ok(file)
    }
    pub fn read(&self, path: &WorkspacePath, limit: usize, at: u64) -> Result<Vec<u8>> {
        self.check_open()?;
        let mut pending = self.operation.lock().map_err(|_| {
            Denial::new(
                "access_uncertain",
                "An interrupted file operation requires investigation",
            )
        })?;
        let mut file = self.prepare(path, LockMode::Read, limit, &mut pending, at)?;
        let bytes = file.read()?;
        if bytes.len() > limit {
            return Err(Denial::new(
                "file_limit",
                "Provider returned more bytes than permitted",
            ));
        }
        Ok(bytes)
    }
    /// An I/O error may follow a partial write. It never releases the hold.
    pub fn write(&self, path: &WorkspacePath, bytes: &[u8], at: u64) -> Result<()> {
        self.check_open()?;
        let mut pending = self.operation.lock().map_err(|_| {
            Denial::new(
                "access_uncertain",
                "An interrupted file operation requires investigation",
            )
        })?;
        let mut file = self.prepare(path, LockMode::Write, bytes.len(), &mut pending, at)?;
        file.write(bytes)
    }
    /// Retry only publication of local preparation evidence, never file creation
    /// or data I/O. Lost local state cannot be reconstructed from a pathname.
    pub fn resolve_creation(&self, at: u64) -> Result<()> {
        let mut pending = self.operation.lock().map_err(|_| {
            Denial::new(
                "access_uncertain",
                "Cannot resolve interrupted file preparation",
            )
        })?;
        let local = pending.as_ref().ok_or_else(|| {
            Denial::new(
                "file_creation",
                "This handle has no local creation evidence",
            )
        })?;
        let coordinator = self.provider.coordinate(&self.binding)?;
        if coordinator.binding() != &self.binding {
            return Err(Denial::new(
                "binding_conflict",
                "I/O coordinator differs from this handle",
            ));
        }
        let view = self.journal.read(&self.session)?.view_with_schemas(
            &self.session,
            None,
            self.journal.schemas(),
        )?;
        let record = view
            .path_locks()
            .get(&self.assignment)
            .ok_or_else(|| Denial::new("locks_missing", "No creation owner"))?;
        if record.acquired.mediated_owner.as_ref() != Some(&self.owner) || record.released.is_some()
        {
            return Err(Denial::new(
                "creation_owner",
                "Local completion does not own this hold",
            ));
        }
        if record.creation.is_none() {
            let complete = match &local.outcome {
                CreationOutcome::NotAttempted => true,
                CreationOutcome::Observed(identity) => record.file_holds.values().any(|hold| {
                    hold.lock.path == local.path
                        && hold
                            .observation
                            .existing
                            .last()
                            .is_some_and(|part| &part.identity == identity)
                }),
                CreationOutcome::Uncertain => false,
            };
            if !complete {
                return Err(Denial::new(
                    "file_creation",
                    "No matching committed creation",
                ));
            }
            *pending = None;
            return Ok(());
        }
        if record.creation.as_ref().expect("pending creation").owner != local.owner {
            return Err(Denial::new(
                "creation_owner",
                "Another attempt owns the pending creation",
            ));
        }
        use crate::workspace_locks::LockChange;
        let change = match &local.outcome {
            CreationOutcome::NotAttempted => LockChange::FileCreationAborted {
                assignment: self.assignment.clone(),
                owner: local.owner.clone(),
            },
            CreationOutcome::Observed(identity) => LockChange::FileCreated {
                assignment: self.assignment.clone(),
                owner: local.owner.clone(),
                identity: identity.clone(),
            },
            CreationOutcome::Uncertain => {
                return Err(Denial::new(
                    "access_uncertain",
                    "Preparation may have created an unidentified object; retain the root barrier",
                ));
            }
        };
        self.commit_creation(change, at)?;
        *pending = None;
        Ok(())
    }
}
/// A sealed, uncommitted access plan. Recorded metadata cannot recreate this plan
/// or its one-time right to issue a live handle.
/// ```compile_fail
/// let plan: ymp_kernel::workspace_guard::PreparedMediation = serde_json::from_str("{}").unwrap();
/// ```
pub struct PreparedMediation {
    session: Id,
    issuer: Arc<()>,
    input: Digest,
    binding: WorkspaceBinding,
    provider: Arc<dyn WorkspaceProvider>,
    acquisition: crate::workspace_locks::LockAcquisition,
    consumed: bool,
}
impl PreparedMediation {
    /// Planned journal data carries no live file capability.
    pub fn acquisition(&self) -> &crate::workspace_locks::LockAcquisition {
        &self.acquisition
    }
    pub fn input(&self) -> &Digest {
        &self.input
    }
}
impl<J: Journal, C: ContentStore> WorkspaceGuard<J, C> {
    /// Creates path ownership for files-only Scripted mediation. Admission,
    /// invocation funding and execution lifecycle remain separate kernel work.
    pub fn mediate(
        &self,
        session: &Id,
        expected: u64,
        at: u64,
        request: LockRequest,
        provider: Arc<dyn WorkspaceProvider>,
    ) -> Result<MediatedAccess<J>> {
        let mut plan = self.prepare_mediation(session, expected, at, request, provider)?;
        self.commit(
            session,
            expected,
            at,
            Event::LockChanged {
                version: 1,
                change: crate::workspace_locks::LockChange::Acquired(Box::new(
                    plan.acquisition.clone(),
                )),
            },
        )?;
        self.complete_mediation(&mut plan)
    }
    /// Inspect and validate an access scope without appending ownership or exposing I/O.
    pub fn prepare_mediation(
        &self,
        session: &Id,
        expected: u64,
        at: u64,
        request: LockRequest,
        provider: Arc<dyn WorkspaceProvider>,
    ) -> Result<PreparedMediation> {
        let view = self.view(session)?;
        if view.revision() != expected {
            return Err(Denial::new(
                "stale_revision",
                "Workspace changed before mediated admission",
            ));
        }
        let binding = view
            .workspace_bindings()
            .get(&request.workspace)
            .ok_or_else(|| {
                Denial::new(
                    "workspace_binding",
                    "Bind the physical root before exposing file access",
                )
            })?
            .clone();
        provider.verify_binding(&binding, self.journal.as_ref())?;
        let registry = view
            .registry()
            .ok_or_else(|| Denial::new("registry_missing", "No profile observation"))?;
        let agent = registry
            .input
            .facts
            .agents
            .iter()
            .find(|a| a.id == request.profile.agent)
            .ok_or_else(|| Denial::new("agent_missing", "No recorded agent"))?;
        let discovery = registry
            .input
            .facts
            .discoveries
            .iter()
            .find(|d| d.provider.id == agent.provider)
            .ok_or_else(|| Denial::new("provider_missing", "No recorded provider"))?;
        use ymp_domain::{
            identity::{DiscoverySource, ProviderKind},
            journal::Capability,
        };
        if discovery.provider.kind != ProviderKind::Scripted
            || discovery.source != DiscoverySource::ScriptedFixture
            || discovery.provider.capabilities.as_ref().is_none_or(|caps| {
                caps.iter()
                    .any(|c| !matches!(c, Capability::ReadFiles | Capability::WriteFiles))
            })
        {
            return Err(Denial::new(
                "mediation_boundary",
                "Files-only mediation cannot certify native or process-capable execution",
            ));
        }
        let modes = provider.file_modes();
        if request.paths.iter().any(|(_, mode)| !modes.contains(mode)) {
            return Err(Denial::new(
                "file_operation",
                "Provider does not supply a requested file operation",
            ));
        }
        let mut random = [0u8; 32];
        getrandom::fill(&mut random)
            .map_err(|_| Denial::new("access_owner", "Cannot generate an access owner"))?;
        let owner = Digest::of(random);
        let workspace = view
            .workspaces()
            .get(&request.workspace)
            .ok_or_else(|| Denial::new("workspace_missing", "Workspace is not open"))?
            .reference()?;
        let proof = AccessEvidence {
            session: session.clone(),
            issuer: self.issuer.clone(),
            workspace: workspace.clone(),
            profile: request.profile.clone(),
            actual: request.paths.clone(),
            mediated_owner: Some(owner.clone()),
            basis: vec![workspace],
        };
        let acquisition = self.prepare_lock(session, request, provider.as_ref(), &proof)?;
        let change = crate::workspace_locks::LockChange::Acquired(Box::new(acquisition.clone()));
        let event = Envelope {
            seq: expected
                .checked_add(1)
                .ok_or_else(|| Denial::new("revision_overflow", "Journal sequence exhausted"))?,
            session: session.clone(),
            at,
            actor: Actor::Runtime,
            policy: None,
            input: None,
            refs: crate::workspace_locks::attribution(&view, &change)?,
            payload: Event::LockChanged { version: 1, change },
        };
        let inventory = self.journal.workspace_inventory(session)?;
        crate::journal::validate_workspace_append(
            &inventory,
            session,
            expected,
            &[event],
            self.journal.schemas(),
        )?;
        Ok(PreparedMediation {
            session: session.clone(),
            issuer: self.issuer.clone(),
            input: view.digest()?,
            binding,
            provider,
            acquisition,
            consumed: false,
        })
    }
    pub(crate) fn validate_prepared_mediation(&self, plan: &PreparedMediation) -> Result<()> {
        if plan.consumed || !Arc::ptr_eq(&self.issuer, &plan.issuer) {
            return Err(Denial::new(
                "access_plan",
                "Access plan is consumed or belongs to another authority",
            ));
        }
        Ok(())
    }
    /// Issue the one live handle after matching ownership has been committed.
    /// Validation failure leaves the plan available for a later resolution attempt.
    pub fn complete_mediation(&self, plan: &mut PreparedMediation) -> Result<MediatedAccess<J>> {
        if plan.consumed || !Arc::ptr_eq(&self.issuer, &plan.issuer) {
            return Err(Denial::new(
                "access_plan",
                "Access plan is consumed or belongs to another authority",
            ));
        }
        let view = self.view(&plan.session)?;
        let assignment = plan.acquisition.assignment.clone();
        let record = view.path_locks().get(&assignment).ok_or_else(|| {
            Denial::new(
                "locks_missing",
                "Prepared access has no committed ownership",
            )
        })?;
        if record.acquired != plan.acquisition
            || record.revoked
            || record.released.is_some()
            || view.workspace_bindings().get(&record.acquired.workspace) != Some(&plan.binding)
        {
            return Err(Denial::new(
                "access_plan",
                "Recorded ownership differs from the unconsumed plan",
            ));
        }
        validate_provider(
            &view.workspaces()[&record.acquired.workspace],
            plan.provider.as_ref(),
        )?;
        plan.provider
            .verify_binding(&plan.binding, self.journal.as_ref())?;
        let owner =
            plan.acquisition.mediated_owner.clone().ok_or_else(|| {
                Denial::new("access_owner", "Prepared access has no mediated owner")
            })?;
        let handle = MediatedAccess {
            journal: self.journal.clone(),
            provider: plan.provider.clone(),
            session: plan.session.clone(),
            assignment,
            owner,
            issuer: self.issuer.clone(),
            binding: plan.binding.clone(),
            acquisition: record.last.clone(),
            closed: std::sync::atomic::AtomicBool::new(false),
            operation: Mutex::new(None),
        };
        plan.consumed = true;
        Ok(handle)
    }
    /// Close new operations first, then wait for every admitted synchronous call.
    /// The returned evidence only covers this files-only capability.
    pub fn withdraw_mediated(&self, access: &MediatedAccess<J>) -> Result<CessationEvidence> {
        if !Arc::ptr_eq(&access.issuer, &self.issuer) {
            return Err(Denial::new(
                "cessation_evidence",
                "Handle belongs to another workspace authority",
            ));
        }
        access.request_withdrawal();
        let _drained = access.operation.lock().map_err(|_| {
            Denial::new(
                "access_uncertain",
                "Cannot certify an interrupted operation",
            )
        })?;
        let view = self.view(&access.session)?;
        let record = view
            .path_locks()
            .get(&access.assignment)
            .ok_or_else(|| Denial::new("locks_missing", "No recorded access"))?;
        if record.acquired.mediated_owner.as_ref() != Some(&access.owner)
            || record.released.is_some()
        {
            return Err(Denial::new(
                "cessation_evidence",
                "Handle does not own this active hold",
            ));
        }
        view.resolve(&access.acquisition)?;
        Ok(CessationEvidence {
            session: access.session.clone(),
            issuer: self.issuer.clone(),
            record: crate::workspace_locks::CessationRecord {
                assignment: access.assignment.clone(),
                workspace: record.acquired.workspace.clone(),
                invocation: record.invocation.clone(),
                state: record.last.clone(),
                kind: if record.invocation.is_some() {
                    crate::workspace_locks::Cessation::AccessWithdrawn
                } else {
                    crate::workspace_locks::Cessation::NeverAuthorized
                },
                basis: vec![access.acquisition.clone(), record.last.clone()],
            },
        })
    }
}
