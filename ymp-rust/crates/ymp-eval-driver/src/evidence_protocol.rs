use crate::{
    export, protocols,
    task_protocol::{exact_contract, NativeAction, TaskHarness},
    write_json,
};
use anyhow::Result;
use serde_json::{json, Value};
use std::{path::Path, time::Instant};
use ymp_core::*;
pub async fn run(root: &Path, directory: &Path) -> Result<Value> {
    let started = Instant::now();
    let h = TaskHarness::create(
        directory,
        "Failing evidence and self-review",
        vec![exact_contract(
            "Controlled totals",
            "posted-account-totals",
            "outputs/totals.txt",
            b"declared totals\n",
        )],
    )
    .await?;
    let mut task = h.new_task("Controlled totals");
    let result = h
        .produce(
            &mut task,
            "a",
            NativeAction {
                writes: vec![("outputs/totals.txt".into(), b"incorrect totals\n".to_vec())],
                reads: vec![],
                response: "Candidate totals ready for independent inspection".into(),
            },
        )
        .await?;
    let self_review = h.review(&task, &result, "a", true).await;
    h.store.event(&h.session.id,"eval_review_attempt",&json!({"result_id":task.id,"agent":"a","accepted":self_review.is_ok(),"error":self_review.err().map(|e|e.to_string())}))?;
    let output = tokio::process::Command::new("/usr/bin/true")
        .output()
        .await?;
    h.store.event(&h.session.id,"eval_unrelated_check",&json!({"command":"true","exit_code":output.status.code(),"stdout":output.stdout,"stderr":output.stderr,"check_id":"unrelated","contract_id":Value::Null}))?;
    let review = h.review(&task, &result, "b", true).await?;
    h.check(&result).await?;
    let accepted = h.accept(&mut task, &result, &review, true);
    h.store.event(&h.session.id,"eval_acceptance_attempt",&json!({"result_id":task.id,"accepted":accepted.is_ok(),"error":accepted.err().map(|e|e.to_string())}))?;
    if task.state == TaskState::Review {
        h.accept(&mut task, &result, &review, false)?;
    }
    // The independent aggregate is an explicit second selected synthetic task
    // directory. It cannot import confirmation from the rejected first result.
    let aggregate_dir = directory.join("aggregate");
    std::fs::create_dir(&aggregate_dir)?;
    let a = TaskHarness::create(
        &aggregate_dir,
        "Mixed confirmation aggregate",
        vec![exact_contract(
            "Component one",
            "component1-content",
            "outputs/component1.txt",
            b"component one\n",
        )],
    )
    .await?;
    let mut components = Vec::new();
    for (title, path, text) in [
        (
            "Component one",
            "outputs/component1.txt",
            b"component one\n".to_vec(),
        ),
        (
            "Component two",
            "outputs/component2.txt",
            b"component two\n".to_vec(),
        ),
    ] {
        let mut task = a.new_task(title);
        let submission = a
            .produce(
                &mut task,
                "a",
                NativeAction {
                    writes: vec![(path.into(), text)],
                    reads: vec![],
                    response: format!("Produced {title}"),
                },
            )
            .await?;
        let review = a.review(&task, &submission, "b", true).await?;
        if title == "Component one" {
            a.check(&submission).await?;
        }
        a.accept(&mut task, &submission, &review, true)?;
        components.push(submission);
    }
    let aggregate = a.aggregate(&components)?;
    let grade = a
        .store
        .confirmation_grade(&a.session.id, aggregate.links.result.as_ref().unwrap())?;
    a.store.event(&a.session.id,"eval_aggregate_grade",&json!({"result_id":aggregate.links.result.as_ref().unwrap().id,"confirmation":grade.0,"components":components.iter().enumerate().map(|(i,result)|json!({"alias":format!("component{}",i+1),"result_id":result.links.result.as_ref().unwrap().id,"confirmation":a.store.confirmation_grade(&a.session.id,result.links.result.as_ref().unwrap()).unwrap().0})).collect::<Vec<_>>()}))?;
    let trace = h.store.trace(&h.session.id)?;
    let second = a.store.trace(&a.session.id)?;
    let mut rows = Vec::new();
    let mut sources = Vec::new();
    for (session, trace) in [("s1", &trace), ("s2", &second)] {
        for event in &trace.history {
            let row = match event.kind.as_str() {
                "eval_review_attempt" => Some(
                    json!({"type":if event.data["accepted"]==true{"self_review_accepted"}else{"review_rejected"},"result":"r1","reason":if event.data["error"].as_str().is_some_and(|e|e.to_lowercase().contains("self") || e.contains("excludes every result producer")){"self_review"}else{"unexpected_review_error"}}),
                ),
                "eval_unrelated_check" => Some(
                    json!({"type":"check_recorded","check":event.data["check_id"],"exit_code":event.data["exit_code"],"applicable":!event.data["contract_id"].is_null()}),
                ),
                "eval_acceptance_attempt" => Some(
                    json!({"type":if event.data["accepted"]==true{"acceptance_accepted"}else{"acceptance_rejected"},"result":"r1","reason":if event.data["error"].as_str().is_some_and(|e|e.contains("Failed applicable evidence")){"applicable_failing_evidence"}else{"unexpected_acceptance_error"}}),
                ),
                "eval_aggregate_grade" => Some(
                    json!({"type":if event.data["confirmation"]=="unconfirmed"{"aggregate_unconfirmed"}else{"aggregate_confirmed"},"confirmed_components":event.data["components"].as_array().unwrap().iter().filter(|c|c["confirmation"]=="confirmed").map(|c|c["alias"].clone()).collect::<Vec<_>>(),"unconfirmed_components":event.data["components"].as_array().unwrap().iter().filter(|c|c["confirmation"]!="confirmed").map(|c|c["alias"].clone()).collect::<Vec<_>>()}),
                ),
                "provenance" => {
                    match serde_json::from_value::<ProvenanceEvent>(event.data.clone())? {
                        ProvenanceEvent::DecisionRecorded { decision }
                            if decision.kind == "candidate_review" && session == "s1" =>
                        {
                            Some(
                                json!({"type":"review_recorded","result":"r1","agent":decision.actor,"decision":if matches!(decision.outcome,Some(DecisionOutcome::Accepted{..})){"accepted"}else{"rejected"},"basis":"agent_agreement"}),
                            )
                        }
                        ProvenanceEvent::DecisionRecorded { decision }
                            if decision.kind == "check_observed" =>
                        {
                            let check = decision.links.check.unwrap();
                            Some(
                                json!({"type":"confirmation_recorded","result":if session=="s1"{"r1"}else{"component1"},"criterion":check.criterion_ids.first(),"outcome":check.outcome}),
                            )
                        }
                        _ => None,
                    }
                }
                _ => None,
            };
            if let Some(row) = row {
                sources.push(json!({"projected_index":rows.len(),"session":session,"runtime_event_seq":event.seq}));
                rows.push(row);
            }
        }
    }
    let rejected = trace.decisions.iter().any(|d| d.kind == "task_rejected");
    let observed = json!({"schema_version":1,"case_id":"evidence-boundaries","events":rows,"final_state":{"result_state":if rejected{"rejected"}else{"accepted"},"reputation_delta":h.store.observations()?.len()+a.store.observations()?.len(),"aggregate_confirmation":grade.0}});
    write_json(
        &directory.join("runtime.json"),
        &json!({"s1":trace,"s2":second}),
    )?;
    h.journal.save(directory)?;
    a.journal.save(&aggregate_dir)?;
    write_json(&directory.join("observed.json"), &observed)?;
    write_json(
        &directory.join("alias-map.json"),
        &json!({"r1":task.id,"component1":components[0].links.result.as_ref().unwrap().id,"component2":components[1].links.result.as_ref().unwrap().id,"sessions":{"s1":h.session.id,"s2":a.session.id},"projection_sources":sources}),
    )?;
    let metrics = json!({"elapsed_seconds":started.elapsed().as_secs_f64(),"s1":export::metrics(&trace,started.elapsed().as_secs_f64(),1)?,"s2":export::metrics(&second,started.elapsed().as_secs_f64(),1)?});
    write_json(&directory.join("metrics.json"), &metrics)?;
    let validation = protocols::validate(root, directory, "evidence-boundaries").await?;
    Ok(
        json!({"case_id":"evidence-boundaries","complete":validation["validator_passed"]==true,"validator":validation,"metrics":metrics}),
    )
}
