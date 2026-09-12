//! Pure projection used by the driver and its actual-binary integration controls.
use anyhow::{ensure, Context, Result};
use serde_json::{json, Value};
use std::path::Path;
use ymp_core::*;
#[derive(Default)]
struct Projection {
    events: Vec<Value>,
    sources: Vec<Value>,
}
impl Projection {
    fn push(&mut self, row: Value, mut source: Value) {
        source["projected_index"] = json!(self.events.len());
        self.events.push(row);
        self.sources.push(source);
    }
}
pub fn project(trace: &SessionTrace, before: &SessionTrace) -> Result<(Value, Value)> {
    use std::collections::BTreeMap;
    let mut assignment_aliases = BTreeMap::new();
    let mut result_aliases = BTreeMap::new();
    let mut task_aliases = BTreeMap::new();
    for task in &trace.tasks {
        if task.title == "Produce totals" {
            task_aliases.insert(task.id.clone(), "totals".to_owned());
        }
    }
    for d in &before.decisions {
        if d.kind == "task_accepted" {
            if let Some(r) = &d.links.result {
                if r.artifacts
                    .iter()
                    .any(|a| a.path == Path::new("outputs/workshop.md"))
                {
                    result_aliases.insert(r.id.clone(), "r1".to_owned());
                }
            }
        }
    }
    for i in &before.invocations {
        if i.state == InvocationState::Running {
            assignment_aliases.insert(i.assignment_id.clone(), "a2".to_owned());
        }
    }
    let mut inspections = 0;
    for a in &trace.assignments {
        if !before.assignments.iter().any(|old| old.id == a.id) {
            inspections += usize::from(a.purpose == "review");
            assignment_aliases.insert(
                a.id.clone(),
                if a.purpose == "review" {
                    format!("inspect{inspections}")
                } else {
                    a.id.clone()
                },
            );
        }
    }
    let alias = |id: &str| {
        assignment_aliases
            .get(id)
            .cloned()
            .unwrap_or_else(|| id.into())
    };
    let result_alias = |id: &str| result_aliases.get(id).cloned().unwrap_or_else(|| id.into());
    let task_alias = |id: &str| task_aliases.get(id).cloned().unwrap_or_else(|| id.into());
    let new_assignments = trace
        .assignments
        .iter()
        .filter(|a| !before.assignments.iter().any(|old| old.id == a.id))
        .collect::<Vec<_>>();
    let replays = new_assignments
        .iter()
        .filter(|a| a.purpose == "execute")
        .count();
    let mut projection = Projection::default();
    let mut terminal_counts = BTreeMap::<String, usize>::new();
    ensure!(
        trace.history.windows(2).all(|p| p[0].seq < p[1].seq),
        "Ambiguous runtime event order"
    );
    for event in &trace.history {
        let mut source = json!({"runtime_event_seq":event.seq,"session_id":event.session_id});
        match event.kind.as_str() {
            "eval_process_terminated"=>for id in event.data["open_invocation_ids"].as_array().context("Missing terminated invocation IDs")? {
                let invocation=trace.invocations.iter().find(|i|Some(i.id.as_str())==id.as_str()).context("Terminated invocation record missing")?;
                let a=trace.assignments.iter().find(|a|a.id==invocation.assignment_id).context("Missing interrupted assignment")?;
                source["invocation_id"]=json!(invocation.id);
                projection.push(json!({"type":"invocation_closed","assignment":alias(&a.id),"agent":a.agent_id,"outcome":invocation.state,"tokens":invocation.usage.as_ref().and_then(|u|u.counts.known_total()),"coverage":coverage(invocation.usage.as_ref())}),source.clone());
            },
            "eval_runtime_restarted"=>projection.push(json!({"type":"runtime_restarted","restored_active_grants":event.data["restored_active_grants"]}),source),
            "eval_expired_tool_response"=>{let reply=&event.data["response"];let error=reply["error"].as_str().unwrap_or("");projection.push(json!({"type":if reply["ok"]==false{"tool_denied"}else{"tool_allowed"},"assignment":"a2","operation":"board_post","reason":if error.contains("expired"){"stale_grant"}else{error}}),source);},
            "eval_recovery_inspected"=>projection.push(json!({"type":"recovery_inspected","preserved":event.data["preserved_result_ids"].as_array().unwrap().iter().map(|id|result_alias(id.as_str().unwrap())).collect::<Vec<_>>(),"unresolved":event.data["unresolved_task_ids"].as_array().unwrap().iter().map(|id|task_alias(id.as_str().unwrap())).collect::<Vec<_>>(),"repeated_uncertain_effects":replays>0}),source),
            "eval_recovery_native_closed"=>{let i=trace.invocations.iter().find(|i|i.native_turn_id.as_deref()==event.data["native_id"].as_str()).context("Closed native observation has no runtime invocation")?;let a=trace.assignments.iter().find(|a|a.id==i.assignment_id).unwrap();let usage:UsageSnapshot=serde_json::from_value(event.data["usage"].clone())?;source["invocation_id"]=json!(i.id);projection.push(json!({"type":"invocation_closed","assignment":alias(&a.id),"agent":a.agent_id,"tokens":usage.counts.known_total(),"coverage":coverage(Some(&usage))}),source);},
            "provenance"=>match serde_json::from_value::<ProvenanceEvent>(event.data.clone())? {
                ProvenanceEvent::DecisionRecorded{decision} if decision.kind=="task_accepted"=>{let r=decision.links.result.context("Acceptance without result")?;for artifact in &r.artifacts{projection.push(json!({"type":"result_preserved","result":result_alias(&r.id),"artifact":artifact.path,"sha256":artifact.sha256,"confirmation":serde_json::to_value(&decision.outcome)?["confirmation"]}),source.clone());}},
                ProvenanceEvent::AssignmentStarted{assignment,..} if new_assignments.iter().any(|a|a.id==assignment.id)=>{
                    let access=trace.decisions.iter().find(|d|d.kind=="workspace_access_admitted"&&d.links.assignment_id.as_ref()==Some(&assignment.id)).and_then(|d|d.links.workspace_access.as_ref()).context("Missing real inspection access")?;
                    let (reads,writes)=match &access.effective_access {WorkspaceAccess::Scoped{reads,writes}=>(reads.clone(),writes.clone()),other=>{projection.push(json!({"type":"unexpected_workspace_access","assignment":alias(&assignment.id),"access":other}),source.clone());(vec![],vec![])}};
                    projection.push(json!({"type":"assignment_started","assignment":alias(&assignment.id),"agent":assignment.agent_id,"purpose":if assignment.purpose=="review"{"inspect_actual_files"}else{&assignment.purpose},"reads":reads,"writes":writes}),source);
                },
                ProvenanceEvent::InvocationFinished { invocation } if assignment_aliases.contains_key(&invocation.assignment_id) => {
                    let count=terminal_counts.entry(invocation.id.clone()).or_default();*count+=1;
                    let matched=trace.history.iter().any(|e|(e.kind=="eval_process_terminated"&&e.data["open_invocation_ids"].as_array().is_some_and(|ids|ids.iter().any(|id|*id==invocation.id)))||(e.kind=="eval_recovery_native_closed"&&e.data["native_id"].as_str()==invocation.native_turn_id.as_deref()));
                    if *count>1||!matched {projection.push(json!({"type":"unexpected_invocation_closed","assignment":alias(&invocation.assignment_id),"invocation_id":invocation.id,"state":invocation.state}),source);}
                },
                ProvenanceEvent::GrantRevoked{grant} if assignment_aliases.contains_key(&grant.assignment_id)=>projection.push(json!({"type":"grant_revoked","assignment":alias(&grant.assignment_id)}),source),
                _=>{},
            },
            _=>{},
        }
    }
    let confirmed = trace
        .decisions
        .iter()
        .filter(|d| {
            d.kind == "task_accepted"
                && matches!(
                    d.outcome,
                    Some(DecisionOutcome::Accepted {
                        confirmation: ConfirmationStatus::Confirmed
                    })
                )
        })
        .filter_map(|d| d.links.result.as_ref())
        .filter(|r| {
            trace
                .tasks
                .iter()
                .any(|t| t.id == r.id && t.state == TaskState::Accepted)
        })
        .map(|r| result_alias(&r.id))
        .collect::<Vec<_>>();
    let budget = trace.budget.as_ref().context("Missing captured budget")?;
    let spent = budget.observed_usage.known_total();
    let total = budget
        .limits
        .resources
        .as_ref()
        .and_then(|r| r.observed_tokens);
    let inspection_units = trace
        .invocations
        .iter()
        .filter(|i| {
            new_assignments
                .iter()
                .any(|a| a.id == i.assignment_id && a.purpose == "review")
        })
        .filter_map(|i| i.usage.as_ref().and_then(|u| u.counts.known_total()))
        .sum::<u64>();
    let restarted = trace
        .history
        .iter()
        .filter(|e| e.kind == "eval_runtime_restarted")
        .any(|e| {
            e.data["restored_active_grants"]
                .as_u64()
                .is_none_or(|n| n > 0)
        });
    Ok((
        json!({"schema_version":1,"case_id":"restart-inspection","events":projection.events,"final_state":{"confirmed_results":confirmed,"automatic_production_replays":replays,"reported_spent_units":spent,"remaining_reported_units":total.zip(spent).map(|(total,spent)|total.saturating_sub(spent)),"usage_coverage":if budget.observed_usage.is_partial(){"partial"}else{"complete"},"old_role_restored":restarted,"inspection_units":inspection_units}}),
        json!({"assignments":assignment_aliases,"results":result_aliases,"tasks":task_aliases,"projection_sources":projection.sources,"process_close_source":"Observed OS process exit precedes runtime revocation; final terminal state and measured usage are linked by the exact invocation ID"}),
    ))
}
fn coverage(usage: Option<&UsageSnapshot>) -> &'static str {
    match usage {
        None => "unknown",
        Some(u)
            if u.partial
                || !u.finalized
                || u.counts.input.is_none()
                || u.counts.output.is_none() =>
        {
            "partial"
        }
        Some(_) => "complete",
    }
}
