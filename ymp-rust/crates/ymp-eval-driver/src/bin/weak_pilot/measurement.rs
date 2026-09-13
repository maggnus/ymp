//! Common measurement checks for raw turns and the production Engine.
//! Task correctness, runtime acceptance and observed limits are separate outcomes.
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use ymp_core::{
    Config, DecisionOutcome, ExecutionSettings, InvocationFailure, InvocationState, ModelEffort,
    ProvenanceEvent, RecoveryStage, SessionTrace, TerminationEvidence,
};

pub fn inspect(trace: Option<&SessionTrace>, expected: &Config) -> Value {
    let requested = expected
        .members()
        .iter()
        .map(|agent| agent.id.clone())
        .collect::<BTreeSet<_>>();
    let mut captured = BTreeSet::new();
    let mut invoked = BTreeSet::new();
    let mut producers = BTreeSet::new();
    let mut reviewers = BTreeSet::new();
    let mut contexts = BTreeMap::new();
    let mut deviations = Vec::new();
    let mut invalid = Vec::new();
    let mut metadata_notes = Vec::new();
    let mut settings = Vec::new();
    let mut acceptance = Value::Null;
    let mut confirmation = Value::Null;
    if let Some(trace) = trace {
        captured.extend(
            trace
                .policy
                .as_ref()
                .map(|policy| &policy.captured_team)
                .unwrap_or(&trace.session.team)
                .iter()
                .map(|agent| agent.id.clone()),
        );
        if captured != requested {
            invalid.push(json!({"code":"captured_roster_differs_from_manifest"}));
        }
        let mut invocation_ids = BTreeSet::new();
        for invocation in &trace.invocations {
            if !invocation_ids.insert(&invocation.id) {
                invalid.push(
                    json!({"code":"duplicate_invocation_record","invocation_id":invocation.id}),
                );
            }
            let Some(assignment) = trace
                .assignments
                .iter()
                .find(|assignment| assignment.id == invocation.assignment_id)
            else {
                invalid.push(
                    json!({"code":"missing_originating_assignment","invocation_id":invocation.id}),
                );
                continue;
            };
            if assignment.session_id != trace.session.id
                || invocation.session_id != trace.session.id
                || trace
                    .assignments
                    .iter()
                    .filter(|candidate| candidate.id == assignment.id)
                    .count()
                    != 1
            {
                invalid.push(
                    json!({"code":"invalid_originating_assignment","invocation_id":invocation.id}),
                );
            }
            invoked.insert(assignment.agent_id.clone());
            if ["execute", "pilot_candidate"].contains(&assignment.purpose.as_str()) {
                producers.insert(assignment.agent_id.clone());
            }
            if assignment.purpose == "final_review" {
                reviewers.insert(assignment.agent_id.clone());
            }
            if let Some(context) = &invocation.native_session_id {
                if let Some(previous) = contexts.get(context) {
                    if previous != &assignment.agent_id {
                        invalid.push(json!({"code":"context_shared_across_actors","context_id":context,"agent_ids":[previous,assignment.agent_id]}));
                    }
                } else {
                    contexts.insert(context.clone(), assignment.agent_id.clone());
                }
            } else {
                invalid.push(json!({"code":"native_context_unobserved","invocation_id":invocation.id,"agent_id":assignment.agent_id}));
            }
            if invocation.ended_at.is_none()
                || invocation.state == InvocationState::Running
                || assignment.ended_at.is_none()
                || assignment.state == InvocationState::Running
            {
                invalid.push(
                    json!({"code":"terminal_accounting_unclosed","invocation_id":invocation.id}),
                );
            }
            if invocation.usage.as_ref().is_none_or(|usage| {
                usage.partial
                    || !usage.finalized
                    || usage.counts.input.is_none()
                    || usage.counts.output.is_none()
            }) {
                invalid.push(
                    json!({"code":"incomplete_invocation_usage","invocation_id":invocation.id}),
                );
            }
            let fixed = expected
                .execution
                .get(&assignment.agent_id)
                .map(|p| &p.fixed);
            settings.push(json!({
                "invocation_id":invocation.id,"agent_id":assignment.agent_id,
                "expected":fixed,"requested":invocation.requested,
                "sent":invocation.sent,"reported":invocation.reported
            }));
            if let Some(fixed) = fixed {
                for (source, observed) in [
                    ("assignment_requested", &assignment.requested),
                    ("requested", &invocation.requested),
                    ("sent", &invocation.sent),
                ] {
                    required_settings(&invocation.id, source, observed, fixed, &mut invalid);
                }
                reported_settings(&invocation.id, &invocation.reported, fixed, &mut invalid);
            } else {
                invalid.push(json!({"code":"missing_frozen_execution_settings","invocation_id":invocation.id,"agent_id":assignment.agent_id}));
            }
            for (field, value) in [
                ("model", &invocation.reported.model),
                ("effort", &invocation.reported.effort),
            ] {
                if value.is_none() {
                    metadata_notes.push(json!({"code":"native_setting_unreported","invocation_id":invocation.id,"field":field}));
                }
            }
            let expected_profile = expected
                .agents
                .iter()
                .find(|profile| profile.id == assignment.agent_id);
            if expected_profile.is_none_or(|profile| {
                assignment.provider_id != profile.provider
                    || expected.provider(&profile.provider).is_err()
            }) {
                invalid.push(json!({"code":"assignment_profile_differs_from_manifest","invocation_id":invocation.id,"agent_id":assignment.agent_id}));
            }
        }
        for invocation in &trace.invocations {
            if let Some(resumed) = &invocation.resumed_from {
                let actor = trace
                    .assignments
                    .iter()
                    .find(|assignment| assignment.id == invocation.assignment_id)
                    .map(|assignment| &assignment.agent_id);
                if contexts.get(resumed) != actor || actor.is_none() {
                    invalid.push(json!({"code":"continuation_owner_unconfirmed_or_changed","invocation_id":invocation.id,"resumed_context_id":resumed}));
                }
            }
        }
        for decision in &trace.decisions {
            if let Some(failure) = &decision.links.failure {
                inspect_failure(failure, &mut invalid);
            }
            if ["final_accepted", "final_rejected"].contains(&decision.kind.as_str()) {
                match &decision.outcome {
                    Some(DecisionOutcome::Accepted {
                        confirmation: grade,
                    }) => {
                        acceptance = json!("accepted");
                        confirmation = json!(grade);
                    }
                    Some(DecisionOutcome::Rejected) => {
                        acceptance = json!("rejected");
                        confirmation = Value::Null;
                    }
                    None => {}
                }
            }
        }
        inspect_history(trace, expected, &mut invalid);
        if trace.usage.total.is_partial() || trace.usage.total.known_total().is_none() {
            invalid.push(json!({"code":"incomplete_native_usage"}));
        }
        if trace.usage.total.calls != trace.invocations.len() as u64 {
            invalid.push(json!({"code":"usage_invocation_count_mismatch"}));
        }
        // Raw arms do not claim independent production acceptance. The Engine
        // must retain a final reviewer outside its actual producer set.
        let cooperation = trace.assignments.iter().any(|assignment| {
            ["plan", "execute", "final_review"].contains(&assignment.purpose.as_str())
        });
        if cooperation && !captured.is_empty() && producers.len() >= captured.len() {
            invalid.push(json!({"code":"no_independent_final_reviewer_remaining"}));
        }
    } else {
        invalid.push(json!({"code":"session_trace_unavailable"}));
        metadata_notes.push(json!({"code":"session_trace_unavailable"}));
    }
    let unused = captured.difference(&invoked).cloned().collect::<Vec<_>>();
    if !unused.is_empty() {
        deviations.push(json!({"code":"underused_roster","agent_ids":unused}));
    }
    let extra = invoked.difference(&captured).cloned().collect::<Vec<_>>();
    if !extra.is_empty() {
        invalid.push(json!({"code":"actors_outside_captured_roster","agent_ids":extra}));
    }
    let self_review = producers
        .intersection(&reviewers)
        .cloned()
        .collect::<Vec<_>>();
    if !self_review.is_empty() {
        invalid.push(json!({"code":"final_reviewer_also_produced","agent_ids":self_review}));
    }
    json!({
        "requested_agent_ids":requested,"requested_participant_count":requested.len(),
        "captured_agent_ids":captured,"invoked_agent_ids":invoked,"actual_participant_count":invoked.len(),
        "producer_ids":producers,"final_reviewer_ids":reviewers,"native_context_owners":contexts,
        "usage":trace.map(|trace| &trace.usage),"budget":trace.and_then(|trace| trace.budget.as_ref()),
        "runtime_acceptance":acceptance,"runtime_confirmation":confirmation,
        "measurement_valid":invalid.is_empty(),"invalid_reasons":invalid,
        "metadata_complete":metadata_notes.is_empty(),"metadata_notes":metadata_notes,
        "settings_observations":settings,"protocol_deviations":deviations
    })
}

fn required_settings(
    invocation_id: &str,
    source: &str,
    observed: &ExecutionSettings,
    fixed: &ModelEffort,
    invalid: &mut Vec<Value>,
) {
    if fixed.model.is_none()
        || fixed.effort.is_none()
        || observed.model != fixed.model
        || observed.effort != fixed.effort
    {
        invalid.push(json!({"code":"settings_differ_from_manifest","invocation_id":invocation_id,"source":source,"observed":observed,"expected":fixed}));
    }
}

fn reported_settings(
    invocation_id: &str,
    observed: &ExecutionSettings,
    fixed: &ModelEffort,
    invalid: &mut Vec<Value>,
) {
    if observed
        .model
        .as_ref()
        .is_some_and(|value| Some(value) != fixed.model.as_ref())
        || observed
            .effort
            .as_ref()
            .is_some_and(|value| Some(value) != fixed.effort.as_ref())
    {
        invalid.push(json!({"code":"reported_settings_contradict_manifest","invocation_id":invocation_id,"reported":observed,"expected":fixed}));
    }
}

fn inspect_failure(failure: &InvocationFailure, invalid: &mut Vec<Value>) {
    if failure.termination != TerminationEvidence::BackendEnded {
        invalid.push(json!({"code":"backend_termination_unverified","invocation_id":failure.invocation_id,"termination":failure.termination}));
    }
}

fn inspect_history(trace: &SessionTrace, expected: &Config, invalid: &mut Vec<Value>) {
    for event in &trace.history {
        if event.kind == "recovery_stage" {
            match serde_json::from_value::<RecoveryStage>(event.data.clone()) {
                Ok(stage) => {
                    for failure in &stage.failures {
                        inspect_failure(failure, invalid);
                    }
                }
                Err(_) => {
                    invalid.push(json!({"code":"invalid_recovery_record","event_seq":event.seq}))
                }
            }
        }
        // Storage merges settings into the current invocation. Check the native
        // observations as well so a later value cannot erase a contradiction.
        if event.kind != "provenance" {
            continue;
        }
        let Ok(ProvenanceEvent::InvocationObserved {
            invocation_id,
            observation,
        }) = serde_json::from_value::<ProvenanceEvent>(event.data.clone())
        else {
            continue;
        };
        let fixed = trace
            .invocations
            .iter()
            .find(|invocation| invocation.id == invocation_id)
            .and_then(|invocation| {
                trace
                    .assignments
                    .iter()
                    .find(|assignment| assignment.id == invocation.assignment_id)
            })
            .and_then(|assignment| expected.execution.get(&assignment.agent_id))
            .map(|policy| &policy.fixed);
        let Some(fixed) = fixed else {
            invalid.push(json!({"code":"observation_origin_unconfirmed","invocation_id":invocation_id,"event_seq":event.seq}));
            continue;
        };
        if let Some(observed) = &observation.sent {
            // Event fragments can omit fields. The final sent snapshot above
            // still requires both fields to match the frozen configuration.
            if observed
                .model
                .as_ref()
                .is_some_and(|value| Some(value) != fixed.model.as_ref())
                || observed
                    .effort
                    .as_ref()
                    .is_some_and(|value| Some(value) != fixed.effort.as_ref())
            {
                invalid.push(json!({"code":"sent_observation_contradicts_manifest","invocation_id":invocation_id,"event_seq":event.seq,"observed":observed,"expected":fixed}));
            }
        }
        if let Some(observed) = &observation.reported {
            reported_settings(&invocation_id, observed, fixed, invalid);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ymp_core::{
        AssignmentRecord, DecisionRecord, FailureClass, HistoryEvent, InvocationObservation,
        InvocationRecord, RecordLinks, Session, SessionUsage, TokenCounts, UsageSnapshot,
        WorkspaceAccess,
    };

    fn example(members: usize, calls: &[(usize, &str)]) -> (Config, SessionTrace) {
        let config = super::super::fixture::config(members, 20, 1_000);
        let session = Session {
            id: "test-session".into(),
            project_id: "test-project".into(),
            title: "Synthetic measurement control".into(),
            status: "completed".into(),
            created_at: "2026-09-14T00:00:00Z".into(),
            team: config.members(),
            turns_used: calls.len(),
        };
        let mut trace = SessionTrace {
            schema_version: 1,
            session,
            policy: None,
            tasks: vec![],
            assignments: vec![],
            invocations: vec![],
            decisions: vec![],
            usage: SessionUsage::default(),
            budget: None,
            team_state: None,
            history: vec![],
        };
        for (index, &(actor, purpose)) in calls.iter().enumerate() {
            let profile = &config.agents[actor];
            let settings = ExecutionSettings {
                model: profile.model.clone(),
                effort: Some("low".into()),
                permission_mode: Some("read_only".into()),
            };
            let timestamp = "2026-09-14T00:00:00Z".to_owned();
            let assignment = AssignmentRecord {
                token_reservation: Some(20),
                agent_identity: None,
                id: format!("assignment-{index}"),
                session_id: trace.session.id.clone(),
                task: None,
                agent_id: profile.id.clone(),
                agent_config_version: profile.version(&config.providers[0]),
                provider_id: profile.provider.clone(),
                purpose: purpose.into(),
                reason: "Synthetic measurement control".into(),
                cwd: "/synthetic-only".into(),
                requested: settings.clone(),
                timeout_secs: 100,
                grant_ids: vec![],
                context: vec![],
                state: InvocationState::Completed,
                started_at: timestamp.clone(),
                ended_at: Some(timestamp.clone()),
            };
            trace.invocations.push(InvocationRecord {
                id: format!("invocation-{index}"),
                session_id: trace.session.id.clone(),
                assignment_id: assignment.id.clone(),
                execution_backend: None,
                turn: index as u64 + 1,
                requested: settings.clone(),
                sent: settings.clone(),
                reported: settings,
                resumed_from: None,
                native_session_id: Some(format!("context-{index}")),
                native_turn_id: Some(format!("native-turn-{index}")),
                native_version: Some("synthetic-only".into()),
                state: InvocationState::Completed,
                started_at: timestamp.clone(),
                ended_at: Some(timestamp),
                usage: Some(UsageSnapshot {
                    counts: TokenCounts {
                        input: Some(8),
                        output: Some(2),
                        cache_read: Some(4),
                        cache_write: Some(1),
                        reasoning: Some(1),
                    },
                    finalized: true,
                    partial: false,
                    note: Some("Synthetic accounting only".into()),
                    native_total: None,
                }),
                terminal_reason: None,
            });
            trace.assignments.push(assignment);
        }
        refresh_usage(&mut trace);
        (config, trace)
    }

    fn refresh_usage(trace: &mut SessionTrace) {
        trace.usage = SessionUsage::default();
        for invocation in &trace.invocations {
            trace
                .usage
                .total
                .include(invocation.usage.as_ref(), invocation.ended_at.is_none());
            let actor = &trace
                .assignments
                .iter()
                .find(|assignment| assignment.id == invocation.assignment_id)
                .unwrap()
                .agent_id;
            trace
                .usage
                .agents
                .entry(actor.clone())
                .or_default()
                .include(invocation.usage.as_ref(), invocation.ended_at.is_none());
        }
    }

    fn has_code(summary: &Value, code: &str) -> bool {
        summary["invalid_reasons"]
            .as_array()
            .unwrap()
            .iter()
            .any(|reason| reason["code"] == code)
    }

    #[test]
    fn missing_and_contradictory_reported_settings_follow_one_rule_in_all_arms() {
        for (members, purpose) in [
            (1, "pilot_candidate"),
            (1, "pilot_candidate"),
            (2, "pilot_candidate"),
            (3, "pilot_candidate"),
            (2, "plan"),
            (3, "plan"),
        ] {
            let (config, mut trace) = example(members, &[(0, purpose)]);
            trace.invocations[0].reported = ExecutionSettings::default();
            let summary = inspect(Some(&trace), &config);
            assert_eq!(summary["measurement_valid"], true, "{summary}");
            assert_eq!(summary["metadata_complete"], false);
            assert!(summary["settings_observations"][0]["reported"]["model"].is_null());
            assert!(summary["settings_observations"][0]["reported"]["effort"].is_null());
            trace.invocations[0].reported.effort = Some("high".into());
            let summary = inspect(Some(&trace), &config);
            assert_eq!(summary["measurement_valid"], false);
            assert!(has_code(&summary, "reported_settings_contradict_manifest"));
        }
    }

    #[test]
    fn requested_and_sent_must_match_frozen_settings_even_when_they_agree_with_each_other() {
        let (config, mut trace) = example(1, &[(0, "pilot_candidate")]);
        trace.invocations[0].requested.effort = Some("high".into());
        trace.invocations[0].sent.effort = Some("high".into());
        let summary = inspect(Some(&trace), &config);
        assert!(has_code(&summary, "settings_differ_from_manifest"));
        assert_eq!(summary["measurement_valid"], false);
    }

    #[test]
    fn rejection_and_underused_roster_are_valid_outcomes_with_actual_count() {
        let (config, mut trace) = example(3, &[(0, "execute"), (1, "final_review")]);
        trace.session.status = "failed".into();
        trace.decisions.push(DecisionRecord {
            id: "negative-review".into(),
            session_id: trace.session.id.clone(),
            kind: "final_rejected".into(),
            actor: Some("weak-2".into()),
            reason: "Synthetic final rejection".into(),
            outcome: Some(DecisionOutcome::Rejected),
            links: RecordLinks::default(),
            created_at: "2026-09-14T00:00:00Z".into(),
        });
        let summary = inspect(Some(&trace), &config);
        assert_eq!(summary["measurement_valid"], true, "{summary}");
        assert_eq!(summary["runtime_acceptance"], "rejected");
        assert_eq!(summary["requested_participant_count"], 3);
        assert_eq!(summary["actual_participant_count"], 2);
        assert_eq!(
            summary["protocol_deviations"][0]["code"],
            "underused_roster"
        );
    }

    #[test]
    fn known_zero_call_denial_needs_no_native_metadata() {
        let (config, mut trace) = example(2, &[]);
        trace.session.status = "waiting".into();
        let summary = inspect(Some(&trace), &config);
        assert_eq!(summary["measurement_valid"], true, "{summary}");
        assert_eq!(summary["usage"]["total"]["calls"], 0);
        assert_eq!(summary["actual_participant_count"], 0);
    }

    #[test]
    fn complete_failure_accounting_is_distinct_from_partial_unknown_or_unclosed_usage() {
        let (config, mut trace) = example(1, &[(0, "pilot_candidate")]);
        trace.invocations[0].state = InvocationState::Failed;
        trace.assignments[0].state = InvocationState::Failed;
        assert_eq!(inspect(Some(&trace), &config)["measurement_valid"], true);
        trace.invocations[0].usage.as_mut().unwrap().partial = true;
        refresh_usage(&mut trace);
        assert!(has_code(
            &inspect(Some(&trace), &config),
            "incomplete_native_usage"
        ));
        trace.invocations[0].usage = None;
        refresh_usage(&mut trace);
        assert_eq!(inspect(Some(&trace), &config)["measurement_valid"], false);
        trace.invocations[0].ended_at = None;
        assert!(has_code(
            &inspect(Some(&trace), &config),
            "terminal_accounting_unclosed"
        ));
    }

    #[test]
    fn impossible_origins_and_shared_native_contexts_are_invalid() {
        let (config, mut trace) = example(2, &[(0, "pilot_candidate"), (1, "pilot_candidate")]);
        trace.invocations[1].native_session_id = trace.invocations[0].native_session_id.clone();
        assert!(has_code(
            &inspect(Some(&trace), &config),
            "context_shared_across_actors"
        ));
        trace.invocations[1].assignment_id = "nonexistent".into();
        assert!(has_code(
            &inspect(Some(&trace), &config),
            "missing_originating_assignment"
        ));
        trace.invocations[0].native_session_id = None;
        assert!(has_code(
            &inspect(Some(&trace), &config),
            "native_context_unobserved"
        ));
    }

    #[test]
    fn retained_contradiction_cannot_be_erased_by_later_matching_metadata() {
        let (config, mut trace) = example(1, &[(0, "pilot_candidate")]);
        trace.history.push(HistoryEvent {
            seq: 1,
            session_id: trace.session.id.clone(),
            kind: "provenance".into(),
            data: serde_json::to_value(ProvenanceEvent::InvocationObserved {
                invocation_id: trace.invocations[0].id.clone(),
                observation: Box::new(InvocationObservation {
                    reported: Some(ExecutionSettings {
                        effort: Some("high".into()),
                        ..Default::default()
                    }),
                    ..Default::default()
                }),
            })
            .unwrap(),
            created_at: "2026-09-14T00:00:00Z".into(),
        });
        let summary = inspect(Some(&trace), &config);
        assert!(has_code(&summary, "reported_settings_contradict_manifest"));
        assert_eq!(
            summary["settings_observations"][0]["reported"]["effort"],
            "low"
        );
        assert_eq!(summary["usage"]["total"]["counts"]["input"], 8);
    }

    #[test]
    fn termination_evidence_overrides_a_closed_timestamp() {
        let (config, mut trace) = example(1, &[(0, "pilot_candidate")]);
        trace.decisions.push(DecisionRecord {
            id: "failure-record".into(),
            session_id: trace.session.id.clone(),
            kind: "invocation_failure".into(),
            actor: None,
            reason: "Synthetic termination evidence".into(),
            outcome: None,
            links: RecordLinks {
                failure: Some(InvocationFailure {
                    class: FailureClass::Cancelled,
                    native_code: None,
                    assignment_id: trace.assignments[0].id.clone(),
                    invocation_id: trace.invocations[0].id.clone(),
                    agent_id: "weak-1".into(),
                    provider_id: "offline".into(),
                    effective_access: WorkspaceAccess::ReadAll,
                    termination: TerminationEvidence::UnverifiedAfterRestart,
                }),
                ..Default::default()
            },
            created_at: "2026-09-14T00:00:00Z".into(),
        });
        assert!(has_code(
            &inspect(Some(&trace), &config),
            "backend_termination_unverified"
        ));
        trace.decisions[0]
            .links
            .failure
            .as_mut()
            .unwrap()
            .termination = TerminationEvidence::BackendEnded;
        assert_eq!(inspect(Some(&trace), &config)["measurement_valid"], true);
    }
}
