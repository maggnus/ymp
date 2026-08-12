use serde_json::{Value, json};
use tempfile::tempdir;
use ymp_agent_mcp::McpServer;
use ymp_application::{Application, WorkspaceSubmission};
use ymp_domain::{Budget, Command, EventKind};

fn request<H: ymp_agent_api::AgentToolHandler>(server: &mut McpServer<H>, request: Value) -> Value {
    serde_json::from_str(
        &server
            .handle_line(&request.to_string())
            .expect("request has response"),
    )
    .expect("parse MCP response")
}

#[test]
fn controller_bound_attempt_reads_state_and_submits_through_mcp() {
    let temporary = tempdir().expect("temporary data root");
    let mut app = Application::create(temporary.path(), "run-1", Budget::new(1, 1))
        .expect("create application");
    app.execute(
        "start",
        Command::StartAttempt {
            attempt_id: "attempt-1".to_owned(),
        },
    )
    .expect("start attempt");
    let source = temporary.path().join("source");
    let workspace = temporary.path().join("workspace");
    std::fs::create_dir(&source).expect("source directory");
    std::fs::write(source.join("result.txt"), b"before\n").expect("source file");
    let base = app
        .artifact_store()
        .capture_source(&source)
        .expect("capture base");
    app.artifact_store()
        .materialize(&base.manifest_digest, &workspace)
        .expect("materialize workspace");
    std::fs::write(workspace.join("result.txt"), b"after\n").expect("candidate edit");

    let submitted_digest;
    {
        let mut server = McpServer::new(app.workspace_agent_session(
            "attempt-1",
            WorkspaceSubmission::new(&base.manifest_digest, &workspace, Vec::new()),
        ));
        request(
            &mut server,
            json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize" }),
        );
        let control = request(
            &mut server,
            json!({
                "jsonrpc": "2.0",
                "id": 2,
                "method": "tools/call",
                "params": { "name": "read_control", "arguments": {} }
            }),
        );
        assert_eq!(control["result"]["structuredContent"]["run_id"], "run-1");

        let submitted = request(
            &mut server,
            json!({
                "jsonrpc": "2.0",
                "id": 3,
                "method": "tools/call",
                "params": {
                    "name": "submit",
                    "arguments": {
                        "command_id": "agent.submit-1"
                    }
                }
            }),
        );
        assert_eq!(submitted["result"]["isError"], false);
        submitted_digest = submitted["result"]["structuredContent"]["candidate"]["snapshot_digest"]
            .as_str()
            .expect("candidate digest")
            .to_owned();

        let events = request(
            &mut server,
            json!({
                "jsonrpc": "2.0",
                "id": 4,
                "method": "tools/call",
                "params": {
                    "name": "read_events",
                    "arguments": { "cursor": 2, "limit": 1 }
                }
            }),
        );
        assert_eq!(
            events["result"]["structuredContent"]["events"]
                .as_array()
                .expect("event page")
                .len(),
            1
        );
        assert_eq!(events["result"]["structuredContent"]["next_cursor"], 3);
    }

    assert_eq!(
        app.state().candidate_digest.as_deref(),
        Some(submitted_digest.as_str())
    );
    assert!(matches!(
        app.events_after(2).expect("read events")[0].event,
        EventKind::CandidateSubmitted { .. }
    ));
}

#[test]
fn lost_mcp_reply_replays_submit_only_under_the_same_command_identifier() {
    let temporary = tempdir().expect("temporary data root");
    let mut app = Application::create(temporary.path(), "run-1", Budget::new(1, 1))
        .expect("create application");
    app.execute(
        "start",
        Command::StartAttempt {
            attempt_id: "attempt-1".to_owned(),
        },
    )
    .expect("start attempt");
    let source = temporary.path().join("source");
    let workspace = temporary.path().join("workspace");
    std::fs::create_dir(&source).expect("source directory");
    std::fs::write(source.join("result.txt"), b"before\n").expect("source file");
    let base = app
        .artifact_store()
        .capture_source(&source)
        .expect("capture base");
    app.artifact_store()
        .materialize(&base.manifest_digest, &workspace)
        .expect("materialize workspace");
    std::fs::write(workspace.join("result.txt"), b"after\n").expect("candidate edit");

    let events_before_submit = app.events_after(0).expect("events before submit").len();
    let committed_digest;
    {
        let mut server = McpServer::new(app.workspace_agent_session(
            "attempt-1",
            WorkspaceSubmission::new(&base.manifest_digest, &workspace, Vec::new()),
        ));
        request(
            &mut server,
            json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize" }),
        );

        let discarded_reply = server
            .handle_line(
                &json!({
                    "jsonrpc": "2.0",
                    "id": 2,
                    "method": "tools/call",
                    "params": {
                        "name": "submit",
                        "arguments": {
                            "command_id": "agent.submit.lost-reply"
                        }
                    }
                })
                .to_string(),
            )
            .expect("submit has response");
        drop(discarded_reply);

        let replay = request(
            &mut server,
            json!({
                "jsonrpc": "2.0",
                "id": 3,
                "method": "tools/call",
                "params": {
                    "name": "submit",
                    "arguments": {
                        "command_id": "agent.submit.lost-reply"
                    }
                }
            }),
        );
        assert_eq!(replay["result"]["isError"], false);
        assert_eq!(
            replay["result"]["structuredContent"]["command"]["replayed"],
            true
        );
        committed_digest = replay["result"]["structuredContent"]["candidate"]["snapshot_digest"]
            .as_str()
            .expect("replayed candidate digest")
            .to_owned();
    }

    assert_eq!(
        app.state().candidate_digest.as_deref(),
        Some(committed_digest.as_str())
    );
    assert_eq!(
        app.events_after(0).expect("final events").len(),
        events_before_submit + 1
    );
}

#[test]
fn attempt_scope_is_controller_bound_and_invalid_pages_are_tool_errors() {
    let temporary = tempdir().expect("temporary data root");
    let mut app = Application::create(temporary.path(), "run-1", Budget::new(1, 1))
        .expect("create application");
    app.execute(
        "start",
        Command::StartAttempt {
            attempt_id: "attempt-authorized".to_owned(),
        },
    )
    .expect("start attempt");
    let mut server = McpServer::new(app.agent_session("attempt-not-authorized"));
    request(
        &mut server,
        json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize" }),
    );

    let rejected = request(
        &mut server,
        json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/call",
            "params": {
                "name": "submit",
                "arguments": {
                    "command_id": "agent.submit-1"
                }
            }
        }),
    );
    assert_eq!(rejected["result"]["isError"], true);
    assert_eq!(rejected["result"]["structuredContent"]["code"], "rejected");

    let invalid_page = request(
        &mut server,
        json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/call",
            "params": { "name": "read_events", "arguments": { "limit": 129 } }
        }),
    );
    assert_eq!(invalid_page["result"]["isError"], true);
    assert_eq!(
        invalid_page["result"]["structuredContent"]["code"],
        "invalid_arguments"
    );

    let tools = request(
        &mut server,
        json!({ "jsonrpc": "2.0", "id": 4, "method": "tools/list" }),
    );
    let names: Vec<_> = tools["result"]["tools"]
        .as_array()
        .expect("tool catalog")
        .iter()
        .map(|tool| tool["name"].as_str().expect("tool name"))
        .collect();
    assert_eq!(names, ["read_control", "read_events", "submit"]);
    assert!(!tools.to_string().contains("oracle"));
    assert!(!tools.to_string().contains("verif"));
}
