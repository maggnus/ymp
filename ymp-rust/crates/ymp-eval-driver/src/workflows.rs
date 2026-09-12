use crate::{
    export, require_non_git,
    script::{NativeJournal, ScriptedBackend},
    write_json,
};
use anyhow::{ensure, Context, Result};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{path::Path, sync::Arc, time::Instant};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use ymp_core::*;
use ymp_runtime::Engine;
use ymp_storage::Store;

#[derive(Clone, Deserialize)]
pub struct WorkflowSpec {
    pub prompt: String,
    pub inputs: Vec<String>,
    pub output: String,
    pub criterion_ids: Vec<String>,
    pub check_id: Option<String>,
    pub follow_up_prompt: Option<String>,
}
pub fn config() -> Config {
    let mut config = Config {
        providers: vec![ProviderConfig {
            id: "scripted".into(),
            kind: ProviderKind::Mock,
            command: "internal-scripted-backend".into(),
            args: vec![],
            env_refs: Default::default(),
            enabled: true,
        }],
        agents: ["a", "b"]
            .iter()
            .map(|id| AgentProfile {
                id: (*id).into(),
                name: format!("Scripted {id}"),
                provider: "scripted".into(),
                model: Some("scripted-small".into()),
                instructions:
                    "Complete only the assigned fixture task using the selected directory.".into(),
                enabled: true,
            })
            .collect(),
        team: vec!["a".into(), "b".into()],
        limits: Limits {
            parallel: 1,
            turns: 40,
            turn_timeout_secs: 10,
            attempts: 2,
            resources: Some(ResourceLimits::default()),
        },
        team_constraints: TeamConstraints {
            fixed_roster: Some(vec!["a".into(), "b".into()]),
            ..Default::default()
        },
        ..Default::default()
    };
    for id in ["a", "b"] {
        config.execution.insert(
            id.into(),
            AgentExecutionPolicy {
                fixed: ModelEffort {
                    model: Some("scripted-small".into()),
                    effort: Some("low".into()),
                },
                ..Default::default()
            },
        );
    }
    config
}
pub async fn run(root: &Path, directory: &Path, case: &str, spec: WorkflowSpec) -> Result<Value> {
    let started = Instant::now();
    let work = directory.join("work");
    std::fs::create_dir(&work)?;
    require_non_git(&work)?;
    for input in &spec.inputs {
        let destination = work.join(input);
        std::fs::create_dir_all(destination.parent().unwrap())?;
        std::fs::copy(
            root.join("ymp-evals/fixtures/universal").join(input),
            destination,
        )?;
    }
    let store = Store::open(&directory.join("metadata"))?;
    let (events, mut received) = mpsc::unbounded_channel();
    let ui = tokio::spawn(async move {
        let mut journal = Vec::new();
        while let Some(event) = received.recv().await {
            journal.push(format!("{event:?}"));
        }
        journal
    });
    let journal = Arc::new(NativeJournal::default());
    let task_title = format!("Deliver {case} artifact");
    let backend = Arc::new(ScriptedBackend {
        case: case.into(),
        task_title: task_title.clone(),
        task_prompt: spec.prompt.clone(),
        output: spec.output.clone(),
        writer: root.join("ymp-evals/driver/artifact_writer.py"),
        journal: journal.clone(),
    });
    let mut engine = Engine::new(store.clone(), config(), events, CancellationToken::new())?
        .with_execution_backend(backend)?;
    engine.use_memory = false;
    let checks = spec
        .check_id
        .as_ref()
        .map(|id| TrustedCheck {
            id: id.clone(),
            criterion_ids: spec.criterion_ids.clone(),
            assertion: CheckAssertion::Command {
                program: "/usr/bin/env".into(),
                args: vec![
                    "python3".into(),
                    root.join("ymp-evals/validators/universal.py")
                        .display()
                        .to_string(),
                    "artifact".into(),
                    "--case".into(),
                    case.into(),
                    "--workdir".into(),
                    "{workdir}".into(),
                ],
                verifier_files: vec![
                    root.join("ymp-evals/validators/universal.py"),
                    root.join("ymp-evals/scenarios/universal-workflows.json"),
                ],
            },
        })
        .into_iter()
        .collect();
    engine.acceptance_contracts.push(AcceptanceContract {
        task_title,
        criteria: spec
            .criterion_ids
            .iter()
            .map(|id| AcceptanceCriterion {
                id: id.clone(),
                description: spec.prompt.clone(),
            })
            .collect(),
        artifacts: vec![spec.output.clone().into()],
        inputs: spec.inputs.iter().map(Into::into).collect(),
        checks,
    });
    let outcome = engine.run(&work, &spec.prompt, None).await?;
    let before = store.trace(&outcome.session.id)?;
    write_json(&directory.join("runtime.json"), &before)?;
    journal.save(directory)?;
    let follow_up = if let Some(prompt) = &spec.follow_up_prompt {
        let response = engine.follow_up(&work, prompt, &outcome.session.id).await?;
        let stored = store
            .outcomes(&outcome.session.id)?
            .into_iter()
            .find(|o| {
                o.artifacts
                    .iter()
                    .any(|a| a.path == work.join(&spec.output) || a.path == Path::new(&spec.output))
            })
            .context("Location follow-up has no stored artifact")?;
        let path = stored.directory.join(&spec.output);
        ensure!(
            response.summary.contains(&path.display().to_string()),
            "Follow-up response did not return stored path: {}",
            response.summary
        );
        Some(
            json!({"session_id":response.session.id,"result_id":stored.result_id,"answer_path":path,"artifact_sha256":crate::file_digest(&path)?,"invocations_started":store.trace(&outcome.session.id)?.invocations.len().saturating_sub(before.invocations.len())}),
        )
    } else {
        None
    };
    let trace = store.trace(&outcome.session.id)?;
    write_json(&directory.join("runtime.json"), &trace)?;
    journal.save(directory)?;
    write_json(&directory.join("alias-map.json"), &export::aliases(&trace))?;
    drop(engine);
    write_json(&directory.join("ui-events.json"), &ui.await?)?;
    let metrics = export::metrics(&trace, started.elapsed().as_secs_f64(), journal.peak())?;
    write_json(&directory.join("metrics.json"), &metrics)?;
    // Export the actual state before asserting completion, so failed runs retain
    // their entire evidence rather than receiving a synthetic positive receipt.
    let receipt = export::workflow(case, &work, &trace, follow_up)?;
    write_json(&directory.join("workflow.json"), &receipt)?;
    let validator = tokio::process::Command::new("python3")
        .arg(root.join("ymp-evals/validators/universal.py"))
        .args(["workflow", "--case", case, "--workdir"])
        .arg(&work)
        .arg("--observed")
        .arg(directory.join("workflow.json"))
        .output()
        .await?;
    let validation: Value =
        serde_json::from_slice(&validator.stdout).context("Validator did not return JSON")?;
    write_json(&directory.join("validator.json"), &validation)?;
    Ok(
        json!({"case_id":case,"complete":validator.status.success() && outcome.session.status=="completed","session_id":outcome.session.id,"runtime_status":outcome.session.status,"validator":validation,"metrics":metrics}),
    )
}
