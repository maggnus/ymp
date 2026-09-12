// Explicit real-provider diagnostic. Never run by cargo test.
use anyhow::{Context, Result};
use std::path::Path;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use ymp_core::{Config, UsageSnapshot};
use ymp_providers::{discovery, run_turn, ProviderEvent, TurnRequest};

#[tokio::main]
async fn main() -> Result<()> {
    let provider_id = std::env::args()
        .nth(1)
        .context("Pass codex, claude or glm")?;
    let mut config = Config::default();
    discovery::discover_glm(&mut config)?;
    let mut profile = config
        .agents
        .iter()
        .find(|a| a.provider == provider_id)
        .context("Unknown provider")?
        .clone();
    profile.id = "probe-agent".into();
    profile.name = "Probe agent".into();
    let provider = config.provider(&provider_id)?.clone();
    let directory = tempfile::tempdir()?;
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .unwrap();
    let mut resume = None;
    let mut baseline = None;
    for turn in 1..=2 {
        let request = TurnRequest {
            settings: config.execution_settings(&profile, &Default::default())?,
            profile: profile.clone(),
            provider: provider.clone(),
            cwd: directory.path().into(),
            prompt: "Reply exactly OK. Do not use tools or change files.".into(),
            purpose: "probe".into(),
            read_only: true,
            resume: resume.clone(),
            usage_baseline: baseline.clone(),
            mcp: None,
            timeout_secs: 180,
            bridge: root.join("ymp-bridges/claude/dist/index.js"),
        };
        let (tx, mut rx) = mpsc::unbounded_channel();
        let future = run_turn(request, CancellationToken::new(), tx);
        tokio::pin!(future);
        let mut latest: Option<UsageSnapshot> = None;
        let mut updates = 0;
        let result = loop {
            tokio::select! {
                result=&mut future=>break result?,
                Some(event)=rx.recv()=>if let ProviderEvent::Usage(snapshot)=event{latest=Some(snapshot);updates+=1;},
            }
        };
        while let Ok(event) = rx.try_recv() {
            if let ProviderEvent::Usage(snapshot) = event {
                latest = Some(snapshot);
                updates += 1;
            }
        }
        let snapshot = latest.context("No usage reported")?;
        println!(
            "{}",
            serde_json::json!({"provider":provider_id,"agent":profile.id,"turn":turn,"updates":updates,"usage":snapshot})
        );
        if let Some(previous) = &baseline {
            if let Some(expected) = snapshot
                .native_total
                .as_ref()
                .and_then(|n| n.checked_difference(previous))
            {
                anyhow::ensure!(
                    snapshot.counts == expected
                        || snapshot.native_total.as_ref() == Some(&snapshot.counts)
                        || snapshot.partial,
                    "History was double-counted"
                );
            } else {
                anyhow::ensure!(
                    snapshot.partial || snapshot.native_total.as_ref() == Some(&snapshot.counts),
                    "A reset must use a confirmed fresh baseline or be marked partial"
                );
            }
        }
        baseline = snapshot.native_total;
        resume = Some(result.session_id);
    }
    Ok(())
}
