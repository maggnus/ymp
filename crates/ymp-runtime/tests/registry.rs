use std::{collections::BTreeSet, sync::Arc};
use ymp_domain::{
    Digest, Id, Proposal, Result,
    identity::*,
    journal::{Capability, PolicySelection, decode, encode},
    task::*,
};
use ymp_kernel::{
    journal::{Journal, ParameterSchemas},
    ports::checks::{NativeDiscovery, ReadinessProbe},
    registry::{ReadinessView, readiness_views},
};
use ymp_runtime::{
    application::{Application, IntakeNote, IntakeRefinement, IntakeRequest},
    memory_journal::MemoryJournal,
    readiness::{StaticDependencyProbe, readiness_response},
};

fn capabilities() -> BTreeSet<Capability> {
    BTreeSet::from([
        Capability::ReadFiles,
        Capability::WriteFiles,
        Capability::RunProcess,
        Capability::TempFiles,
        Capability::Sockets,
        Capability::Network,
        Capability::Browser,
        Capability::VcsRead,
        Capability::VcsWrite,
    ])
}
fn facts() -> RegistryFacts {
    let provider = Provider {
        id: Id::new("provider").unwrap(),
        kind: ProviderKind::Scripted,
        version: Some("fixture-v1".into()),
        capabilities: Some(capabilities()),
    };
    let agents = ["agent-a", "agent-b"]
        .into_iter()
        .map(|id| Agent {
            id: Id::new(id).unwrap(),
            name: id.into(),
            provider: provider.id.clone(),
            defaults: ProfileSettings {
                model: Some("unavailable-default".into()),
                effort: None,
            },
            instructions: "Preserve the task criteria".into(),
            enabled: true,
        })
        .collect();
    let offerings = vec![
        ModelOffering {
            provider: provider.id.clone(),
            model: Some("tiny".into()),
            family: None,
            efforts: Some(BTreeSet::from(["low".into()])),
            default_effort: None,
        },
        ModelOffering {
            provider: provider.id.clone(),
            model: Some("large".into()),
            family: Some("fixture-family".into()),
            efforts: Some(BTreeSet::from(["high".into()])),
            default_effort: Some("high".into()),
        },
        ModelOffering {
            provider: provider.id.clone(),
            model: None,
            family: None,
            efforts: None,
            default_effort: None,
        },
    ];
    RegistryFacts {
        agents,
        discoveries: vec![Discovery {
            provider,
            offerings,
            default_model: Some("tiny".into()),
            adapter_available: true,
            source: DiscoverySource::ScriptedFixture,
            method: "explicit metadata fixture".into(),
            observed_at: 10,
        }],
        dependencies: vec![],
    }
}
fn task() -> IntakeRequest {
    IntakeRequest {
        task: Task {
            id: Id::new("task").unwrap(),
            goal: Goal {
                request: "Inspect a fixture pool".into(),
                assumptions: vec![],
                clarifications: vec![],
            },
            contract: Id::new("contract").unwrap(),
            constraints: Constraints {
                budget: Real::new(100.0).unwrap(),
                verification_reserve: Real::new(10.0).unwrap(),
                deadline: None,
                pins: Pins::unrestricted(),
                allowed: capabilities(),
                parallel_limit: 2,
                attempt_limit: 2,
                max_members: 3,
            },
        },
        criteria: vec![Criterion {
            id: Id::new("criterion").unwrap(),
            text: "Report available profiles".into(),
            kind: CriterionKind::ArtifactPresence,
            weight: Real::new(1.0).unwrap(),
            required: true,
            origin: CriterionOrigin::User,
            needs_class: BTreeSet::new(),
        }],
    }
}
fn open(
    journal: Arc<MemoryJournal>,
    policy: &PolicySelection,
) -> (
    Application<MemoryJournal>,
    Id,
    ymp_kernel::decision::SessionControl,
) {
    let app = Application::new(journal);
    let session = Id::new("registry-session").unwrap();
    let method = PolicySelection::new(
        "MethodRouter",
        "FixedMethod",
        "1",
        serde_json::json!({"kind":"SoloWithVerifier","ladder":["StopPreserving"]}),
    )
    .unwrap();
    let owner = app
        .open(session.clone(), 10, task(), vec![method, policy.clone()])
        .unwrap();
    (app, session, owner)
}
fn observe(
    app: &Application<MemoryJournal>,
    session: &Id,
    facts: RegistryFacts,
    probe: &dyn ReadinessProbe,
) {
    let input = app.registry_input(session, facts, 20).unwrap();
    let proposals = readiness_views(&input)
        .iter()
        .map(|v| readiness_response(probe, v).unwrap())
        .collect();
    let revision = app.view(session, None).unwrap().revision();
    app.record_pool(
        session,
        revision,
        20,
        input,
        probe.selection().clone(),
        proposals,
    )
    .unwrap();
}
fn settings(model: &str, effort: Option<&str>) -> ProfileSettings {
    ProfileSettings {
        model: Some(model.into()),
        effort: effort.map(str::to_owned),
    }
}

#[test]
fn registry_preserves_native_metadata_and_resolves_overrides_without_inventing_reports() {
    struct Fixture;
    impl NativeDiscovery for Fixture {
        fn discover(&self) -> Result<Discovery> {
            Ok(facts().discoveries.remove(0))
        }
    }
    let mut observed = facts();
    observed.discoveries = vec![Fixture.discover().unwrap()];
    let probe = StaticDependencyProbe::new().unwrap();
    let journal = Arc::new(MemoryJournal::new());
    let (app, session, _) = open(journal.clone(), probe.selection());
    observe(&app, &session, observed.clone(), &probe);
    let pool = app.pool(&session).unwrap();
    let agent = Id::new("agent-a").unwrap();
    assert!(pool.eligible.contains(&agent));
    assert_eq!(pool.offerings[&agent][2].model, None);
    assert!(
        app.profile(&session, &agent, &ProfileSettings::default())
            .is_err()
    );
    let requested = settings("tiny", Some("low"));
    let profile = app.profile(&session, &agent, &requested).unwrap();
    assert_eq!(profile.family, None);
    assert_eq!(profile.provider_version, Some("fixture-v1".into()));
    assert_eq!(profile.effort, Some("low".into()));
    assert_eq!(
        app.profile(&session, &agent, &settings("tiny", None))
            .unwrap()
            .effort,
        None
    );
    assert_eq!(requested, settings("tiny", Some("low")));
    assert_eq!(
        observed.agents[0].defaults.model.as_deref(),
        Some("unavailable-default")
    );
    assert!(
        app.profile(&session, &agent, &settings("tiny", Some("high")))
            .is_err()
    );
    assert!(
        app.profile(&session, &agent, &settings("missing", Some("low")))
            .is_err()
    );
    let view = app.view(&session, None).unwrap();
    let record = view.registry().unwrap();
    assert_eq!(record.input.facts, observed);
    assert_eq!(record.effective, probe.selection().clone());
    for (view, decision) in readiness_views(&record.input).iter().zip(&record.decisions) {
        assert_eq!(decision.input, Digest::of_value(view).unwrap());
    }
    let history = journal.read(&session).unwrap();
    let bytes = encode(&history.events).unwrap();
    let events: Vec<ymp_domain::journal::Envelope<ymp_kernel::events::Event>> =
        decode(&bytes).unwrap();
    let replay = ymp_kernel::view::SessionView::replay(session, &events).unwrap();
    assert_eq!(replay, view);
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct MaintenanceParameters {
    suspended_agents: BTreeSet<Id<Agent>>,
}
struct Maintenance {
    selection: PolicySelection,
}
impl ReadinessProbe for Maintenance {
    fn selection(&self) -> &PolicySelection {
        &self.selection
    }
    fn probe(&self, view: &ReadinessView) -> Proposal<Readiness> {
        let parameters: MaintenanceParameters =
            decode(&encode(&self.selection.parameters).unwrap()).unwrap();
        let value = if parameters.suspended_agents.contains(&view.agent.id) {
            Readiness::NotReady(Exclusion::new(
                ExclusionCode::Policy,
                "Agent is in a recorded maintenance window",
            ))
        } else {
            Readiness::Ready
        };
        Proposal {
            value,
            rationale: "Apply the selected maintenance policy".into(),
            basis: vec![],
            policy: self.selection.policy.clone(),
        }
    }
}
#[test]
fn replacement_changes_eligibility_but_cannot_override_kernel_exclusions() {
    let static_probe = StaticDependencyProbe::new().unwrap();
    let (base, session, _) = open(Arc::new(MemoryJournal::new()), static_probe.selection());
    observe(&base, &session, facts(), &static_probe);
    assert_eq!(base.pool(&session).unwrap().eligible.len(), 2);
    let mut schemas = ParameterSchemas::default();
    schemas
        .register("ReadinessProbe", "MaintenanceTest", "1", |selection| {
            let _: MaintenanceParameters = decode(&encode(&selection.parameters)?)?;
            Ok(())
        })
        .unwrap();
    let maintenance = Maintenance {
        selection: PolicySelection::new(
            "ReadinessProbe",
            "MaintenanceTest",
            "1",
            serde_json::json!({"suspended_agents":["agent-b"]}),
        )
        .unwrap(),
    };
    let (app, session, _) = open(
        Arc::new(MemoryJournal::with_schemas(schemas)),
        maintenance.selection(),
    );
    observe(&app, &session, facts(), &maintenance);
    assert_eq!(
        app.pool(&session).unwrap().eligible,
        BTreeSet::from([Id::new("agent-a").unwrap()])
    );
    let mut input = facts();
    input.agents[0].enabled = false;
    observe(&app, &session, input, &maintenance);
    assert!(app.pool(&session).unwrap().eligible.is_empty());
    assert_eq!(
        app.pool(&session).unwrap().excluded[&Id::new("agent-a").unwrap()].code,
        ExclusionCode::Disabled
    );
    let mut input = facts();
    input.discoveries[0].adapter_available = false;
    observe(&app, &session, input, &maintenance);
    assert!(app.pool(&session).unwrap().eligible.is_empty());
}

#[test]
fn capabilities_require_matching_enforcement_and_intersect_all_three_limits() {
    let probe = StaticDependencyProbe::new().unwrap();
    let (app, session, _) = open(Arc::new(MemoryJournal::new()), probe.selection());
    let mut input = facts();
    input.discoveries[0].provider.capabilities = Some(BTreeSet::from([
        Capability::ReadFiles,
        Capability::RunProcess,
    ]));
    observe(&app, &session, input, &probe);
    let profile = app
        .profile(
            &session,
            &Id::new("agent-a").unwrap(),
            &settings("tiny", Some("low")),
        )
        .unwrap();
    let mut workspace = WorkspaceCapabilities {
        workspace: Id::new("workspace").unwrap(),
        profile: profile.clone(),
        enforced: None,
    };
    assert!(app.capabilities(&session, &profile, &workspace).is_err());
    workspace.enforced = Some(BTreeSet::from([
        Capability::ReadFiles,
        Capability::WriteFiles,
    ]));
    assert_eq!(
        app.capabilities(&session, &profile, &workspace).unwrap(),
        BTreeSet::from([Capability::ReadFiles])
    );
    workspace.profile.effort = Some("high".into());
    assert!(app.capabilities(&session, &profile, &workspace).is_err());
}

#[test]
fn constraint_changes_make_the_observation_stale_and_pins_are_enforced_after_refresh() {
    let probe = StaticDependencyProbe::new().unwrap();
    let (app, session, owner) = open(Arc::new(MemoryJournal::new()), probe.selection());
    observe(&app, &session, facts(), &probe);
    let initial = app.view(&session, None).unwrap();
    let mut constraints = initial.task().unwrap().constraints.clone();
    constraints.pins.models = Some(BTreeSet::from(["tiny".into()]));
    constraints.pins.efforts = Some(BTreeSet::from(["low".into()]));
    constraints.allowed = BTreeSet::from([Capability::ReadFiles]);
    app.refine(
        &owner,
        IntakeRefinement {
            expected_revision: 3,
            at: 21,
            constraints,
            criteria: initial.criteria().to_vec(),
            reason: "Use the owner's selected economical model and effort".into(),
            note: IntakeNote::Clarification(Clarification {
                question: "Which profile?".into(),
                answer: "tiny low".into(),
                at: 21,
            }),
        },
    )
    .unwrap();
    assert_eq!(app.pool(&session).unwrap_err().code, "registry_stale");
    observe(&app, &session, facts(), &probe);
    let agent = Id::new("agent-a").unwrap();
    assert!(
        app.profile(&session, &agent, &settings("large", Some("high")))
            .is_err()
    );
    let profile = app
        .profile(&session, &agent, &settings("tiny", Some("low")))
        .unwrap();
    let scope = WorkspaceCapabilities {
        workspace: Id::new("workspace").unwrap(),
        profile: profile.clone(),
        enforced: Some(capabilities()),
    };
    assert_eq!(
        app.capabilities(&session, &profile, &scope).unwrap(),
        BTreeSet::from([Capability::ReadFiles])
    );
    assert_eq!(app.view(&session, Some(3)).unwrap(), initial);
}

#[test]
fn invalid_inputs_and_forged_pool_results_are_rejected_atomically() {
    let probe = StaticDependencyProbe::new().unwrap();
    let journal = Arc::new(MemoryJournal::new());
    let (app, session, _) = open(journal.clone(), probe.selection());
    let initial = journal.read(&session).unwrap();
    let mut invalid = facts();
    invalid.agents.push(invalid.agents[0].clone());
    assert!(app.registry_input(&session, invalid, 20).is_err());
    assert_eq!(journal.read(&session).unwrap(), initial);
    let mut data = facts();
    data.agents[0].enabled = false;
    observe(&app, &session, data, &probe);
    let mut forged = journal.read(&session).unwrap().events[2].clone();
    if let ymp_kernel::events::Event::PoolRecorded { data, .. } = &mut forged.payload {
        data.outcome.eligible.insert(Id::new("agent-a").unwrap());
    }
    let target = MemoryJournal::new();
    target.append(&session, 0, &initial.events).unwrap();
    assert!(target.append(&session, 2, &[forged]).is_err());
    assert_eq!(target.read(&session).unwrap(), initial);
}

#[test]
fn static_capture_observes_files_without_running_them() {
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };
    let root = std::env::temp_dir().join(format!(
        "ymp-registry-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&root).unwrap();
    struct Cleanup(std::path::PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let _cleanup = Cleanup(root.clone());
    let program = root.join("provider");
    let marker = root.join("SHOULD_NOT_EXIST");
    let quoted = marker.to_str().unwrap().replace('\'', "'\\''");
    fs::write(&program, format!("#!/bin/sh\n: > '{}'\n", quoted)).unwrap();
    let provider = Id::new("provider").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::{PermissionsExt, symlink};
        fs::set_permissions(&program, fs::Permissions::from_mode(0o600)).unwrap();
        assert_eq!(
            StaticDependencyProbe::capture(
                provider.clone(),
                None,
                &program,
                DependencyKind::Executable,
                20
            )
            .unwrap()
            .state,
            DependencyState::NotExecutable
        );
        fs::set_permissions(&program, fs::Permissions::from_mode(0o700)).unwrap();
        assert_eq!(
            StaticDependencyProbe::capture(
                provider.clone(),
                None,
                &program,
                DependencyKind::Executable,
                20
            )
            .unwrap()
            .state,
            DependencyState::Available
        );
        symlink("loop", root.join("loop")).unwrap();
        assert_eq!(
            StaticDependencyProbe::capture(
                provider.clone(),
                None,
                &root.join("loop"),
                DependencyKind::File,
                20
            )
            .unwrap()
            .state,
            DependencyState::Unreadable
        );
    }
    assert_eq!(
        StaticDependencyProbe::capture(
            provider.clone(),
            None,
            &root,
            DependencyKind::Executable,
            20
        )
        .unwrap()
        .state,
        DependencyState::WrongKind
    );
    assert_eq!(
        StaticDependencyProbe::capture(
            provider.clone(),
            None,
            &root.join("missing"),
            DependencyKind::File,
            20
        )
        .unwrap()
        .state,
        DependencyState::Missing
    );
    assert!(!root.join("SHOULD_NOT_EXIST").exists());
    let probe = StaticDependencyProbe::new().unwrap();
    let (app, session, _) = open(Arc::new(MemoryJournal::new()), probe.selection());
    let mut data = facts();
    data.dependencies.push(
        StaticDependencyProbe::capture(
            provider,
            None,
            &root.join("missing"),
            DependencyKind::File,
            20,
        )
        .unwrap(),
    );
    observe(&app, &session, data, &probe);
    assert!(app.pool(&session).unwrap().eligible.is_empty());
}

#[test]
fn fixture_source_cannot_authorize_native_readiness_and_native_needs_executable_facts() {
    let probe = StaticDependencyProbe::new().unwrap();
    let journal = Arc::new(MemoryJournal::new());
    let (app, session, _) = open(journal.clone(), probe.selection());
    let before = journal.read(&session).unwrap();
    for kind in [
        ProviderKind::Codex,
        ProviderKind::Claude,
        ProviderKind::Glm,
        ProviderKind::Other("custom".into()),
    ] {
        let mut data = facts();
        data.discoveries[0].provider.kind = kind;
        assert!(app.registry_input(&session, data.clone(), 20).is_err());
        assert_eq!(journal.read(&session).unwrap(), before);
        data.discoveries[0].source = DiscoverySource::Native;
        let input = app.registry_input(&session, data, 20).unwrap();
        let responses: Vec<_> = readiness_views(&input)
            .iter()
            .map(|v| readiness_response(&probe, v).unwrap())
            .collect();
        let outcome =
            ymp_kernel::registry::evaluate(&input, probe.selection(), &responses).unwrap();
        assert!(outcome.outcome.eligible.is_empty());
        assert!(
            outcome
                .outcome
                .excluded
                .values()
                .all(|e| e.code == ExclusionCode::MissingDependency)
        );
    }
}

#[test]
fn reordered_responses_keep_their_profile_and_forged_bindings_are_denied() {
    let mut schemas = ParameterSchemas::default();
    schemas
        .register("ReadinessProbe", "MaintenanceTest", "1", |selection| {
            let _: MaintenanceParameters = decode(&encode(&selection.parameters)?)?;
            Ok(())
        })
        .unwrap();
    let probe = Maintenance {
        selection: PolicySelection::new(
            "ReadinessProbe",
            "MaintenanceTest",
            "1",
            serde_json::json!({"suspended_agents":["agent-b"]}),
        )
        .unwrap(),
    };
    let journal = Arc::new(MemoryJournal::with_schemas(schemas));
    let (app, session, _) = open(journal.clone(), probe.selection());
    let input = app.registry_input(&session, facts(), 20).unwrap();
    let mut responses: Vec<_> = readiness_views(&input)
        .iter()
        .map(|v| readiness_response(&probe, v).unwrap())
        .collect();
    responses.reverse();
    app.record_pool(&session, 2, 20, input, probe.selection().clone(), responses)
        .unwrap();
    assert_eq!(
        app.pool(&session).unwrap().eligible,
        BTreeSet::from([Id::new("agent-a").unwrap()])
    );
    let before = journal.read(&session).unwrap();
    let input = app.registry_input(&session, facts(), 20).unwrap();
    let responses: Vec<_> = readiness_views(&input)
        .iter()
        .map(|v| readiness_response(&probe, v).unwrap())
        .collect();
    for case in 0..4 {
        let mut bad = responses.clone();
        match case {
            0 => bad[0].input = Digest::of(b"unrelated input"),
            1 => bad[0] = bad[1].clone(),
            2 => {
                bad.pop();
            }
            3 => bad[0].profile.agent = Id::new("foreign-agent").unwrap(),
            _ => unreachable!(),
        }
        assert!(
            app.record_pool(
                &session,
                3,
                20,
                input.clone(),
                probe.selection().clone(),
                bad
            )
            .is_err()
        );
        assert_eq!(journal.read(&session).unwrap(), before);
    }
}

#[test]
fn requested_sent_and_reported_settings_are_independent_observations() {
    let mut invocation = InvocationSettings::requested(settings("tiny", Some("low"))).unwrap();
    assert_eq!(invocation.sent, ProfileSettings::default());
    assert_eq!(invocation.reported, ProfileSettings::default());
    invocation.sent = settings("tiny", Some("low"));
    invocation.validate().unwrap();
    let replay: InvocationSettings = decode(&encode(&invocation).unwrap()).unwrap();
    assert_eq!(replay.reported, ProfileSettings::default());
    assert_eq!(replay.requested, settings("tiny", Some("low")));
    invocation.reported = settings("tiny", None);
    assert_eq!(invocation.reported.effort, None);
    assert_eq!(invocation.sent.effort.as_deref(), Some("low"));
    let probe = StaticDependencyProbe::new().unwrap();
    let (app, session, _) = open(Arc::new(MemoryJournal::new()), probe.selection());
    let mut data = facts();
    data.discoveries[0].provider.version = None;
    data.discoveries[0].offerings[0].efforts = None;
    observe(&app, &session, data, &probe);
    let profile = app
        .profile(
            &session,
            &Id::new("agent-a").unwrap(),
            &settings("tiny", None),
        )
        .unwrap();
    assert_eq!(profile.provider_version, None);
    assert_eq!(profile.family, None);
    assert_eq!(profile.effort, None);
    assert!(
        app.profile(&session, &profile.agent, &invocation.requested)
            .is_err()
    );
    assert_eq!(invocation.reported, settings("tiny", None));
}
