//! Pure path ownership transitions and aggregate conflict checks across sessions.
use crate::{events::Event, view::SessionView};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use ymp_domain::{
    Denial, Digest, Id, Ref, Result,
    identity::{ExecutionProfile, Readiness},
    journal::{Capability, Envelope},
    require_text,
    workspace::*,
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LockAcquisition {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mediated_owner: Option<Digest>,
    pub workspace: Id<Workspace>,
    pub assignment: Id,
    pub profile: ExecutionProfile,
    pub requested: Vec<PathLock>,
    pub effective: Vec<ObservedPathLock>,
    pub basis: Vec<Ref>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Cessation {
    NeverAuthorized,
    Terminated,
    AccessWithdrawn,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CessationRecord {
    pub assignment: Id,
    pub workspace: Id<Workspace>,
    pub invocation: Option<Id>,
    pub state: Ref,
    pub kind: Cessation,
    pub basis: Vec<Ref>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaptureRead {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub protected_by: Option<Id>,
    pub snapshot: Id<Snapshot>,
    pub workspace: Id<Workspace>,
    pub observation: PathObservation,
    pub started: Ref,
    pub owner: Digest,
    pub started_at: u64,
    pub ended: Option<Ref>,
    pub failure: Option<String>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum LockChange {
    FileAccessPrepared {
        assignment: Id,
        target: PathObservation,
        mode: LockMode,
    },
    FileCreationStarted {
        assignment: Id,
        owner: Digest,
        target: PathObservation,
    },
    FileCreated {
        assignment: Id,
        owner: Digest,
        identity: FileIdentity,
    },
    FileCreationAborted {
        assignment: Id,
        owner: Digest,
    },
    CaptureStarted {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        protected_by: Option<Id>,
        owner: Digest,
        snapshot: Id<Snapshot>,
        workspace: Id<Workspace>,
        observation: PathObservation,
    },
    CaptureAborted {
        snapshot: Id<Snapshot>,
        started: Ref,
        reason: String,
    },
    Acquired(Box<LockAcquisition>),
    Authorized {
        assignment: Id,
        invocation: Id,
    },
    Revoked {
        assignment: Id,
        reason: String,
    },
    Released(CessationRecord),
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileCreation {
    pub owner: Digest,
    pub target: PathObservation,
    pub started: Ref,
}
pub const MAX_FILE_HOLDS: usize = 4096;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssignmentLocks {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub creation: Option<FileCreation>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub file_holds: BTreeMap<Digest, ObservedPathLock>,
    pub acquired: LockAcquisition,
    pub invocation: Option<Id>,
    pub revoked: bool,
    pub released: Option<CessationRecord>,
    pub last: Ref,
}
pub fn validate_profile(view: &SessionView, acquisition: &LockAcquisition) -> Result<()> {
    let task = view
        .task()
        .ok_or_else(|| Denial::new("task_missing", "Path access requires an explicit task"))?;
    let pool = view.registry().ok_or_else(|| {
        Denial::new(
            "registry_missing",
            "Path access requires a recorded profile",
        )
    })?;
    if pool.input.constraints != task.constraints
        || !pool
            .decisions
            .iter()
            .any(|d| d.profile == acquisition.profile && d.outcome == Readiness::Ready)
    {
        return Err(Denial::new(
            "profile_excluded",
            "The access profile is not currently eligible",
        ));
    }
    let agent = pool
        .input
        .facts
        .agents
        .iter()
        .find(|a| a.id == acquisition.profile.agent)
        .ok_or_else(|| Denial::new("agent_missing", "No recorded agent"))?;
    let capabilities = pool
        .input
        .facts
        .discoveries
        .iter()
        .find(|d| d.provider.id == agent.provider)
        .and_then(|d| d.provider.capabilities.as_ref())
        .ok_or_else(|| Denial::new("capabilities_unknown", "Backend capabilities are unknown"))?;
    for access in acquisition
        .effective
        .iter()
        .map(|a| &a.lock)
        .chain(acquisition.requested.iter())
    {
        let capability = match access.mode {
            LockMode::Read => Capability::ReadFiles,
            LockMode::Write => Capability::WriteFiles,
        };
        if !task.constraints.allowed.contains(&capability) || !capabilities.contains(&capability) {
            return Err(Denial::new(
                "capability_denied",
                "Effective workspace access exceeds the task or backend capabilities",
            ));
        }
    }
    Ok(())
}
fn basis(view: &SessionView, refs: &[Ref]) -> Result<()> {
    if refs.is_empty() || refs.len() > 32 {
        return Err(Denial::new(
            "workspace_basis",
            "An attributable basis is required",
        ));
    }
    for reference in refs {
        view.resolve(reference)?;
    }
    Ok(())
}
pub fn validate_cessation(view: &SessionView, record: &CessationRecord) -> Result<()> {
    let prior = view
        .path_locks()
        .get(&record.assignment)
        .ok_or_else(|| Denial::new("locks_missing", "Assignment holds no recorded access"))?;
    basis(view, &record.basis)?;
    if prior.released.is_some()
        || record.workspace != prior.acquired.workspace
        || record.invocation != prior.invocation
        || record.state != prior.last
        || (record.kind == Cessation::NeverAuthorized && prior.invocation.is_some())
        || (record.kind != Cessation::NeverAuthorized && prior.invocation.is_none())
    {
        return Err(Denial::new(
            "cessation_evidence",
            "Evidence does not bind the current access and authorization",
        ));
    }
    Ok(())
}
pub fn apply(
    view: &SessionView,
    change: &LockChange,
    reference: Ref,
) -> Result<BTreeMap<Id, AssignmentLocks>> {
    let mut locks = view.path_locks().clone();
    match change {
        LockChange::CaptureStarted { .. } | LockChange::CaptureAborted { .. } => {
            return Err(Denial::new(
                "capture_change",
                "Not an assignment ownership transition",
            ));
        }
        LockChange::Acquired(acquisition) => {
            let workspace = view
                .workspaces()
                .get(&acquisition.workspace)
                .ok_or_else(|| Denial::new("workspace_missing", "Workspace is not open"))?;
            validate_profile(view, acquisition)?;
            basis(view, &acquisition.basis)?;
            if view
                .admission()
                .assignments()
                .values()
                .any(|record| record.intent.assignment.id.erased() == acquisition.assignment)
                || locks.contains_key(&acquisition.assignment)
                || acquisition.requested.is_empty()
                || acquisition.effective.is_empty()
                || acquisition.requested.len() > 256
                || acquisition.effective.len() > 256
            {
                return Err(Denial::new(
                    "path_locks",
                    "Assignment identity or path count is invalid",
                ));
            }
            for actual in &acquisition.effective {
                actual.observation.validate(&workspace.location)?;
                if actual.lock.holder != acquisition.assignment
                    || actual.lock.path != actual.observation.path
                {
                    return Err(Denial::new(
                        "path_locks",
                        "Observed access differs from its owner or path",
                    ));
                }
            }
            for requested in &acquisition.requested {
                if requested.holder != acquisition.assignment
                    || !acquisition.effective.iter().any(|actual| {
                        actual.lock.path.contains(&requested.path)
                            && (actual.lock.mode == LockMode::Write
                                || requested.mode == LockMode::Read)
                    })
                {
                    return Err(Denial::new(
                        "access_scope",
                        "Requested paths exceed the observed effective access",
                    ));
                }
            }
            locks.insert(
                acquisition.assignment.clone(),
                AssignmentLocks {
                    creation: None,
                    file_holds: BTreeMap::new(),
                    acquired: (**acquisition).clone(),
                    invocation: None,
                    revoked: false,
                    released: None,
                    last: reference,
                },
            );
        }
        LockChange::Authorized {
            assignment,
            invocation,
        } => {
            if view.capture_reads().values().any(|capture| {
                capture.protected_by.as_ref() == Some(assignment) && capture.ended.is_none()
            }) {
                return Err(Denial::new(
                    "capture_active",
                    "Production cannot start while its protected baseline capture is active",
                ));
            }
            if locks
                .values()
                .any(|lock| lock.invocation.as_ref() == Some(invocation))
            {
                return Err(Denial::new(
                    "invocation_authority",
                    "Workspace authorization cannot reuse an invocation",
                ));
            }
            let prior = locks
                .get_mut(assignment)
                .ok_or_else(|| Denial::new("locks_missing", "No path ownership for assignment"))?;
            if prior.invocation.is_some() || prior.revoked || prior.released.is_some() {
                return Err(Denial::new(
                    "invocation_authority",
                    "Workspace authorization is single-use",
                ));
            }
            validate_profile(view, &prior.acquired)?;
            prior.invocation = Some(invocation.clone());
            prior.last = reference;
        }
        LockChange::FileAccessPrepared {
            assignment,
            target,
            mode,
        } => {
            let prior = locks
                .get_mut(assignment)
                .ok_or_else(|| Denial::new("locks_missing", "No physical file owner"))?;
            target.validate(&view.workspaces()[&prior.acquired.workspace].location)?;
            validate_profile(view, &prior.acquired)?;
            if prior.acquired.mediated_owner.is_none()
                || prior.invocation.is_none()
                || prior.revoked
                || prior.released.is_some()
                || prior.creation.is_some()
                || !target.missing.is_empty()
                || !prior
                    .acquired
                    .requested
                    .iter()
                    .any(|lock| lock.mode == *mode && lock.path.contains(&target.path))
                || ymp_domain::journal::encode(target)?.len() > 48 * 1024
            {
                return Err(Denial::new(
                    "file_access",
                    "Prepared file lacks active bounded ownership",
                ));
            }
            let key = Digest::of_value(
                &target
                    .existing
                    .last()
                    .expect("validated observation")
                    .identity,
            )?;
            if !prior.file_holds.contains_key(&key) && prior.file_holds.len() >= MAX_FILE_HOLDS {
                return Err(Denial::new(
                    "file_limit",
                    "Physical file ownership capacity exhausted",
                ));
            }
            let held_mode = if prior
                .file_holds
                .get(&key)
                .is_some_and(|held| held.lock.mode == LockMode::Write)
            {
                LockMode::Write
            } else {
                *mode
            };
            prior.file_holds.insert(
                key,
                ObservedPathLock {
                    lock: PathLock {
                        path: target.path.clone(),
                        mode: held_mode,
                        holder: assignment.clone(),
                    },
                    observation: target.clone(),
                },
            );
            prior.last = reference;
        }
        LockChange::FileCreationStarted {
            assignment,
            owner,
            target,
        } => {
            let prior = locks
                .get_mut(assignment)
                .ok_or_else(|| Denial::new("locks_missing", "No path ownership for creation"))?;
            let workspace = &view.workspaces()[&prior.acquired.workspace];
            target.validate(&workspace.location)?;
            validate_profile(view, &prior.acquired)?;
            if prior.acquired.mediated_owner.is_none()
                || prior.invocation.is_none()
                || prior.revoked
                || prior.released.is_some()
                || prior.creation.is_some()
                || target.missing.len() != 1
                || prior
                    .file_holds
                    .values()
                    .any(|hold| hold.lock.path == target.path)
                || prior.file_holds.len() >= MAX_FILE_HOLDS
                || ymp_domain::journal::encode(target)?.len() > 48 * 1024
                || !prior
                    .acquired
                    .requested
                    .iter()
                    .any(|lock| lock.mode == LockMode::Write && lock.path.contains(&target.path))
            {
                return Err(Denial::new(
                    "file_creation",
                    "Creation requires active bounded mediated write ownership",
                ));
            }
            prior.creation = Some(FileCreation {
                owner: owner.clone(),
                target: target.clone(),
                started: reference.clone(),
            });
            prior.last = reference;
        }
        LockChange::FileCreated {
            assignment,
            owner,
            identity,
        } => {
            let prior = locks
                .get_mut(assignment)
                .ok_or_else(|| Denial::new("locks_missing", "No creation owner"))?;
            let creation = prior
                .creation
                .as_ref()
                .ok_or_else(|| Denial::new("file_creation", "No pending creation"))?;
            if &creation.owner != owner
                || prior.released.is_some()
                || identity.inode == 0
                || identity.device != view.workspaces()[&prior.acquired.workspace].location.device
            {
                return Err(Denial::new(
                    "file_creation",
                    "Creation completion differs from its physical owner",
                ));
            }
            let mut observed = creation.target.clone();
            observed.existing.push(PathComponent {
                name: observed.missing.remove(0),
                identity: identity.clone(),
            });
            observed.validate(&view.workspaces()[&prior.acquired.workspace].location)?;
            let key = Digest::of_value(&identity)?;
            prior.file_holds.insert(
                key,
                ObservedPathLock {
                    lock: PathLock {
                        path: observed.path.clone(),
                        mode: LockMode::Write,
                        holder: assignment.clone(),
                    },
                    observation: observed,
                },
            );
            prior.creation = None;
            prior.last = reference;
        }
        LockChange::FileCreationAborted { assignment, owner } => {
            let prior = locks
                .get_mut(assignment)
                .ok_or_else(|| Denial::new("locks_missing", "No creation owner"))?;
            if prior.released.is_some()
                || prior
                    .creation
                    .as_ref()
                    .is_none_or(|creation| &creation.owner != owner)
            {
                return Err(Denial::new(
                    "file_creation",
                    "No matching unattempted creation",
                ));
            }
            prior.creation = None;
            prior.last = reference;
        }
        LockChange::Revoked { assignment, reason } => {
            require_text(reason, 4096)?;
            let prior = locks
                .get_mut(assignment)
                .ok_or_else(|| Denial::new("locks_missing", "No path ownership for assignment"))?;
            if prior.revoked || prior.released.is_some() {
                return Err(Denial::new(
                    "workspace_revocation",
                    "Access is no longer active",
                ));
            }
            prior.revoked = true;
            prior.last = reference;
        }
        LockChange::Released(record) => {
            validate_cessation(view, record)?;
            let prior = locks
                .get_mut(&record.assignment)
                .expect("validated assignment");
            prior.released = Some(record.clone());
            prior.last = reference;
        }
    }
    Ok(locks)
}
/// Common kernel validation invoked under each adapter's aggregate commit lock.
/// Revocation does not remove a hold. Release has separate evidence requirements.
#[derive(Clone, Debug, Serialize)]
pub struct WorkspaceOwnership {
    session: Id,
    locks: Vec<(Id, Ref, Vec<ObservedPathLock>)>,
    pending: Vec<(WorkspaceLocation, Ref)>,
}
impl WorkspaceOwnership {
    pub fn from_view(view: &SessionView) -> Self {
        let mut locks: Vec<_> = view
            .path_locks()
            .values()
            .filter(|l| l.released.is_none())
            .map(|l| {
                (
                    l.acquired.assignment.clone(),
                    l.last.clone(),
                    l.acquired
                        .effective
                        .iter()
                        .cloned()
                        .chain(l.file_holds.values().cloned())
                        .collect::<Vec<_>>(),
                )
            })
            .collect();
        for capture in view.capture_reads().values().filter(|c| c.ended.is_none()) {
            let holder = capture
                .protected_by
                .clone()
                .unwrap_or_else(|| capture.snapshot.erased());
            let path = ObservedPathLock {
                lock: PathLock {
                    path: WorkspacePath::root(),
                    mode: LockMode::Read,
                    holder: holder.clone(),
                },
                observation: capture.observation.clone(),
            };
            if capture.protected_by.is_some()
                && let Some((_, _, paths)) = locks.iter_mut().find(|(id, _, _)| id == &holder)
            {
                paths.push(path);
            } else {
                locks.push((holder, capture.started.clone(), vec![path]));
            }
        }
        Self {
            session: view.session().clone(),
            pending: view
                .path_locks()
                .values()
                .filter(|lock| lock.released.is_none())
                .filter_map(|lock| {
                    lock.creation.as_ref().map(|creation| {
                        (
                            view.workspaces()[&lock.acquired.workspace].location.clone(),
                            creation.started.clone(),
                        )
                    })
                })
                .collect(),
            locks,
        }
    }
    /// Check a freshly observed operation target against retained physical owners.
    pub fn validate_operation(
        &self,
        session: &Id,
        assignment: &Id,
        target: &ObservedPathLock,
    ) -> Result<()> {
        for (root, reference) in &self.pending {
            if target.observation.existing.iter().any(|part| {
                part.identity.device == root.device && part.identity.inode == root.inode
            }) {
                return Err(Denial::new(
                    "file_creation_pending",
                    "Unresolved file creation prevents new operations in this root",
                )
                .with_ref(reference.clone()));
            }
        }
        for (holder, reference, paths) in &self.locks {
            if &self.session == session && holder == assignment {
                continue;
            }
            if paths.iter().any(|other| {
                (target.lock.mode == LockMode::Write || other.lock.mode == LockMode::Write)
                    && target.observation.overlaps(&other.observation)
            }) {
                return Err(Denial::new(
                    "path_conflict",
                    format!(
                        "Operation conflicts with {holder} in session {}",
                        self.session
                    ),
                )
                .with_ref(reference.clone()));
            }
        }
        Ok(())
    }
    pub fn retained_weight(&self) -> Result<(usize, usize)> {
        Ok((
            ymp_domain::journal::encode(self)?.len() + self.pending.len() * 64 * 1024,
            self.locks.len(),
        ))
    }
    pub fn is_empty(&self) -> bool {
        self.locks.is_empty()
    }
    pub fn session(&self) -> &Id {
        &self.session
    }
}
pub fn validate_inventory_size(bytes: usize, assignments: usize) -> Result<()> {
    if bytes > 16 * 1024 * 1024 || assignments > 4096 {
        return Err(Denial::new(
            "workspace_inventory",
            "Active ownership exceeds supported projection bounds",
        ));
    }
    Ok(())
}
pub fn validate_ownership<'a>(
    ownership: impl IntoIterator<Item = &'a WorkspaceOwnership>,
) -> Result<()> {
    let mut active: Vec<(&Id, &Id, &Ref, &Vec<ObservedPathLock>)> = vec![];
    let mut bytes = 0usize;
    let mut sessions = std::collections::BTreeSet::new();
    let mut pending_roots = std::collections::BTreeSet::new();
    for owner in ownership {
        if !sessions.insert(owner.session.clone()) {
            return Err(Denial::new(
                "workspace_inventory",
                "Duplicate session ownership",
            ));
        }
        for (root, reference) in &owner.pending {
            if !pending_roots.insert((root.device, root.inode)) {
                return Err(Denial::new(
                    "file_creation_pending",
                    "Only one creation can await publication per root",
                )
                .with_ref(reference.clone()));
            }
        }
        bytes = bytes
            .checked_add(owner.retained_weight()?.0)
            .ok_or_else(|| Denial::new("workspace_inventory", "Ownership size overflow"))?;
        if bytes > 16 * 1024 * 1024 {
            return Err(Denial::new(
                "workspace_inventory",
                "Active ownership exceeds the bounded projection size",
            ));
        }
        for (assignment, reference, paths) in &owner.locks {
            if active.len() >= 4096 {
                return Err(Denial::new(
                    "workspace_inventory",
                    "Too many active assignments",
                ));
            }
            for (other_session, other_assignment, other_ref, other_paths) in &active {
                if paths.iter().any(|left| {
                    other_paths.iter().any(|right| {
                        (left.lock.mode == LockMode::Write || right.lock.mode == LockMode::Write)
                            && left.observation.overlaps(&right.observation)
                    })
                }) {
                    return Err(Denial::new("path_conflict", format!("Assignment {assignment} conflicts with {other_assignment} in session {other_session}")).with_ref((*other_ref).clone()).with_ref(reference.clone()));
                }
            }
            active.push((&owner.session, assignment, reference, paths));
        }
    }
    Ok(())
}
pub fn validate_conflicts<'a>(views: impl IntoIterator<Item = &'a SessionView>) -> Result<()> {
    let owners: Vec<_> = views
        .into_iter()
        .map(WorkspaceOwnership::from_view)
        .collect();
    validate_ownership(&owners)
}
/// Space for authorization, optional revocation and release. These control
/// events carry at most 32 basis references and fit within 64 KiB each.
pub fn control_reserve(view: &SessionView) -> (usize, usize) {
    let events = view
        .path_locks()
        .values()
        .filter(|l| l.released.is_none())
        .map(|lock| {
            if lock.revoked {
                1
            } else if lock.invocation.is_some() {
                2
            } else {
                3
            }
        })
        .sum::<usize>();
    let events = events
        + view
            .capture_reads()
            .values()
            .filter(|c| c.ended.is_none())
            .count();
    let events = events
        + view
            .path_locks()
            .values()
            .filter(|lock| lock.released.is_none() && lock.creation.is_some())
            .count();
    (events, events * 64 * 1024)
}
pub fn attribution(view: &SessionView, change: &LockChange) -> Result<Vec<Ref>> {
    let mut refs = match change {
        LockChange::CaptureStarted {
            workspace,
            protected_by,
            ..
        } => {
            let mut refs = vec![
                view.workspaces()
                    .get(workspace)
                    .ok_or_else(|| Denial::new("workspace_missing", "Workspace is not open"))?
                    .reference()?,
            ];
            if let Some(assignment) = protected_by {
                refs.push(
                    view.path_locks()
                        .get(assignment)
                        .ok_or_else(|| Denial::new("locks_missing", "No baseline protection"))?
                        .last
                        .clone(),
                );
            }
            refs
        }
        LockChange::CaptureAborted { started, .. } => vec![started.clone()],

        LockChange::Acquired(acquisition) => {
            let mut refs = acquisition.basis.clone();
            refs.push(
                view.workspaces()
                    .get(&acquisition.workspace)
                    .ok_or_else(|| Denial::new("workspace_missing", "Workspace is not open"))?
                    .reference()?,
            );
            refs
        }
        LockChange::Released(record) => {
            let mut refs = record.basis.clone();
            refs.push(record.state.clone());
            refs
        }
        LockChange::Authorized { assignment, .. }
        | LockChange::Revoked { assignment, .. }
        | LockChange::FileAccessPrepared { assignment, .. }
        | LockChange::FileCreationStarted { assignment, .. }
        | LockChange::FileCreated { assignment, .. }
        | LockChange::FileCreationAborted { assignment, .. } => vec![
            view.path_locks()
                .get(assignment)
                .ok_or_else(|| Denial::new("locks_missing", "No path ownership for assignment"))?
                .last
                .clone(),
        ],
    };
    refs.sort();
    refs.dedup();
    Ok(refs)
}
pub fn touches_ownership(events: &[Envelope<Event>]) -> bool {
    events.iter().any(|e| {
        matches!(
            e.payload,
            Event::LockChanged { .. } | Event::SnapshotTaken { version: 2, .. }
        )
    })
}

pub fn apply_capture(
    view: &SessionView,
    change: &LockChange,
    reference: Ref,
    at: u64,
) -> Result<CaptureRead> {
    match change {
        LockChange::CaptureStarted {
            protected_by,
            owner,
            snapshot,
            workspace,
            observation,
        } => {
            let recorded = view
                .workspaces()
                .get(workspace)
                .ok_or_else(|| Denial::new("workspace_missing", "Workspace is not open"))?;
            observation.validate(&recorded.location)?;
            if let Some(assignment) = protected_by {
                let lock = view
                    .path_locks()
                    .get(assignment)
                    .ok_or_else(|| Denial::new("locks_missing", "No baseline protection"))?;
                if lock.acquired.workspace != *workspace
                    || lock.acquired.mediated_owner.is_none()
                    || lock.invocation.is_some()
                    || lock.revoked
                    || lock.released.is_some()
                    || !lock
                        .acquired
                        .requested
                        .iter()
                        .any(|p| p.mode == LockMode::Write)
                {
                    return Err(Denial::new(
                        "capture_protection",
                        "Baseline capture requires an unstarted mediated writer in this workspace",
                    ));
                }
            }
            if observation.path != WorkspacePath::root()
                || view.capture_reads().contains_key(snapshot)
                || view.snapshots().contains_key(snapshot)
            {
                return Err(Denial::new(
                    "capture_identity",
                    "Capture must hold the root under a fresh snapshot identity",
                ));
            }
            Ok(CaptureRead {
                protected_by: protected_by.clone(),
                snapshot: snapshot.clone(),
                workspace: workspace.clone(),
                observation: observation.clone(),
                started: reference,
                owner: owner.clone(),
                started_at: at,
                ended: None,
                failure: None,
            })
        }
        LockChange::CaptureAborted {
            snapshot,
            started,
            reason,
        } => {
            require_text(reason, 4096)?;
            let mut capture = view
                .capture_reads()
                .get(snapshot)
                .cloned()
                .ok_or_else(|| Denial::new("capture_missing", "No recorded capture"))?;
            if capture.ended.is_some() || capture.started != *started || at < capture.started_at {
                return Err(Denial::new(
                    "capture_state",
                    "Capture is ended or belongs to another start",
                ));
            }
            capture.ended = Some(reference);
            capture.failure = Some(reason.clone());
            Ok(capture)
        }
        _ => Err(Denial::new(
            "capture_change",
            "Not a capture ownership transition",
        )),
    }
}
