#![forbid(unsafe_code)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use ymp_application::Application;
use ymp_domain::{Budget, EventKind};
use ymp_runtime_api::RuntimeEventKind;
use ymp_runtime_claude::{ClaudeProfile, ClaudeRuntime};
use ymp_runtime_supervisor::{
    ManagedCandidateRequest, ManagedContract, ManagedRunEvent, start_managed_candidate,
};

const COORDINATED_INIT: &str = r#"{"type":"system","subtype":"init","session_id":"session-product","claude_code_version":"2.1.227","model":"claude-opus-5","permissionMode":"acceptEdits","tools":["Bash","Edit","Glob","Grep","Read","Write","mcp__ymp__read_control","mcp__ymp__read_events","mcp__ymp__submit","mcp__ymp__yield"],"mcp_servers":[{"name":"ymp","status":"connected"}],"slash_commands":[],"plugins":[],"skills":[]}"#;

/// Writes a Claude Code stand-in that answers the pinned version and authentication probes and
/// then runs `body` inside the generated workspace, which the driver sets as its working directory.
fn fixture(path: &Path, body: &str) -> PathBuf {
    fs::write(
        path,
        format!(
            r##"#!/bin/sh
if [ "$1" = "--version" ]; then
  printf '%s\n' '2.1.227 (Claude Code)'
  exit 0
elif [ "$1" = "auth" ]; then
  printf '%s\n' '{{"loggedIn":true,"authMethod":"claude.ai","apiProvider":"firstParty"}}'
  exit 0
fi
config=''
previous=''
for argument in "$@"; do
  if [ "$previous" = '--mcp-config' ]; then config=$argument; fi
  previous=$argument
done
bridge=$(printf '%s' "$config" | sed -n 's/.*"command":"\([^"]*\)".*/\1/p')
{body}
"##
        ),
    )
    .expect("write Claude fixture");
    let mut permissions = fs::metadata(path).expect("metadata").permissions();
    permissions.set_mode(0o700);
    fs::set_permissions(path, permissions).expect("make executable");
    path.to_path_buf()
}

fn managed_source(root: &Path, name: &str) -> PathBuf {
    let source = root.join(name);
    fs::create_dir(&source).expect("source directory");
    fs::write(source.join("input.txt"), b"before\n").expect("source file");
    source
}

fn contract(contract_id: &str, source: PathBuf, prompt: &str) -> ManagedContract {
    ManagedContract {
        contract_id: contract_id.to_owned(),
        contract_digest: "c".repeat(64),
        source,
        prompt: prompt.to_owned(),
        capture_exclusions: Vec::new(),
        verifier: None,
    }
}

struct Observed {
    candidate: Option<String>,
    failure: Option<String>,
    runtime: Vec<RuntimeEventKind>,
}

fn drain(handle: &ymp_runtime_supervisor::ManagedRunHandle, seconds: u64) -> Observed {
    let mut observed = Observed {
        candidate: None,
        failure: None,
        runtime: Vec::new(),
    };
    let deadline = Instant::now() + Duration::from_secs(seconds);
    let collect = |event: ManagedRunEvent, observed: &mut Observed| match event {
        ManagedRunEvent::CandidateAvailable {
            candidate_digest, ..
        } => observed.candidate = Some(candidate_digest),
        ManagedRunEvent::Failed { detail } => observed.failure = Some(detail),
        ManagedRunEvent::Runtime(event) => observed.runtime.push(event.event),
        ManagedRunEvent::Finished => {}
    };
    while Instant::now() < deadline && !handle.is_finished() {
        while let Some(event) = handle.try_next() {
            collect(event, &mut observed);
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    while let Some(event) = handle.try_next() {
        collect(event, &mut observed);
    }
    observed
}

#[test]
fn generated_claude_environment_completes_a_candidate_through_product_mcp() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let source = managed_source(temporary.path(), "source");
    let executable = fixture(
        &temporary.path().join("claude-product-fixture"),
        &format!(
            r##"test -n "$bridge" || exit 31
printf '%s\n' "$0" > actual.executable
printf '%s\n' "$@" > actual.args
env | sed 's/=.*//' | sort > actual.env-keys
{{
  printf 'HOME=%s\n' "$HOME"
  printf 'CLAUDE_CONFIG_DIR=%s\n' "$CLAUDE_CONFIG_DIR"
  printf 'ANTHROPIC_API_KEY=%s\n' "${{ANTHROPIC_API_KEY-unset}}"
  printf 'CLAUDE_CODE_OAUTH_TOKEN=%s\n' "${{CLAUDE_CODE_OAUTH_TOKEN-unset}}"
  printf 'YMP_ATTEMPT_ID=%s\n' "${{YMP_ATTEMPT_ID-unset}}"
  printf 'YMP_INVOCATION_ID=%s\n' "${{YMP_INVOCATION_ID-unset}}"
  printf 'YMP_AGENT_TOKEN_PRESENT=%s\n' "$(test -n "${{YMP_AGENT_TOKEN-}}" && printf yes || printf no)"
  test -d "$CLAUDE_CONFIG_DIR" && printf 'CONFIG_DIR_PRESENT=yes\n'
  test -f "$CLAUDE_CONFIG_DIR/.credentials.json" && printf 'CREDENTIAL_DELEGATED=yes\n' || printf 'CREDENTIAL_DELEGATED=no\n'
}} > actual.environment
printf '%s\n' 'after through product MCP' > input.txt
{{
  printf '%s\n' '{{"jsonrpc":"2.0","id":1,"method":"initialize","params":{{"protocolVersion":"2025-11-25"}}}}'
  printf '%s\n' '{{"jsonrpc":"2.0","method":"notifications/initialized"}}'
  printf '%s\n' '{{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{{"name":"read_control","arguments":{{}}}}}}'
  printf '%s\n' '{{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{{"name":"submit","arguments":{{"command_id":"agent.submit.claude-product"}}}}}}'
}} | "$bridge" internal agent-mcp > mcp.responses
grep -q '"snapshot_digest"' mcp.responses || exit 39
cat >/dev/null
printf '%s\n' '{COORDINATED_INIT}'
printf '%s\n' '{{"type":"assistant","request_id":"req_1","parent_tool_use_id":null,"message":{{"content":[{{"type":"tool_use","id":"toolu_1","name":"mcp__ymp__submit","input":{{"command_id":"agent.submit.claude-product"}}}}]}}}}'
printf '%s\n' '{{"type":"user","message":{{"content":[{{"type":"tool_result","tool_use_id":"toolu_1","content":[{{"type":"text","text":"committed"}}]}}]}}}}'
printf '%s\n' '{{"type":"result","subtype":"success","is_error":false,"terminal_reason":"completed","total_cost_usd":0.017,"modelUsage":{{"claude-opus-5":{{"costUSD":0.017}}}},"usage":{{"input_tokens":17,"cache_creation_input_tokens":40,"cache_read_input_tokens":5,"output_tokens":3,"output_tokens_details":{{"thinking_tokens":1}}}}}}'
"##
        ),
    );

    let data_root = temporary.path().join("data");
    let application = Arc::new(Mutex::new(
        Application::create(&data_root, "run-claude-product", Budget::new(1, 1))
            .expect("create application"),
    ));
    let handle = start_managed_candidate(
        Arc::clone(&application),
        Box::new(ClaudeRuntime::new(&executable).without_delegated_credential()),
        ManagedCandidateRequest {
            contract: contract(
                "claude-product-contract",
                source,
                "complete the fake project and submit it",
            ),
            bridge_executable: env!("CARGO_BIN_EXE_ymp").into(),
        },
    )
    .expect("start managed candidate");
    let attempt_id = handle.attempt_id().to_owned();
    let observed = drain(&handle, 30);
    assert!(handle.is_finished(), "managed product path did not finish");
    assert_eq!(observed.failure, None);
    assert_eq!(
        observed.candidate.as_deref(),
        application
            .lock()
            .expect("application lock")
            .state()
            .candidate_digest
            .as_deref()
    );

    let workspace = data_root.join("workspaces").join(&attempt_id);
    let responses = fs::read_to_string(workspace.join("mcp.responses")).expect("MCP responses");
    assert!(responses.contains("\"isError\":false"));
    assert!(!responses.contains("YMP_AGENT_TOKEN"));

    let environment = fs::read_to_string(workspace.join("actual.environment"))
        .expect("observed safe environment");
    assert!(environment.contains("ANTHROPIC_API_KEY=unset"));
    assert!(environment.contains("CLAUDE_CODE_OAUTH_TOKEN=unset"));
    assert!(environment.contains("CONFIG_DIR_PRESENT=yes"));
    assert!(environment.contains("CREDENTIAL_DELEGATED=no"));
    assert!(environment.contains("YMP_AGENT_TOKEN_PRESENT=yes"));
    assert!(environment.contains(&format!("YMP_ATTEMPT_ID={attempt_id}")));
    let observed_home = environment
        .lines()
        .find_map(|line| line.strip_prefix("HOME="))
        .expect("observed home");
    assert_ne!(
        Some(observed_home),
        std::env::var("HOME").ok().as_deref(),
        "the managed invocation inherited the operator home"
    );
    // `PWD`, `SHLVL` and `_` are created by the observing shell itself rather than inherited,
    // so the admitted set is compared without them.
    let keys: Vec<_> = fs::read_to_string(workspace.join("actual.env-keys"))
        .expect("observed environment keys")
        .lines()
        .filter(|name| !["PWD", "SHLVL", "_"].contains(name))
        .map(str::to_owned)
        .collect();
    assert_eq!(
        keys,
        [
            "CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC",
            "CLAUDE_CONFIG_DIR",
            "HOME",
            "NO_COLOR",
            "PATH",
            "TMPDIR",
            "YMP_AGENT_SOCKET",
            "YMP_AGENT_TOKEN",
            "YMP_ATTEMPT_ID",
            "YMP_INVOCATION_ID",
        ]
    );

    let profile: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(
            data_root
                .join("runtime-evidence")
                .join(&attempt_id)
                .join("profile.json"),
        )
        .expect("runtime profile evidence"),
    )
    .expect("parse runtime profile evidence");
    assert_eq!(profile["profile"]["schema_version"], 3);
    assert_eq!(profile["profile"]["runtime_kind"], "claude_code");
    assert_eq!(
        profile["profile"]["probe"]["version"],
        "2.1.227 (Claude Code)"
    );
    let detail = profile["profile"]["probe"]["detail"]
        .as_str()
        .expect("probe detail");
    // Admission is a floor plus a measurement, so the record names both the floor it applied and
    // the executable it measured, and not just a version string it could have copied from a pin.
    assert!(detail.contains("minimum_version=2.1.227 (Claude Code)"));
    assert!(detail.contains("executable_digest="));
    assert!(detail.contains("model=claude-opus-5"));
    assert!(detail.contains("api_provider=firstParty"));
    assert!(detail.contains("native_subagents=disabled"));
    assert!(detail.contains("setting_sources=none"));
    assert!(detail.contains("strict_mcp_config=true"));
    assert!(detail.contains("max_budget_usd=1.000000"));
    assert_eq!(
        profile["profile"]["environment_policy"],
        "synthetic_allowlist_with_delegated_credential_v1"
    );
    assert_eq!(
        profile["profile"]["coordination"]["transport"],
        "stdio_mcp_via_private_rpc"
    );
    assert_eq!(
        profile["profile"]["coordination"]["invocation_scoped"],
        true
    );
    assert_eq!(
        profile["profile"]["coordination"]["credential_values_recorded"],
        false
    );

    let launch = &profile["profile"]["launch_descriptor"];
    assert_eq!(launch["attempt_id"], attempt_id);
    assert_eq!(launch["invocation_id"], profile["profile"]["invocation_id"]);
    assert_eq!(
        launch["coordination_executable_digest"],
        profile["profile"]["coordination"]["bridge_executable_digest"]
    );
    assert_eq!(
        fs::read_to_string(workspace.join("actual.executable"))
            .expect("observed executable")
            .trim(),
        launch["executable"].as_str().expect("launch executable")
    );
    let launched_arguments: Vec<_> = launch["arguments"]
        .as_array()
        .expect("launch arguments")
        .iter()
        .map(|argument| argument.as_str().expect("argument string").to_owned())
        .collect();
    assert_eq!(
        fs::read_to_string(workspace.join("actual.args"))
            .expect("observed arguments")
            .lines()
            .map(str::to_owned)
            .collect::<Vec<_>>(),
        launched_arguments
    );
    for expected in [
        "--print",
        "--strict-mcp-config",
        "--disable-slash-commands",
        "--setting-sources",
        "--max-budget-usd",
        "--allowed-tools",
    ] {
        assert!(
            launched_arguments
                .iter()
                .any(|argument| argument == expected),
            "launch arguments omit {expected}"
        );
    }
    assert!(
        !launched_arguments
            .iter()
            .any(|argument| argument.contains("Task")),
        "launch arguments admit a delegation tool"
    );
    let token_entry = launch["environment"]
        .as_array()
        .expect("launch environment")
        .iter()
        .find(|variable| variable["name"] == "YMP_AGENT_TOKEN")
        .expect("token environment evidence");
    assert_eq!(token_entry["confidential"], true);
    assert!(token_entry["value"].is_null());
    assert_admitted_programs(&profile);

    let evidence = fs::read_to_string(
        data_root
            .join("runtime-evidence")
            .join(&attempt_id)
            .join("events.jsonl"),
    )
    .expect("runtime evidence");
    assert!(evidence.contains("\"wall_time_ms\":"));
    assert!(evidence.contains("\"in_flight_excess\":"));
    assert!(evidence.contains("\"cost_microusd\":17000"));
    let session_records = evidence
        .lines()
        .filter(|line| line.contains("\"type\":\"started\""))
        .count();
    assert_eq!(session_records, 1);
    for record in evidence.lines() {
        let record: serde_json::Value =
            serde_json::from_str(record).expect("runtime event evidence");
        assert_eq!(record["schema_version"], 3);
        assert_eq!(record["profile_digest"], profile["digest"]);
        assert_eq!(record["invocation_id"], profile["profile"]["invocation_id"]);
    }
    assert!(!evidence.contains("session-product"));
    handle.join().expect("join worker");
}

#[test]
fn claude_child_stderr_cannot_persist_its_mcp_token_or_raw_diagnostic() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let source = managed_source(temporary.path(), "source-secret");
    let executable = fixture(
        &temporary.path().join("claude-secret-fixture"),
        r##"cat >/dev/null
printf '%s' "$YMP_AGENT_TOKEN" > "/tmp/ymp-claude-observed-token-$YMP_ATTEMPT_ID"
printf 'raw-child-diagnostic:%s\n' "$YMP_AGENT_TOKEN" >&2
exit 47
"##,
    );

    let data_root = temporary.path().join("data-secret");
    let application = Arc::new(Mutex::new(
        Application::create(&data_root, "run-claude-secret", Budget::new(1, 1))
            .expect("create application"),
    ));
    let handle = start_managed_candidate(
        Arc::clone(&application),
        Box::new(ClaudeRuntime::new(&executable).without_delegated_credential()),
        ManagedCandidateRequest {
            contract: contract("claude-secret-contract", source, "emit the fixture failure"),
            bridge_executable: env!("CARGO_BIN_EXE_ymp").into(),
        },
    )
    .expect("start managed candidate");
    let token_path = PathBuf::from(format!(
        "/tmp/ymp-claude-observed-token-{}",
        handle.attempt_id()
    ));
    let observed = drain(&handle, 30);
    assert!(handle.is_finished());
    assert_eq!(
        observed.failure.as_deref(),
        Some("managed_runtime_supervision_failed")
    );
    let token = fs::read(&token_path).expect("observed child token");
    assert!(!token.is_empty());
    fs::remove_file(&token_path).expect("remove transient token observation");

    fn assert_tree_excludes(root: &Path, forbidden: &[u8], label: &str) {
        for entry in fs::read_dir(root).expect("read durable directory") {
            let entry = entry.expect("durable entry");
            let path = entry.path();
            if path.is_dir() {
                assert_tree_excludes(&path, forbidden, label);
            } else {
                let bytes = fs::read(&path).expect("read durable file");
                assert!(
                    !bytes
                        .windows(forbidden.len())
                        .any(|window| window == forbidden),
                    "durable file {} contains {label}",
                    path.display()
                );
            }
        }
    }
    assert_tree_excludes(&data_root, &token, "the MCP token");
    assert_tree_excludes(
        &data_root,
        b"raw-child-diagnostic",
        "raw child diagnostic text",
    );
    handle.join().expect("join worker");
}

#[test]
fn durable_claude_accounting_covers_success_budget_stop_cancel_and_timeout() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let executable = fixture(
        &temporary.path().join("claude-accounting-fixture"),
        &format!(
            r##"input=$(cat)
printf '%s\n' '{COORDINATED_INIT}'
printf '%s\n' '{{"type":"assistant","request_id":"req_1","message":{{"content":[{{"type":"text","text":"accounting-ready"}}]}}}}'
case "$input" in
  *success*)
    responses=$({{
      printf '%s\n' '{{"jsonrpc":"2.0","id":1,"method":"initialize","params":{{"protocolVersion":"2025-11-25"}}}}'
      printf '%s\n' '{{"jsonrpc":"2.0","method":"notifications/initialized"}}'
      printf '%s\n' '{{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{{"name":"submit","arguments":{{"command_id":"durable-submit"}}}}}}'
    }} | "$bridge" internal agent-mcp) || exit 41
    printf '%s' "$responses" | grep -q '"snapshot_digest"' || exit 42
    printf '%s\n' '{{"type":"result","subtype":"success","is_error":false,"terminal_reason":"completed","total_cost_usd":0.031,"modelUsage":{{"claude-opus-5":{{"costUSD":0.031}}}},"usage":{{"input_tokens":11,"cache_creation_input_tokens":3,"cache_read_input_tokens":4,"output_tokens":5,"output_tokens_details":{{"thinking_tokens":2}}}}}}'
    ;;
  *budget*)
    printf '%s\n' '{{"type":"result","subtype":"error_max_budget_usd","is_error":true,"terminal_reason":"budget_exhausted","total_cost_usd":0.031,"modelUsage":{{"claude-opus-5":{{"costUSD":0.031}}}},"usage":{{"input_tokens":11,"cache_creation_input_tokens":3,"cache_read_input_tokens":4,"output_tokens":5,"output_tokens_details":{{"thinking_tokens":2}}}}}}'
    exit 1
    ;;
  *) sleep 30 ;;
esac
"##
        ),
    );

    for outcome in ["success", "budget", "cancel", "timeout"] {
        let source = managed_source(temporary.path(), &format!("source-{outcome}"));
        let data_root = temporary.path().join(format!("data-{outcome}"));
        let application = Arc::new(Mutex::new(
            Application::create(&data_root, format!("run-{outcome}"), Budget::new(1, 1))
                .expect("create application"),
        ));
        let profile = ClaudeProfile {
            wall_time_limit_ms: if outcome == "timeout" { 1_000 } else { 20_000 },
            ..ClaudeProfile::default()
        };
        let handle = start_managed_candidate(
            Arc::clone(&application),
            Box::new(
                ClaudeRuntime::with_profile(&executable, profile).without_delegated_credential(),
            ),
            ManagedCandidateRequest {
                contract: contract(
                    &format!("claude-contract-{outcome}"),
                    source,
                    &format!("{outcome} accounting"),
                ),
                bridge_executable: env!("CARGO_BIN_EXE_ymp").into(),
            },
        )
        .expect("start managed outcome");
        let attempt_id = handle.attempt_id().to_owned();

        let deadline = Instant::now() + Duration::from_secs(40);
        let mut terminal_usage = None;
        let mut cancel_sent = false;
        // The fixture's first assistant message carries the request identifier the runtime counts,
        // so an observed `accounting-ready` output states that the run reached a model request
        // before it ended. The event stream is ordered, so nothing observed here arrives after the
        // terminal event.
        let mut request_observed = false;
        let collect = |event: RuntimeEventKind, terminal_usage: &mut Option<_>| {
            *terminal_usage = match event {
                RuntimeEventKind::Completed { usage }
                | RuntimeEventKind::Failed { usage, .. }
                | RuntimeEventKind::TimedOut { usage, .. }
                | RuntimeEventKind::Cancelled { usage } => Some(usage),
                _ => terminal_usage.take(),
            };
        };
        while Instant::now() < deadline && !handle.is_finished() {
            while let Some(event) = handle.try_next() {
                if let ManagedRunEvent::Runtime(event) = event {
                    if matches!(&event.event, RuntimeEventKind::Output { text } if text == "accounting-ready")
                    {
                        request_observed = true;
                        if outcome == "cancel" && !cancel_sent {
                            handle.cancel("fixture cancellation").expect("cancel run");
                            cancel_sent = true;
                        }
                    }
                    collect(event.event, &mut terminal_usage);
                }
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        while let Some(event) = handle.try_next() {
            if let ManagedRunEvent::Runtime(event) = event {
                collect(event.event, &mut terminal_usage);
            }
        }
        assert!(handle.is_finished(), "{outcome} did not finish");
        let usage = terminal_usage.expect("terminal usage");
        assert!(usage.wall_time_ms > 0, "{outcome} wall time");
        if outcome == "success" || outcome == "budget" {
            assert_eq!(usage.input_tokens, 14, "{outcome} input tokens");
            assert_eq!(usage.cached_input_tokens, 4, "{outcome} cached tokens");
            assert_eq!(usage.output_tokens, 5, "{outcome} output tokens");
            assert_eq!(
                usage.reasoning_output_tokens, 2,
                "{outcome} thinking tokens"
            );
            assert_eq!(usage.cost_microusd, Some(31_000), "{outcome} cost");
            assert_eq!(usage.in_flight_excess.model_requests, 0, "{outcome} excess");
        } else {
            assert_eq!(usage.cost_microusd, None, "{outcome} reported no cost");
            // No accounting record covered this run, so the excess counts exactly the model
            // requests its transcript showed: every observed request stays unaccounted, and a run
            // that ended before its first request has none invented for it.
            assert_eq!(
                usage.in_flight_excess.model_requests,
                u64::from(request_observed),
                "{outcome} excess does not match the model requests the transcript showed"
            );
        }

        let evidence = fs::read_to_string(
            data_root
                .join("runtime-evidence")
                .join(&attempt_id)
                .join("events.jsonl"),
        )
        .expect("durable runtime evidence");
        let expected_type = match outcome {
            "success" => "completed",
            "budget" => "failed",
            "cancel" => "cancelled",
            "timeout" => "timed_out",
            _ => unreachable!(),
        };
        let terminal = evidence
            .lines()
            .map(|line| serde_json::from_str::<serde_json::Value>(line).expect("evidence record"))
            .find(|record| record["event"]["type"] == expected_type)
            .unwrap_or_else(|| panic!("{outcome} has no durable terminal record"));
        assert_eq!(
            terminal["event"]["usage"]["cost_microusd"].as_u64(),
            usage.cost_microusd
        );
        assert_eq!(
            terminal["event"]["usage"]["in_flight_excess"]["model_requests"].as_u64(),
            Some(usage.in_flight_excess.model_requests)
        );
        // The record names the models that produced the cost, so attribution survives in the
        // record as data instead of existing only while the admission rule runs.
        let recorded: Vec<(String, u64)> = terminal["event"]["usage"]["cost_by_model"]
            .as_array()
            .expect("the record carries a per-model breakdown")
            .iter()
            .map(|spend| {
                (
                    spend["model"].as_str().expect("model name").to_owned(),
                    spend["cost_microusd"].as_u64().expect("model spend"),
                )
            })
            .collect();
        if outcome == "success" || outcome == "budget" {
            assert_eq!(
                recorded,
                vec![("claude-opus-5".to_owned(), 31_000)],
                "{outcome} per-model spend"
            );
            assert_eq!(
                recorded.iter().map(|(_, cost)| cost).sum::<u64>(),
                terminal["event"]["usage"]["cost_microusd"]
                    .as_u64()
                    .expect("recorded total"),
                "{outcome} named spend does not add up to the recorded total"
            );
        } else {
            assert!(
                recorded.is_empty(),
                "{outcome} named a spend for a run that reported no cost"
            );
        }
        assert!(usage.cost_is_attributed(), "{outcome} attribution");
        handle.join().expect("join worker");
    }
}

#[test]
fn fabricated_claude_stdout_lifecycle_cannot_submit_or_yield() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let source = managed_source(temporary.path(), "source-fabricated");
    let executable = fixture(
        &temporary.path().join("claude-fabricated-fixture"),
        &format!(
            r##"cat >/dev/null
printf '%s\n' '{COORDINATED_INIT}'
printf '%s\n' '{{"type":"assistant","request_id":"req_1","message":{{"content":[{{"type":"tool_use","id":"toolu_1","name":"mcp__ymp__submit","input":{{"command_id":"fabricated-submit"}}}}]}}}}'
printf '%s\n' '{{"type":"user","message":{{"content":[{{"type":"tool_result","tool_use_id":"toolu_1","content":[{{"type":"text","text":"{{\"snapshot_digest\":\"fabricated\"}}"}}]}}]}}}}'
printf '%s\n' '{{"type":"result","subtype":"success","is_error":false,"terminal_reason":"completed","total_cost_usd":0.001,"modelUsage":{{"claude-opus-5":{{"costUSD":0.001}}}},"usage":{{"input_tokens":1,"output_tokens":1}}}}'
"##
        ),
    );

    let data_root = temporary.path().join("data-fabricated");
    let application = Arc::new(Mutex::new(
        Application::create(&data_root, "run-claude-fabricated", Budget::new(1, 1))
            .expect("create application"),
    ));
    let handle = start_managed_candidate(
        Arc::clone(&application),
        Box::new(ClaudeRuntime::new(&executable).without_delegated_credential()),
        ManagedCandidateRequest {
            contract: contract(
                "claude-fabricated-contract",
                source,
                "fabricate lifecycle output",
            ),
            bridge_executable: env!("CARGO_BIN_EXE_ymp").into(),
        },
    )
    .expect("start fabricated lifecycle fixture");
    let observed = drain(&handle, 30);
    assert!(handle.is_finished());
    assert_eq!(
        observed.candidate, None,
        "fabricated output submitted a candidate"
    );
    assert!(
        !observed
            .runtime
            .iter()
            .any(|event| matches!(event, RuntimeEventKind::Yielded { .. })),
        "fabricated output yielded the invocation"
    );
    assert!(
        observed.runtime.iter().any(|event| matches!(
            event,
            RuntimeEventKind::Failed {
                kind: ymp_runtime_api::RuntimeFailureKind::Protocol,
                ..
            }
        )),
        "fabricated lifecycle had no typed rejection"
    );
    assert!(observed.failure.is_some());
    assert!(
        application
            .lock()
            .expect("application lock")
            .state()
            .candidate_digest
            .is_none()
    );
    handle.join().expect("join worker");
}

#[test]
fn managed_claude_yield_wake_resume_keeps_one_identity_and_effect() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let source = managed_source(temporary.path(), "source-resume");
    let executable = fixture(
        &temporary.path().join("claude-resume-fixture"),
        &format!(
            r##"test -n "$bridge" || exit 51
count=$(($(cat invocation.count 2>/dev/null || printf '0') + 1))
printf '%s\n' "$count" > invocation.count
printf '%s\n' "$@" > "invocation-$count.args"
cat >/dev/null
resume=no
previous=''
for argument in "$@"; do
  if [ "$previous" = '--resume' ]; then resume=yes; fi
  previous=$argument
done
printf '%s\n' '{COORDINATED_INIT}'
if [ "$resume" = yes ]; then
  printf '%s\n' 'submitted after resume' > input.txt
  responses=$({{
    printf '%s\n' '{{"jsonrpc":"2.0","id":1,"method":"initialize","params":{{"protocolVersion":"2025-11-25"}}}}'
    printf '%s\n' '{{"jsonrpc":"2.0","method":"notifications/initialized"}}'
    printf '%s\n' '{{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{{"name":"submit","arguments":{{"command_id":"resume-submit"}}}}}}'
  }} | "$bridge" internal agent-mcp) || exit 53
  printf '%s' "$responses" | grep -q '"snapshot_digest"' || exit 54
  printf '%s\n' '{{"type":"assistant","request_id":"req_2","message":{{"content":[{{"type":"text","text":"resumed exactly once"}}]}}}}'
  printf '%s\n' '{{"type":"result","subtype":"success","is_error":false,"terminal_reason":"completed","total_cost_usd":0.017,"modelUsage":{{"claude-opus-5":{{"costUSD":0.017}}}},"usage":{{"input_tokens":7,"output_tokens":3}}}}'
else
  responses=$({{
    printf '%s\n' '{{"jsonrpc":"2.0","id":1,"method":"initialize","params":{{"protocolVersion":"2025-11-25"}}}}'
    printf '%s\n' '{{"jsonrpc":"2.0","method":"notifications/initialized"}}'
    printf '%s\n' '{{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{{"name":"yield","arguments":{{"command_id":"resume-yield"}}}}}}'
    printf '%s\n' '{{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{{"name":"yield","arguments":{{"command_id":"resume-yield"}}}}}}'
  }} | "$bridge" internal agent-mcp) || exit 55
  test "$(printf '%s' "$responses" | grep -c '"isError":false')" -ge 2 || exit 56
  printf '%s\n' '{{"type":"result","subtype":"success","is_error":false,"terminal_reason":"completed","total_cost_usd":0.011,"modelUsage":{{"claude-opus-5":{{"costUSD":0.011}}}},"usage":{{"input_tokens":5,"output_tokens":2}}}}'
fi
"##
        ),
    );

    let data_root = temporary.path().join("data-resume");
    let application = Arc::new(Mutex::new(
        Application::create(&data_root, "run-claude-resume", Budget::new(1, 1))
            .expect("create application"),
    ));
    let handle = start_managed_candidate(
        Arc::clone(&application),
        Box::new(ClaudeRuntime::new(&executable).without_delegated_credential()),
        ManagedCandidateRequest {
            contract: contract("claude-resume-contract", source, "yield once"),
            bridge_executable: env!("CARGO_BIN_EXE_ymp").into(),
        },
    )
    .expect("start managed resume");
    let attempt_id = handle.attempt_id().to_owned();
    let invocation_id = handle.invocation_id().to_owned();

    let deadline = Instant::now() + Duration::from_secs(40);
    let mut session_id = None;
    let mut launches = Vec::new();
    let mut yielded_command = None;
    while Instant::now() < deadline && yielded_command.is_none() && !handle.is_finished() {
        while let Some(event) = handle.try_next() {
            if let ManagedRunEvent::Runtime(event) = event {
                assert_eq!(event.invocation_id, invocation_id);
                match event.event {
                    RuntimeEventKind::Launch { descriptor } => launches.push(*descriptor),
                    RuntimeEventKind::Started { opaque_session_id } => {
                        session_id = Some(opaque_session_id)
                    }
                    RuntimeEventKind::Yielded { cursor } => yielded_command = Some(cursor),
                    _ => {}
                }
            }
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(yielded_command.as_deref(), Some("resume-yield"));
    handle
        .wake("wake-resume-1", "continue once")
        .expect("first wake");
    handle
        .wake("wake-resume-1", "continue once")
        .expect("idempotent repeated wake");
    assert!(handle.wake("wake-resume-1", "different input").is_err());

    let mut resumed_outputs = 0;
    let mut candidates = 0;
    let collect = |event: ManagedRunEvent,
                   launches: &mut Vec<_>,
                   resumed_outputs: &mut i32,
                   candidates: &mut i32| match event {
        ManagedRunEvent::Runtime(event) => {
            assert_eq!(event.invocation_id, invocation_id);
            match event.event {
                RuntimeEventKind::Launch { descriptor } => launches.push(*descriptor),
                RuntimeEventKind::Started { opaque_session_id } => {
                    assert_eq!(Some(&opaque_session_id), session_id.as_ref())
                }
                RuntimeEventKind::Output { text } if text == "resumed exactly once" => {
                    *resumed_outputs += 1
                }
                _ => {}
            }
        }
        ManagedRunEvent::CandidateAvailable { .. } => *candidates += 1,
        _ => {}
    };
    while Instant::now() < deadline && !handle.is_finished() {
        while let Some(event) = handle.try_next() {
            collect(event, &mut launches, &mut resumed_outputs, &mut candidates);
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    while let Some(event) = handle.try_next() {
        collect(event, &mut launches, &mut resumed_outputs, &mut candidates);
    }

    assert!(handle.is_finished());
    assert_eq!(resumed_outputs, 1);
    assert_eq!(candidates, 1);
    assert_eq!(
        fs::read_to_string(
            data_root
                .join("workspaces")
                .join(&attempt_id)
                .join("invocation.count")
        )
        .expect("invocation count"),
        "2\n"
    );
    let controller_events = application
        .lock()
        .expect("application lock")
        .events_after(0)
        .expect("controller events");
    let submissions: Vec<_> = controller_events
        .iter()
        .filter(|event| matches!(&event.event, EventKind::CandidateSubmitted { .. }))
        .collect();
    assert_eq!(submissions.len(), 1);
    assert_eq!(submissions[0].command_id, "resume-submit");

    assert_eq!(launches.len(), 2);
    let initial = &launches[0];
    let resumed = &launches[1];
    assert_eq!(initial.attempt_id, attempt_id);
    assert_eq!(resumed.invocation_id, initial.invocation_id);
    assert_eq!(resumed.executable, initial.executable);
    assert_eq!(resumed.executable_digest, initial.executable_digest);
    assert_eq!(resumed.environment, initial.environment);
    assert_eq!(resumed.working_directory, initial.working_directory);
    let mut expected_arguments = initial.arguments.clone();
    expected_arguments.extend([
        "--resume".to_owned(),
        session_id.expect("managed session identifier"),
    ]);
    assert_eq!(resumed.arguments, expected_arguments);
    handle.join().expect("join worker");
}

/// Reads back, from the run's own evidence, every program the run executed on its own behalf: the
/// programs the launch entered, the program that built the workspace baseline, and the utilities
/// that observe and terminate what the run started. Each is named with its role, the binding its
/// identity rests on, and the digest of the bytes admitted for it, so the enumeration comes from the
/// run's own record rather than from the source.
fn assert_admitted_programs(profile: &serde_json::Value) {
    let chain = profile["profile"]["launch_descriptor"]["launch_chain"]
        .as_array()
        .expect("admitted launch chain");
    let mut expected = vec![("launch_shell", "/bin/sh")];
    if std::path::Path::new("/usr/bin/env").is_file() {
        expected.push(("environment_sanitiser", "/usr/bin/env"));
    }
    assert_eq!(chain.len(), expected.len());
    for (program, (role, path)) in chain.iter().zip(expected) {
        assert_eq!(program["role"], role);
        assert_eq!(program["path"], path);
        assert_admitted(program);
    }
    let workspace_program = &profile["profile"]["workspace_program"];
    assert_eq!(workspace_program["role"], "workspace");
    assert_admitted(workspace_program);

    let lifecycle = profile["profile"]["lifecycle_programs"]
        .as_array()
        .expect("admitted lifecycle programs");
    let roles: Vec<&str> = lifecycle
        .iter()
        .map(|program| program["role"].as_str().expect("lifecycle role"))
        .collect();
    assert!(roles.contains(&"process_table"), "{roles:?}");
    assert!(roles.contains(&"signal"), "{roles:?}");
    if !std::path::Path::new("/proc/self/fd").is_dir() {
        assert!(roles.contains(&"descriptor_holders"), "{roles:?}");
    }
    for program in lifecycle {
        assert_admitted(program);
    }
}

/// Every program the record names is bound to a location this account cannot write, and carries the
/// digest of the bytes standing there.
fn assert_admitted(program: &serde_json::Value) {
    let path = program["path"].as_str().expect("admitted program path");
    assert_eq!(program["identity"], "system_path", "{path}");
    assert_eq!(
        program["digest"],
        ymp_domain::digest_bytes(&fs::read(path).expect("admitted program bytes")),
        "{path}"
    );
}
