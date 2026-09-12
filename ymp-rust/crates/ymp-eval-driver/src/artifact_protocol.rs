use crate::{
    export, protocols,
    task_protocol::{NativeAction, TaskHarness},
    write_json,
};
use anyhow::{ensure, Result};
use serde_json::{json, Value};
use std::{path::Path, time::Instant};
use ymp_core::*;
pub async fn run(root: &Path, directory: &Path) -> Result<Value> {
    let started = Instant::now();
    let checker = root.join("ymp-evals/driver/protocol_checks.py");
    let contract=AcceptanceContract{knowledge_correction:None,task_title:"Versioned artifact".into(),criteria:vec![AcceptanceCriterion{id:"versioned-content".into(),description:"The controlled artifact contains either valid versioned payload; each changed byte version requires fresh checking".into()}],artifacts:vec!["outputs/version.txt".into()],inputs:vec![],checks:vec![TrustedCheck{id:"versioned-bytes".into(),criterion_ids:vec!["versioned-content".into()],assertion:CheckAssertion::Command{program:"/usr/bin/env".into(),args:vec!["python3".into(),checker.display().to_string(),"artifact-version".into(),"{workdir}".into()],verifier_files:vec![checker]}}]};
    let h = TaskHarness::create(directory, "Artifact version freshness", vec![contract]).await?;
    let mut task = h.new_task("Versioned artifact");
    let first = h
        .produce(
            &mut task,
            "a",
            NativeAction {
                writes: vec![("outputs/version.txt".into(), b"version 1\n".to_vec())],
                reads: vec![],
                response: "Produced the first controlled artifact version".into(),
            },
        )
        .await?;
    let checks1 = h.check(&first).await?;
    let review1 = h.review(&task, &first, "b", true).await?;
    let before = FileSnapshot::capture(&h.directory, Path::new("outputs/version.txt"))?;
    std::fs::write(h.directory.join("outputs/version.txt"), b"version 2\n")?;
    let after = FileSnapshot::capture(&h.directory, Path::new("outputs/version.txt"))?;
    let current = h
        .store
        .result_is_current(&h.session.id, first.links.result.as_ref().unwrap())?;
    h.store.event(&h.session.id,"eval_artifact_changed",&json!({"result_id":task.id,"version":task.attempts+1,"before":before,"after":after,"previous_confirmation":checks1[0].id,"confirmation_valid":current}))?;
    // A confirmed candidate has not yet been accepted. Its changed artifact
    // invalidates that attempt before a fresh producing assignment inspects it.
    h.accept(&mut task, &first, &review1, false)?;
    let second = h
        .produce(
            &mut task,
            "a",
            NativeAction {
                writes: vec![],
                reads: vec!["outputs/version.txt".into()],
                response: "Inspected and submitted the actual second artifact version".into(),
            },
        )
        .await?;
    ensure!(
        second.links.result.as_ref().unwrap().artifacts[0] == after,
        "New result is not the observed changed artifact"
    );
    let stale = h.accept(&mut task, &second, &review1, true);
    h.store.event(&h.session.id,"eval_acceptance_attempt",&json!({"result_id":task.id,"version":task.attempts,"review_id":review1.id,"accepted":stale.is_ok(),"error":stale.err().map(|e|e.to_string())}))?;
    if task.state == TaskState::Review {
        let review2 = h.review(&task, &second, "b", true).await?;
        h.check(&second).await?;
        h.accept(&mut task, &second, &review2, true)?;
    }
    let trace = h.store.trace(&h.session.id)?;
    let mut rows = Vec::new();
    let mut sources = Vec::new();
    let mut confirmation_aliases = std::collections::BTreeMap::new();
    let mut prior_reviews = std::collections::BTreeMap::new();
    for event in &trace.history {
        let row = if event.kind == "eval_artifact_changed" {
            Some(
                json!({"type":"artifact_changed","result":"r1","version":event.data["version"],"previous_confirmation":confirmation_aliases.get(event.data["previous_confirmation"].as_str().unwrap()),"confirmation_valid":event.data["confirmation_valid"]}),
            )
        } else if event.kind == "eval_acceptance_attempt" {
            Some(
                json!({"type":if event.data["accepted"]==true{"acceptance_accepted"}else{"acceptance_rejected"},"result":"r1","version":event.data["version"],"reason":if event.data["error"].as_str().is_some_and(|e|e.contains("review binding mismatch") || e.contains("another task attempt")){"stale_review_and_confirmation"}else{"unexpected_acceptance_result"}}),
            )
        } else if event.kind == "provenance" {
            match serde_json::from_value::<ProvenanceEvent>(event.data.clone())? {
                ProvenanceEvent::DecisionRecorded { decision }
                    if decision.kind == "candidate_review" =>
                {
                    let result = decision.links.result.unwrap();
                    prior_reviews.insert((result.id, result.version), decision.actor);
                    None
                }
                ProvenanceEvent::DecisionRecorded { decision }
                    if decision.kind == "check_observed" =>
                {
                    let result = decision.links.result.unwrap();
                    let alias = format!("c{}", confirmation_aliases.len() + 1);
                    confirmation_aliases.insert(decision.id.clone(), alias.clone());
                    let check = decision.links.check.unwrap();
                    let mut row = json!({"type":if check.outcome==ConfirmationCheckOutcome::Passed{"result_confirmed"}else{"result_check_failed"},"result":"r1","version":result.version,"confirmation":alias});
                    if let Some(actor) = prior_reviews.get(&(result.id, result.version)) {
                        row["reviewer"] = json!(actor);
                    }
                    Some(row)
                }
                _ => None,
            }
        } else {
            None
        };
        if let Some(row) = row {
            sources.push(json!({"projected_index":rows.len(),"runtime_event_seq":event.seq}));
            rows.push(row);
        }
    }
    let current = h
        .store
        .confirmation_grade(&h.session.id, second.links.result.as_ref().unwrap())?;
    let observed = json!({"schema_version":1,"case_id":"artifact-version","events":rows,"final_state":{"current_version":task.attempts,"current_confirmation":current.1.first().and_then(|id|confirmation_aliases.get(id)),"historical_confirmations":trace.decisions.iter().filter(|d|d.kind=="check_observed").filter_map(|d|confirmation_aliases.get(&d.id)).collect::<Vec<_>>()}});
    write_json(&directory.join("runtime.json"), &trace)?;
    h.journal.save(directory)?;
    write_json(&directory.join("observed.json"), &observed)?;
    write_json(
        &directory.join("alias-map.json"),
        &json!({"r1":task.id,"confirmations":confirmation_aliases,"projection_sources":sources}),
    )?;
    let metrics = export::metrics(&trace, started.elapsed().as_secs_f64(), 1)?;
    write_json(&directory.join("metrics.json"), &metrics)?;
    let validation = protocols::validate(root, directory, "artifact-version").await?;
    Ok(
        json!({"case_id":"artifact-version","complete":validation["validator_passed"]==true,"validator":validation,"metrics":metrics}),
    )
}
