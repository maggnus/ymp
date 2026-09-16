//! Explicit Scripted ownership facts for financial-independent workspace tests.
use super::kernel::{
    events::Event,
    intake::{Intake, IntakeRequest},
    journal::{ContentStore, Journal},
    ports::execution::WorkspaceProvider,
    registry::{ReadinessResponse, Registry, readiness_views},
    workspace_guard::WorkspaceGuard,
    workspace_locks::{LockAcquisition, LockChange, attribution},
};
use std::{collections::BTreeSet, sync::Arc};
use ymp_domain::{
    Digest, Id, Proposal,
    identity::*,
    journal::{Actor, Capability, Envelope, PolicySelection},
    task::*,
    workspace::*,
};
fn id<T>(value: &str) -> Id<T> {
    Id::new(value).unwrap()
}
pub fn open<J: Journal, C: ContentStore>(
    journal: Arc<J>,
    store: Arc<C>,
    session: &Id,
    provider: &dyn WorkspaceProvider,
) -> (WorkspaceGuard<J, C>, ExecutionProfile) {
    open_with_capabilities(
        journal,
        store,
        session,
        provider,
        BTreeSet::from([Capability::ReadFiles, Capability::WriteFiles]),
    )
}
pub fn open_with_capabilities<J: Journal, C: ContentStore>(
    journal: Arc<J>,
    store: Arc<C>,
    session: &Id,
    provider: &dyn WorkspaceProvider,
    capabilities: BTreeSet<Capability>,
) -> (WorkspaceGuard<J, C>, ExecutionProfile) {
    open_with_policies(journal, store, session, provider, capabilities, vec![])
}
pub fn open_with_policies<J: Journal, C: ContentStore>(
    journal: Arc<J>,
    store: Arc<C>,
    session: &Id,
    provider: &dyn WorkspaceProvider,
    capabilities: BTreeSet<Capability>,
    mut selections: Vec<PolicySelection>,
) -> (WorkspaceGuard<J, C>, ExecutionProfile) {
    let probe = PolicySelection::new(
        "ReadinessProbe",
        "StaticDependencyProbe",
        "1",
        serde_json::json!({}),
    )
    .unwrap();
    selections.extend([provider.selection().clone(), probe.clone()]);
    Intake::new(journal.clone())
        .open(
            session.clone(),
            1,
            IntakeRequest {
                task: Task {
                    id: id("task"),
                    goal: Goal {
                        request: "Exercise workspace ownership".into(),
                        assumptions: vec![],
                        clarifications: vec![],
                    },
                    contract: id("contract"),
                    constraints: Constraints {
                        budget: Real::new(100.0).unwrap(),
                        verification_reserve: Real::new(20.0).unwrap(),
                        deadline: None,
                        pins: Pins::unrestricted(),
                        allowed: capabilities.clone(),
                        parallel_limit: 4,
                        attempt_limit: 3,
                        max_members: 2,
                    },
                },
                criteria: vec![Criterion {
                    id: id("criterion"),
                    text: "Preserve enforceable ownership".into(),
                    kind: CriterionKind::Constraint,
                    weight: Real::new(1.0).unwrap(),
                    required: true,
                    origin: CriterionOrigin::User,
                    needs_class: BTreeSet::new(),
                }],
            },
            selections,
        )
        .unwrap();
    let registry = Registry::new(journal.clone());
    let facts = RegistryFacts {
        agents: vec![Agent {
            id: id("agent"),
            name: "Scripted fixture".into(),
            provider: id("provider"),
            defaults: ProfileSettings::default(),
            instructions: String::new(),
            enabled: true,
        }],
        discoveries: vec![Discovery {
            provider: Provider {
                id: id("provider"),
                kind: ProviderKind::Scripted,
                version: None,
                capabilities: Some(capabilities),
            },
            offerings: vec![ModelOffering {
                provider: id("provider"),
                model: Some("fixture".into()),
                family: None,
                efforts: None,
                default_effort: None,
            }],
            default_model: Some("fixture".into()),
            adapter_available: true,
            source: DiscoverySource::ScriptedFixture,
            method: "Explicit access fixture".into(),
            observed_at: 1,
        }],
        dependencies: vec![],
    };
    let input = registry.prepare(session, facts, 2).unwrap();
    let responses = readiness_views(&input)
        .iter()
        .map(|v| ReadinessResponse {
            profile: v.profile.clone(),
            input: Digest::of_value(v).unwrap(),
            proposal: Proposal {
                value: Readiness::Ready,
                rationale: "Fixture capabilities only".into(),
                basis: vec![],
                policy: probe.policy.clone(),
            },
        })
        .collect();
    registry
        .record(session, 2, 2, input, probe, responses)
        .unwrap();
    let profile = registry
        .profile(session, &id("agent"), &ProfileSettings::default())
        .unwrap();
    let guard = WorkspaceGuard::new(journal, store);
    guard
        .open(session, 3, 3, id("workspace"), provider)
        .unwrap();
    (guard, profile)
}

/// Explicit synthetic boundary data for the aggregate adapter/replay contract.
/// Consumer tests separately exercise the opaque evidence capability.
#[allow(dead_code)]
pub fn acquire<J: Journal>(
    journal: &J,
    session: &Id,
    provider: &dyn WorkspaceProvider,
    path: &str,
    mode: LockMode,
) -> Envelope<Event> {
    let view = journal
        .read(session)
        .unwrap()
        .view_with_schemas(session, None, journal.schemas())
        .unwrap();
    let workspace = &view.workspaces()[&id("workspace")];
    let profile = view.registry().unwrap().decisions[0].profile.clone();
    let path = WorkspacePath::new(path).unwrap();
    let lock = PathLock {
        path: path.clone(),
        mode,
        holder: id("assignment"),
    };
    let change = LockChange::Acquired(Box::new(LockAcquisition {
        mediated_owner: None,
        workspace: id("workspace"),
        assignment: id("assignment"),
        profile,
        requested: vec![lock.clone()],
        effective: vec![ObservedPathLock {
            lock,
            observation: provider.observe_paths(&[path]).unwrap().remove(0),
        }],
        basis: vec![workspace.reference().unwrap()],
    }));
    Envelope {
        seq: view.revision() + 1,
        session: session.clone(),
        at: 5,
        actor: Actor::Runtime,
        policy: None,
        input: None,
        refs: attribution(&view, &change).unwrap(),
        payload: Event::LockChanged { version: 1, change },
    }
}
