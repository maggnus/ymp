//! Executable discovery and selection with native-protocol fixtures only.
use serde_json::{json, Value};
use std::{
    path::PathBuf,
    process::{Command, Output},
};
use ymp_core::*;
use ymp_storage::Store;

struct Fixture {
    _temp: tempfile::TempDir,
    home: PathBuf,
    cwd: PathBuf,
    spec: PathBuf,
    wire: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let home = temp.path().join("home");
        let cwd = temp.path().join("project");
        std::fs::create_dir_all(&cwd).unwrap();
        std::fs::create_dir_all(&home).unwrap();
        let spec = home.join("fixture-spec.json");
        let wire = home.join("wire.jsonl");
        std::fs::write(&spec, "{}").unwrap();
        let mut config = Config::default();
        config.providers.retain(|p| p.id != "claude");
        config.agents.retain(|a| a.provider != "claude");
        config.team = vec!["codex".into(), "glm".into()];
        config.limits.turns = 4;
        config.limits.attempts = 1;
        config.limits.turn_timeout_secs = 5;
        for provider in &mut config.providers {
            provider.enabled = true;
            provider.command = "python3".into();
            provider.args = vec![
                format!("{}/tests/fixtures/catalog.py", env!("CARGO_MANIFEST_DIR")),
                if provider.id == "codex" {
                    "codex"
                } else {
                    "acp"
                }
                .into(),
                spec.to_string_lossy().into(),
                wire.to_string_lossy().into(),
            ];
        }
        for agent in &mut config.agents {
            agent.enabled = true;
        }
        config.save(&home).unwrap();
        Self {
            _temp: temp,
            home,
            cwd,
            spec,
            wire,
        }
    }
    fn command(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_ymp"))
            .arg("--home")
            .arg(&self.home)
            .arg("--cwd")
            .arg(&self.cwd)
            .args(args)
            .output()
            .unwrap()
    }
    fn json(&self, args: &[&str]) -> Value {
        let output = self.command(args);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice(&output.stdout).unwrap()
    }
    fn scan(&self) -> Value {
        self.json(&["catalog", "--refresh", "--timeout-secs", "5"])
    }
    fn wire(&self) -> Vec<Value> {
        std::fs::read_to_string(&self.wire)
            .unwrap_or_default()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect()
    }
    fn variant(&self, value: Value) {
        std::fs::write(&self.spec, serde_json::to_vec(&value).unwrap()).unwrap();
    }
}

#[test]
fn executable_scan_populates_native_names_and_selection_without_enrolling_the_catalog() {
    let f = Fixture::new();
    let before = f.json(&["catalog"]);
    let unconfigured = Config::load(&f.home).unwrap();
    let unknown_agent = unconfigured.agent("codex").unwrap();
    let unknown_provider = unconfigured.provider("codex").unwrap();
    let unknown_version = execution_config_version(
        unknown_agent,
        unknown_provider,
        &ExecutionSettings::default(),
        &ExecutionSettings::default(),
        None,
    );
    assert_eq!(
        before["pool"]["agents"][0]["identity"]["status"],
        "unresolved"
    );
    assert!(before["pool"]["agents"][0]["exclusions"]
        .as_array()
        .unwrap()
        .contains(&json!("native_model_unresolved")));
    assert!(
        f.wire().is_empty(),
        "ordinary pool inspection launched a provider"
    );
    // Positive control proves the wire capture detects a released model prompt.
    assert!(f
        .command(&[
            "ask",
            "codex",
            "fixture-only",
            "--model",
            "wire-a",
            "--effort",
            "quiet"
        ])
        .status
        .success());
    assert!(f.wire().iter().any(|r| r["method"] == "turn/start"));
    std::fs::remove_file(&f.wire).unwrap();

    let value = f.scan();
    let agents = value["pool"]["agents"].as_array().unwrap();
    assert_eq!(agents.len(), 4);
    assert_eq!(agents[0]["identity"]["name"], "Orchid · native A");
    assert_eq!(agents[0]["identity"]["model"], "wire-a");
    assert_eq!(agents[0]["identity"]["effort"], Value::Null);
    assert_eq!(agents[0]["identity"]["status"], "native");
    assert_eq!(agents[0]["profile"]["id"], "codex");
    assert_eq!(agents[0]["profile"]["name"], "Orchid · native A");
    let codex = &value["pool"]["capabilities"]["codex"];
    assert_eq!(codex["models"][0]["picker_id"], "picker-wire-a");
    assert_eq!(codex["models"][1]["display_name"], "Quartz / native B");
    assert_eq!(codex["source"]["method"], "model/list");
    let acp = &value["pool"]["capabilities"]["glm"]["models"];
    assert_eq!(acp[0]["controls"][0]["display_name"], "Native thought mode");
    assert_eq!(
        acp[0]["controls"][0]["value_names"]["max"],
        "Native maximum"
    );
    assert_eq!(acp[0]["controls"][1]["id"], "native_tempo");
    assert!(acp[1].get("controls").is_none());
    assert!(f.wire().iter().all(
        |r| ["initialize", "initialized", "model/list", "session/new"]
            .contains(&r["method"].as_str().unwrap())
    ));
    let config = Config::load(&f.home).unwrap();
    assert_eq!(config.team, vec!["codex", "glm"]);
    assert!(config.agents.iter().all(|a| a.model.is_some()));
    let concrete_agent = config.agent("codex").unwrap();
    let concrete_settings = config
        .execution_settings(concrete_agent, &Default::default())
        .unwrap();
    assert_ne!(
        concrete_agent.version(config.provider("codex").unwrap()),
        unknown_agent.version(unknown_provider)
    );
    assert_ne!(
        execution_config_version(
            concrete_agent,
            config.provider("codex").unwrap(),
            &concrete_settings,
            &ExecutionSettings::default(),
            None
        ),
        unknown_version,
        "genuinely unknown-to-concrete execution must retain a distinct version"
    );
    assert_eq!(
        config
            .agents
            .iter()
            .filter(|a| a.provider == "codex")
            .count(),
        2
    );
    let selected = config
        .agents
        .iter()
        .find(|a| a.provider == "codex" && a.model.as_deref() == Some("wire-b"))
        .unwrap();
    assert!(f
        .command(&["ask", &selected.id, "fixture-only", "--effort", "quiet"])
        .status
        .success());
    assert!(f
        .wire()
        .iter()
        .any(|r| r["method"] == "turn/start" && r["params"]["model"] == "wire-b"));
    let bytes = std::fs::read(&f.wire).unwrap();
    let _ = f.json(&["catalog"]);
    assert_eq!(std::fs::read(&f.wire).unwrap(), bytes);
}

#[test]
fn repeat_change_and_partial_failure_preserve_actor_pins_custom_profiles_and_history() {
    let f = Fixture::new();
    let mut config = Config::load(&f.home).unwrap();
    config.agents[0].instructions = "Owner instructions survive migration".into();
    let mut custom = config.agents[0].clone();
    custom.id = "my-second-actor".into();
    custom.name = "Owner's personal alias".into();
    custom.model = Some("wire-a".into());
    config.agents.push(custom.clone());
    config.execution.insert(
        "codex".into(),
        AgentExecutionPolicy {
            fixed: ModelEffort {
                model: Some("wire-a".into()),
                effort: Some("quiet".into()),
            },
            ..Default::default()
        },
    );
    config.team_constraints.fixed_roster = Some(vec!["codex".into(), "glm".into()]);
    config.save(&f.home).unwrap();
    let store = Store::open(&f.home).unwrap();
    let project = store.project(&f.cwd).unwrap();
    let session = Session {
        id: new_id(),
        project_id: project.id,
        title: "Historical capture".into(),
        status: "completed".into(),
        created_at: now(),
        team: config.agents.clone(),
        turns_used: 0,
    };
    store.save_session(&session).unwrap();
    let historical = serde_json::to_value(store.session(&session.id).unwrap()).unwrap();
    let initial = f.scan();
    let config = Config::load(&f.home).unwrap();
    let original = serde_json::to_value(&config).unwrap();
    let version = config
        .agent("codex")
        .unwrap()
        .version(config.provider("codex").unwrap());
    assert_eq!(config.agent("my-second-actor").unwrap(), &custom);
    let repeated = f.scan();
    assert_eq!(repeated["scan"]["created_agents"], json!([]));
    assert_eq!(repeated["scan"]["migrated_agents"], json!([]));
    assert_eq!(
        serde_json::to_value(Config::load(&f.home).unwrap()).unwrap(),
        original
    );
    f.variant(json!({"codex":"changed", "acp":"fail"}));
    let changed = f.scan();
    assert_eq!(
        changed["pool"]["agents"][0]["identity"]["name"],
        "Renamed by native provider"
    );
    assert_eq!(changed["pool"]["agents"][1]["identity"]["status"], "stale");
    assert_eq!(changed["scan"]["providers"][1]["retained_previous"], true);
    let old_b = initial["pool"]["agents"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["profile"]["provider"] == "codex" && a["profile"]["model"] == "wire-b")
        .unwrap()["profile"]["id"]
        .as_str()
        .unwrap();
    let old = changed["pool"]["agents"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["profile"]["id"] == old_b)
        .unwrap();
    assert_eq!(old["model_status"], "unlisted");
    let config = Config::load(&f.home).unwrap();
    assert_eq!(
        version,
        config
            .agent("codex")
            .unwrap()
            .version(config.provider("codex").unwrap())
    );
    assert_eq!(
        config.execution["codex"].fixed.effort.as_deref(),
        Some("quiet")
    );
    assert_eq!(
        config.team_constraints.fixed_roster.as_ref().unwrap(),
        &vec!["codex", "glm"]
    );
    assert_eq!(
        config.agent("codex").unwrap().instructions,
        "Owner instructions survive migration"
    );
    assert_eq!(config.agent("my-second-actor").unwrap(), &custom);
    assert_eq!(
        serde_json::to_value(store.session(&session.id).unwrap()).unwrap(),
        historical
    );
    let cache = std::fs::read_to_string(f.home.join(NATIVE_CATALOG_FILE)).unwrap();
    assert!(!cache.contains("SYNTHETIC_SECRET_DIAGNOSTIC"));
}

#[test]
fn failed_initial_and_bounded_scans_leave_explicit_unresolved_state() {
    let f = Fixture::new();
    f.variant(json!({"codex":"fail", "acp":"hang"}));
    let began = std::time::Instant::now();
    let value = f.json(&["catalog", "--refresh", "--timeout-secs", "1"]);
    assert!(began.elapsed().as_secs() < 6);
    assert!(value["scan"]["providers"]
        .as_array()
        .unwrap()
        .iter()
        .all(|p| p["status"] == "failed"));
    assert!(value["pool"]["agents"]
        .as_array()
        .unwrap()
        .iter()
        .all(|a| a["identity"]["status"] == "unresolved"));
    let before = std::fs::read(&f.wire).unwrap();
    assert!(!f
        .command(&["catalog", "--refresh", "--timeout-secs", "0"])
        .status
        .success());
    assert_eq!(std::fs::read(&f.wire).unwrap(), before);
}

#[test]
fn effective_overrides_use_native_aliases_and_unknown_claims_never_become_native() {
    let f = Fixture::new();
    f.scan();
    let mut config = Config::load(&f.home).unwrap();
    let agent = config.agent("codex").unwrap();
    {
        let id = "wire-b";
        let settings = ExecutionSettings {
            model: Some(id.into()),
            effort: Some("future-control".into()),
            permission_mode: None,
        };
        let identity = config.agent_identity(agent, &settings);
        assert_eq!(identity.name, "Quartz / native B");
        assert_eq!(identity.model.as_deref(), Some(id));
        assert_eq!(identity.effort.as_deref(), Some("future-control"));
    }
    let picker = ExecutionSettings {
        model: Some("picker-wire-b".into()),
        ..Default::default()
    };
    assert_eq!(
        config.agent_identity(agent, &picker).status,
        AgentIdentityStatus::Unknown
    );
    assert!(!f
        .command(&["ask", "codex", "fixture-only", "--model", "picker-wire-b"])
        .status
        .success());
    let settings = ExecutionSettings {
        model: Some("user-unscanned-model".into()),
        ..Default::default()
    };
    assert_eq!(
        config.agent_identity(agent, &settings).status,
        AgentIdentityStatus::Unknown
    );
    let native = ExecutionSettings {
        model: Some("wire-a".into()),
        ..Default::default()
    };
    let agent = config.agent("codex").unwrap().clone();
    config
        .native_catalog
        .providers
        .get_mut("codex")
        .unwrap()
        .catalog
        .as_mut()
        .unwrap()
        .source = CapabilitySource::NativeMetadata {
        method: "model/list".into(),
        observed_at: "2020-01-01T00:00:00Z".into(),
    };
    assert_eq!(
        config.agent_identity(&agent, &native).status,
        AgentIdentityStatus::Stale
    );
    config
        .providers
        .iter_mut()
        .find(|p| p.id == "codex")
        .unwrap()
        .args
        .push("different-native-installation".into());
    assert_eq!(
        config.agent_identity(&agent, &native).status,
        AgentIdentityStatus::Unknown
    );
    assert!(config.agent_identity(&agent, &native).source.is_none());
    config.native_catalog = Default::default();
    config.capabilities.insert("codex".into(), serde_json::from_value(json!({"source":{"kind":"native_metadata","method":"FORGED TOML CLAIM","observed_at":now()},"models":[{"id":"wire-a","display_name":"Forged native name"}]})).unwrap());
    config.save(&f.home).unwrap();
    std::fs::remove_file(f.home.join(NATIVE_CATALOG_FILE)).unwrap();
    let value = f.json(&["catalog"]);
    assert_eq!(
        value["pool"]["capabilities"]["codex"]["source"]["kind"],
        "configured"
    );
    assert_eq!(value["pool"]["agents"][0]["identity"]["status"], "unknown");
    assert_eq!(value["pool"]["agents"][0]["identity"]["name"], "wire-a");
}

#[test]
fn executable_runtime_captures_the_effective_native_assignment_identity() {
    let f = Fixture::new();
    f.scan();
    let mut config = Config::load(&f.home).unwrap();
    config
        .providers
        .iter_mut()
        .find(|p| p.id == "glm")
        .unwrap()
        .enabled = false;
    config.team = config
        .agents
        .iter()
        .filter(|a| a.provider == "codex")
        .map(|a| a.id.clone())
        .collect();
    config.save(&f.home).unwrap();
    let rules = f.home.join("assignment-settings.json");
    std::fs::write(&rules, serde_json::to_vec(&json!([{ "agent_id":"codex", "purpose":"plan", "settings":{"model":"wire-b", "effort":"quiet"}}])).unwrap()).unwrap();
    // The fixture deliberately gives no valid plan. Admission/capture precede
    // that failure; no success or real provider inference is being asserted.
    let _ = f.command(&[
        "run",
        "Fixture native assignment capture",
        "--json",
        "--no-memory",
        "--no-adaptive",
        "--assignment-settings",
        rules.to_str().unwrap(),
    ]);
    let store = Store::open(&f.home).unwrap();
    let session = store.sessions(None).unwrap().remove(0);
    let trace = store.trace(&session.id).unwrap();
    let assignment = trace
        .assignments
        .iter()
        .find(|a| a.agent_id == "codex")
        .expect("fixture reached real assignment admission");
    assert_eq!(assignment.requested.model.as_deref(), Some("wire-b"));
    assert_eq!(
        assignment.agent_identity.as_ref().unwrap().name,
        "Quartz / native B"
    );
    assert_eq!(
        assignment
            .agent_identity
            .as_ref()
            .unwrap()
            .effort
            .as_deref(),
        Some("quiet")
    );
    assert_eq!(
        session.team.iter().find(|a| a.id == "codex").unwrap().name,
        "Orchid · native A"
    );
}

#[test]
#[ignore = "requires npm ci and npm run build in ymp-bridges/claude; still fixture-only"]
fn executable_claude_sdk_scan_preserves_native_alias_names_and_selects_offerings() {
    let f = Fixture::new();
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .unwrap()
        .to_path_buf();
    let bridge = root.join("ymp-bridges/claude/dist/index.js");
    assert!(
        bridge.is_file(),
        "build the Claude bridge before this explicit fixture check"
    );
    let mut config = Config::default();
    config.providers.retain(|p| p.id == "claude");
    config.agents.retain(|a| a.id == "claude");
    config.team = vec!["claude".into()];
    config.providers[0].command = root
        .join("ymp-bridges/claude/tests/fixtures/claude.py")
        .to_string_lossy()
        .into();
    config.save(&f.home).unwrap();
    let command = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_ymp"))
            .arg("--home")
            .arg(&f.home)
            .arg("--cwd")
            .arg(&f.home)
            .env("YMP_CLAUDE_BRIDGE", &bridge)
            .args(args)
            .output()
            .unwrap()
    };
    let scan = command(&["catalog", "--refresh", "--timeout-secs", "5"]);
    assert!(
        scan.status.success(),
        "{}",
        String::from_utf8_lossy(&scan.stderr)
    );
    let value: Value = serde_json::from_slice(&scan.stdout).unwrap();
    assert_eq!(value["scan"]["providers"][0]["status"], "updated");
    assert_eq!(value["pool"]["agents"][0]["identity"]["name"], "Fixture");
    assert_eq!(value["pool"]["agents"][0]["identity"]["model"], "opus[1m]");
    assert_eq!(
        value["pool"]["agents"][0]["identity"]["resolved_model"],
        "claude-opus-5[1m]"
    );
    assert_eq!(value["pool"]["agents"].as_array().unwrap().len(), 2);
    let wire = std::fs::read_to_string(f.home.join("native.jsonl")).unwrap();
    assert!(!wire
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .any(|q| q["type"] == "user"));
    let config = Config::load(&f.home).unwrap();
    let resolved = ExecutionSettings {
        model: Some("claude-opus-5[1m]".into()),
        ..Default::default()
    };
    assert_eq!(
        config.agent_identity(&config.agents[0], &resolved).name,
        "Fixture"
    );
    let small = config
        .agents
        .iter()
        .find(|a| a.model.as_deref() == Some("small"))
        .unwrap();
    assert!(command(&["ask", &small.id, "offline fixture"])
        .status
        .success());
    let wire = std::fs::read_to_string(f.home.join("native.jsonl")).unwrap();
    assert!(wire
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .any(|q| q["type"] == "user"));
}

#[test]
fn init_upgrades_existing_generated_placeholders_and_keeps_disabled_custom_actors() {
    let f = Fixture::new();
    let mut config = Config::load(&f.home).unwrap();
    let mut custom = config.agents[0].clone();
    custom.id = "disabled-custom".into();
    custom.name = "My alias".into();
    custom.model = Some("wire-b".into());
    custom.enabled = false;
    config.agents.push(custom.clone());
    config.save(&f.home).unwrap();
    let result = f.command(&["init"]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let config = Config::load(&f.home).unwrap();
    assert_eq!(
        config.agent("codex").unwrap().model.as_deref(),
        Some("wire-a")
    );
    assert_eq!(config.agent("disabled-custom").unwrap(), &custom);
    assert_eq!(config.team, vec!["codex", "glm"]);
    assert!(f
        .wire()
        .iter()
        .all(|r| r["method"] != "turn/start" && r["method"] != "session/prompt"));
}

#[test]
fn simultaneous_scans_are_serialized_and_edits_during_discovery_are_preserved() {
    let f = Fixture::new();
    f.variant(json!({"codex":"hang"}));
    let mut first = Command::new(env!("CARGO_BIN_EXE_ymp"))
        .arg("--home")
        .arg(&f.home)
        .arg("--cwd")
        .arg(&f.cwd)
        .args([
            "catalog",
            "--refresh",
            "--provider",
            "codex",
            "--timeout-secs",
            "1",
        ])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    while f.wire().is_empty() && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(
        !f.wire().is_empty(),
        "first scan reached fixture native metadata"
    );
    let second = f.command(&[
        "catalog",
        "--refresh",
        "--provider",
        "codex",
        "--timeout-secs",
        "1",
    ]);
    assert!(!second.status.success());
    assert!(String::from_utf8_lossy(&second.stderr).contains("already running"));
    let mut config = Config::load(&f.home).unwrap();
    config.agents[0].instructions = "An edit made while metadata is being scanned".into();
    config.save(&f.home).unwrap();
    assert!(!first.wait().unwrap().success());
    assert_eq!(
        Config::load(&f.home).unwrap().agents[0].instructions,
        "An edit made while metadata is being scanned"
    );
    assert!(!f.home.join(NATIVE_CATALOG_FILE).exists());
    assert_eq!(
        f.wire().len(),
        1,
        "second scan never launched a native provider"
    );
}

/// Seed real qualified runtime history without invoking an installed provider.
/// The original configured actor/provider identity remains bound at admission;
/// only this injected fixture backend executes through the deterministic mock.
struct QualifiedHistoryBackend;
impl ymp_providers::ExecutionBackend for QualifiedHistoryBackend {
    fn identity(&self) -> ExecutionBackendIdentity {
        ExecutionBackendIdentity {
            id: "catalog-qualified-history-fixture".into(),
            version: "1".into(),
        }
    }
    fn execute(
        &self,
        mut request: ymp_providers::TurnRequest,
        events: tokio::sync::mpsc::UnboundedSender<ymp_providers::ProviderEvent>,
    ) -> ymp_providers::ExecutionFuture<'_> {
        request.provider.kind = ProviderKind::Mock;
        ymp_providers::NativeExecutionBackend.execute(request, events)
    }
}

fn history_engine(store: &Store, config: Config) -> ymp_runtime::Engine {
    let (events, _) = tokio::sync::mpsc::unbounded_channel();
    let mut engine = ymp_runtime::Engine::new(
        store.clone(),
        config,
        events,
        tokio_util::sync::CancellationToken::new(),
    )
    .unwrap()
    .with_execution_backend(std::sync::Arc::new(QualifiedHistoryBackend))
    .unwrap();
    engine.use_memory = false;
    engine.acceptance_contracts.push(AcceptanceContract {
        knowledge_correction: None,
        task_title: "Create a greeting".into(),
        criteria: vec![AcceptanceCriterion {
            id: "exact-content".into(),
            description: "The artifact contains the requested greeting".into(),
        }],
        artifacts: vec!["greeting.txt".into()],
        inputs: vec![],
        checks: vec![TrustedCheck {
            id: "exact-greeting".into(),
            criterion_ids: vec!["exact-content".into()],
            assertion: CheckAssertion::ExactBytes {
                artifact: "greeting.txt".into(),
                expected: b"Hello from ymp\n".to_vec(),
            },
        }],
    });
    engine
}

async fn first_scan_preserves_qualified_policy_history(fixed: bool) {
    let f = Fixture::new();
    let mut config = Config::load(&f.home).unwrap();
    config.providers.retain(|p| p.id == "codex");
    config.agents.retain(|a| a.id == "codex");
    let mut reviewer = config.agents[0].clone();
    reviewer.id = "reviewer".into();
    reviewer.name = "Independent fixture reviewer".into();
    reviewer.model = Some("wire-a".into());
    config.agents.push(reviewer);
    config.team = vec!["codex".into(), "reviewer".into()];
    config.team_constraints.fixed_roster = Some(config.team.clone());
    config.limits.turns = 40;
    let choice = ModelEffort {
        model: Some("wire-a".into()),
        effort: Some("quiet".into()),
    };
    let policy = if fixed {
        AgentExecutionPolicy {
            fixed: choice.clone(),
            ..Default::default()
        }
    } else {
        AgentExecutionPolicy {
            defaults: choice.clone(),
            ..Default::default()
        }
    };
    config.execution.insert("codex".into(), policy);
    config.execution.insert(
        "reviewer".into(),
        AgentExecutionPolicy {
            fixed: choice,
            ..Default::default()
        },
    );
    config.save(&f.home).unwrap();
    let before_profile = config.agent("codex").unwrap().clone();
    assert!(before_profile.model.is_none());
    let before_settings = config
        .execution_settings(&before_profile, &Default::default())
        .unwrap();
    let before_version = execution_config_version(
        &before_profile,
        config.provider("codex").unwrap(),
        &before_settings,
        &before_settings,
        Some("same-native-version"),
    );
    let store = Store::open(&f.home).unwrap();
    let first = history_engine(&store, config.clone())
        .run(&f.cwd, "Create a greeting", None)
        .await
        .unwrap();
    assert_eq!(first.session.status, "completed", "{}", first.summary);
    let first_trace = store.trace(&first.session.id).unwrap();
    let producer = first_trace
        .assignments
        .iter()
        .find(|a| a.purpose == "execute")
        .unwrap();
    assert_eq!(producer.agent_id, "codex");
    let observed = store.observations().unwrap();
    assert_eq!(observed.len(), 1);
    let observation = &observed[0];
    assert_eq!(observation.confirmation, ConfirmationStatus::Confirmed);
    assert_eq!(
        store
            .reputation(
                &observation.agent_version,
                &observation.competence,
                &observation.difficulty
            )
            .unwrap()
            .successes,
        1
    );
    let history = serde_json::to_value(&first_trace).unwrap();

    // The first refresh goes through the public executable/native metadata
    // protocol, after qualified experience already exists for this configuration.
    let scan = f.scan();
    assert_eq!(
        scan["pool"]["agents"][0]["identity"]["name"],
        "Orchid · native A"
    );
    assert_eq!(scan["pool"]["agents"][0]["identity"]["model"], "wire-a");
    assert_eq!(scan["pool"]["agents"][0]["identity"]["status"], "native");
    assert!(scan["pool"]["agents"][0]["exclusions"]
        .as_array()
        .unwrap()
        .is_empty());
    assert_eq!(
        serde_json::to_value(store.trace(&first.session.id).unwrap()).unwrap(),
        history,
        "catalog migration must not rewrite qualified history"
    );
    let after = Config::load(&f.home).unwrap();
    let after_profile = after.agent("codex").unwrap();
    let after_settings = after
        .execution_settings(after_profile, &Default::default())
        .unwrap();
    assert_eq!(after_settings, before_settings);
    assert_eq!(after.execution, config.execution);

    // Exercise the real allocation lookup in a fresh session. Reusing the first
    // session would hide the defect behind its immutable captured profile.
    let second = history_engine(&store, after.clone())
        .run(&f.cwd, "Create a greeting", None)
        .await
        .unwrap();
    assert_eq!(second.session.status, "completed", "{}", second.summary);
    let decisions = store.allocation_decisions(&second.session.id).unwrap();
    let execution = decisions
        .iter()
        .find(|d| d.input.demand.purpose == "execute")
        .unwrap();
    let candidate = execution
        .input
        .candidates
        .iter()
        .find(|c| {
            c.agent_id == "codex"
                && c.settings.model.as_deref() == Some("wire-a")
                && c.settings.effort.as_deref() == Some("quiet")
        })
        .unwrap();
    assert_eq!(candidate.experience.successes, 1, "first native scan disconnected existing qualified experience for the unchanged execution policy");
    assert_eq!(candidate.configuration_version, observation.agent_version);
    assert_eq!(
        after_profile, &before_profile,
        "an explicit policy already supplies the concrete model"
    );
    assert_eq!(
        execution_config_version(
            after_profile,
            after.provider("codex").unwrap(),
            &after_settings,
            &after_settings,
            Some("same-native-version")
        ),
        before_version
    );
    let mut genuinely_changed = after_settings.clone();
    genuinely_changed.model = Some("wire-b".into());
    assert_ne!(
        execution_config_version(
            after_profile,
            after.provider("codex").unwrap(),
            &genuinely_changed,
            &genuinely_changed,
            Some("same-native-version")
        ),
        before_version
    );
    let repeated = f.scan();
    assert_eq!(repeated["scan"]["migrated_agents"], json!([]));
}

#[tokio::test]
async fn first_scan_preserves_fixed_model_policy_qualified_experience() {
    first_scan_preserves_qualified_policy_history(true).await;
}

#[tokio::test]
async fn first_scan_preserves_defaulted_model_policy_qualified_experience() {
    first_scan_preserves_qualified_policy_history(false).await;
}
