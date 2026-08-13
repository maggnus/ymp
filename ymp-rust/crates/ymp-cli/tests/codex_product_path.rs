#![forbid(unsafe_code)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use ymp_application::Application;
use ymp_domain::{Budget, EventKind};
use ymp_runtime_api::RuntimeEventKind;
use ymp_runtime_codex::{CodexProfile, CodexRuntime};
use ymp_runtime_supervisor::{
    ManagedCandidateRequest, ManagedContract, ManagedRunEvent, start_managed_candidate,
};

#[test]
fn generated_codex_environment_completes_a_candidate_through_product_mcp() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let source = temporary.path().join("source");
    fs::create_dir(&source).expect("source directory");
    fs::write(source.join("input.txt"), b"before\n").expect("source file");

    let executable = temporary.path().join("codex-product-fixture");
    fs::write(
        &executable,
        r##"#!/bin/sh
if [ "$1" = "--version" ]; then
  printf '%s\n' 'codex-cli 0.147.0'
elif [ "$1" = "login" ]; then
  exit 0
else
  bridge=''
  workspace=''
  previous=''
  for argument in "$@"; do
    if [ "$previous" = '-C' ]; then
      workspace=$argument
    fi
    case "$argument" in
      mcp_servers.ymp.command=*)
        bridge=${argument#mcp_servers.ymp.command=}
        bridge=${bridge#\"}
        bridge=${bridge%\"}
        ;;
    esac
    previous=$argument
  done
  test -n "$bridge" || exit 31
  test -n "$workspace" || exit 32
  printf '%s\n' "$0" > "$workspace/actual.executable"
  printf '%s\n' "$@" > "$workspace/actual.args"
  test "$OPENAI_BASE_URL" = 'https://api.openai.com/v1' || exit 33
  test -z "${OPENAI_ORGANIZATION+x}" || exit 34
  test -z "${OPENAI_PROJECT+x}" || exit 35
  test -n "$YMP_AGENT_SOCKET" || exit 36
  test -n "$YMP_AGENT_TOKEN" || exit 37
  test -n "$YMP_ATTEMPT_ID" || exit 38
  test -n "$YMP_INVOCATION_ID" || exit 40
  {
    printf 'OPENAI_BASE_URL=%s\n' "$OPENAI_BASE_URL"
    printf 'YMP_ATTEMPT_ID=%s\n' "$YMP_ATTEMPT_ID"
    printf 'YMP_INVOCATION_ID=%s\n' "$YMP_INVOCATION_ID"
    printf 'YMP_AGENT_TOKEN_PRESENT=yes\n'
  } > "$workspace/actual.environment"
  printf '%s\n' 'after through product MCP' > "$workspace/input.txt"
  {
    printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25"}}'
    printf '%s\n' '{"jsonrpc":"2.0","method":"notifications/initialized"}'
    printf '%s\n' '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"read_control","arguments":{}}}'
    printf '%s\n' '{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"submit","arguments":{"command_id":"agent.submit.fake-product"}}}'
  } | "$bridge" internal agent-mcp > "$workspace/mcp.responses"
  grep -q '"snapshot_digest"' "$workspace/mcp.responses" || exit 39
  cat >/dev/null
  printf '%s\n' '{"type":"thread.started","thread_id":"thread-fake-product-1"}'
  printf '%s\n' '{"type":"item.completed","item":{"type":"mcp_tool_call","server":"ymp","tool":"read_control","status":"completed","arguments":{},"result":{"status":"running"},"error":null}}'
  printf '%s\n' '{"type":"item.completed","item":{"type":"mcp_tool_call","server":"ymp","tool":"submit","status":"completed","arguments":{"command_id":"agent.submit.fake-product"},"result":{"committed":true},"error":null}}'
  printf '%s\n' '{"type":"turn.completed","usage":{"input_tokens":17,"cached_input_tokens":5,"output_tokens":3,"reasoning_output_tokens":1}}'
fi
"##,
    )
    .expect("write Codex fixture");
    let mut permissions = fs::metadata(&executable).expect("metadata").permissions();
    permissions.set_mode(0o700);
    fs::set_permissions(&executable, permissions).expect("make executable");

    let data_root = temporary.path().join("data");
    let application = Arc::new(Mutex::new(
        Application::create(&data_root, "run-fake-product", Budget::new(1, 1))
            .expect("create application"),
    ));
    let handle = start_managed_candidate(
        Arc::clone(&application),
        Box::new(CodexRuntime::new(&executable)),
        ManagedCandidateRequest {
            contract: ManagedContract {
                contract_id: "fake-product-contract".to_owned(),
                contract_digest: "c".repeat(64),
                source,
                prompt: "complete the fake project and submit it".to_owned(),
                capture_exclusions: Vec::new(),
                verifier: None,
            },
            bridge_executable: env!("CARGO_BIN_EXE_ymp").into(),
        },
    )
    .expect("start managed candidate");
    let attempt_id = handle.attempt_id().to_owned();
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut candidate = None;
    let mut failure = None;
    while Instant::now() < deadline && !handle.is_finished() {
        while let Some(event) = handle.try_next() {
            match event {
                ManagedRunEvent::CandidateAvailable {
                    candidate_digest, ..
                } => candidate = Some(candidate_digest),
                ManagedRunEvent::Failed { detail } => failure = Some(detail),
                _ => {}
            }
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    while let Some(event) = handle.try_next() {
        match event {
            ManagedRunEvent::CandidateAvailable {
                candidate_digest, ..
            } => candidate = Some(candidate_digest),
            ManagedRunEvent::Failed { detail } => failure = Some(detail),
            _ => {}
        }
    }
    assert!(handle.is_finished(), "managed product path did not finish");
    assert_eq!(failure, None);
    assert_eq!(
        candidate.as_deref(),
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
    let evidence = fs::read_to_string(
        data_root
            .join("runtime-evidence")
            .join(&attempt_id)
            .join("events.jsonl"),
    )
    .expect("runtime evidence");
    assert!(evidence.contains("\"wall_time_ms\":"));
    assert!(evidence.contains("\"protected_queries\":0"));
    assert!(evidence.contains("\"in_flight_excess\":"));
    assert!(!evidence.contains("fixture-secret"));

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
    assert_eq!(profile["profile"]["runtime_kind"], "codex");
    assert_eq!(profile["profile"]["probe"]["version"], "codex-cli 0.147.0");
    assert!(
        profile["profile"]["probe"]["detail"]
            .as_str()
            .expect("probe detail")
            .contains("model=gpt-5.6-sol, api_origin=https://api.openai.com/v1")
    );
    assert_eq!(
        profile["profile"]["environment_policy"],
        "synthetic_allowlist_v1"
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
    assert_eq!(launch["invocation_id"], profile["profile"]["invocation_id"]);
    assert_eq!(launch["attempt_id"], attempt_id);
    assert_eq!(
        launch["coordination_executable_digest"],
        profile["profile"]["coordination"]["bridge_executable_digest"]
    );
    assert!(launch["coordination_executable"].as_str().is_some());
    assert_eq!(
        fs::read_to_string(workspace.join("actual.executable"))
            .expect("actual executable")
            .trim(),
        launch["executable"].as_str().expect("launch executable")
    );
    let actual_arguments: Vec<_> = fs::read_to_string(workspace.join("actual.args"))
        .expect("actual arguments")
        .lines()
        .map(str::to_owned)
        .collect();
    assert_eq!(
        actual_arguments,
        launch["arguments"]
            .as_array()
            .expect("launch arguments")
            .iter()
            .map(|argument| argument.as_str().expect("argument string").to_owned())
            .collect::<Vec<_>>()
    );
    let actual_environment = fs::read_to_string(workspace.join("actual.environment"))
        .expect("actual safe environment observation");
    assert!(actual_environment.contains("OPENAI_BASE_URL=https://api.openai.com/v1"));
    assert!(actual_environment.contains(&format!("YMP_ATTEMPT_ID={attempt_id}")));
    assert!(actual_environment.contains(&format!(
        "YMP_INVOCATION_ID={}",
        profile["profile"]["invocation_id"]
            .as_str()
            .expect("profile invocation")
    )));
    let token_entry = launch["environment"]
        .as_array()
        .expect("launch environment")
        .iter()
        .find(|variable| variable["name"] == "YMP_AGENT_TOKEN")
        .expect("token environment evidence");
    assert_eq!(token_entry["confidential"], true);
    assert!(token_entry["value"].is_null());
    for digest in [
        &profile["digest"],
        &profile["profile"]["runtime_executable_digest"],
        &profile["profile"]["launch_descriptor_digest"],
        &profile["profile"]["coordination"]["bridge_executable_digest"],
        &profile["profile"]["coordination"]["endpoint_path_digest"],
    ] {
        assert_eq!(digest.as_str().expect("evidence digest").len(), 64);
    }
    assert_admitted_programs(&profile);
    for event in evidence.lines() {
        let event: serde_json::Value = serde_json::from_str(event).expect("runtime event evidence");
        assert_eq!(event["schema_version"], 3);
        assert_eq!(event["profile_digest"], profile["digest"]);
        assert_eq!(event["invocation_id"], profile["profile"]["invocation_id"]);
    }
    assert!(!profile.to_string().contains("fixture-secret"));
}

#[test]
fn child_stderr_cannot_persist_its_mcp_token_or_raw_diagnostic() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let source = temporary.path().join("source");
    fs::create_dir(&source).expect("source directory");
    fs::write(source.join("input.txt"), b"before\n").expect("source file");
    let executable = temporary.path().join("codex-secret-stderr-fixture");
    fs::write(
        &executable,
        r##"#!/bin/sh
if [ "$1" = "--version" ]; then
  printf '%s\n' 'codex-cli 0.147.0'
elif [ "$1" = "login" ]; then
  exit 0
else
  cat >/dev/null
  printf '%s' "$YMP_AGENT_TOKEN" > "/tmp/ymp-observed-token-$YMP_ATTEMPT_ID"
  printf 'raw-child-diagnostic:%s\n' "$YMP_AGENT_TOKEN" >&2
  exit 47
fi
"##,
    )
    .expect("write Codex fixture");
    let mut permissions = fs::metadata(&executable).expect("metadata").permissions();
    permissions.set_mode(0o700);
    fs::set_permissions(&executable, permissions).expect("make executable");

    let data_root = temporary.path().join("data");
    let application = Arc::new(Mutex::new(
        Application::create(&data_root, "run-secret-stderr", Budget::new(1, 1))
            .expect("create application"),
    ));
    let handle = start_managed_candidate(
        Arc::clone(&application),
        Box::new(CodexRuntime::new(&executable)),
        ManagedCandidateRequest {
            contract: ManagedContract {
                contract_id: "secret-stderr-contract".to_owned(),
                contract_digest: "e".repeat(64),
                source,
                prompt: "emit the fixture failure".to_owned(),
                capture_exclusions: Vec::new(),
                verifier: None,
            },
            bridge_executable: env!("CARGO_BIN_EXE_ymp").into(),
        },
    )
    .expect("start managed candidate");
    let token_path =
        std::path::PathBuf::from(format!("/tmp/ymp-observed-token-{}", handle.attempt_id()));
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut managed_failure = None;
    while Instant::now() < deadline && !handle.is_finished() {
        while let Some(event) = handle.try_next() {
            if let ManagedRunEvent::Failed { detail } = event {
                managed_failure = Some(detail);
            }
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    while let Some(event) = handle.try_next() {
        if let ManagedRunEvent::Failed { detail } = event {
            managed_failure = Some(detail);
        }
    }
    assert!(handle.is_finished());
    assert_eq!(
        managed_failure.as_deref(),
        Some("managed_runtime_supervision_failed")
    );
    let token = fs::read(&token_path).expect("observed child token");
    assert!(!token.is_empty());
    fs::remove_file(&token_path).expect("remove transient token observation");

    fn assert_tree_excludes(root: &std::path::Path, forbidden: &[u8], label: &str) {
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
fn durable_accounting_covers_success_error_cancel_and_timeout() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let executable = temporary.path().join("codex-durable-accounting-fixture");
    fs::write(
        &executable,
        r##"#!/bin/sh
if [ "$1" = "--version" ]; then
  printf '%s\n' 'codex-cli 0.147.0'
elif [ "$1" = "login" ]; then
  exit 0
else
  input=$(cat)
  bridge=''
  for argument in "$@"; do
    case "$argument" in
      mcp_servers.ymp.command=*)
        bridge=${argument#mcp_servers.ymp.command=}
        bridge=${bridge#\"}
        bridge=${bridge%\"}
        ;;
    esac
  done
  printf '%s\n' '{"type":"thread.started","thread_id":"thread-durable-accounting"}'
  printf '%s\n' '{"type":"turn.started","usage":{"input_tokens":7,"cached_input_tokens":2,"output_tokens":3,"reasoning_output_tokens":1,"cost_microusd":23,"protected_queries":2,"in_flight_excess":{"model_requests":1,"input_tokens":5,"cached_input_tokens":1,"output_tokens":2,"reasoning_output_tokens":1,"cost_microusd":11}}}'
  printf '%s\n' '{"type":"item.completed","item":{"type":"agent_message","text":"accounting-ready"}}'
  case "$input" in
    *success*)
      responses=$({
        printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25"}}'
        printf '%s\n' '{"jsonrpc":"2.0","method":"notifications/initialized"}'
        printf '%s\n' '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"submit","arguments":{"command_id":"durable-submit"}}}'
      } | "$bridge" internal agent-mcp) || exit 41
      printf '%s' "$responses" | grep -q '"snapshot_digest"' || exit 42
      printf '%s\n' '{"type":"item.completed","item":{"type":"mcp_tool_call","server":"ymp","tool":"submit","status":"completed","arguments":{"command_id":"durable-submit"},"result":{"committed":true},"error":null}}'
      printf '%s\n' '{"type":"turn.completed","usage":{"input_tokens":11,"cached_input_tokens":4,"output_tokens":5,"reasoning_output_tokens":2,"cost_microusd":31,"protected_queries":3,"in_flight_excess":{"model_requests":2,"cost_microusd":3}}}'
      ;;
    *error*)
      printf '%s\n' '{"type":"turn.failed","usage":{"input_tokens":11,"cached_input_tokens":4,"output_tokens":5,"reasoning_output_tokens":2,"cost_microusd":31,"protected_queries":3,"in_flight_excess":{"model_requests":1,"input_tokens":5,"cached_input_tokens":1,"output_tokens":2,"reasoning_output_tokens":1,"cost_microusd":11}},"error":{"code":"fixture"}}'
      ;;
    *) sleep 30 ;;
  esac
fi
"##,
    )
    .expect("write fixture");
    let mut permissions = fs::metadata(&executable).expect("metadata").permissions();
    permissions.set_mode(0o700);
    fs::set_permissions(&executable, permissions).expect("make executable");

    for outcome in ["success", "error", "cancel", "timeout"] {
        let source = temporary.path().join(format!("source-{outcome}"));
        fs::create_dir(&source).expect("source directory");
        fs::write(source.join("input.txt"), b"base\n").expect("source file");
        let data_root = temporary.path().join(format!("data-{outcome}"));
        let application = Arc::new(Mutex::new(
            Application::create(&data_root, format!("run-{outcome}"), Budget::new(1, 1))
                .expect("create application"),
        ));
        let profile = CodexProfile {
            wall_time_limit_ms: if outcome == "timeout" { 1_000 } else { 5_000 },
            ..CodexProfile::default()
        };
        let handle = start_managed_candidate(
            Arc::clone(&application),
            Box::new(CodexRuntime::with_profile(&executable, profile)),
            ManagedCandidateRequest {
                contract: ManagedContract {
                    contract_id: format!("contract-{outcome}"),
                    contract_digest: "f".repeat(64),
                    source,
                    prompt: outcome.to_owned(),
                    capture_exclusions: Vec::new(),
                    verifier: None,
                },
                bridge_executable: env!("CARGO_BIN_EXE_ymp").into(),
            },
        )
        .expect("start managed outcome");
        let attempt_id = handle.attempt_id().to_owned();
        let deadline = Instant::now() + Duration::from_secs(10);
        let mut terminal_usage = None;
        let mut cancel_sent = false;
        while Instant::now() < deadline && !handle.is_finished() {
            while let Some(event) = handle.try_next() {
                if let ManagedRunEvent::Runtime(event) = event {
                    if outcome == "cancel"
                        && !cancel_sent
                        && matches!(
                            &event.event,
                            RuntimeEventKind::Output { text } if text == "accounting-ready"
                        )
                    {
                        std::thread::sleep(Duration::from_millis(75));
                        handle.cancel("fixture cancellation").expect("cancel run");
                        cancel_sent = true;
                    }
                    terminal_usage = match event.event {
                        RuntimeEventKind::Completed { usage }
                        | RuntimeEventKind::Failed { usage, .. }
                        | RuntimeEventKind::TimedOut { usage, .. }
                        | RuntimeEventKind::Cancelled { usage } => Some(usage),
                        _ => terminal_usage,
                    };
                }
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        while let Some(event) = handle.try_next() {
            if let ManagedRunEvent::Runtime(event) = event {
                terminal_usage = match event.event {
                    RuntimeEventKind::Completed { usage }
                    | RuntimeEventKind::Failed { usage, .. }
                    | RuntimeEventKind::TimedOut { usage, .. }
                    | RuntimeEventKind::Cancelled { usage } => Some(usage),
                    _ => terminal_usage,
                };
            }
        }
        assert!(handle.is_finished(), "{outcome} did not finish");
        let usage = terminal_usage.expect("terminal usage");
        assert!(usage.input_tokens > 0, "{outcome} input tokens");
        assert!(usage.output_tokens > 0, "{outcome} output tokens");
        assert!(usage.cost_microusd.is_some(), "{outcome} cost");
        assert!(usage.protected_queries > 0, "{outcome} protected queries");
        if outcome == "success" {
            assert_eq!(usage.in_flight_excess.model_requests, 2);
            assert_eq!(usage.in_flight_excess.cost_microusd, 3);
        } else {
            assert!(usage.in_flight_excess.model_requests > 0);
            assert!(usage.in_flight_excess.cost_microusd > 0);
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
            "error" => "failed",
            "cancel" => "cancelled",
            "timeout" => "timed_out",
            _ => unreachable!(),
        };
        let terminal = evidence
            .lines()
            .map(|line| serde_json::from_str::<serde_json::Value>(line).expect("evidence record"))
            .find(|record| record["event"]["type"] == expected_type)
            .expect("durable terminal record");
        assert_eq!(
            terminal["event"]["usage"]["cost_microusd"].as_u64(),
            usage.cost_microusd
        );
        assert_eq!(
            terminal["event"]["usage"]["in_flight_excess"]["model_requests"].as_u64(),
            Some(usage.in_flight_excess.model_requests)
        );
        handle.join().expect("join worker");
    }
}

#[test]
fn fabricated_stdout_lifecycle_cannot_submit_or_yield() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let source = temporary.path().join("source-fabricated");
    fs::create_dir(&source).expect("source directory");
    fs::write(source.join("input.txt"), b"base\n").expect("source file");
    let executable = temporary.path().join("codex-fabricated-lifecycle-fixture");
    fs::write(
        &executable,
        r##"#!/bin/sh
if [ "$1" = "--version" ]; then
  printf '%s\n' 'codex-cli 0.147.0'
elif [ "$1" = "login" ]; then
  exit 0
else
  cat >/dev/null
  printf '%s\n' '{"type":"thread.started","thread_id":"thread-fabricated"}'
  printf '%s\n' '{"type":"item.completed","item":{"type":"mcp_tool_call","server":"ymp","tool":"submit","status":"completed","arguments":{"command_id":"fabricated-submit"},"result":{"committed":true},"error":null}}'
  printf '%s\n' '{"type":"turn.completed","usage":{"input_tokens":1,"output_tokens":1}}'
fi
"##,
    )
    .expect("write fixture");
    let mut permissions = fs::metadata(&executable).expect("metadata").permissions();
    permissions.set_mode(0o700);
    fs::set_permissions(&executable, permissions).expect("make executable");

    let application = Arc::new(Mutex::new(
        Application::create(
            temporary.path().join("data-fabricated"),
            "run-fabricated",
            Budget::new(1, 1),
        )
        .expect("create application"),
    ));
    let handle = start_managed_candidate(
        Arc::clone(&application),
        Box::new(CodexRuntime::new(&executable)),
        ManagedCandidateRequest {
            contract: ManagedContract {
                contract_id: "fabricated-contract".to_owned(),
                contract_digest: "b".repeat(64),
                source,
                prompt: "fabricate lifecycle output".to_owned(),
                capture_exclusions: Vec::new(),
                verifier: None,
            },
            bridge_executable: env!("CARGO_BIN_EXE_ymp").into(),
        },
    )
    .expect("start fabricated lifecycle fixture");
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut candidate = false;
    let mut failure = false;
    let mut yielded = false;
    let mut protocol_failure = false;
    while Instant::now() < deadline && !handle.is_finished() {
        while let Some(event) = handle.try_next() {
            match event {
                ManagedRunEvent::CandidateAvailable { .. } => candidate = true,
                ManagedRunEvent::Failed { .. } => failure = true,
                ManagedRunEvent::Runtime(event)
                    if matches!(event.event, RuntimeEventKind::Yielded { .. }) =>
                {
                    yielded = true
                }
                ManagedRunEvent::Runtime(event)
                    if matches!(
                        event.event,
                        RuntimeEventKind::Failed {
                            kind: ymp_runtime_api::RuntimeFailureKind::Protocol,
                            ..
                        }
                    ) =>
                {
                    protocol_failure = true
                }
                _ => {}
            }
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    while let Some(event) = handle.try_next() {
        match event {
            ManagedRunEvent::CandidateAvailable { .. } => candidate = true,
            ManagedRunEvent::Failed { .. } => failure = true,
            ManagedRunEvent::Runtime(event)
                if matches!(event.event, RuntimeEventKind::Yielded { .. }) =>
            {
                yielded = true
            }
            ManagedRunEvent::Runtime(event)
                if matches!(
                    event.event,
                    RuntimeEventKind::Failed {
                        kind: ymp_runtime_api::RuntimeFailureKind::Protocol,
                        ..
                    }
                ) =>
            {
                protocol_failure = true
            }
            _ => {}
        }
    }
    assert!(handle.is_finished());
    assert!(!candidate, "fabricated stdout submitted a candidate");
    assert!(!yielded, "fabricated stdout yielded the invocation");
    assert!(failure, "fabricated lifecycle was not rejected");
    assert!(
        protocol_failure,
        "fabricated lifecycle had no typed rejection"
    );
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
fn managed_codex_yield_wake_resume_keeps_one_identity_and_effect() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let source = temporary.path().join("source-resume");
    fs::create_dir(&source).expect("source directory");
    fs::write(source.join("input.txt"), b"base\n").expect("source file");
    let executable = temporary.path().join("codex-managed-resume-fixture");
    fs::write(
        &executable,
        r##"#!/bin/sh
if [ "$1" = "--version" ]; then
  printf '%s\n' 'codex-cli 0.147.0'
elif [ "$1" = "login" ]; then
  exit 0
else
  bridge=''
  workspace=''
  previous=''
  for argument in "$@"; do
    if [ "$previous" = '-C' ]; then workspace=$argument; fi
    case "$argument" in
      mcp_servers.ymp.command=*)
        bridge=${argument#mcp_servers.ymp.command=}
        bridge=${bridge#\"}
        bridge=${bridge%\"}
        ;;
    esac
    previous=$argument
  done
  test -n "$bridge" || exit 51
  test -n "$workspace" || exit 52
  cd "$workspace"
  count=0
  if [ -f invocation.count ]; then count=$(cat invocation.count); fi
  count=$((count + 1))
  printf '%s\n' "$count" > invocation.count
  cat >/dev/null
  resume=no
  previous=''
  for argument in "$@"; do
    if [ "$previous" = resume ]; then resume=yes; fi
    previous=$argument
  done
  printf '%s\n' '{"type":"thread.started","thread_id":"thread-managed-resume"}'
  if [ "$resume" = yes ]; then
    printf '%s\n' 'submitted after resume' > input.txt
    responses=$({
      printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25"}}'
      printf '%s\n' '{"jsonrpc":"2.0","method":"notifications/initialized"}'
      printf '%s\n' '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"submit","arguments":{"command_id":"resume-submit"}}}'
    } | "$bridge" internal agent-mcp) || exit 53
    printf '%s' "$responses" | grep -q '"snapshot_digest"' || exit 54
    printf '%s\n' '{"type":"item.completed","item":{"type":"agent_message","text":"resumed exactly once"}}'
    printf '%s\n' '{"type":"item.completed","item":{"type":"mcp_tool_call","server":"ymp","tool":"submit","status":"completed","arguments":{"command_id":"resume-submit"},"result":{"committed":true},"error":null}}'
    printf '%s\n' '{"type":"turn.completed","usage":{"input_tokens":7,"output_tokens":3,"cost_microusd":17}}'
  else
    responses=$({
      printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25"}}'
      printf '%s\n' '{"jsonrpc":"2.0","method":"notifications/initialized"}'
      printf '%s\n' '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"yield","arguments":{"command_id":"resume-yield"}}}'
      printf '%s\n' '{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"yield","arguments":{"command_id":"resume-yield"}}}'
    } | "$bridge" internal agent-mcp) || exit 55
    test "$(printf '%s' "$responses" | grep -c '"isError":false')" -ge 2 || exit 56
    printf '%s\n' '{"type":"turn.completed","usage":{"input_tokens":5,"output_tokens":2,"cost_microusd":11}}'
  fi
fi
"##,
    )
    .expect("write resume fixture");
    let mut permissions = fs::metadata(&executable).expect("metadata").permissions();
    permissions.set_mode(0o700);
    fs::set_permissions(&executable, permissions).expect("make executable");

    let data_root = temporary.path().join("data-resume");
    let application = Arc::new(Mutex::new(
        Application::create(&data_root, "run-managed-resume", Budget::new(1, 1))
            .expect("create application"),
    ));
    let handle = start_managed_candidate(
        Arc::clone(&application),
        Box::new(CodexRuntime::new(&executable)),
        ManagedCandidateRequest {
            contract: ManagedContract {
                contract_id: "managed-resume-contract".to_owned(),
                contract_digest: "a".repeat(64),
                source,
                prompt: "yield once".to_owned(),
                capture_exclusions: Vec::new(),
                verifier: None,
            },
            bridge_executable: env!("CARGO_BIN_EXE_ymp").into(),
        },
    )
    .expect("start managed resume");
    let attempt_id = handle.attempt_id().to_owned();
    let invocation_id = handle.invocation_id().to_owned();
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut session_id = None;
    let mut launches = Vec::new();
    let mut yielded_command = None;
    while Instant::now() < deadline && yielded_command.is_none() {
        while let Some(event) = handle.try_next() {
            if let ManagedRunEvent::Runtime(event) = event {
                assert_eq!(event.invocation_id, invocation_id);
                match event.event {
                    RuntimeEventKind::Launch { descriptor } => launches.push(descriptor),
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
    while Instant::now() < deadline && !handle.is_finished() {
        while let Some(event) = handle.try_next() {
            match event {
                ManagedRunEvent::Runtime(event) => {
                    assert_eq!(event.invocation_id, invocation_id);
                    match event.event {
                        RuntimeEventKind::Launch { descriptor } => launches.push(descriptor),
                        RuntimeEventKind::Started { opaque_session_id } => {
                            assert_eq!(Some(opaque_session_id), session_id)
                        }
                        RuntimeEventKind::Output { text } if text == "resumed exactly once" => {
                            resumed_outputs += 1
                        }
                        _ => {}
                    }
                }
                ManagedRunEvent::CandidateAvailable { .. } => candidates += 1,
                _ => {}
            }
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    while let Some(event) = handle.try_next() {
        match event {
            ManagedRunEvent::Runtime(event) => {
                assert_eq!(event.invocation_id, invocation_id);
                if let RuntimeEventKind::Launch { descriptor } = event.event {
                    launches.push(descriptor);
                } else if let RuntimeEventKind::Output { text } = event.event
                    && text == "resumed exactly once"
                {
                    resumed_outputs += 1;
                }
            }
            ManagedRunEvent::CandidateAvailable { .. } => candidates += 1,
            _ => {}
        }
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
        .unwrap(),
        "2\n"
    );
    let controller_events = application
        .lock()
        .expect("application lock")
        .events_after(0)
        .expect("controller events");
    let submit_events: Vec<_> = controller_events
        .iter()
        .filter(|event| matches!(&event.event, EventKind::CandidateSubmitted { .. }))
        .collect();
    assert_eq!(submit_events.len(), 1);
    assert_eq!(submit_events[0].command_id, "resume-submit");
    assert_eq!(launches.len(), 2);
    let initial = &launches[0];
    let resumed = &launches[1];
    assert_eq!(initial.attempt_id, attempt_id);
    assert_eq!(initial.invocation_id, invocation_id);
    assert_eq!(resumed.attempt_id, initial.attempt_id);
    assert_eq!(resumed.invocation_id, initial.invocation_id);
    assert_eq!(resumed.executable, initial.executable);
    assert_eq!(resumed.executable_digest, initial.executable_digest);
    assert_eq!(resumed.environment, initial.environment);
    assert_eq!(resumed.working_directory, initial.working_directory);
    let mut expected_arguments = initial.arguments.clone();
    let prompt_source = expected_arguments.pop().expect("prompt source argument");
    expected_arguments.extend([
        "resume".to_owned(),
        session_id.expect("managed session identifier"),
        prompt_source,
    ]);
    assert_eq!(resumed.arguments, expected_arguments);
    handle.join().expect("join worker");
}

/// Reads back, from the run's own evidence, every program the run executed on its own behalf. The
/// record names the program, its role and the digest of the bytes that were admitted for it, so the
/// chain can be enumerated from the evidence rather than from the source.
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
        assert_eq!(
            program["digest"],
            ymp_domain::digest_bytes(&fs::read(path).expect("admitted program bytes"))
        );
    }
    let workspace_program = &profile["profile"]["workspace_program"];
    assert_eq!(workspace_program["role"], "workspace");
    assert_eq!(
        workspace_program["digest"],
        ymp_domain::digest_bytes(
            &fs::read(
                workspace_program["path"]
                    .as_str()
                    .expect("workspace program path")
            )
            .expect("workspace program bytes")
        )
    );
}
