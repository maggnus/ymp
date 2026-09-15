//! Pure path ownership transitions and aggregate conflict checks across sessions.
use crate::{events::Event, view::SessionView};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use ymp_domain::{
    Denial, Id, Ref, Result,
    identity::{ExecutionProfile, Readiness},
    journal::{Capability, Envelope},
    require_text,
    workspace::*,
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LockAcquisition {
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
pub enum LockChange {
    Acquired(Box<LockAcquisition>),
    Authorized { assignment: Id, invocation: Id },
    Revoked { assignment: Id, reason: String },
    Released(CessationRecord),
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssignmentLocks {
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
        LockChange::Acquired(acquisition) => {
            let workspace = view
                .workspaces()
                .get(&acquisition.workspace)
                .ok_or_else(|| Denial::new("workspace_missing", "Workspace is not open"))?;
            validate_profile(view, acquisition)?;
            basis(view, &acquisition.basis)?;
            if locks.contains_key(&acquisition.assignment)
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
}
impl WorkspaceOwnership {
    pub fn from_view(view: &SessionView) -> Self {
        Self {
            session: view.session().clone(),
            locks: view
                .path_locks()
                .values()
                .filter(|l| l.released.is_none())
                .map(|l| {
                    (
                        l.acquired.assignment.clone(),
                        l.last.clone(),
                        l.acquired.effective.clone(),
                    )
                })
                .collect(),
        }
    }
    pub fn retained_weight(&self) -> Result<(usize, usize)> {
        Ok((ymp_domain::journal::encode(self)?.len(), self.locks.len()))
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
    for owner in ownership {
        if !sessions.insert(owner.session.clone()) {
            return Err(Denial::new(
                "workspace_inventory",
                "Duplicate session ownership",
            ));
        }
        bytes = bytes
            .checked_add(ymp_domain::journal::encode(owner)?.len())
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
    (events, events * 64 * 1024)
}
pub fn attribution(view: &SessionView, change: &LockChange) -> Result<Vec<Ref>> {
    let mut refs = match change {
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
        LockChange::Authorized { assignment, .. } | LockChange::Revoked { assignment, .. } => vec![
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
    events
        .iter()
        .any(|e| matches!(e.payload, Event::LockChanged { .. }))
}
