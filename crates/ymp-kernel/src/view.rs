//! Deterministic, read-only projections. No clock, provider or strategy is used by replay.

use crate::{events::Event, journal::ParameterSchemas};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use ymp_domain::{
    Denial, Digest, Id, Ref, Result,
    journal::{Actor, Decision, Envelope, Method, PolicySelection},
    task::{AcceptanceContract, Criterion, SessionStatus, Task},
};

/// The implemented journal portion of a session, not a claim that a runnable Session exists.
/// A strategy cannot change the projection or obtain a journal writer from it.
///
/// ```compile_fail
/// use ymp_kernel::view::SessionView;
/// fn change_state(view: &SessionView) {
///     view.revision = 20;
/// }
/// ```
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct SessionView {
    session: Id,
    revision: u64,
    latest_at: u64,
    opened: bool,
    policies: BTreeMap<String, PolicySelection>,
    method: Option<Method>,
    decisions: Vec<Decision<Method>>,
    references: BTreeSet<Ref>,
    task: Option<Task>,
    contract: Option<AcceptanceContract>,
    criteria: Vec<Criterion>,
    checks: BTreeMap<Id<ymp_domain::verification::Check>, ymp_domain::verification::Check>,
    check_runs:
        BTreeMap<Id<ymp_domain::verification::CheckRun>, ymp_domain::verification::CheckRun>,
    registry: Option<Box<crate::registry::PoolRecorded>>,
    treasury: Option<Box<crate::treasury::TreasuryView>>,
    coordination: crate::arbiter::CoordinationView,
    admission: crate::gatekeeper::AdmissionView,
    execution: crate::execution::ExecutionView,
    results: crate::results::ResultsView,
    workspaces: BTreeMap<Id<ymp_domain::workspace::Workspace>, ymp_domain::workspace::Workspace>,
    snapshots: BTreeMap<Id<ymp_domain::workspace::Snapshot>, ymp_domain::workspace::Snapshot>,
    path_locks: BTreeMap<Id, crate::workspace_locks::AssignmentLocks>,
    workspace_bindings:
        BTreeMap<Id<ymp_domain::workspace::Workspace>, ymp_domain::workspace::WorkspaceBinding>,
    capture_reads:
        BTreeMap<Id<ymp_domain::workspace::Snapshot>, crate::workspace_locks::CaptureRead>,
}

impl SessionView {
    pub fn empty(session: Id) -> Self {
        Self {
            session,
            revision: 0,
            latest_at: 0,
            opened: false,
            policies: BTreeMap::new(),
            method: None,
            decisions: Vec::new(),
            references: BTreeSet::new(),
            task: None,
            contract: None,
            criteria: Vec::new(),
            checks: BTreeMap::new(),
            check_runs: BTreeMap::new(),
            registry: None,
            treasury: None,
            coordination: crate::arbiter::CoordinationView::default(),
            admission: crate::gatekeeper::AdmissionView::default(),
            execution: crate::execution::ExecutionView::default(),
            results: crate::results::ResultsView::default(),
            workspaces: BTreeMap::new(),
            snapshots: BTreeMap::new(),
            path_locks: BTreeMap::new(),
            capture_reads: BTreeMap::new(),
            workspace_bindings: BTreeMap::new(),
        }
    }
    pub fn workspace_bindings(
        &self,
    ) -> &BTreeMap<Id<ymp_domain::workspace::Workspace>, ymp_domain::workspace::WorkspaceBinding>
    {
        &self.workspace_bindings
    }
    pub fn capture_reads(
        &self,
    ) -> &BTreeMap<Id<ymp_domain::workspace::Snapshot>, crate::workspace_locks::CaptureRead> {
        &self.capture_reads
    }
    pub fn path_locks(&self) -> &BTreeMap<Id, crate::workspace_locks::AssignmentLocks> {
        &self.path_locks
    }
    pub fn workspaces(
        &self,
    ) -> &BTreeMap<Id<ymp_domain::workspace::Workspace>, ymp_domain::workspace::Workspace> {
        &self.workspaces
    }
    pub fn snapshots(
        &self,
    ) -> &BTreeMap<Id<ymp_domain::workspace::Snapshot>, ymp_domain::workspace::Snapshot> {
        &self.snapshots
    }
    pub fn admission(&self) -> &crate::gatekeeper::AdmissionView {
        &self.admission
    }
    pub fn execution(&self) -> &crate::execution::ExecutionView {
        &self.execution
    }
    pub fn results(&self) -> &crate::results::ResultsView {
        &self.results
    }
    pub fn coordination(&self) -> &crate::arbiter::CoordinationView {
        &self.coordination
    }
    pub fn treasury(&self) -> Option<&crate::treasury::TreasuryView> {
        self.treasury.as_deref()
    }
    pub fn registry(&self) -> Option<&crate::registry::PoolRecorded> {
        self.registry.as_deref()
    }
    pub fn task(&self) -> Option<&Task> {
        self.task.as_ref()
    }
    pub fn contract(&self) -> Option<&AcceptanceContract> {
        self.contract.as_ref()
    }
    pub fn criteria(&self) -> &[Criterion] {
        &self.criteria
    }
    pub fn checks(
        &self,
    ) -> &BTreeMap<Id<ymp_domain::verification::Check>, ymp_domain::verification::Check> {
        &self.checks
    }
    pub fn check_runs(
        &self,
    ) -> &BTreeMap<Id<ymp_domain::verification::CheckRun>, ymp_domain::verification::CheckRun> {
        &self.check_runs
    }
    pub fn status(&self) -> Option<SessionStatus> {
        self.task.as_ref().map(|_| SessionStatus::Intake)
    }
    pub fn session(&self) -> &Id {
        &self.session
    }
    /// Latest timestamp already represented in this journal projection.
    pub fn latest_at(&self) -> u64 {
        self.latest_at
    }
    pub fn revision(&self) -> u64 {
        self.revision
    }
    pub fn opened(&self) -> bool {
        self.opened
    }
    pub fn policies(&self) -> &BTreeMap<String, PolicySelection> {
        &self.policies
    }
    pub fn method(&self) -> Option<&Method> {
        self.method.as_ref()
    }
    pub fn decisions(&self) -> &[Decision<Method>] {
        &self.decisions
    }
    pub fn digest(&self) -> Result<Digest> {
        // Stable v1 commitment to the complete input journal prefix. Adding a
        // derived view field must not change earlier decision input digests.
        Digest::of_value(&(
            "SessionView",
            1_u32,
            &self.session,
            self.revision,
            &self.references,
        ))
    }
    pub fn resolve(&self, reference: &Ref) -> Result<()> {
        if self.references.contains(reference) {
            Ok(())
        } else {
            Err(Denial::new(
                "missing_ref",
                "The versioned reference is absent from this session view",
            )
            .with_ref(reference.clone()))
        }
    }

    pub fn replay(session: Id, events: &[Envelope<Event>]) -> Result<Self> {
        Self::replay_with_schemas(session, events, &ParameterSchemas::default())
    }

    pub fn replay_with_schemas(
        session: Id,
        events: &[Envelope<Event>],
        schemas: &ParameterSchemas,
    ) -> Result<Self> {
        let mut view = Self::empty(session);
        for event in events {
            view.apply(event, schemas)?;
        }
        view.validate_complete()?;
        Ok(view)
    }

    pub(crate) fn apply(
        &mut self,
        event: &Envelope<Event>,
        schemas: &ParameterSchemas,
    ) -> Result<()> {
        if event.payload.version() != 1
            && !matches!(event.payload, Event::SnapshotTaken { version: 2, .. })
        {
            return Err(Denial::new(
                "event_version",
                "Unsupported event payload version",
            ));
        }
        if event.session != self.session
            || event.seq
                != self
                    .revision
                    .checked_add(1)
                    .ok_or_else(|| Denial::new("revision_overflow", "Journal sequence exhausted"))?
        {
            return Err(Denial::new(
                "event_order",
                "Event belongs to another session or is not the next sequence",
            ));
        }
        if event.actor != Actor::Runtime {
            return Err(Denial::new(
                "actor",
                "Session opening and method decisions must be committed by the runtime",
            ));
        }
        for reference in &event.refs {
            self.resolve(reference)?;
        }
        self.coordination.check_next(&event.payload)?;
        let execution = crate::execution::apply(self, event)?;
        let admission = crate::gatekeeper::apply(self, event)?;
        let results = crate::results::apply(self, event)?;
        match &event.payload {
            Event::PlanCommitted { .. }
            | Event::AttemptStarted { .. }
            | Event::ResultSubmitted { .. }
            | Event::AttemptAbandoned { .. } => {
                self.validate_base_complete()?;
                if event.policy.is_some()
                    || event.input.is_some()
                    || event.refs != crate::results::attribution(self, &event.payload)?
                {
                    return Err(Denial::new(
                        "result_attribution",
                        "Result event differs from its authoritative basis",
                    ));
                }
                match &event.payload {
                    Event::PlanCommitted { data, .. } => {
                        self.references.insert(data.plan.reference()?);
                        self.references.insert(data.item.reference()?);
                    }
                    Event::ResultSubmitted { result, .. } => {
                        self.references.insert(result.reference()?);
                    }
                    _ => {}
                }
            }
            Event::InvocationStarted { .. }
            | Event::InvocationObserved { .. }
            | Event::InvocationEnded { .. } => {
                self.validate_base_complete()?;
                if crate::execution::attribution(self, &event.payload)?
                    != (
                        event.policy.clone(),
                        event.input.clone(),
                        event.refs.clone(),
                    )
                {
                    return Err(Denial::new(
                        "invocation_attribution",
                        "Invocation observation differs from its recorded basis",
                    ));
                }
            }
            Event::CheckRegistered { data, .. } => {
                self.validate_complete()?;
                schemas.validate(&data.effective)?;
                crate::acceptance::validate_registered(self, event, data)?;
                let check = &data.proposal.value;
                self.references.insert(check.reference());
                self.references.insert(data.contract.reference());
                self.checks.insert(check.id.clone(), check.clone());
                self.contract = Some(data.contract.clone());
            }
            Event::CheckRunRecorded { data, .. } => {
                self.validate_complete()?;
                schemas.validate(&data.environment.runner)?;
                crate::acceptance::validate_run(self, event, data)?;
                self.references.insert(data.run.reference()?);
                self.check_runs
                    .insert(data.run.id.clone(), data.run.clone());
            }
            Event::AssignmentRevoked { assignment, .. } => {
                self.validate_base_complete()?;
                if event.policy.is_some()
                    || event.input.is_some()
                    || event.refs != crate::gatekeeper::revocation_refs(self, assignment)?
                {
                    return Err(Denial::new(
                        "revocation_attribution",
                        "Revocation basis differs from recorded authority",
                    ));
                }
            }
            Event::GrantIssued { .. } | Event::AssignmentAdmitted { .. } => {
                self.validate_base_complete()?;
                if event.policy.is_some()
                    || event.input.is_some()
                    || event.refs != crate::gatekeeper::attribution(self)?
                {
                    return Err(Denial::new(
                        "admission_attribution",
                        "Admission references disagree",
                    ));
                }
            }
            Event::ContributionProposed { .. }
            | Event::SolicitationOpened { .. }
            | Event::OfferSubmitted { .. }
            | Event::Awarded { .. }
            | Event::CommitmentChanged { .. } => {
                if !matches!(
                    event.payload,
                    Event::SolicitationOpened { .. }
                        | Event::CommitmentChanged {
                            change: crate::arbiter::CommitmentChange::Proposed(_),
                            ..
                        }
                ) {
                    self.validate_base_complete()?;
                }
                self.coordination = crate::arbiter::apply(self, event, schemas)?;
                if let Event::ContributionProposed { contribution, .. } = &event.payload {
                    self.references.insert(contribution.reference()?);
                }
            }
            Event::WorkspaceBound {
                workspace, binding, ..
            } => {
                self.validate_complete()?;
                binding.validate()?;
                let recorded = self
                    .workspaces
                    .get(workspace)
                    .ok_or_else(|| Denial::new("workspace_missing", "Workspace is not open"))?;
                if binding.root != recorded.location
                    || self.workspace_bindings.contains_key(workspace)
                    || event.policy.is_some()
                    || event.input.is_some()
                    || event.refs != vec![recorded.reference()?]
                {
                    return Err(Denial::new(
                        "workspace_binding",
                        "Binding does not match the unbound workspace",
                    ));
                }
                self.workspace_bindings
                    .insert(workspace.clone(), (**binding).clone());
            }

            Event::LockChanged { change, .. } => {
                self.validate_base_complete()?;
                if event.policy.is_some()
                    || event.input.is_some()
                    || event.refs != crate::workspace_locks::attribution(self, change)?
                {
                    return Err(Denial::new(
                        "lock_attribution",
                        "Path ownership event metadata differs from its basis",
                    ));
                }
                if matches!(
                    change,
                    crate::workspace_locks::LockChange::CaptureStarted { .. }
                        | crate::workspace_locks::LockChange::CaptureAborted { .. }
                ) {
                    let capture = crate::workspace_locks::apply_capture(
                        self,
                        change,
                        event.reference()?,
                        event.at,
                    )?;
                    self.capture_reads.insert(capture.snapshot.clone(), capture);
                } else {
                    self.path_locks =
                        crate::workspace_locks::apply(self, change, event.reference()?)?;
                }
                crate::workspace_locks::validate_conflicts([&*self])?;
            }

            Event::WorkspaceOpened { workspace, .. } => {
                self.validate_complete()?;
                workspace.location.validate()?;
                schemas.validate(&workspace.provider)?;
                if self.task.is_none()
                    || workspace.kind != ymp_domain::workspace::WorkspaceKind::Direct
                    || self.policies.get("WorkspaceProvider") != Some(&workspace.provider)
                    || self.workspaces.contains_key(&workspace.id)
                    || self.workspaces.values().any(|w| {
                        w.location == workspace.location
                            || (w.location.device == workspace.location.device
                                && w.location.inode == workspace.location.inode)
                    })
                    || event.policy.as_ref() != Some(&workspace.provider.policy)
                    || event.input.is_some()
                    || !event.refs.is_empty()
                {
                    return Err(Denial::new(
                        "workspace_open",
                        "Workspace identity or provider selection is invalid",
                    ));
                }
                self.references.insert(workspace.reference()?);
                self.workspaces
                    .insert(workspace.id.clone(), (**workspace).clone());
            }
            Event::SnapshotTaken { snapshot, version } => {
                self.validate_complete()?;
                snapshot.tree.validate()?;
                let workspace = self.workspaces.get(&snapshot.workspace).ok_or_else(|| {
                    Denial::new("workspace_missing", "Snapshot has no recorded workspace")
                })?;
                let mut refs = vec![workspace.reference()?];
                if *version == 2 {
                    let capture = self.capture_reads.get(&snapshot.id).ok_or_else(|| {
                        Denial::new("capture_missing", "Snapshot requires a held capture read")
                    })?;
                    if capture.ended.is_some()
                        || capture.workspace != snapshot.workspace
                        || snapshot.taken < capture.started_at
                    {
                        return Err(Denial::new(
                            "capture_state",
                            "Snapshot capture is ended or names another workspace",
                        ));
                    }
                    refs.push(capture.started.clone());
                    refs.sort();
                    refs.dedup();
                }
                if self.snapshots.contains_key(&snapshot.id)
                    || (*version == 1 && snapshot.taken != event.at)
                    || (*version == 2 && snapshot.taken > event.at)
                    || event.policy.as_ref() != Some(&workspace.provider.policy)
                    || event.input.is_some()
                    || event.refs != refs
                {
                    return Err(Denial::new(
                        "snapshot_record",
                        "Snapshot identity or attribution is invalid",
                    ));
                }
                if *version == 2 {
                    self.capture_reads
                        .get_mut(&snapshot.id)
                        .expect("validated capture")
                        .ended = Some(event.reference()?);
                }
                self.references.insert(snapshot.reference()?);
                self.snapshots
                    .insert(snapshot.id.clone(), (**snapshot).clone());
            }
            Event::BudgetOpened { .. }
            | Event::ReservationChanged { .. }
            | Event::ReceiptSettled { .. }
            | Event::ReportingStarted { .. } => {
                self.validate_base_complete()?;
                let (policy, input, refs) = crate::treasury::attribution(&event.payload)?;
                if event.policy != policy || event.input != input || event.refs != refs {
                    return Err(Denial::new(
                        "accounting_attribution",
                        "Financial event metadata disagrees with its decision",
                    ));
                }
                let book = crate::treasury::apply(self, event)?;
                if let Event::ReservationChanged {
                    change: crate::treasury::ReservationChange::Observed { receipt, .. },
                    ..
                } = &event.payload
                {
                    self.references.insert(Ref {
                        id: receipt.id.erased(),
                        version: Digest::of_value(receipt)?,
                    });
                }
                self.treasury = Some(Box::new(book));
            }
            Event::PoolRecorded { data, .. } => {
                self.validate_complete()?;
                schemas.validate(&data.effective)?;
                crate::registry::validate_record(self, data, event.at)?;
                let refs = data
                    .decisions
                    .iter()
                    .flat_map(|d| d.proposal.basis.iter().cloned())
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .collect::<Vec<_>>();
                if event.policy.as_ref() != Some(&data.effective.policy)
                    || event.input.as_ref() != Some(&Digest::of_value(&data.input)?)
                    || event.refs != refs
                {
                    return Err(Denial::new(
                        "registry_attribution",
                        "Pool observation attribution disagrees with its decision",
                    ));
                }
                self.registry = Some(data.clone());
            }
            Event::SessionOpened { selections, .. } => {
                if self.opened
                    || self.revision != 0
                    || event.policy.is_some()
                    || event.input.is_some()
                    || !event.refs.is_empty()
                {
                    return Err(Denial::new(
                        "session_open",
                        "A session can only be opened once at its initial revision",
                    ));
                }
                if selections.is_empty() {
                    return Err(Denial::new(
                        "policy_selection",
                        "Explicit session policy selections are required",
                    ));
                }
                let mut policies = BTreeMap::new();
                for selection in selections {
                    schemas.validate(selection)?;
                    if policies
                        .insert(selection.policy.port.clone(), selection.clone())
                        .is_some()
                    {
                        return Err(Denial::new(
                            "policy_selection",
                            "A session selects exactly one implementation per port",
                        ));
                    }
                }
                self.policies = policies;
                self.opened = true;
            }
            Event::CriteriaCommitted { data, .. } => {
                if !self.opened {
                    return Err(Denial::new(
                        "session_missing",
                        "Open a session journal before committing intake",
                    ));
                }
                crate::intake::validate_commit(self.task.as_ref(), self.contract.as_ref(), data)?;
                for id in &data.contract.checks {
                    let check = self
                        .checks
                        .values()
                        .find(|check| check.id.erased() == *id)
                        .ok_or_else(|| {
                            Denial::new("check_missing", "Contract names an unregistered check")
                        })?;
                    if !data.criteria.iter().any(|criterion| {
                        criterion.id == check.criterion
                            && criterion
                                .reference()
                                .is_ok_and(|reference| reference.version == check.criterion_version)
                    }) {
                        return Err(Denial::new(
                            "check_criterion",
                            "Refined criteria cannot inherit a check for another criterion version",
                        ));
                    }
                }
                if event.policy.is_some()
                    || event.input.is_some()
                    || event.refs != data.previous.iter().cloned().collect::<Vec<_>>()
                {
                    return Err(Denial::new(
                        "intake_attribution",
                        "Explicit user intake must carry only its previous contract as basis",
                    ));
                }
                for criterion in &data.criteria {
                    self.references.insert(criterion.reference()?);
                }
                self.references.insert(data.contract.reference());
                self.references.insert(Ref {
                    id: data.task.id.erased(),
                    version: Digest::of_value(&data.task)?,
                });
                self.task = Some(data.task.clone());
                self.contract = Some(data.contract.clone());
                self.criteria = data.criteria.clone();
            }
            Event::ClarificationRecorded { clarification, .. } => {
                self.validate_intake_note(event)?;
                clarification.validate()?;
                self.task
                    .as_mut()
                    .ok_or_else(|| Denial::new("task_missing", "No task is open"))?
                    .goal
                    .clarifications
                    .push(clarification.clone());
            }
            Event::AssumptionRecorded { assumption, .. } => {
                self.validate_intake_note(event)?;
                assumption.validate()?;
                self.task
                    .as_mut()
                    .ok_or_else(|| Denial::new("task_missing", "No task is open"))?
                    .goal
                    .assumptions
                    .push(assumption.clone());
            }
            Event::MethodChosen { decision, .. } => {
                self.validate_complete()?;
                if decision.effective.policy.port != "MethodRouter" {
                    return Err(Denial::new(
                        "policy_port",
                        "MethodChosen requires a MethodRouter policy",
                    ));
                }
                if !self.opened {
                    return Err(Denial::new(
                        "session_missing",
                        "Open the session before recording a method",
                    ));
                }
                decision.proposal.validate()?;
                decision.proposal.value.validate()?;
                schemas.validate(&decision.effective)?;
                let current = self.policies.get("MethodRouter").ok_or_else(|| {
                    Denial::new("policy_selection", "No MethodRouter is selected")
                })?;
                match &decision.selection_change {
                    None if current != &decision.effective => {
                        return Err(Denial::new(
                            "policy_selection",
                            "The proposal does not use the selected effective policy",
                        ));
                    }
                    Some(change)
                        if change.previous != current.policy
                            || change.boundary != self.revision
                            || current == &decision.effective =>
                    {
                        return Err(Denial::new(
                            "policy_change",
                            "Policy change is stale, redundant or bound to another work boundary",
                        ));
                    }
                    _ => {}
                }
                if decision.proposal.policy != decision.effective.policy
                    || decision.outcome.policy != decision.effective.policy
                    || decision.outcome != decision.proposal.value
                    || event.policy.as_ref() != Some(&decision.effective.policy)
                    || event.input.as_ref() != Some(&decision.input)
                    || decision.input != self.digest()?
                    || event.refs != decision.proposal.basis
                {
                    return Err(Denial::new(
                        "decision",
                        "Decision attribution, input view, proposal or kernel outcome disagrees",
                    ));
                }
                for reference in &decision.proposal.basis {
                    self.resolve(reference)?;
                }
                self.references.insert(Ref {
                    id: decision.outcome.id.erased(),
                    version: Digest::of_value(&decision.outcome)?,
                });
                self.method = Some(decision.outcome.clone());
                self.policies
                    .insert("MethodRouter".into(), decision.effective.clone());
                self.decisions.push((**decision).clone());
            }
        }
        if let Some(admission) = admission {
            self.admission = admission;
        }
        if let Some(execution) = execution {
            self.execution = execution;
        }
        self.references.insert(event.reference()?);
        self.results = results;
        self.revision = event.seq;
        self.latest_at = self.latest_at.max(event.at);
        Ok(())
    }
    fn validate_intake_note(&self, event: &Envelope<Event>) -> Result<()> {
        let contract = self
            .contract
            .as_ref()
            .ok_or_else(|| Denial::new("task_missing", "No task is open"))?;
        if self.status() != Some(SessionStatus::Intake)
            || event.policy.is_some()
            || event.input.is_some()
            || event.refs != vec![contract.reference()]
        {
            return Err(Denial::new(
                "intake_attribution",
                "A user clarification or assumption must name the current intake contract",
            ));
        }
        Ok(())
    }

    pub(crate) fn validate_complete(&self) -> Result<()> {
        self.execution.complete()?;
        self.results.complete()?;
        self.admission.complete()?;
        crate::gatekeeper::validate_commitments(self)?;
        self.validate_base_complete()
    }
    fn validate_base_complete(&self) -> Result<()> {
        self.coordination.complete()?;
        if let Some(book) = &self.treasury {
            let task = self
                .task
                .as_ref()
                .ok_or_else(|| Denial::new("task_missing", "Budget lost its task"))?;
            if task.constraints.budget != book.budget.limit
                || task.constraints.verification_reserve != book.budget.verification_reserve
            {
                return Err(Denial::new(
                    "budget_frozen",
                    "An opened budget cannot be silently replaced through intake",
                ));
            }
        }
        match (&self.task, &self.contract) {
            (Some(task), Some(contract)) => contract.validate(task, &self.criteria),
            (None, None) => Ok(()),
            _ => Err(Denial::new(
                "intake_incomplete",
                "Task and acceptance contract must be committed together",
            )),
        }
    }
}
