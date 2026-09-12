use crate::Store;
use anyhow::{ensure, Context, Result};
use rusqlite::{params, Connection, OptionalExtension, Transaction};
use ymp_core::*;

fn rows<T: serde::de::DeserializeOwned>(
    db: &Connection,
    table: &str,
    session: &str,
) -> Result<Vec<T>> {
    let mut q = db.prepare(&format!("SELECT data FROM {table} WHERE session_id=?"))?;
    let raw = q.query_map([session], |r| r.get::<_, String>(0))?;
    raw.map(|r| Ok(serde_json::from_str(&r?)?)).collect()
}
fn allowance(assignment: &AssignmentRecord, resources: &ResourceLimits) -> Result<Option<u64>> {
    if let Some(requested) = assignment.token_reservation {
        ensure!(requested > 0 && resources.invocation_tokens.is_some_and(|ceiling| requested <= ceiling), "token_reservation_limit: requested allowance must be positive and within the captured per-invocation ceiling");
        Ok(Some(requested))
    } else {
        Ok(resources.invocation_tokens)
    }
}

pub(super) fn snapshot(db: &Connection, session: &str) -> Result<Option<SessionBudget>> {
    let policy: Option<String> = db
        .query_row(
            "SELECT data FROM session_policies WHERE session_id=?",
            [session],
            |r| r.get(0),
        )
        .optional()?;
    let limits = if let Some(policy) = policy {
        serde_json::from_str::<SessionPolicy>(&policy)?.limits
    } else {
        let captured: Option<String> = db
            .query_row(
                "SELECT value FROM kv WHERE key=?",
                [format!("legacy_budget_limits:{session}")],
                |r| r.get(0),
            )
            .optional()?;
        let Some(captured) = captured else {
            return Ok(None);
        };
        serde_json::from_str::<Limits>(&captured)?
    };
    let usage = super::usage::session_usage(db, session)?.total;
    let assignments: Vec<AssignmentRecord> = rows(db, "assignments", session)?;
    let invocations: Vec<InvocationRecord> = rows(db, "invocations", session)?;
    let tasks: Vec<Task> = rows(db, "tasks", session)?;
    let startup = assignments
        .iter()
        .filter(|a| startup_purpose(&a.purpose))
        .count() as u64;
    let reviews = assignments
        .iter()
        .filter(|a| required_review_purpose(&a.purpose))
        .count() as u64;
    let running_reviews = assignments
        .iter()
        .filter(|a| required_review_purpose(&a.purpose) && a.state == InvocationState::Running)
        .count() as u64;
    let mut protected = limits
        .resources
        .as_ref()
        .map_or(0, |r| r.required_review_invocations.saturating_sub(reviews));
    let mut task_review_pending = false;
    if limits.resources.is_some() && !tasks.is_empty() {
        // Each unresolved task needs review; final review remains required until
        // the runtime has recorded approval, not merely a completed model call.
        let pending = tasks
            .iter()
            .filter(|t| t.state != TaskState::Accepted)
            .count() as u64;
        let decisions: Vec<DecisionRecord> = rows(db, "decisions", session)?;
        let final_approved = decisions.iter().any(|d| {
            d.kind == "final_review" && matches!(d.outcome, Some(DecisionOutcome::Accepted { .. }))
        });
        task_review_pending = pending > 0 || !final_approved;
        protected =
            protected.max((pending + u64::from(!final_approved)).saturating_sub(running_reviews));
    }
    let resources = limits.resources.as_ref();
    let mut reserved = 0u64;
    let mut live_review_reservations = 0u64;
    let mut review_spend = 0u64;
    for invocation in &invocations {
        let assignment = assignments
            .iter()
            .find(|a| a.id == invocation.assignment_id)
            .context("Missing reservation assignment")?;
        let observed = invocation
            .usage
            .as_ref()
            .and_then(|u| u.counts.known_total())
            .unwrap_or(0);
        if required_review_purpose(&assignment.purpose) {
            review_spend = review_spend
                .checked_add(observed)
                .context("Review usage arithmetic overflow")?;
        }
        if invocation.state == InvocationState::Running {
            let tokens = resources
                .map(|r| allowance(assignment, r))
                .transpose()?
                .flatten()
                .unwrap_or(0)
                .saturating_sub(observed);
            reserved = reserved
                .checked_add(tokens)
                .context("Reservation arithmetic overflow")?;
            if required_review_purpose(&assignment.purpose) {
                live_review_reservations = live_review_reservations
                    .checked_add(tokens)
                    .context("Review reservation arithmetic overflow")?;
            }
        }
    }
    let reserved_tokens = resources
        .and_then(|r| r.invocation_tokens)
        .map(|_| reserved);
    let protected_review_tokens = if let Some(resources) = resources {
        if let Some(total) = resources.review_reserve_tokens {
            let completed = assignments
                .iter()
                .filter(|a| {
                    required_review_purpose(&a.purpose) && a.state == InvocationState::Completed
                })
                .count() as u64;
            Some(
                if completed < resources.required_review_invocations || task_review_pending {
                    total
                        .saturating_sub(review_spend)
                        .saturating_sub(live_review_reservations)
                } else {
                    0
                },
            )
        } else {
            resources
                .invocation_tokens
                .map(|each| {
                    each.checked_mul(protected)
                        .context("Review protection arithmetic overflow")
                })
                .transpose()?
        }
    } else {
        None
    };
    let last: Option<String> = db
        .query_row(
            "SELECT value FROM kv WHERE key=?",
            [format!("budget_denial:{session}")],
            |r| r.get(0),
        )
        .optional()?;
    let overshoot = limits
        .resources
        .as_ref()
        .and_then(|r| r.observed_tokens)
        .and_then(|ceiling| {
            usage
                .known_total()
                .map(|known| known.saturating_sub(ceiling))
        });
    Ok(Some(SessionBudget {
        limits,
        admitted_invocations: usage.calls,
        in_flight_invocations: usage.open_calls,
        startup_invocations: startup,
        protected_review_invocations: protected,
        protected_review_tokens,
        reserved_tokens,
        observed_usage: usage,
        observed_token_overshoot: overshoot,
        last_denial: last.map(|raw| serde_json::from_str(&raw)).transpose()?,
        strict_token_bound: false,
    }))
}

pub(super) fn denial(
    db: &Connection,
    assignment: &AssignmentRecord,
) -> Result<Option<BudgetDenial>> {
    let Some(b) = snapshot(db, &assignment.session_id)? else {
        return Ok(None);
    };
    let fail = |code: &str, message: &str| {
        Some(BudgetDenial {
            code: code.into(),
            message: message.into(),
            purpose: assignment.purpose.clone(),
            at: now(),
        })
    };
    if b.admitted_invocations >= b.limits.turns as u64 {
        return Ok(fail(
            "invocation_limit",
            "Captured session invocation allowance is exhausted",
        ));
    }
    if b.in_flight_invocations >= b.limits.parallel as u64 {
        return Ok(fail(
            "concurrency_limit",
            "Captured session concurrency allowance is occupied",
        ));
    }
    if assignment.timeout_secs == 0 || assignment.timeout_secs > b.limits.turn_timeout_secs {
        return Ok(fail(
            "timeout_limit",
            "Assignment timeout exceeds the captured session limit",
        ));
    }
    let Some(r) = &b.limits.resources else {
        return Ok(if assignment.token_reservation.is_some() {
            fail(
                "token_reservation_limit",
                "A token reservation requires a captured per-invocation ceiling",
            )
        } else {
            None
        });
    };
    r.validate()?;
    let requested_tokens = match allowance(assignment, r) {
        Ok(tokens) => tokens,
        Err(error) => return Ok(fail("token_reservation_limit", &error.to_string())),
    };
    if startup_purpose(&assignment.purpose) && b.startup_invocations >= r.startup_invocations {
        return Ok(fail(
            "startup_limit",
            "Captured initial-work allowance is exhausted",
        ));
    }
    if !required_review_purpose(&assignment.purpose)
        && b.admitted_invocations
            .saturating_add(1)
            .saturating_add(b.protected_review_invocations)
            > b.limits.turns as u64
    {
        return Ok(fail(
            "review_reserve",
            "Remaining invocations are protected for required review",
        ));
    }
    // Context references include the complete transmitted prompt as well as the
    // instructions, so nested/context-only sources are not charged twice.
    let context_chars = assignment
        .context
        .iter()
        .filter(|c| {
            matches!(
                c.kind,
                ContextKind::Prompt | ContextKind::ProfileInstructions
            )
        })
        .fold(0u64, |sum, c| {
            sum.saturating_add(c.included_chars.unwrap_or(0) as u64)
        });
    if context_chars > r.context_chars(&assignment.purpose) {
        return Ok(fail(
            "context_limit",
            "Assignment context exceeds the captured character allowance",
        ));
    }
    if let (Some(total), Some(each)) = (r.observed_tokens, requested_tokens) {
        let invocations: Vec<InvocationRecord> = rows(db, "invocations", &assignment.session_id)?;
        let incomplete_closed = invocations.iter().any(|i| {
            i.state != InvocationState::Running
                && i.usage.as_ref().is_none_or(|u| {
                    u.partial
                        || !u.finalized
                        || u.counts.input.is_none()
                        || u.counts.output.is_none()
                })
        });
        if r.unknown_usage == UnknownUsagePolicy::Stop
            && (incomplete_closed || b.observed_usage.calls > invocations.len() as u64)
        {
            return Ok(fail(
                "unknown_usage",
                "Incomplete native accounting prevents further admission under the token policy",
            ));
        }
        let committed = b
            .observed_usage
            .known_total()
            .unwrap_or(0)
            .checked_add(b.reserved_tokens.unwrap_or(0));
        let requested_total = committed.and_then(|value| value.checked_add(each));
        if requested_total.is_none_or(|value| value > total) {
            return Ok(fail(
                "token_limit",
                "Observed usage and in-flight reservations exhaust the token allowance",
            ));
        }
        let protected = if required_review_purpose(&assignment.purpose) {
            0
        } else {
            b.protected_review_tokens.unwrap_or(0)
        };
        if requested_total
            .and_then(|value| value.checked_add(protected))
            .is_none_or(|value| value > total)
        {
            return Ok(fail(
                "token_review_reserve",
                "Remaining token allowance is protected for required review",
            ));
        }
    }
    Ok(None)
}
pub(super) fn record_denial(
    tx: &Transaction<'_>,
    session: &str,
    denial: &BudgetDenial,
) -> Result<()> {
    let data = serde_json::to_string(denial)?;
    tx.execute("INSERT INTO kv(key,value) VALUES (?,?) ON CONFLICT(key) DO UPDATE SET value=excluded.value", params![format!("budget_denial:{session}"), data])?;
    tx.execute(
        "INSERT INTO events(session_id,kind,data,created_at) VALUES (?,'budget_denied',?,?)",
        params![session, data, denial.at],
    )?;
    Ok(())
}
pub(super) fn record_state(
    tx: &Transaction<'_>,
    session: &str,
    id: &str,
    kind: &str,
) -> Result<()> {
    if let Some(budget) = snapshot(tx, session)? {
        tx.execute(
            "INSERT INTO events(session_id,kind,data,created_at) VALUES (?,?,?,?)",
            params![
                session,
                kind,
                serde_json::json!({"invocation_id":id,"budget":budget}).to_string(),
                now()
            ],
        )?;
    }
    Ok(())
}
impl Store {
    /// Runtime-classified native limit stop. Raw provider errors are not stored.
    pub fn record_budget_stop(&self, session: &str, reason: &BudgetDenial) -> Result<()> {
        anyhow::ensure!(
            ["token_limit", "output_limit", "timeout_limit"].contains(&reason.code.as_str()),
            "Unknown native budget stop code"
        );
        let mut db = self.db()?;
        let tx = db.transaction()?;
        record_denial(&tx, session, reason)?;
        tx.commit()?;
        Ok(())
    }
    /// Capture a new admission boundary for an old session, without fabricating
    /// historical policy. INSERT OR IGNORE preserves it on subsequent resumes.
    pub fn capture_legacy_budget_limits(&self, session: &str, limits: &Limits) -> Result<()> {
        let mut db = self.db()?;
        let tx = db.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let exists: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM sessions WHERE id=?)",
            [session],
            |r| r.get(0),
        )?;
        anyhow::ensure!(exists, "Unknown session");
        let captured: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM session_policies WHERE session_id=?)",
            [session],
            |r| r.get(0),
        )?;
        if !captured {
            let data = serde_json::to_string(limits)?;
            if tx.execute(
                "INSERT OR IGNORE INTO kv(key,value) VALUES (?,?)",
                params![format!("legacy_budget_limits:{session}"), data],
            )? > 0
            {
                tx.execute("INSERT INTO events(session_id,kind,data,created_at) VALUES (?,'legacy_budget_captured',?,?)", params![session, data, now()])?;
            }
        }
        tx.commit()?;
        Ok(())
    }
    pub fn session_budget(&self, session: &str) -> Result<Option<SessionBudget>> {
        let mut db = self.db()?;
        let tx = db.transaction()?;
        let result = snapshot(&tx, session)?;
        tx.commit()?;
        Ok(result)
    }
}
