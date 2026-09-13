use serde_json::json;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use ymp_providers::{run_turn, ProviderEvent, TurnRequest};

#[tokio::test]
async fn bridge_tool_activity_follows_native_execution_metadata() {
    let dir = tempfile::tempdir().unwrap();
    let bridge = dir.path().join("bridge.cjs");
    std::fs::write(&bridge, r#"
const rl = require('node:readline').createInterface({ input: process.stdin });
const send = value => process.stdout.write(JSON.stringify(value) + '\n');
rl.on('line', line => {
  const request = JSON.parse(line);
  send({method:'execution',params:{reported:{model:'native-fixture-model',effort:'xhigh',permission_mode:null}}});
  send({method:'tool',params:{name:'Read'}});
  send({jsonrpc:'2.0',id:request.id,result:{text:'done',session_id:'fixture',usage:null}});
});
"#).unwrap();
    let request: TurnRequest = serde_json::from_value(json!({
        "profile":{"id":"fixture-actor","name":"Caption","provider":"fixture","instructions":""},
        "provider":{"id":"fixture","kind":"claude","command":"/bin/false"},
        "settings":{},"cwd":dir.path(),"prompt":"fixture","purpose":"review","read_only":true,
        "resume":null,"mcp":null,"timeout_secs":5,"bridge":bridge
    }))
    .unwrap();
    let (tx, mut rx) = mpsc::unbounded_channel();
    let result = run_turn(request, CancellationToken::new(), tx)
        .await
        .unwrap();
    assert_eq!(result.text, "done");
    let mut observed = false;
    let mut tools = vec![];
    while let Ok(event) = rx.try_recv() {
        match event {
            ProviderEvent::Execution(value) => {
                observed |= value
                    .reported
                    .as_ref()
                    .is_some_and(|r| r.effort.as_deref() == Some("xhigh"));
            }
            ProviderEvent::Tool(name) => {
                assert!(observed, "Tool activity overtook its native metadata");
                tools.push(name);
            }
            _ => {}
        }
    }
    assert_eq!(tools, ["Read"]);
}
