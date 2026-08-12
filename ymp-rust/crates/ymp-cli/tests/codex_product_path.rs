#![forbid(unsafe_code)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use ymp_application::Application;
use ymp_domain::Budget;
use ymp_runtime_codex::CodexRuntime;
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
  test "$OPENAI_BASE_URL" = 'https://api.openai.com/v1' || exit 33
  test -z "${OPENAI_ORGANIZATION+x}" || exit 34
  test -z "${OPENAI_PROJECT+x}" || exit 35
  test -n "$YMP_AGENT_SOCKET" || exit 36
  test -n "$YMP_AGENT_TOKEN" || exit 37
  test -n "$YMP_ATTEMPT_ID" || exit 38
  printf '%s\n' 'after through product MCP' > "$workspace/input.txt"
  {
    printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25"}}'
    printf '%s\n' '{"jsonrpc":"2.0","method":"notifications/initialized"}'
    printf '%s\n' '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"read_control","arguments":{}}}'
    printf '%s\n' '{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"submit","arguments":{"command_id":"agent.submit.fake-product"}}}'
  } | "$bridge" internal agent-mcp > "$(dirname "$0")/mcp.responses"
  grep -q '"snapshot_digest"' "$(dirname "$0")/mcp.responses" || exit 39
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

    let responses =
        fs::read_to_string(temporary.path().join("mcp.responses")).expect("MCP responses");
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
    assert!(!evidence.contains("YMP_AGENT_TOKEN"));

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
    assert_eq!(profile["profile"]["schema_version"], 2);
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
    for digest in [
        &profile["digest"],
        &profile["profile"]["runtime_executable_digest"],
        &profile["profile"]["coordination"]["bridge_executable_digest"],
        &profile["profile"]["coordination"]["endpoint_path_digest"],
    ] {
        assert_eq!(digest.as_str().expect("evidence digest").len(), 64);
    }
    for event in evidence.lines() {
        let event: serde_json::Value = serde_json::from_str(event).expect("runtime event evidence");
        assert_eq!(event["schema_version"], 2);
        assert_eq!(event["profile_digest"], profile["digest"]);
        assert_eq!(event["invocation_id"], profile["profile"]["invocation_id"]);
    }
    assert!(!profile.to_string().contains("YMP_AGENT_TOKEN"));
}
