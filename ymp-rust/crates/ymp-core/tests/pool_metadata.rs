use serde_json::json;
use ymp_core::*;

fn catalog() -> ProviderCapabilities {
    serde_json::from_value(json!({
        "models_complete": true,
        "models": [{"id": "fixture-model", "controls": [
            {"id": "thought_mode", "values": {"kind": "choices", "options": ["off", "adaptive"]}, "default": "off"},
            {"id": "thinking_enabled", "values": {"kind": "boolean"}, "default": true},
            {"id": "thinking_allowance", "values": {"kind": "integer", "min": 128, "max": 1024}, "default": 256}
        ]}],
        "default_model": "fixture-model"
    }))
    .unwrap()
}

#[test]
fn legacy_profiles_round_trip_without_renaming_or_inventing_capabilities() {
    let temp = tempfile::tempdir().unwrap();
    let mut config = Config::default();
    config.agents[0].id = "personal-agent".into();
    config.agents[0].name = "My existing name".into();
    config.team[0] = "personal-agent".into();
    let mut second = config.agents[0].clone();
    second.id = "another-agent".into();
    config.agents.push(second);
    config.save(temp.path()).unwrap();
    let text = std::fs::read_to_string(temp.path().join("config.toml")).unwrap();
    assert!(!text.contains("capabilities"));
    let loaded = Config::load(temp.path()).unwrap();
    assert!(loaded.capabilities.is_empty());
    assert_eq!(loaded.agents[0].id, "personal-agent");
    assert_eq!(loaded.agents[0].name, "My existing name");
    assert!(loaded.agents[0].model.is_none());
    assert_eq!(loaded.agents[3].provider, loaded.agents[0].provider);
    assert_ne!(
        loaded.agents[3].version(&loaded.providers[0]),
        loaded.agents[0].version(&loaded.providers[0])
    );
    config.agents.push(config.agents[0].clone());
    assert!(
        config.validate().is_err(),
        "duplicate agent IDs were accepted"
    );
}

#[test]
fn configured_native_controls_round_trip_with_exact_types_and_unknowns() {
    let temp = tempfile::tempdir().unwrap();
    let mut config = Config::default();
    let version = config.agents[0].version(&config.providers[0]);
    let mut capabilities = catalog();
    capabilities.models.push(ModelCapabilities {
        picker_id: None,
        display_name: None,
        aliases: vec![],
        resolved_model: None,
        id: "controls-unknown".into(),
        controls: None,
    });
    capabilities.models.push(ModelCapabilities {
        picker_id: None,
        display_name: None,
        aliases: vec![],
        resolved_model: None,
        id: "no-controls".into(),
        controls: Some(vec![]),
    });
    config.capabilities.insert("codex".into(), capabilities);
    config.save(temp.path()).unwrap();
    let loaded = Config::load(temp.path()).unwrap();
    let capabilities = &loaded.capabilities["codex"];
    assert_eq!(capabilities, &config.capabilities["codex"]);
    assert_eq!(capabilities.source, CapabilitySource::Configured);
    assert!(capabilities.models[1].controls.is_none());
    assert_eq!(capabilities.models[2].controls, Some(vec![]));
    assert_eq!(loaded.agents[0].version(&loaded.providers[0]), version);
    let controls = capabilities.models[0].controls.as_ref().unwrap();
    assert!(controls[0]
        .values
        .contains(&NativeControlValue::Choice("adaptive".into())));
    assert!(!controls[0]
        .values
        .contains(&NativeControlValue::Choice("max".into())));
    assert!(!controls[1]
        .values
        .contains(&NativeControlValue::Choice("true".into())));
    assert!(!controls[2]
        .values
        .contains(&NativeControlValue::Integer(1025)));
}

#[test]
fn invalid_capability_claims_fail_before_pool_inspection_or_invocation() {
    let valid = serde_json::to_value(catalog()).unwrap();
    let invalid_controls = [
        json!({"id":"x", "values":{"kind":"choices", "options":[]}}),
        json!({"id":"x", "values":{"kind":"choices", "options":["on", "on"]}}),
        json!({"id":"x", "values":{"kind":"choices", "options":["on"]}, "default":"max"}),
        json!({"id":"x", "values":{"kind":"boolean"}, "default":"true"}),
        json!({"id":"x", "values":{"kind":"integer", "min":10, "max":2}}),
        json!({"id":"", "values":{"kind":"boolean"}}),
    ];
    for control in invalid_controls {
        let mut value = valid.clone();
        value["models"][0]["controls"] = json!([control]);
        let capability: ProviderCapabilities = serde_json::from_value(value).unwrap();
        assert!(capability.validate().is_err(), "accepted {capability:?}");
    }
    let mut duplicate = catalog();
    duplicate.models.push(duplicate.models[0].clone());
    assert!(duplicate.validate().is_err());
    let mut missing_default = catalog();
    missing_default.default_model = Some("unlisted-model".into());
    assert!(missing_default.validate().is_err());
    let mut config = Config::default();
    config
        .capabilities
        .insert("unknown-provider".into(), catalog());
    assert!(config.validate().is_err());
}

fn captured_session() -> Session {
    Session {
        id: "captured-session".into(),
        project_id: "project".into(),
        title: "Identity history".into(),
        status: "completed".into(),
        created_at: now(),
        team: Config::default().members(),
        turns_used: 4,
    }
}

#[test]
fn captured_names_survive_profile_edits_and_do_not_imply_membership_or_activity() {
    let session = captured_session();
    let mut current_config = Config::default();
    current_config.agents[0].name = "Renamed now".into();
    current_config.agents.remove(1);
    current_config.team.clear();
    current_config.validate().unwrap();
    let unknown = SessionAgentView::from_captured(&session, None, None).unwrap();
    assert_eq!(unknown.captured_name("codex"), Some("Codex"));
    assert_eq!(unknown.captured_name("claude"), Some("Claude"));
    assert_eq!(unknown.captured_name("unrecorded"), None);
    assert!(unknown.current_members.is_none());
    assert!(unknown.active_invocations.is_none());
    let view =
        SessionAgentView::from_captured(&session, Some(&["codex".into()]), Some(&[])).unwrap();
    assert_eq!(view.captured_participants.len(), 2);
    assert_eq!(view.current_members.as_ref().unwrap().len(), 1);
    assert!(view.active_invocations.as_ref().unwrap().is_empty());
    let restored: SessionAgentView =
        serde_json::from_str(&serde_json::to_string(&view).unwrap()).unwrap();
    assert_eq!(restored.captured_name("claude"), Some("Claude"));
}

#[test]
fn live_activity_requires_bound_distinct_invocations_for_current_members() {
    let session = captured_session();
    let current = ["codex".into()];
    let active = AgentInvocationRef {
        invocation_id: "invocation-one".into(),
        session_id: session.id.clone(),
        agent_id: "codex".into(),
    };
    assert!(SessionAgentView::from_captured(
        &session,
        Some(&current),
        Some(std::slice::from_ref(&active))
    )
    .is_ok());
    for (session_id, agent_id) in [
        ("foreign-session", "codex"),
        ("captured-session", "claude"),
        ("captured-session", "unknown"),
    ] {
        let wrong = AgentInvocationRef {
            session_id: session_id.into(),
            agent_id: agent_id.into(),
            ..active.clone()
        };
        assert!(SessionAgentView::from_captured(&session, Some(&current), Some(&[wrong])).is_err());
    }
    assert!(SessionAgentView::from_captured(
        &session,
        Some(&current),
        Some(&[active.clone(), active])
    )
    .is_err());
    assert!(SessionAgentView::from_captured(&session, Some(&["unknown".into()]), None).is_err());
}
