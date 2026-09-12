//! Real executable checks use only the built-in deterministic mock provider.
use serde_json::{json, Value};
use std::{path::PathBuf, process::Command};
use ymp_core::*;
use ymp_storage::Store;

struct Fixture {
    _temp: tempfile::TempDir,
    home: PathBuf,
    project: PathBuf,
}

fn contract() -> Value {
    json!({
        "task_title":"Create a greeting",
        "criteria":[{"id":"content","description":"The greeting has the requested content"}],
        "artifacts":["greeting.txt"],"inputs":[],
        "checks":[{"id":"exact","criterion_ids":["content"],
            "assertion":{"kind":"exact_bytes","artifact":"greeting.txt","expected":b"Hello from ymp\n"}}]
    })
}

impl Fixture {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let home = temp.path().join("state");
        let project = temp.path().join("project");
        std::fs::create_dir_all(&home).unwrap();
        std::fs::create_dir_all(&project).unwrap();
        Self {
            _temp: temp,
            home,
            project,
        }
    }

    fn configure(&self, contracts: Option<Value>, markers: &str) {
        let mut config = json!({
            "version":1,"providers":[{"id":"mock","kind":"mock","command":"internal"}],
            "agents":[{"id":"one","name":"One","provider":"mock","instructions":markers},
                {"id":"two","name":"Two","provider":"mock","instructions":markers}],
            "team":["one","two"],"limits":{"attempts":1},
            "execution":{"one":{"fixed":{"effort":"low"}},"two":{"fixed":{"effort":"low"}}}
        });
        if let Some(contracts) = contracts {
            config["acceptance_contracts"] = contracts;
        }
        // Serialize raw configuration so this test detects an older loader silently
        // discarding the new field, rather than testing only a Rust-only ingress.
        std::fs::write(
            self.home.join("config.toml"),
            toml::to_string(&config).unwrap(),
        )
        .unwrap();
    }

    fn command(&self, args: &[&str]) -> std::process::Output {
        Command::new(env!("CARGO_BIN_EXE_ymp"))
            .arg("--home")
            .arg(&self.home)
            .arg("--cwd")
            .arg(&self.project)
            .args(args)
            .output()
            .unwrap()
    }

    fn run(&self) -> (std::process::Output, SessionTrace) {
        let output = self.command(&["run", "Create a greeting", "--json"]);
        let value: Value = serde_json::from_slice(&output.stdout)
            .unwrap_or_else(|_| panic!("{}", String::from_utf8_lossy(&output.stderr)));
        let store = Store::open(&self.home).unwrap();
        let trace = store
            .trace(value["session"]["id"].as_str().unwrap())
            .unwrap();
        (output, trace)
    }
}

fn final_confirmation(trace: &SessionTrace) -> Option<ConfirmationStatus> {
    trace
        .decisions
        .iter()
        .find(|d| d.kind == "final_accepted")
        .and_then(|d| match d.outcome {
            Some(DecisionOutcome::Accepted { confirmation }) => Some(confirmation),
            _ => None,
        })
}

#[test]
fn executable_contract_confirms_and_supplies_supported_knowledge_to_a_later_session() {
    let f = Fixture::new();
    f.configure(Some(json!([contract()])), "[mock:no-checks]");
    let (output, first) = f.run();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        final_confirmation(&first),
        Some(ConfirmationStatus::Confirmed)
    );
    assert_eq!(
        std::fs::read(f.project.join("greeting.txt")).unwrap(),
        b"Hello from ymp\n"
    );
    assert!(first
        .invocations
        .iter()
        .all(|i| i.requested.effort.as_deref() == Some("low")));
    let store = Store::open(&f.home).unwrap();
    assert_eq!(store.observations().unwrap().len(), 1);
    let source = store
        .memory_inventory(Some(&first.session.project_id))
        .unwrap()
        .into_iter()
        .find(|m| m.kind == "outcome" && m.status == "active")
        .unwrap();
    assert_eq!(
        source.provenance.as_ref().unwrap().confirmation,
        ConfirmationStatus::Confirmed
    );
    f.configure(None, "[mock:fail:plan]");
    let (output, next) = f.run();
    assert!(!output.status.success());
    assert_ne!(first.session.id, next.session.id);
    assert!(next
        .history
        .iter()
        .filter(|e| e.kind == "memory_retrieval")
        .any(|e| e.data["entries"]
            .as_array()
            .unwrap()
            .iter()
            .any(|m| m["id"] == source.id
                && m["source_session"] == first.session.id
                && m["confirmation"] == "confirmed")));
}

#[test]
fn executable_absent_partial_and_failed_contracts_do_not_create_supported_credit() {
    for mode in ["absent", "partial", "failed"] {
        let f = Fixture::new();
        let mut declared = contract();
        if mode == "partial" {
            declared["criteria"]
                .as_array_mut()
                .unwrap()
                .push(json!({"id":"quality","description":"Qualitative usefulness"}));
        }
        f.configure(
            (mode != "absent").then(|| json!([declared])),
            if mode == "failed" {
                "[mock:no-checks][mock:broken-output]"
            } else {
                "[mock:no-checks]"
            },
        );
        let (output, trace) = f.run();
        assert_eq!(output.status.success(), mode != "failed");
        assert_eq!(
            final_confirmation(&trace),
            (mode != "failed").then_some(ConfirmationStatus::Unconfirmed)
        );
        assert!(Store::open(&f.home)
            .unwrap()
            .observations()
            .unwrap()
            .is_empty());
        if mode != "absent" {
            assert!(trace
                .decisions
                .iter()
                .any(|d| d.links.check.as_ref().is_some_and(|c| c.outcome
                    == if mode == "failed" {
                        ConfirmationCheckOutcome::Failed
                    } else {
                        ConfirmationCheckOutcome::Passed
                    })));
        }
    }
}

#[test]
fn executable_missing_and_duplicate_targets_stop_before_production() {
    for duplicate in [false, true] {
        let f = Fixture::new();
        let mut declared = contract();
        declared["task_title"] = json!("Required target omitted by the mock planner");
        f.configure(
            Some(if duplicate {
                json!([declared, declared])
            } else {
                json!([declared])
            }),
            "",
        );
        let output = f.command(&["run", "Create a greeting", "--json"]);
        assert!(
            !output.status.success(),
            "Ignored mandatory binding: {}",
            String::from_utf8_lossy(&output.stdout)
        );
        assert!(!f.project.join("greeting.txt").exists());
        let store = Store::open(&f.home).unwrap();
        for session in store.sessions(None).unwrap() {
            assert!(store
                .trace(&session.id)
                .unwrap()
                .assignments
                .iter()
                .all(|a| a.purpose != "execute"));
        }
    }
}

#[test]
fn executable_resume_rejects_changed_or_added_authority_and_allows_capture_only_resume() {
    for originally_configured in [false, true] {
        let f = Fixture::new();
        f.configure(
            originally_configured.then(|| json!([contract()])),
            "[mock:no-checks]",
        );
        let (_, first) = f.run();
        let store = Store::open(&f.home).unwrap();
        for replacement in [json!([]), json!([contract()])] {
            let mut replacement = replacement;
            if !replacement.as_array().unwrap().is_empty() {
                replacement[0]["criteria"][0]["description"] = json!("Changed authority");
            } else if !originally_configured {
                continue;
            }
            f.configure(Some(replacement), "[mock:no-checks]");
            let before = serde_json::to_value(store.trace(&first.session.id).unwrap()).unwrap();
            let output = f.command(&["resume", &first.session.id, "--headless"]);
            assert!(!output.status.success());
            assert!(String::from_utf8_lossy(&output.stderr).contains("immutable on resume"));
            assert_eq!(
                serde_json::to_value(store.trace(&first.session.id).unwrap()).unwrap(),
                before
            );
        }
        f.configure(None, "[mock:no-checks]");
        let output = f.command(&["resume", &first.session.id, "--headless"]);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let after = store.trace(&first.session.id).unwrap();
        let capture_ids = |trace: &SessionTrace| {
            trace
                .decisions
                .iter()
                .filter(|d| d.links.acceptance_contract.is_some())
                .map(|d| d.id.clone())
                .collect::<Vec<_>>()
        };
        assert_eq!(capture_ids(&first), capture_ids(&after));
        assert_eq!(
            store.observations().unwrap().len(),
            usize::from(originally_configured)
        );
        assert_eq!(
            final_confirmation(&after),
            Some(if originally_configured {
                ConfirmationStatus::Confirmed
            } else {
                ConfirmationStatus::Unconfirmed
            })
        );
    }
}

#[test]
fn demo_keeps_explicit_contract_configuration() {
    let f = Fixture::new();
    f.configure(Some(json!([contract()])), "");
    let output = f.command(&["demo"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let store = Store::open(&f.home).unwrap();
    let session = store.sessions(None).unwrap().remove(0);
    assert_eq!(
        final_confirmation(&store.trace(&session.id).unwrap()),
        Some(ConfirmationStatus::Confirmed)
    );
}

#[test]
fn executable_correction_contract_scope_and_public_mcp_history_are_reachable() {
    use std::io::Write;
    use std::process::Stdio;
    let f = Fixture::new();
    std::fs::write(f.project.join("source-v1.txt"), "old input\n").unwrap();
    std::fs::write(f.project.join("source-v2.txt"), "corrected input\n").unwrap();
    let configure_scope = || {
        let path = f.home.join("config.toml");
        let mut config: Value = toml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        config["knowledge_scope"] = json!({"site":"Hill"});
        std::fs::write(path, toml::to_string(&config).unwrap()).unwrap();
    };
    let mut first_contract = contract();
    first_contract["inputs"] = json!(["source-v1.txt"]);
    f.configure(Some(json!([first_contract])), "[mock:no-checks]");
    configure_scope();
    let (output, first) = f.run();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let store = Store::open(&f.home).unwrap();
    let old = store
        .memory_inventory(Some(&first.session.project_id))
        .unwrap()
        .into_iter()
        .find(|e| e.kind == "outcome")
        .unwrap();
    let mut correction = contract();
    correction["inputs"] = json!(["source-v2.txt"]);
    correction["knowledge_correction"] = json!({
        "projection":"project_outcome", "target":{"id":old.id,"version":content_digest(&serde_json::to_string(&old).unwrap())},
        "applicability":{"site":"Hill"}, "criterion_ids":["content"],
        "source_replacement":{"previous_input":"source-v1.txt","replacement_input":"source-v2.txt"}
    });
    f.configure(Some(json!([correction])), "[mock:no-checks]");
    configure_scope();
    let (output, second) = f.run();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let new = store
        .memory_inventory(Some(&first.session.project_id))
        .unwrap()
        .into_iter()
        .find(|e| e.kind == "outcome" && e.source_session == second.session.id)
        .unwrap();
    assert_eq!(new.supersedes.as_deref(), Some(old.id.as_str()));
    assert_eq!(store.observations().unwrap().len(), 2);
    let scoped = f.command(&["memory", "greeting", "--scope", "site=Hill"]);
    assert!(scoped.status.success());
    let rows: Value = serde_json::from_slice(&scoped.stdout).unwrap();
    assert!(rows.as_array().unwrap().iter().any(|e| e["id"] == new.id));
    assert!(!rows.as_array().unwrap().iter().any(|e| e["id"] == old.id));
    let wrong = f.command(&["memory", "greeting", "--scope", "site=Harbor"]);
    assert_eq!(
        serde_json::from_slice::<Value>(&wrong.stdout).unwrap(),
        json!([])
    );
    let history = f.command(&["memory", "--history", "--scope", "site=Hill"]);
    let rows: Value = serde_json::from_slice(&history.stdout).unwrap();
    assert!(rows
        .as_array()
        .unwrap()
        .iter()
        .any(|r| r["entry"]["id"] == old.id
            && r["availability"] == "superseded"
            && r["replaced_by"] == new.id));

    // Use the actual stdio executable with read-only scope; tool arguments cannot
    // supply correction authority or activate a record.
    let mut child = Command::new(env!("CARGO_BIN_EXE_ymp"))
        .arg("--home")
        .arg(&f.home)
        .arg("--cwd")
        .arg(&f.project)
        .arg("mcp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut input = child.stdin.take().unwrap();
    let requests = [
        json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"correction-test","version":"1"}}}),
        json!({"jsonrpc":"2.0","method":"notifications/initialized","params":{}}),
        json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"ymp_knowledge_v1","arguments":{"id":new.id,"scope":{"site":"Hill"}}}}),
        json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"ymp_knowledge_v1","arguments":{"id":old.id,"scope":{"site":"Hill"},"history":true}}}),
        json!({"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"ymp_knowledge_v1","arguments":{"query":"greeting","scope":{"site":"Harbor"}}}}),
        json!({"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"ymp_knowledge_v1","arguments":{"id":old.id,"scope":{"site":"Hill"},"status":"active"}}}),
    ];
    for request in requests {
        writeln!(input, "{request}").unwrap();
    }
    drop(input);
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let messages = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .collect::<Vec<_>>();
    let result = |id| messages.iter().find(|v| v["id"] == id).unwrap()["result"].clone();
    assert_eq!(
        result(2)["structuredContent"]["items"][0]["value"]["id"],
        new.id
    );
    assert_eq!(
        result(3)["structuredContent"]["items"][0]["value"]["availability"],
        "superseded"
    );
    assert_eq!(result(4)["structuredContent"]["items"], json!([]));
    assert_eq!(result(5)["isError"], true);
    assert_eq!(store.observations().unwrap().len(), 2);
}
