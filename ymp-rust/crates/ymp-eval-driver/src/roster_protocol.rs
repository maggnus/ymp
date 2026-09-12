use crate::{
    export, protocols,
    task_protocol::{NativeAction, TaskHarness},
    workflows, write_json,
};
use anyhow::{ensure, Result};
use serde_json::{json, Value};
use std::{path::Path, sync::Arc, time::Instant};
use ymp_core::*;
use ymp_runtime::AllocationPolicy;
struct Replacement;
impl AllocationPolicy for Replacement {
    fn identity(&self) -> ExecutionBackendIdentity {
        ExecutionBackendIdentity {
            id: "ymp.evals.roster-proposal".into(),
            version: "1".into(),
        }
    }
    fn propose(&self, _: &AllocationInput) -> Result<AllocationProposal> {
        Ok(AllocationProposal {
            members: vec!["a".into(), "c".into()],
            executor: None,
            reserved_final_reviewer: Some("b".into()),
            method: "explicit_replacement_probe".into(),
            rationale: "Replace b with c at the current boundary".into(),
        })
    }
}
pub async fn run(root: &Path, directory: &Path) -> Result<Value> {
    let started = Instant::now();
    let mut config = workflows::config();
    let mut c = config.agents[0].clone();
    c.id = "c".into();
    c.name = "Scripted c".into();
    config.agents.push(c);
    let harness = TaskHarness::create_with_config(
        directory,
        "Pinned roster adverse aggregate review",
        vec![],
        config,
    )
    .await?;
    let demand = AllocationDemand {
        purpose: "execute".into(),
        task_id: None,
        competence: "implementation".into(),
        difficulty: "simple".into(),
        risk: TaskRisk::Standard,
        ready_work: 0,
    };
    let replacement = harness
        .engine
        .clone()
        .with_allocation_policy(Arc::new(Replacement))?
        .reconsider_allocation(
            &harness.session.id,
            AllocationBoundary::GoalChanged,
            demand.clone(),
        );
    let mut tasks = Vec::new();
    let mut submissions = Vec::new();
    for agent in ["a", "b"] {
        let mut task = harness.new_task(&format!("Contribution from {agent}"));
        let submission = harness
            .produce(
                &mut task,
                agent,
                NativeAction {
                    writes: vec![],
                    reads: vec![],
                    response: format!(
                        "The bounded contribution from {agent} is ready for independent review"
                    ),
                },
            )
            .await?;
        let peer = if agent == "a" { "b" } else { "a" };
        let review = harness.review(&task, &submission, peer, true).await?;
        harness.accept(&mut task, &submission, &review, true)?;
        tasks.push(task);
        submissions.push(submission);
    }
    let aggregate = harness.aggregate(&submissions)?;
    let result = aggregate.links.result.as_ref().unwrap();
    let admission = harness.engine.reconsider_allocation(
        &harness.session.id,
        AllocationBoundary::ResultAvailable,
        AllocationDemand {
            purpose: "final_review".into(),
            competence: "verification".into(),
            ready_work: 1,
            ..demand
        },
    );
    let trace = harness.store.trace(&harness.session.id)?;
    let mut events = Vec::new();
    let mut sources = Vec::new();
    for event in trace.history.iter().filter(|e| e.kind == "provenance") {
        let row = match serde_json::from_value::<ProvenanceEvent>(event.data.clone())? {
            ProvenanceEvent::SessionCaptured { policy } => Some(
                json!({"type":"team_captured","session":"s1","members":policy.captured_team.iter().map(|a|a.id.clone()).collect::<Vec<_>>()}),
            ),
            ProvenanceEvent::DecisionRecorded { decision }
                if decision.kind == "result_aggregated" =>
            {
                let result = decision.links.result.unwrap();
                let mut producers = result
                    .producer_assignment_ids
                    .iter()
                    .map(|id| {
                        trace
                            .assignments
                            .iter()
                            .find(|a| &a.id == id)
                            .unwrap()
                            .agent_id
                            .clone()
                    })
                    .collect::<Vec<_>>();
                producers.sort();
                producers.dedup();
                Some(
                    json!({"type":"candidate_recorded","result":if result.id==harness.session.id{"r1"}else{&result.id},"producers":producers}),
                )
            }
            ProvenanceEvent::DecisionRecorded { decision } => {
                if let Some(allocation) = decision.links.allocation {
                    if allocation.accepted {
                        None
                    } else if allocation.input.demand.purpose == "final_review" {
                        Some(
                            json!({"type":"admission_denied","phase":"review","reason":allocation.reason.split(':').next().unwrap_or(&allocation.reason)}),
                        )
                    } else {
                        Some(
                            json!({"type":"proposal_rejected","reason":allocation.reason.split(':').next().unwrap_or(&allocation.reason),"agent":allocation.proposal.members.iter().find(|id|!harness.session.team.iter().any(|a|&a.id==*id))}),
                        )
                    }
                } else {
                    None
                }
            }
            _ => None,
        };
        if let Some(row) = row {
            sources.push(json!({"projected_index":events.len(),"runtime_event_seq":event.seq}));
            events.push(row);
        }
    }
    let grade = harness
        .store
        .confirmation_grade(&harness.session.id, result)?
        .0;
    let accepted = trace.decisions.iter().any(|d| d.kind == "final_accepted");
    let observed = json!({"schema_version":1,"case_id":"fixed-roster","events":events,"final_state":{"members":trace.team_state.as_ref().map(|s|s.current_members.clone()).unwrap_or_else(||trace.session.team.iter().map(|a|a.id.clone()).collect()),"result_state":if accepted{"accepted"}else{"awaiting_review"},"confirmation":grade,"reputation_delta":harness.store.observations()?.len()}});
    write_json(&directory.join("runtime.json"), &trace)?;
    harness.journal.save(directory)?;
    write_json(&directory.join("observed.json"), &observed)?;
    let mut aliases = export::aliases(&trace);
    aliases["aliases"]["r1"] = json!(result.id);
    aliases["projection_sources"] = json!(sources);
    write_json(&directory.join("alias-map.json"), &aliases)?;
    write_json(
        &directory.join("boundary-actions.json"),
        &json!({"replacement_error":replacement.err().map(|e|e.to_string()),"review_admission_error":admission.err().map(|e|e.to_string()),"setup":"Two independently accepted unconfirmed contributions were produced and reviewed through actual admitted scripted turns before probing final independent review. No final-review reservation was installed in this intentionally adverse captured setup."}),
    )?;
    ensure!(
        tasks.iter().all(|task| task.state == TaskState::Accepted),
        "Adverse setup failed to produce accepted components"
    );
    let metrics = export::metrics(&trace, started.elapsed().as_secs_f64(), 1)?;
    write_json(&directory.join("metrics.json"), &metrics)?;
    let validation = protocols::validate(root, directory, "fixed-roster").await?;
    Ok(
        json!({"case_id":"fixed-roster","complete":validation["validator_passed"]==true,"validator":validation,"metrics":metrics}),
    )
}
