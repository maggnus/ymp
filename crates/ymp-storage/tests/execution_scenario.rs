//! The admitted execution scenario persists through the SQLite journal and
//! replays exactly after the journal is closed and reopened (bounded
//! execution contract: durable execution facts are session events under the
//! existing revision rules). Phase 1 runs discovery, admission, invocation,
//! termination observation and accounting against one `SqliteJournal`;
//! phase 2 reopens the database file, rebuilds the in-memory port state by
//! replay, and continues the session with a second invocation.
//!
//! Every adapter here is scripted: no native discovery, no credential
//! access, no network calls and no user-data mutation occur.

#![forbid(unsafe_code)]

mod support;

use std::time::Duration;

use support::{TempRoot, sample_task, session_id};
use ymp_kernel::Journal;
use ymp_runtime::Pool;
use ymp_runtime::{
    AgentId, Allowance, AssignmentRequest, ExecutionScenario, InvocationStatus, ModelOffering,
    ObservationOutcome, ObservedUsage, OfferingId, ResourceAmount, Revision, Role, ScriptedOutcome,
    ScriptedProvider, SessionStatus, SettingKey, SettingValue, Settings, StartOutcome,
    SupportedControl, Termination, WorkspaceScope,
};
use ymp_storage::SqliteJournal;

fn offering() -> ModelOffering {
    ModelOffering::new(
        OfferingId::new("scripted-offering").expect("valid offering ID"),
        vec![
            SupportedControl::new(
                SettingKey::new("effort").expect("valid setting key"),
                vec![
                    SettingValue::new("low").expect("valid setting value"),
                    SettingValue::new("high").expect("valid setting value"),
                ],
            ),
            SupportedControl::new(
                SettingKey::new("thinking").expect("valid setting key"),
                vec![SettingValue::new("on").expect("valid setting value")],
            ),
        ],
    )
    .expect("valid offering")
}

fn provider() -> ScriptedProvider {
    ScriptedProvider::new(
        AgentId::new("claude-opus-5").expect("valid agent ID"),
        offering(),
    )
    .with_effective_workspaces([WorkspaceScope::new("session-primary").expect("valid scope")])
}

fn request() -> AssignmentRequest {
    AssignmentRequest::new(
        AgentId::new("claude-opus-5").expect("valid agent ID"),
        Role::new("implementer").expect("valid role"),
        Settings::from_pairs([(
            SettingKey::new("effort").expect("valid setting key"),
            SettingValue::new("high").expect("valid setting value"),
        )])
        .expect("valid settings"),
        Allowance::new(
            ResourceAmount::new(4),
            ymp_runtime::InvocationLimits::new(6, 2048, Duration::from_millis(60_000))
                .expect("valid limits"),
        )
        .expect("valid allowance"),
        WorkspaceScope::new("session-primary").expect("valid scope"),
    )
}

#[test]
fn admitted_scenario_persists_and_replays_after_reopen() {
    let root = TempRoot::new("execution-scenario");
    let sid = session_id("execution-scenario-session");
    let task = sample_task("scenario");

    // Phase 1: the whole admitted scenario runs over the SQLite journal.
    let journal = SqliteJournal::open(root.path()).expect("journal opens");
    let scenario = ExecutionScenario::new(journal.clone(), provider(), ResourceAmount::new(10));

    let opened = scenario
        .open_session(sid.clone(), task)
        .expect("session opens");
    assert_eq!(opened.revision(), Revision::new(1));

    let pool: Pool = scenario.scan().expect("scripted scan succeeds");
    assert_eq!(pool.entries().len(), 1);
    assert_eq!(pool.entries()[0].agent().as_str(), "claude-opus-5");

    scenario.script_outcome(ScriptedOutcome::completes(
        Some(
            Settings::from_pairs([(
                SettingKey::new("effort").expect("valid setting key"),
                SettingValue::new("low").expect("valid setting value"),
            )])
            .expect("valid settings"),
        ),
        ObservedUsage::unknown()
            .with_turns(3)
            .with_output_chars(900)
            .with_wall_clock(Duration::from_millis(2500)),
    ));

    let assignment = scenario
        .admit(&sid, request(), opened.revision())
        .expect("admission commits through the durable journal");
    assert_eq!(
        scenario
            .invoke(&sid, &assignment, Revision::new(2))
            .expect("invocation starts"),
        StartOutcome::Started
    );
    match scenario
        .observe(&sid, assignment.invocation(), Revision::new(3))
        .expect("termination observes")
    {
        ObservationOutcome::Terminated {
            termination: Termination::Completed,
            ..
        } => {}
        other => panic!("expected a completed termination, got {other:?}"),
    }
    scenario
        .settle(&sid, assignment.invocation(), Revision::new(4))
        .expect("settlement commits through the durable journal");

    let view_before = scenario.execution_view(&sid).expect("view replays");
    let accounting_before = scenario.accounting(&sid).expect("accounting replays");
    assert_eq!(accounting_before.completed(), 1);
    assert_eq!(accounting_before.coordination(), 1);
    assert_eq!(scenario.treasury_held().value(), 0);
    drop(scenario);
    drop(journal);

    // Phase 2: reopen the same database file and rebuild by replay.
    let reopened = SqliteJournal::open(root.path()).expect("journal reopens");
    let scenario = ExecutionScenario::new(reopened.clone(), provider(), ResourceAmount::new(10));
    scenario
        .reattach(&sid)
        .expect("port state rebuilds from history");

    let read = scenario.read_session(&sid).expect("session reads");
    assert_eq!(read.revision(), Revision::new(5));
    assert_eq!(read.status(), SessionStatus::Open);

    // The replayed execution view equals the pre-close view event for
    // event: every execution fact survived the payload round-trip through
    // SQLite exactly.
    let view_after = scenario.execution_view(&sid).expect("view replays");
    assert_eq!(view_after, view_before);
    assert_eq!(
        scenario.accounting(&sid).expect("accounting replays"),
        accounting_before
    );
    assert_eq!(scenario.treasury_held().value(), 0);

    let first = view_after
        .invocation(assignment.invocation())
        .expect("first invocation replays");
    assert_eq!(first.status(), InvocationStatus::Terminated);
    let profile = first.profile();
    assert_eq!(
        profile.requested().get(&SettingKey::new("effort").unwrap()),
        Some(&SettingValue::new("high").unwrap())
    );
    assert_eq!(
        profile
            .reported()
            .unwrap()
            .get(&SettingKey::new("effort").unwrap()),
        Some(&SettingValue::new("low").unwrap())
    );

    assert_eq!(reopened.read(&sid).expect("history reads").len(), 5);

    // The flow continues after the reopen: discovery is explicit and not
    // durable, so the pool is empty with its typed reason until a fresh
    // scan runs; afterwards a successor invocation fails and is accounted
    // on the reopened journal.
    assert_eq!(scenario.pool().entries().len(), 0);
    let pool = scenario.scan().expect("scan after reopen succeeds");
    assert_eq!(pool.entries().len(), 1);
    scenario.script_outcome(ScriptedOutcome::fails(
        "provider_overloaded",
        ObservedUsage::unknown(),
    ));
    let second = scenario
        .admit(&sid, request(), Revision::new(5))
        .expect("second admission commits after reopen");
    assert_eq!(second.invocation().as_str(), "invocation-2");
    scenario
        .invoke(&sid, &second, Revision::new(6))
        .expect("second invocation starts");
    match scenario
        .observe(&sid, second.invocation(), Revision::new(7))
        .expect("second termination observes")
    {
        ObservationOutcome::Terminated {
            termination: Termination::Failed { class },
            ..
        } => assert_eq!(class.as_str(), "provider_overloaded"),
        other => panic!("expected a failed termination, got {other:?}"),
    }
    scenario
        .settle(&sid, second.invocation(), Revision::new(8))
        .expect("second settlement commits");
    let accounting = scenario.accounting(&sid).expect("accounting replays");
    assert_eq!(accounting.completed(), 1);
    assert_eq!(accounting.failed(), 1);
    assert_eq!(accounting.usage().turns_known(), 3);
    assert_eq!(accounting.usage().turns_unknown_reports(), 1);
    assert_eq!(scenario.treasury_held().value(), 0);
}
