//! The sections of the team page that describe a loaded session's current team and stopped work.
//!
//! They present the backend read model the controller last received. The actions a row offers
//! are chosen in the state layer from the same typed model; nothing here is a permission.

use crate::control::{Read, Request, Snapshot};
use crate::label;
use crate::table::{Cell, Column};
use crate::text;
use crate::views::{field, paragraph, Ctx, Item};
use ratatui::text::{Line, Span};
use ymp_core::{
    FailureClass, FreshPlanReviewNextAction, InvocationFailure, OwnerRunControl,
    RecoveryControlKind, RecoveryStage, RecoveryStatus, RecoveryWaitReason, TerminationEvidence,
};

pub const SESSION_KEY: &str = "control:session";
pub const RETRY_KEY: &str = "control:retry";
pub const CONTINUE_KEY: &str = "control:continue";
pub const MEMBER_PREFIX: &str = "member:";
pub const STAGE_PREFIX: &str = "stage:";

const SESSION_COLUMNS: [Column; 3] = [
    Column::left(""),
    Column::left("SESSION").flex(),
    Column::left("STATE"),
];
const TEAM_COLUMNS: [Column; 4] = [
    Column::left(""),
    Column::left("AGENT").flex(),
    Column::left("STATE"),
    Column::left("WORK").hide(1),
];
const STAGE_COLUMNS: [Column; 4] = [
    Column::left(""),
    Column::left("STAGE").flex(),
    Column::left("STATUS").hide(1),
    Column::left("REASON"),
];

const NOT_INITIALIZED: &str = "This session has no initialized team, so its membership and recovery cannot be changed here. Starting preferences apply to new sessions and are not edited for a loaded session.";

/// The rows for the loaded session, or nothing when no session is loaded.
pub fn items(ctx: &Ctx) -> Vec<Item> {
    let Some(session) = ctx.session else {
        return Vec::new();
    };
    let theme = ctx.theme;
    let control = ctx.control;
    let mut items = vec![Item::table("Session control", &SESSION_COLUMNS)];
    if let Some(pending) = control.pending() {
        let subject = if pending.session() == session {
            "this session".to_owned()
        } else {
            format!("session {}", text::short_id(pending.session()))
        };
        items.push(Item::note(
            "control:pending",
            vec![
                Span::styled(format!("{} ", theme.markers.busy), theme.info()),
                Span::styled(
                    format!("{} for {subject}", pending.activity()),
                    theme.text(),
                ),
            ],
        ));
    }
    let snapshot = match &control.read {
        Read::Ready(_) => control.ready(),
        _ => None,
    };
    match (&control.read, snapshot) {
        (_, Some(snapshot)) => items.push(session_row(ctx, snapshot)),
        (Read::NotInitialized, _) => items.push(
            Item::note(
                "control:uninitialized",
                vec![
                    Span::styled(format!("{} ", theme.markers.warn), theme.warn()),
                    Span::styled("No initialized team".to_owned(), theme.text()),
                    Span::styled("  membership cannot be changed".to_owned(), theme.muted()),
                ],
            )
            .with_detail(paragraph(theme, NOT_INITIALIZED, ctx.width)),
        ),
        (Read::Failed(error), _) => items.push(
            Item::note(
                "control:unreadable",
                vec![
                    Span::styled(format!("{} ", theme.markers.fail), theme.bad()),
                    Span::styled(
                        text::one_line(&format!("The session team could not be read: {error}")),
                        theme.text(),
                    ),
                ],
            )
            .with_detail(paragraph(theme, error, ctx.width)),
        ),
        _ => items.push(Item::note(
            "control:reading",
            vec![
                Span::styled(format!("{} ", theme.markers.idle), theme.faint()),
                Span::styled("Reading the session team".to_owned(), theme.muted()),
            ],
        )),
    }
    if let Some(retry) = control.retry.as_ref().filter(|r| r.session() == session) {
        items.push(retry_row(ctx, retry));
    }
    if let Some(receipt) = control
        .continuation
        .as_ref()
        .filter(|receipt| receipt.command.context.session_id == session)
    {
        let context = &receipt.command.context;
        let mut detail = field(
            theme,
            "directory",
            &context.directory.display().to_string(),
            ctx.width,
        );
        detail.extend(field(
            theme,
            "authorization",
            &receipt.authorization_id,
            ctx.width,
        ));
        detail.extend(field(
            theme,
            "acknowledged",
            &format!(
                "{} failed calls, effects unverified",
                context.failures.len()
            ),
            ctx.width,
        ));
        detail.push(Line::default());
        detail.extend(paragraph(
            theme,
            "The authorization is recorded and the run has not started. Enter starts the ordinary continuation of this session in its directory; the authorization is not requested again.",
            ctx.width,
        ));
        items.push(
            Item::row(
                CONTINUE_KEY,
                vec![
                    Cell::text(theme.markers.warn, theme.warn()),
                    Cell::text("Continue with current files: authorized", theme.text()),
                    Cell::text("not started", theme.warn()),
                ],
            )
            .with_detail(detail),
        );
    }
    let Some(snapshot) = snapshot else {
        return items;
    };
    items.push(Item::table("Current team", &TEAM_COLUMNS));
    for (id, state) in team_rows(snapshot) {
        items.push(member_row(ctx, snapshot, &id, state));
    }
    items.push(Item::table("Stopped work", &STAGE_COLUMNS));
    let stopped: Vec<&RecoveryStage> = snapshot
        .view
        .recovery_stages
        .iter()
        .filter(|stage| stage.status != RecoveryStatus::Complete)
        .collect();
    if stopped.is_empty() {
        items.push(Item::note(
            "control:no-stages",
            vec![Span::styled(
                "Nothing is stopped in this session.".to_owned(),
                theme.muted(),
            )],
        ));
    }
    for stage in stopped {
        items.push(stage_row(ctx, snapshot, stage));
    }
    items
}

/// How an agent stands in the session team.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Standing {
    Member,
    Leaving,
    Joining,
    Removed,
}

impl Standing {
    pub fn word(self) -> &'static str {
        match self {
            Standing::Member => "member",
            Standing::Leaving => "leaving",
            Standing::Joining => "joining",
            Standing::Removed => "removed",
        }
    }
}

/// Every agent the session team mentions, in a stable order.
pub fn team_rows(snapshot: &Snapshot) -> Vec<(String, Standing)> {
    let view = &snapshot.view;
    let mut rows: Vec<(String, Standing)> = Vec::new();
    for id in &view.effective.current_members {
        let leaving = view.pending_departures.iter().any(|d| &d.agent_id == id);
        rows.push((
            id.clone(),
            if leaving {
                Standing::Leaving
            } else {
                Standing::Member
            },
        ));
    }
    for id in &view.desired_members {
        if !rows.iter().any(|(known, _)| known == id) {
            rows.push((id.clone(), Standing::Joining));
        }
    }
    for id in &snapshot.excluded {
        if !rows.iter().any(|(known, _)| known == id) {
            rows.push((id.clone(), Standing::Removed));
        }
    }
    rows
}

fn session_row(ctx: &Ctx, snapshot: &Snapshot) -> Item {
    let theme = ctx.theme;
    let view = &snapshot.view;
    let (marker, state, style) = match &view.control {
        OwnerRunControl::Continue => (theme.markers.ok, "no hold".to_owned(), theme.good()),
        OwnerRunControl::Paused => (theme.markers.paused, "paused".to_owned(), theme.warn()),
        OwnerRunControl::Waiting { .. } => {
            (theme.markers.paused, "waiting".to_owned(), theme.warn())
        }
    };
    let mut detail = field(theme, "session", &view.session_id, ctx.width);
    detail.extend(field(theme, "hold", &state, ctx.width));
    if let OwnerRunControl::Waiting { condition } = &view.control {
        detail.extend(field(theme, "condition", condition, ctx.width));
    }
    detail.extend(field(
        theme,
        "team revision",
        &view.revision.to_string(),
        ctx.width,
    ));
    detail.extend(field(theme, "read at", &snapshot.read_at, ctx.width));
    detail.push(Line::default());
    detail.extend(paragraph(
        theme,
        "Enter lists the session actions. Pause and Wait hold the whole session; Continue releases that hold without starting a run, and /resume starts it.",
        ctx.width,
    ));
    Item::row(
        SESSION_KEY,
        vec![
            Cell::text(marker, style),
            Cell::text(
                format!(
                    "Session {} · {} in team",
                    text::short_id(&view.session_id),
                    view.effective.current_members.len()
                ),
                theme.text(),
            ),
            Cell::text(state, style),
        ],
    )
    .with_detail(detail)
}

fn retry_row(ctx: &Ctx, retry: &Request) -> Item {
    let theme = ctx.theme;
    let mut detail = field(theme, "command", retry.activity(), ctx.width);
    detail.extend(field(theme, "session", retry.session(), ctx.width));
    detail.push(Line::default());
    detail.extend(paragraph(
        theme,
        "Its outcome did not arrive as a success. Enter sends exactly the same command again: if the first one was recorded, its recorded outcome is returned and nothing runs twice. A changed situation needs a new choice instead.",
        ctx.width,
    ));
    Item::row(
        RETRY_KEY,
        vec![
            Cell::text(theme.markers.warn, theme.warn()),
            Cell::text(format!("Last command: {}", retry.activity()), theme.text()),
            Cell::text("not confirmed", theme.warn()),
        ],
    )
    .with_detail(detail)
}

fn member_row(ctx: &Ctx, snapshot: &Snapshot, id: &str, standing: Standing) -> Item {
    let theme = ctx.theme;
    let view = &snapshot.view;
    let model = label::agent(
        id,
        ctx.records.trace.as_ref(),
        ctx.pool.agent(id).map(|agent| &agent.identity),
        ctx.config,
    );
    // Two members can run the same model; the ID a command names tells them apart.
    let name = if model == id {
        model
    } else {
        format!("{model}  {id}")
    };
    let work: Vec<_> = view
        .responsibilities
        .iter()
        .filter(|responsibility| responsibility.agent_id == id)
        .collect();
    let (marker, style) = match standing {
        Standing::Member => (theme.markers.ok, theme.good()),
        Standing::Leaving => (theme.markers.warn, theme.warn()),
        Standing::Joining => (theme.markers.idle, theme.info()),
        Standing::Removed => (theme.markers.idle, theme.faint()),
    };
    let mut detail = field(theme, "agent", id, ctx.width);
    detail.extend(field(theme, "state", standing.word(), ctx.width));
    if let Some(departure) = view.pending_departures.iter().find(|d| d.agent_id == id) {
        detail.extend(field(
            theme,
            "replacement",
            departure.replacement_id.as_deref().unwrap_or("none"),
            ctx.width,
        ));
    }
    if let Some(departure) = view
        .pending_departures
        .iter()
        .find(|d| d.replacement_id.as_deref() == Some(id))
    {
        detail.extend(field(theme, "replaces", &departure.agent_id, ctx.width));
    }
    if work.is_empty() {
        detail.extend(field(theme, "work", "none", ctx.width));
    }
    for responsibility in &work {
        let task = responsibility
            .task
            .as_ref()
            .map(|task| {
                format!(
                    " · task {} attempt {}",
                    text::short_id(&task.task_id),
                    task.attempt
                )
            })
            .unwrap_or_default();
        detail.extend(field(
            theme,
            "work",
            &format!(
                "{} {}{task}",
                responsibility.kind.replace('_', " "),
                text::short_id(&responsibility.record_id)
            ),
            ctx.width,
        ));
    }
    let eligible = view
        .eligible_candidates
        .iter()
        .any(|candidate| candidate.profile.id == id);
    detail.extend(field(
        theme,
        "eligible",
        if eligible { "true" } else { "false" },
        ctx.width,
    ));
    detail.push(Line::default());
    detail.extend(paragraph(
        theme,
        match standing {
            Standing::Leaving => "A leaving agent keeps its current work until it ends and is given no new work. Its replacement joins once that work has ended.",
            Standing::Removed => "Removed by the owner. It is not selected again until the owner adds it.",
            _ => "Enter lists the membership actions. Changes apply to this session only; starting preferences are unchanged.",
        },
        ctx.width,
    ));
    let work_cell = match work.len() {
        0 => "idle".to_owned(),
        1 => work[0].kind.replace('_', " "),
        n => format!("{} +{}", work[0].kind.replace('_', " "), n - 1),
    };
    Item::row(
        format!("{MEMBER_PREFIX}{id}"),
        vec![
            Cell::text(marker, style),
            Cell::text(name, theme.text()),
            Cell::text(standing.word(), style),
            Cell::text(
                work_cell,
                if work.is_empty() {
                    theme.faint()
                } else {
                    theme.info()
                },
            ),
        ],
    )
    .with_detail(detail)
}

fn stage_row(ctx: &Ctx, snapshot: &Snapshot, stage: &RecoveryStage) -> Item {
    let theme = ctx.theme;
    let (marker, style) = match stage.status {
        RecoveryStatus::Running => (theme.markers.busy, theme.info()),
        RecoveryStatus::Pending => (theme.markers.idle, theme.muted()),
        _ => (theme.markers.warn, theme.warn()),
    };
    let subject = match &stage.plan {
        Some(plan) => format!(
            "{} · {}",
            purpose_word(&stage.purpose),
            text::one_line(&plan.plan.summary)
        ),
        None => purpose_word(&stage.purpose),
    };
    let mut detail = field(theme, "stage", &stage.id, ctx.width);
    detail.extend(field(theme, "status", status_word(stage.status), ctx.width));
    detail.extend(field(theme, "reason", &reason(stage), ctx.width));
    if let Some(condition) = &stage.condition {
        detail.extend(field(theme, "condition", condition, ctx.width));
    }
    if let Some(plan) = &stage.plan {
        detail.extend(field(theme, "saved plan", &plan.plan.summary, ctx.width));
        detail.extend(field(
            theme,
            "plan version",
            &format!("{} revision {}", plan.proposal_id, plan.revision),
            ctx.width,
        ));
    }
    if let Some(task) = &stage.task {
        detail.extend(field(
            theme,
            "task",
            &format!("{} attempt {}", task.task_id, task.attempt),
            ctx.width,
        ));
    }
    for failure in &stage.failures {
        detail.extend(field(
            theme,
            "failed call",
            &failure_words(failure),
            ctx.width,
        ));
    }
    if !stage.failures.is_empty() {
        detail.extend(field(
            theme,
            "effects",
            if stage.effect_resolution.is_some() {
                "inspected"
            } else if stage.effects_resolved() {
                "read-only calls ended"
            } else {
                "unverified"
            },
            ctx.width,
        ));
    }
    if let Some(review) = &stage.fresh_plan_review {
        detail.extend(field(
            theme,
            "new review",
            match review.next_action {
                FreshPlanReviewNextAction::AwaitOwnerContinuation => {
                    "recorded, awaiting your choice"
                }
                FreshPlanReviewNextAction::ConsumeRecordedVerdict => {
                    "recorded, used by the next run"
                }
                FreshPlanReviewNextAction::VerdictConsumed => "used by a run",
            },
            ctx.width,
        ));
    }
    if snapshot.authorization(stage).is_some() {
        detail.extend(field(
            theme,
            "current files",
            "continuation authorized; effects stay unverified",
            ctx.width,
        ));
    }
    if let Some(denial) = &stage.admission_denial {
        detail.extend(field(theme, "limit", &denial.to_string(), ctx.width));
    }
    if let Some(origin) = &stage.owner_hold {
        detail.extend(field(
            theme,
            "after release",
            &format!(
                "{} · {}",
                status_word(origin.status),
                origin
                    .wait_reason
                    .map(wait_reason_word)
                    .unwrap_or("reason not recorded")
            ),
            ctx.width,
        ));
    }
    let controls = snapshot
        .view
        .recovery_actions
        .iter()
        .find(|actions| actions.stage_id == stage.id)
        .map(|actions| actions.controls.clone())
        .unwrap_or_else(|| stage.manual_actions().controls);
    detail.extend(field(
        theme,
        "actions",
        &controls
            .iter()
            .map(|control| control_word(*control))
            .collect::<Vec<_>>()
            .join(", "),
        ctx.width,
    ));
    detail.push(Line::default());
    detail.extend(paragraph(
        theme,
        "Enter lists the actions this stage supports. Failed calls stay recorded, and their effects stay unverified until an inspection establishes them; continuing with current files does not change that.",
        ctx.width,
    ));
    Item::row(
        format!("{STAGE_PREFIX}{}", stage.id),
        vec![
            Cell::text(marker, style),
            Cell::text(subject, theme.text()),
            Cell::text(status_word(stage.status), style),
            Cell::text(reason(stage), style),
        ],
    )
    .with_detail(detail)
}

pub fn purpose_word(purpose: &str) -> String {
    match purpose {
        "plan" => "planning".into(),
        "review_plan" => "plan review".into(),
        "review" => "result review".into(),
        "final_review" => "final review".into(),
        other => other.replace('_', " "),
    }
}

pub fn status_word(status: RecoveryStatus) -> &'static str {
    match status {
        RecoveryStatus::Pending => "pending",
        RecoveryStatus::Running => "running",
        RecoveryStatus::Waiting => "waiting",
        RecoveryStatus::OwnerAction => "needs owner",
        RecoveryStatus::Paused => "paused",
        RecoveryStatus::Complete => "complete",
    }
}

fn wait_reason_word(reason: RecoveryWaitReason) -> &'static str {
    match reason {
        RecoveryWaitReason::OwnerWait => "held by the owner",
        RecoveryWaitReason::OwnerPause => "paused by the owner",
        RecoveryWaitReason::ParticipantAvailability => "no eligible participant",
        RecoveryWaitReason::RecoveryPolicy => "recovery limit reached",
        RecoveryWaitReason::Admission => "resource limit reached",
        RecoveryWaitReason::UncertainEffects => "failed call effects unverified",
        RecoveryWaitReason::Cancelled => "cancelled",
        RecoveryWaitReason::LegacyUnbound => "earlier call not bound to this stage",
        RecoveryWaitReason::EffectsInspected => "effects inspected",
        RecoveryWaitReason::FreshPlanReviewRecorded => "new plan review recorded",
    }
}

/// Why a stage is stopped, from its typed record.
pub fn reason(stage: &RecoveryStage) -> String {
    match stage.wait_reason {
        Some(reason) => wait_reason_word(reason).to_owned(),
        None if stage.status == RecoveryStatus::Running => "in progress".into(),
        None if !stage.failures.is_empty() => "failed call".into(),
        None => "reason not recorded".into(),
    }
}

fn failure_words(failure: &InvocationFailure) -> String {
    let class = match failure.class {
        FailureClass::TransientTransport => "transport failure",
        FailureClass::Authentication => "authentication",
        FailureClass::QuotaExhausted => "quota exhausted",
        FailureClass::UnsupportedConfiguration => "unsupported configuration",
        FailureClass::Timeout => "timeout",
        FailureClass::Cancelled => "cancelled",
        FailureClass::MalformedResponse => "malformed response",
        FailureClass::Unknown => "unknown failure",
    };
    format!(
        "{} · {class} · {} · {} · invocation {}",
        failure.agent_id,
        if failure.effective_access.is_read_only() {
            "read-only"
        } else {
            "could write"
        },
        match failure.termination {
            TerminationEvidence::BackendEnded => "ended",
            TerminationEvidence::UnverifiedAfterRestart => "end not verified",
        },
        text::short_id(&failure.invocation_id)
    )
}

pub fn control_word(control: RecoveryControlKind) -> &'static str {
    match control {
        RecoveryControlKind::Retry => "Retry",
        RecoveryControlKind::Continue => "Continue",
        RecoveryControlKind::Wait => "Wait",
        RecoveryControlKind::Pause => "Pause",
        RecoveryControlKind::ReleaseHold => "Release hold",
        RecoveryControlKind::InspectEffects => "Inspect effects",
        RecoveryControlKind::ReviewSavedPlanFresh => "Review saved plan again",
    }
}
