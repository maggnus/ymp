//! Kernel-owned registration and execution of versioned visible checks.
use crate::{
    decision::{DecisionConsumer, SessionControl},
    events::Event,
    gatekeeper::{Gatekeeper, GrantToken},
    journal::{ContentStore, Journal, validate_append},
    ports::checks::{CheckExecution, CheckRunner},
    view::SessionView,
    workspace_guard::WorkspaceGuard,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use ymp_domain::{
    Denial, Digest, Id, Proposal, Ref, Result,
    assignment::{RoleKind, TeamOperation},
    journal::{Actor, Envelope, PolicySelection, encode},
    task::AcceptanceContract,
    verification::*,
    workspace::Snapshot,
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CheckRegistered {
    pub proposal: Proposal<Check>,
    pub effective: PolicySelection,
    pub contract: AcceptanceContract,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CheckRunRecorded {
    pub run: CheckRun,
    pub environment: CheckEnvironment,
}

pub struct RegisterCheck {
    pub expected_revision: u64,
    pub at: u64,
    pub proposal: Proposal<Check>,
    pub effective: PolicySelection,
}
pub struct RunCheck {
    pub expected_revision: u64,
    pub at: u64,
    pub id: Id<CheckRun>,
    pub check: Ref,
    pub target: Id<Snapshot>,
    pub role: CheckRunRole,
}
pub struct AcceptanceAuthority<J: Journal, C: ContentStore> {
    journal: Arc<J>,
    content: Arc<C>,
    owner: DecisionConsumer<J>,
}
impl<J: Journal, C: ContentStore> AcceptanceAuthority<J, C> {
    pub(crate) fn new(journal: Arc<J>, content: Arc<C>, owner: DecisionConsumer<J>) -> Self {
        Self {
            journal,
            content,
            owner,
        }
    }
    pub fn register_check(&self, owner: &SessionControl, request: RegisterCheck) -> Result<Check> {
        let session = self.owner.authorize(owner)?;
        if request.proposal.value.author != CheckAuthor::User {
            return Err(Denial::new(
                "check_author",
                "Owner registration cannot impersonate an assignment",
            ));
        }
        self.register(&session, request)
    }
    pub fn register_agent_check(
        &self,
        gatekeeper: &Gatekeeper<J, C>,
        token: &GrantToken,
        request: RegisterCheck,
    ) -> Result<Check> {
        gatekeeper.require_journal(&self.journal)?;
        let assignment = gatekeeper.authorize_revision(
            token,
            Some(TeamOperation::CheckPropose),
            request.at,
            request.expected_revision,
        )?;
        if request.proposal.value.author != CheckAuthor::Agent(assignment.id) {
            return Err(Denial::new(
                "check_author",
                "Check author differs from the authenticated assignment",
            ));
        }
        self.register(&assignment.session, request)
    }
    fn register(&self, session: &Id, request: RegisterCheck) -> Result<Check> {
        let view = self.journal.view(session, None)?;
        boundary(&view, request.expected_revision, request.at)?;
        let check = request.proposal.value.clone();
        let prior = view
            .contract()
            .ok_or_else(|| Denial::new("contract_missing", "No acceptance contract"))?;
        let mut ids = prior.checks.clone();
        ids.push(check.id.erased());
        let data = CheckRegistered {
            proposal: request.proposal,
            effective: request.effective,
            contract: AcceptanceContract::new(
                view.task().expect("contract task"),
                view.criteria(),
                ids,
            )?,
        };
        let event = Envelope {
            seq: next(view.revision())?,
            session: session.clone(),
            at: request.at,
            actor: Actor::Runtime,
            policy: Some(data.proposal.policy.clone()),
            input: Some(view.digest()?),
            refs: registration_refs(&view, &data)?,
            payload: Event::CheckRegistered {
                version: 1,
                data: Box::new(data),
            },
        };
        self.append(session, view.revision(), event)?;
        Ok(check)
    }
    pub fn run(
        &self,
        owner: &SessionControl,
        request: RunCheck,
        runner: &dyn CheckRunner,
    ) -> Result<CheckRun> {
        let session = self.owner.authorize(owner)?;
        let view = self.journal.view(&session, None)?;
        boundary(&view, request.expected_revision, request.at)?;
        if view.check_runs().contains_key(&request.id) {
            return Err(Denial::new(
                "check_run_id",
                "Check run identity was already used",
            ));
        }
        let check = find_check(&view, &request.check)?;
        executable_scope(&view, check, request.at)?;
        let guard = WorkspaceGuard::new(self.journal.clone(), self.content.clone());
        let target = guard.retained(&session, &request.target)?;
        let verifier = check
            .verifier
            .as_ref()
            .map(|reference| {
                let snapshot = snapshot_ref(&view, reference)?;
                guard.retained(&session, &snapshot.id)
            })
            .transpose()?;
        let environment = runner.environment()?;
        environment.validate()?;
        self.journal.schemas().validate(&environment.runner)?;
        let env = retain(self.content.as_ref(), &encode(&environment)?)?;
        let observation = runner.run(
            &CheckExecution {
                check,
                target: &target,
                verifier: verifier.as_ref(),
                environment: &environment,
            },
            self.content.as_ref(),
        )?;
        if observation.check != check.reference()
            || observation.target != target.reference()?
            || observation.environment != env
        {
            return Err(Denial::new(
                "check_observation",
                "Runner substituted the check, target snapshot or environment",
            ));
        }
        if observation.stdout.len() > environment.limits.output_bytes
            || observation.stderr.len() > environment.limits.output_bytes
        {
            return Err(Denial::new(
                "check_output",
                "Runner exceeded its bounded retained output",
            ));
        }
        let (exit, outcome) = derive(check, &target, &observation.kind)?;
        let run = CheckRun {
            id: request.id,
            check: check.id.clone(),
            check_version: check.version.clone(),
            target: target.id.clone(),
            target_version: target.reference()?.version,
            role: request.role,
            exit,
            stdout: retain(self.content.as_ref(), &observation.stdout)?,
            stderr: retain(self.content.as_ref(), &observation.stderr)?,
            env,
            observation: observation.kind,
            outcome,
            at: request.at,
        };
        let data = CheckRunRecorded {
            run: run.clone(),
            environment,
        };
        let event = Envelope {
            seq: next(view.revision())?,
            session: session.clone(),
            at: request.at,
            actor: Actor::Runtime,
            policy: Some(data.environment.runner.policy.clone()),
            input: Some(view.digest()?),
            refs: vec![check.reference(), target.reference()?],
            payload: Event::CheckRunRecorded {
                version: 1,
                data: Box::new(data),
            },
        };
        self.append(&session, view.revision(), event)?;
        Ok(run)
    }
    fn append(&self, session: &Id, expected: u64, event: Envelope<Event>) -> Result<()> {
        let current = self.journal.read(session)?;
        validate_append(
            &current,
            session,
            expected,
            std::slice::from_ref(&event),
            self.journal.schemas(),
        )?;
        if self.journal.append(session, expected, &[event])? != next(expected)? {
            return Err(Denial::new(
                "journal_append",
                "Unexpected committed revision",
            ));
        }
        Ok(())
    }
}
fn next(value: u64) -> Result<u64> {
    value
        .checked_add(1)
        .ok_or_else(|| Denial::new("revision_overflow", "Journal sequence exhausted"))
}
fn boundary(view: &SessionView, expected: u64, at: u64) -> Result<()> {
    if view.revision() != expected || at < view.latest_at() {
        return Err(Denial::new(
            "stale_revision",
            "Check request has a stale work boundary",
        ));
    }
    Ok(())
}
fn retain(store: &dyn ContentStore, bytes: &[u8]) -> Result<Digest> {
    let digest = Digest::of(bytes);
    if store.put(bytes)? != digest || store.get(&digest, bytes.len())? != bytes {
        return Err(Denial::new(
            "check_content",
            "Content store did not retain the exact check bytes",
        ));
    }
    Ok(digest)
}
fn snapshot_ref<'a>(view: &'a SessionView, reference: &Ref) -> Result<&'a Snapshot> {
    view.snapshots()
        .values()
        .find(|snapshot| snapshot.reference().as_ref() == Ok(reference))
        .ok_or_else(|| {
            Denial::new(
                "snapshot_missing",
                "Exact verifier snapshot is not recorded",
            )
        })
}
fn find_check<'a>(view: &'a SessionView, reference: &Ref) -> Result<&'a Check> {
    view.checks()
        .values()
        .find(|check| check.reference() == *reference)
        .ok_or_else(|| Denial::new("check_missing", "Exact check version is not registered"))
}
pub(crate) fn independence(view: &SessionView, check: &Check, at: u64) -> Result<Independence> {
    match &check.author {
        CheckAuthor::User => Ok(Independence::Trusted),
        CheckAuthor::Agent(id) => {
            let record =
                view.admission().assignments().get(id).ok_or_else(|| {
                    Denial::new("check_author", "Unknown check author assignment")
                })?;
            let assignment = &record.intent.assignment;
            crate::gatekeeper::validate_active_grant(
                view,
                record,
                Some(TeamOperation::CheckPropose),
                at,
            )?;
            if !check.needs.is_subset(&assignment.access) {
                return Err(Denial::new(
                    "check_author",
                    "Check needs exceed the author's access",
                ));
            }
            let contribution = view
                .coordination()
                .contributions()
                .get(&assignment.contribution)
                .ok_or_else(|| Denial::new("check_author", "Missing author contribution"))?;
            if !contribution.value.targets.contains(&check.criterion) {
                return Err(Denial::new(
                    "check_author",
                    "Check criterion is outside the author's contribution",
                ));
            }
            let produced = view.admission().assignments().values().any(|a| {
                a.intent.assignment.agent == assignment.agent
                    && a.intent.assignment.role == RoleKind::Producer
                    && view
                        .coordination()
                        .contributions()
                        .get(&a.intent.assignment.contribution)
                        .is_some_and(|c| c.value.targets.contains(&check.criterion))
            });
            Ok(if produced {
                Independence::ProducerAuthored
            } else {
                Independence::IndependentVisible
            })
        }
    }
}
pub(crate) fn registration_refs(view: &SessionView, data: &CheckRegistered) -> Result<Vec<Ref>> {
    let check = &data.proposal.value;
    let mut refs = data.proposal.basis.clone();
    refs.push(
        view.contract()
            .ok_or_else(|| Denial::new("contract_missing", "No contract"))?
            .reference(),
    );
    refs.push(Ref {
        id: check.criterion.erased(),
        version: check.criterion_version.clone(),
    });
    if let Some(verifier) = &check.verifier {
        refs.push(verifier.clone());
    }
    if let CheckAuthor::Agent(id) = &check.author {
        let record = view
            .admission()
            .assignments()
            .get(id)
            .ok_or_else(|| Denial::new("check_author", "Unknown assignment"))?;
        refs.extend(record.references.clone());
    }
    refs.sort();
    refs.dedup();
    Ok(refs)
}
pub(crate) fn validate_registered(
    view: &SessionView,
    event: &Envelope<Event>,
    data: &CheckRegistered,
) -> Result<()> {
    data.proposal.validate()?;
    boundary(view, view.revision(), event.at)?;
    data.effective.validate()?;
    if data.effective.policy.port != "VerificationDesigner"
        || data.effective.policy != data.proposal.policy
        || view.policies().get("VerificationDesigner") != Some(&data.effective)
    {
        return Err(Denial::new(
            "check_policy",
            "Check proposal does not use its recorded VerificationDesigner implementation",
        ));
    }
    let check = &data.proposal.value;
    check.validate()?;
    let task = view
        .task()
        .ok_or_else(|| Denial::new("task_missing", "No check task"))?;
    let criterion = view
        .criteria()
        .iter()
        .find(|c| c.id == check.criterion)
        .ok_or_else(|| Denial::new("criterion_missing", "Unknown check criterion"))?;
    let prior = view.contract().expect("task contract");
    let mut ids = prior.checks.clone();
    ids.push(check.id.erased());
    let expected = AcceptanceContract::new(task, view.criteria(), ids)?;
    if view.checks().contains_key(&check.id)
        || criterion.reference()?.version != check.criterion_version
        || !check.needs.is_subset(&task.constraints.allowed)
        || check.independence != independence(view, check, event.at)?
        || data.contract != expected
        || event.policy.as_ref() != Some(&data.proposal.policy)
        || event.input.as_ref() != Some(&view.digest()?)
        || event.refs != registration_refs(view, data)?
    {
        return Err(Denial::new(
            "check_registration",
            "Check authority, independence, criterion version or contract attribution disagrees",
        ));
    }
    if let Some(reference) = &check.verifier {
        snapshot_ref(view, reference)?;
    }
    Ok(())
}
pub(crate) fn validate_run(
    view: &SessionView,
    event: &Envelope<Event>,
    data: &CheckRunRecorded,
) -> Result<()> {
    let run = &data.run;
    let check = find_check(
        view,
        &Ref {
            id: run.check.erased(),
            version: run.check_version.clone(),
        },
    )?;
    let target = snapshot_ref(
        view,
        &Ref {
            id: run.target.erased(),
            version: run.target_version.clone(),
        },
    )?;
    data.environment.validate()?;
    executable_scope(view, check, event.at)?;
    let (exit, outcome) = derive(check, target, &run.observation)?;
    if view.check_runs().contains_key(&run.id)
        || run.env != Digest::of_value(&data.environment)?
        || run.at != event.at
        || run.exit != exit
        || run.outcome != outcome
        || event.policy.as_ref() != Some(&data.environment.runner.policy)
        || event.input.as_ref() != Some(&view.digest()?)
        || event.refs != vec![check.reference(), target.reference()?]
    {
        return Err(Denial::new(
            "check_run",
            "Check run observation, environment or attribution disagrees",
        ));
    }
    Ok(())
}
fn derive(
    check: &Check,
    target: &Snapshot,
    kind: &CheckObservationKind,
) -> Result<(Option<i32>, CheckOutcome)> {
    match (&check.spec, kind) {
        (_, CheckObservationKind::Error { class, reason }) => {
            ymp_domain::require_text(reason, 16_384)?;
            Ok((None, CheckOutcome::Error(*class)))
        }
        (CheckSpec::Command { .. }, CheckObservationKind::Exited(code))
            if (0..=255).contains(code) =>
        {
            Ok((
                Some(*code),
                if *code == 0 {
                    CheckOutcome::Pass
                } else {
                    CheckOutcome::Fail
                },
            ))
        }
        (CheckSpec::ExactBytes { path, digest }, CheckObservationKind::ExactBytes(actual))
            if actual.as_ref() == target.tree.files.get(path).map(|file| &file.digest) =>
        {
            Ok((
                None,
                if actual.as_ref() == Some(digest) {
                    CheckOutcome::Pass
                } else {
                    CheckOutcome::Fail
                },
            ))
        }
        _ => Err(Denial::new(
            "check_observation",
            "Observation does not describe the registered check and retained snapshot",
        )),
    }
}

fn executable_scope(view: &SessionView, check: &Check, at: u64) -> Result<()> {
    boundary(view, view.revision(), at)?;
    let task = view
        .task()
        .ok_or_else(|| Denial::new("task_missing", "No check task"))?;
    if !check.needs.is_subset(&task.constraints.allowed)
        || task
            .constraints
            .deadline
            .is_some_and(|deadline| at >= deadline)
    {
        return Err(Denial::new(
            "check_scope",
            "Current task capabilities or deadline do not authorize this check",
        ));
    }
    Ok(())
}
