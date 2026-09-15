//! Registry validation and recorded readiness decisions. No provider or strategy is run here.

use crate::{
    events::Event,
    journal::{Journal, validate_append},
    view::SessionView,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};
use ymp_domain::{
    Denial, Digest, Id, Proposal, Result,
    identity::*,
    journal::{Actor, Capability, Envelope, PolicySelection},
    require_text,
    task::Constraints,
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegistryInput {
    pub journal: Digest,
    pub constraints: Constraints,
    pub facts: RegistryFacts,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReadinessView {
    pub profile: ExecutionProfile,
    pub agent: Agent,
    pub discovery: Discovery,
    pub dependencies: Vec<DependencyObservation>,
    pub constraints: Constraints,
}
/// A proposal bound to the exact profile and view supplied to the probe.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReadinessResponse {
    pub profile: ExecutionProfile,
    pub input: Digest,
    pub proposal: Proposal<Readiness>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReadinessDecision {
    pub profile: ExecutionProfile,
    pub input: Digest,
    pub proposal: Proposal<Readiness>,
    pub outcome: Readiness,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PoolRecorded {
    pub input: RegistryInput,
    pub effective: PolicySelection,
    pub decisions: Vec<ReadinessDecision>,
    pub outcome: Pool,
}

pub fn readiness_views(input: &RegistryInput) -> Vec<ReadinessView> {
    let mut views = BTreeMap::new();
    for agent in &input.facts.agents {
        let Some(discovery) = input
            .facts
            .discoveries
            .iter()
            .find(|d| d.provider.id == agent.provider)
        else {
            continue;
        };
        for offering in &discovery.offerings {
            let Some(model) = &offering.model else {
                continue;
            };
            let mut efforts = BTreeSet::from([offering.default_effort.clone()]);
            if let Some(known) = &offering.efforts {
                efforts.extend(known.iter().cloned().map(Some));
            }
            for effort in efforts {
                let profile = ExecutionProfile {
                    agent: agent.id.clone(),
                    provider_version: discovery.provider.version.clone(),
                    model: model.clone(),
                    family: offering.family.clone(),
                    effort,
                };
                let dependencies = input
                    .facts
                    .dependencies
                    .iter()
                    .filter(|d| {
                        d.provider == agent.provider && d.model.as_ref().is_none_or(|m| m == model)
                    })
                    .cloned()
                    .collect();
                views.insert(
                    profile.clone(),
                    ReadinessView {
                        profile,
                        agent: agent.clone(),
                        discovery: discovery.clone(),
                        dependencies,
                        constraints: input.constraints.clone(),
                    },
                );
            }
        }
    }
    views.into_values().collect()
}

fn exclusion(view: &ReadinessView) -> Option<Exclusion> {
    if !view.agent.enabled {
        return Some(Exclusion::new(ExclusionCode::Disabled, "Agent is disabled"));
    }
    if !view.discovery.adapter_available {
        return Some(Exclusion::new(
            ExclusionCode::MissingAdapter,
            "Execution adapter is unavailable",
        ));
    }
    if view.discovery.provider.capabilities.is_none() {
        return Some(Exclusion::new(
            ExclusionCode::UnknownCapabilities,
            "Backend capability enforcement is unknown",
        ));
    }
    if view.discovery.source == DiscoverySource::Native
        && !view
            .dependencies
            .iter()
            .any(|d| d.kind == DependencyKind::Executable)
    {
        return Some(Exclusion::new(
            ExclusionCode::MissingDependency,
            "No native executable observation was supplied",
        ));
    }
    let pins = &view.constraints.pins;
    if pins
        .roster
        .as_ref()
        .is_some_and(|ids| !ids.contains(&view.agent.id.erased()))
    {
        return Some(Exclusion::new(
            ExclusionCode::RosterPin,
            "Agent is outside the pinned roster",
        ));
    }
    if pins
        .models
        .as_ref()
        .is_some_and(|models| !models.contains(&view.profile.model))
    {
        return Some(Exclusion::new(
            ExclusionCode::ModelPin,
            "Model is outside the user's pins",
        ));
    }
    if pins.efforts.as_ref().is_some_and(|efforts| {
        view.profile
            .effort
            .as_ref()
            .is_none_or(|e| !efforts.contains(e))
    }) {
        return Some(Exclusion::new(
            ExclusionCode::EffortPin,
            "Effort is unknown or outside the user's pins",
        ));
    }
    dependency_exclusion(&view.dependencies)
}

pub fn dependency_exclusion(dependencies: &[DependencyObservation]) -> Option<Exclusion> {
    dependencies.iter().find_map(|d| match d.state {
        DependencyState::Available => None,
        DependencyState::Missing => Some(Exclusion::new(
            ExclusionCode::MissingDependency,
            format!("Missing dependency: {}", d.path.display()),
        )),
        DependencyState::Unreadable => Some(Exclusion::new(
            ExclusionCode::UnreadableDependency,
            format!("Cannot inspect dependency: {}", d.path.display()),
        )),
        DependencyState::WrongKind | DependencyState::NotExecutable => Some(Exclusion::new(
            ExclusionCode::InvalidDependency,
            format!("Invalid dependency: {}", d.path.display()),
        )),
    })
}

pub fn evaluate(
    input: &RegistryInput,
    effective: &PolicySelection,
    responses: &[ReadinessResponse],
) -> Result<PoolRecorded> {
    if effective.policy.port != "ReadinessProbe" {
        return Err(Denial::new(
            "policy_port",
            "Registry readiness requires ReadinessProbe",
        ));
    }
    let views = readiness_views(input);
    if views.len() != responses.len() {
        return Err(Denial::new(
            "registry_proposals",
            "A readiness proposal is required for each candidate profile",
        ));
    }
    let mut indexed = BTreeMap::new();
    for response in responses {
        if indexed.insert(&response.profile, response).is_some() {
            return Err(Denial::new(
                "registry_proposals",
                "Duplicate profile response",
            ));
        }
    }
    let mut decisions = Vec::new();
    for view in &views {
        let response = indexed.get(&view.profile).ok_or_else(|| {
            Denial::new(
                "registry_proposals",
                "Missing response for a candidate profile",
            )
        })?;
        if response.input != Digest::of_value(view)? {
            return Err(Denial::new(
                "registry_proposals",
                "Readiness response belongs to another input view",
            ));
        }
        let proposal = &response.proposal;
        proposal.validate()?;
        if proposal.policy != effective.policy {
            return Err(Denial::new(
                "policy_selection",
                "Readiness proposal uses a different selected policy",
            ));
        }
        if let Readiness::NotReady(reason) = &proposal.value {
            require_text(&reason.message, 4096)?;
        }
        let outcome = exclusion(view)
            .map(Readiness::NotReady)
            .unwrap_or_else(|| proposal.value.clone());
        decisions.push(ReadinessDecision {
            profile: view.profile.clone(),
            input: response.input.clone(),
            proposal: proposal.clone(),
            outcome,
        });
    }
    let mut pool = Pool {
        eligible: BTreeSet::new(),
        offerings: BTreeMap::new(),
        excluded: BTreeMap::new(),
    };
    for agent in &input.facts.agents {
        let discovery = input
            .facts
            .discoveries
            .iter()
            .find(|d| d.provider.id == agent.provider);
        pool.offerings.insert(
            agent.id.clone(),
            discovery.map(|d| d.offerings.clone()).unwrap_or_default(),
        );
        let profiles: Vec<_> = decisions
            .iter()
            .filter(|d| d.profile.agent == agent.id)
            .collect();
        if profiles.iter().any(|d| d.outcome == Readiness::Ready) {
            pool.eligible.insert(agent.id.clone());
            continue;
        }
        let reason = if !agent.enabled {
            Exclusion::new(ExclusionCode::Disabled, "Agent is disabled")
        } else if discovery.is_none() {
            Exclusion::new(
                ExclusionCode::MissingProvider,
                "Provider has no discovery observation",
            )
        } else {
            profiles
                .iter()
                .find_map(|d| match &d.outcome {
                    Readiness::NotReady(reason) => Some(reason.clone()),
                    Readiness::Ready => None,
                })
                .unwrap_or_else(|| {
                    Exclusion::new(
                        ExclusionCode::MissingModel,
                        "No concrete model was reported",
                    )
                })
        };
        pool.excluded.insert(agent.id.clone(), reason);
    }
    if let Some(roster) = &input.constraints.pins.roster {
        for id in roster {
            let id = Id::<Agent>::new(id.as_str())?;
            if !input.facts.agents.iter().any(|a| a.id == id) {
                pool.excluded.insert(
                    id,
                    Exclusion::new(
                        ExclusionCode::MissingProvider,
                        "Pinned agent has no registration",
                    ),
                );
            }
        }
    }
    Ok(PoolRecorded {
        input: input.clone(),
        effective: effective.clone(),
        decisions,
        outcome: pool,
    })
}

/// Used by both the live consumer and replay; no returned probe verdict bypasses these checks.
pub fn validate_record(view: &SessionView, record: &PoolRecorded, at: u64) -> Result<()> {
    record.input.facts.validate(at)?;
    record.input.constraints.validate()?;
    if record.input.journal != view.digest()?
        || view
            .task()
            .is_none_or(|task| task.constraints != record.input.constraints)
    {
        return Err(Denial::new(
            "registry_input",
            "Registry input is stale or does not match the task constraints",
        ));
    }
    if view.policies().get("ReadinessProbe") != Some(&record.effective) {
        return Err(Denial::new(
            "policy_selection",
            "ReadinessProbe does not match the session selection",
        ));
    }
    for decision in &record.decisions {
        for reference in &decision.proposal.basis {
            view.resolve(reference)?;
        }
    }
    let responses: Vec<_> = record
        .decisions
        .iter()
        .map(|d| ReadinessResponse {
            profile: d.profile.clone(),
            input: d.input.clone(),
            proposal: d.proposal.clone(),
        })
        .collect();
    if evaluate(&record.input, &record.effective, &responses)? != *record {
        return Err(Denial::new(
            "registry_outcome",
            "Recorded readiness or Pool does not follow its inputs and proposals",
        ));
    }
    Ok(())
}

pub struct Registry<J: Journal> {
    journal: Arc<J>,
}
impl<J: Journal> Registry<J> {
    pub fn new(journal: Arc<J>) -> Self {
        Self { journal }
    }
    pub fn prepare(&self, session: &Id, facts: RegistryFacts, at: u64) -> Result<RegistryInput> {
        facts.validate(at)?;
        let view =
            self.journal
                .read(session)?
                .view_with_schemas(session, None, self.journal.schemas())?;
        let constraints = view
            .task()
            .ok_or_else(|| {
                Denial::new(
                    "task_missing",
                    "Open an explicit task before observing its pool",
                )
            })?
            .constraints
            .clone();
        Ok(RegistryInput {
            journal: view.digest()?,
            constraints,
            facts,
        })
    }
    pub fn record(
        &self,
        session: &Id,
        expected: u64,
        at: u64,
        input: RegistryInput,
        effective: PolicySelection,
        responses: Vec<ReadinessResponse>,
    ) -> Result<u64> {
        let current = self.journal.read(session)?;
        let view = current.view_with_schemas(session, None, self.journal.schemas())?;
        self.journal.schemas().validate(&effective)?;
        let record = evaluate(&input, &effective, &responses)?;
        validate_record(&view, &record, at)?;
        let refs = record
            .decisions
            .iter()
            .flat_map(|d| d.proposal.basis.iter().cloned())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        let event = Envelope {
            seq: expected
                .checked_add(1)
                .ok_or_else(|| Denial::new("revision_overflow", "Journal sequence exhausted"))?,
            session: session.clone(),
            at,
            actor: Actor::Runtime,
            policy: Some(effective.policy.clone()),
            input: Some(Digest::of_value(&input)?),
            refs,
            payload: Event::PoolRecorded {
                version: 1,
                data: Box::new(record),
            },
        };
        let events = [event];
        let validated =
            validate_append(&current, session, expected, &events, self.journal.schemas())?;
        let committed = self.journal.append(session, expected, &events)?;
        if committed != validated.revision() {
            return Err(Denial::new(
                "journal_append",
                "Journal returned an unexpected committed revision",
            ));
        }
        Ok(committed)
    }
    fn snapshot(&self, session: &Id) -> Result<PoolRecorded> {
        let view =
            self.journal
                .read(session)?
                .view_with_schemas(session, None, self.journal.schemas())?;
        let record = view
            .registry()
            .ok_or_else(|| Denial::new("registry_missing", "No Pool observation is recorded"))?;
        if view
            .task()
            .is_none_or(|t| t.constraints != record.input.constraints)
        {
            return Err(Denial::new(
                "registry_stale",
                "Task constraints changed; refresh readiness before selecting a profile",
            ));
        }
        Ok(record.clone())
    }
    pub fn pool(&self, session: &Id) -> Result<Pool> {
        Ok(self.snapshot(session)?.outcome)
    }
    pub fn profile(
        &self,
        session: &Id,
        agent: &Id<Agent>,
        settings: &ProfileSettings,
    ) -> Result<ExecutionProfile> {
        settings.validate()?;
        let record = self.snapshot(session)?;
        let agent = record
            .input
            .facts
            .agents
            .iter()
            .find(|a| &a.id == agent)
            .ok_or_else(|| Denial::new("agent_missing", "Agent is not registered"))?;
        let discovery = record
            .input
            .facts
            .discoveries
            .iter()
            .find(|d| d.provider.id == agent.provider)
            .ok_or_else(|| {
                Denial::new("provider_missing", "No provider observation is available")
            })?;
        let model = settings
            .model
            .as_ref()
            .or(agent.defaults.model.as_ref())
            .or(discovery.default_model.as_ref())
            .ok_or_else(|| {
                Denial::new(
                    "model_unknown",
                    "No model was requested or reported as default",
                )
            })?;
        let offering = discovery
            .offerings
            .iter()
            .find(|o| o.model.as_ref() == Some(model))
            .ok_or_else(|| {
                Denial::new(
                    "model_unsupported",
                    "The requested model was not discovered",
                )
            })?;
        let requested_effort = settings.effort.as_ref().or(agent.defaults.effort.as_ref());
        if requested_effort.is_some_and(|effort| {
            !offering
                .efforts
                .as_ref()
                .is_some_and(|set| set.contains(effort))
        }) {
            return Err(Denial::new(
                "effort_unsupported",
                "The native effort is unknown or unsupported for this model",
            ));
        }
        let profile = ExecutionProfile {
            agent: agent.id.clone(),
            provider_version: discovery.provider.version.clone(),
            model: model.clone(),
            family: offering.family.clone(),
            effort: requested_effort
                .cloned()
                .or(offering.default_effort.clone()),
        };
        let decision = record
            .decisions
            .iter()
            .find(|d| d.profile == profile)
            .ok_or_else(|| {
                Denial::new(
                    "profile_missing",
                    "The requested profile has no readiness decision",
                )
            })?;
        if let Readiness::NotReady(reason) = &decision.outcome {
            return Err(Denial::new("profile_excluded", &reason.message));
        }
        Ok(profile)
    }
    pub fn capabilities(
        &self,
        session: &Id,
        profile: &ExecutionProfile,
        workspace: &WorkspaceCapabilities,
    ) -> Result<BTreeSet<Capability>> {
        if &workspace.profile != profile {
            return Err(Denial::new(
                "workspace_profile",
                "Workspace enforcement belongs to another execution profile",
            ));
        }
        let record = self.snapshot(session)?;
        if !record
            .decisions
            .iter()
            .any(|d| &d.profile == profile && d.outcome == Readiness::Ready)
        {
            return Err(Denial::new(
                "profile_excluded",
                "The profile is not currently eligible",
            ));
        }
        let agent = record
            .input
            .facts
            .agents
            .iter()
            .find(|a| a.id == profile.agent)
            .ok_or_else(|| Denial::new("agent_missing", "Agent is not registered"))?;
        let backend = record
            .input
            .facts
            .discoveries
            .iter()
            .find(|d| d.provider.id == agent.provider)
            .and_then(|d| d.provider.capabilities.as_ref())
            .ok_or_else(|| Denial::new("capabilities_unknown", "Backend enforcement is unknown"))?;
        let workspace = workspace.enforced.as_ref().ok_or_else(|| {
            Denial::new("capabilities_unknown", "Workspace enforcement is unknown")
        })?;
        Ok(backend
            .intersection(workspace)
            .filter(|c| record.input.constraints.allowed.contains(c))
            .cloned()
            .collect())
    }
}
