use super::{
    provenance::{event, record, records, validate_task},
    Store,
};
use anyhow::{ensure, Context, Result};
use rusqlite::{params, Connection, Transaction};
use serde_json::{json, Value};
use ymp_core::*;

fn key(id: &str) -> String {
    format!("team_grant:v1:{id}")
}
pub(super) fn grant(db: &Connection, id: &str) -> Result<GrantRecord> {
    let raw: String = db.query_row("SELECT value FROM kv WHERE key=?", [key(id)], |r| r.get(0))?;
    let value: GrantRecord = serde_json::from_str(&raw)?;
    ensure!(value.schema_version == 1, "Unsupported team grant record");
    Ok(value)
}
pub(super) fn issue(
    tx: &Transaction<'_>,
    assignment: &AssignmentRecord,
    invocation: &InvocationRecord,
    value: &GrantRecord,
) -> Result<()> {
    ensure!(
        value.schema_version == 1
            && !value.id.is_empty()
            && !value.operations.is_empty()
            && value.revoked_at.is_none()
            && value.revocation_reason.is_none(),
        "Invalid new grant"
    );
    ensure!(
        value.session_id == assignment.session_id
            && value.agent_id == assignment.agent_id
            && value.assignment_id == assignment.id
            && value.invocation_id == invocation.id,
        "Grant scope differs from the admitted assignment"
    );
    tx.execute(
        "INSERT INTO kv(key,value) VALUES (?,?)",
        params![key(&value.id), serde_json::to_string(value)?],
    )?;
    event(
        tx,
        &value.session_id,
        &ProvenanceEvent::GrantIssued {
            grant: Box::new(value.clone()),
        },
    )?;
    Ok(())
}
pub(super) fn revoke(
    tx: &Transaction<'_>,
    assignment: &AssignmentRecord,
    reason: &str,
) -> Result<()> {
    for id in &assignment.grant_ids {
        let mut value = grant(tx, id)?;
        if value.revoked_at.is_some() {
            continue;
        }
        value.revoked_at = Some(now());
        value.revocation_reason = Some(reason.into());
        tx.execute(
            "UPDATE kv SET value=? WHERE key=?",
            params![serde_json::to_string(&value)?, key(id)],
        )?;
        event(
            tx,
            &assignment.session_id,
            &ProvenanceEvent::GrantRevoked {
                grant: Box::new(value),
            },
        )?;
    }
    Ok(())
}

impl Store {
    pub fn team_grant(&self, session: &str, id: &str) -> Result<GrantRecord> {
        let value = grant(&*self.db()?, id)?;
        ensure!(
            value.session_id == session,
            "Grant belongs to another session"
        );
        Ok(value)
    }

    /// Called only after the local runtime validates its process-owned token.
    /// The database transaction prevents completion/reassignment from racing a
    /// validated write. A persisted grant ID alone is never an API credential.
    pub fn team_call(
        &self,
        bound: &GrantRecord,
        operation: TeamOperation,
        arguments: &Value,
        request_id: &str,
    ) -> Result<(Value, Option<Message>)> {
        let mut db = self.db()?;
        let tx = db.transaction()?;
        let current = grant(&tx, &bound.id)?;
        ensure!(
            current == *bound && current.revoked_at.is_none(),
            "Team grant is no longer active"
        );
        ensure!(
            current.operations.contains(&operation),
            "Operation is outside this assignment's grant"
        );
        let assignment: AssignmentRecord = record(&tx, "assignments", &current.assignment_id)?;
        let invocation: InvocationRecord = record(&tx, "invocations", &current.invocation_id)?;
        ensure!(
            assignment.state == InvocationState::Running
                && invocation.state == InvocationState::Running,
            "Assignment is no longer running"
        );
        ensure!(
            assignment.session_id == current.session_id
                && assignment.agent_id == current.agent_id
                && invocation.assignment_id == assignment.id
                && invocation.session_id == current.session_id
                && assignment.grant_ids.contains(&current.id),
            "Assignment grant binding mismatch"
        );
        if let Some(task) = &assignment.task {
            validate_task(&tx, &current.session_id, task, true)?;
            if assignment.purpose == "execute" {
                let current_task: Task = record(&tx, "tasks", &task.task_id)?;
                ensure!(
                    current_task.state == TaskState::Running
                        && current_task.assignee.as_ref() == Some(&current.agent_id),
                    "Execution claim ended or was reassigned"
                );
            }
        }
        let session: Session = record(&tx, "sessions", &current.session_id)?;
        let a = arguments
            .as_object()
            .context("Tool arguments must be an object")?;
        // Runtime bindings cannot be overridden by hidden tool arguments. Reject
        // unknown properties too, including self-granted permission/status fields.
        let allowed: &[&str] = match operation {
            TeamOperation::TeamPost => &["text", "recipient"],
            TeamOperation::TeamRead => &["after", "limit"],
            TeamOperation::TasksList => &[],
            TeamOperation::TaskPropose => &["title", "description"],
            TeamOperation::MemorySearch => &["query", "include_unconfirmed", "cursor", "limit"],
            TeamOperation::MemoryPropose => &["title", "content"],
        };
        ensure!(
            a.keys().all(|field| allowed.contains(&field.as_str())),
            "Tool arguments cannot override runtime authority or contain unknown fields"
        );
        let mut message = None;
        let result = match operation {
            TeamOperation::TeamPost | TeamOperation::TaskPropose => {
                let (kind, text) = if operation == TeamOperation::TeamPost {
                    (
                        "chat",
                        arguments["text"]
                            .as_str()
                            .context("text is required")?
                            .to_owned(),
                    )
                } else {
                    ensure!(
                        arguments["title"].is_string() && arguments["description"].is_string(),
                        "title and description are required"
                    );
                    ("proposal", arguments.to_string())
                };
                ensure!(text.len() <= 32_000, "Message exceeds 32000 bytes");
                let recipient = arguments["recipient"].as_str();
                ensure!(
                    recipient.is_none_or(|id| session.team.iter().any(|agent| agent.id == id)),
                    "Unknown recipient"
                );
                let created_at = now();
                tx.execute("INSERT INTO messages(session_id,author,recipient,kind,text,created_at) VALUES (?,?,?,?,?,?)",
                    params![current.session_id, current.agent_id, recipient, kind, text, created_at])?;
                let msg = Message {
                    seq: tx.last_insert_rowid(),
                    session_id: current.session_id.clone(),
                    author: current.agent_id.clone(),
                    recipient: recipient.map(str::to_owned),
                    kind: kind.into(),
                    text,
                    created_at,
                };
                tx.execute(
                    "INSERT INTO events(session_id,kind,data,created_at) VALUES (?,'message',?,?)",
                    params![current.session_id, serde_json::to_string(&msg)?, now()],
                )?;
                let result = if operation == TeamOperation::TeamPost {
                    json!({"seq":msg.seq})
                } else {
                    json!({"status":"proposed","note":"Proposal requires runtime validation at a planning boundary"})
                };
                message = Some(msg);
                result
            }
            TeamOperation::TeamRead => {
                let mut query = tx.prepare("SELECT seq,session_id,author,recipient,kind,text,created_at FROM messages WHERE session_id=? AND seq>? ORDER BY seq LIMIT ?")?;
                let values = query
                    .query_map(
                        params![
                            current.session_id,
                            arguments["after"].as_i64().unwrap_or(0),
                            arguments["limit"].as_u64().unwrap_or(30).min(100)
                        ],
                        |r| {
                            Ok(Message {
                                seq: r.get(0)?,
                                session_id: r.get(1)?,
                                author: r.get(2)?,
                                recipient: r.get(3)?,
                                kind: r.get(4)?,
                                text: r.get(5)?,
                                created_at: r.get(6)?,
                            })
                        },
                    )?
                    .collect::<rusqlite::Result<Vec<_>>>()?;
                serde_json::to_value(values)?
            }
            TeamOperation::TasksList => {
                serde_json::to_value(records::<Task>(&tx, "tasks", &current.session_id)?)?
            }
            TeamOperation::MemorySearch => {
                let mode = if arguments["include_unconfirmed"] == true {
                    KnowledgeRetrievalMode::IncludeUnconfirmed
                } else {
                    KnowledgeRetrievalMode::Supported
                };
                let values = super::read_memory_candidates(
                    &tx,
                    Some(&session.project_id),
                    arguments["query"].as_str().unwrap_or(""),
                )?
                .into_iter()
                .filter_map(|entry| {
                    match super::knowledge::applicable(
                        &tx,
                        &entry,
                        Some(&session.project_id),
                        &Default::default(),
                        mode,
                    ) {
                        Ok(true) => Some(Ok(entry)),
                        Ok(false) => None,
                        Err(error) => Some(Err(error)),
                    }
                })
                .collect::<Result<Vec<_>>>()?;
                let offset = arguments
                    .get("cursor")
                    .map(|v| {
                        v.as_str()
                            .context("cursor must be a string")?
                            .parse::<usize>()
                            .context("Invalid memory cursor")
                    })
                    .transpose()?
                    .unwrap_or(0);
                let limit = arguments
                    .get("limit")
                    .map(|v| v.as_u64().context("limit must be a positive integer"))
                    .transpose()?
                    .unwrap_or(10);
                let rows = values.iter().map(|entry| Ok(serde_json::json!({"id":entry.id,"version":content_digest(&serde_json::to_string(entry)?),"entry":entry}))).collect::<Result<Vec<_>>>()?;
                super::projection::page(&rows, offset, usize::try_from(limit)?)?
            }
            TeamOperation::MemoryPropose => {
                let value = MemoryEntry {
                    provenance: Some(KnowledgeProvenance {
                        confirmation: ConfirmationStatus::Unconfirmed,
                        applicability: Default::default(),
                        source: None,
                        assignment_id: Some(current.assignment_id.clone()),
                        invocation_id: Some(current.invocation_id.clone()),
                        policy: KnowledgePolicyIdentity {
                            id: "ymp.team-proposal".into(),
                            version: "1".into(),
                        },
                    }),
                    id: new_id(),
                    project_id: Some(session.project_id),
                    kind: "procedure".into(),
                    title: arguments["title"]
                        .as_str()
                        .context("title required")?
                        .into(),
                    content: arguments["content"]
                        .as_str()
                        .context("content required")?
                        .into(),
                    source_session: current.session_id.clone(),
                    author: current.agent_id.clone(),
                    reviewer: None,
                    status: "proposed".into(),
                    created_at: now(),
                    supersedes: None,
                };
                ensure!(
                    value.title.len() <= 1000 && value.content.len() <= 32_000,
                    "Memory proposal exceeds size limit"
                );
                tx.execute(
                    "INSERT INTO memory(id,project_id,status,data) VALUES (?,?,?,?)",
                    params![
                        value.id,
                        value.project_id,
                        value.status,
                        serde_json::to_string(&value)?
                    ],
                )?;
                tx.execute(
                    "INSERT INTO memory_search(id,title,content) VALUES (?,?,?)",
                    params![value.id, value.title, value.content],
                )?;
                json!({"id":value.id,"status":"proposed"})
            }
        };
        event(
            &tx,
            &current.session_id,
            &ProvenanceEvent::TeamOperationCommitted {
                grant_id: current.id,
                assignment_id: current.assignment_id,
                invocation_id: current.invocation_id,
                agent_id: current.agent_id,
                request_id: request_id.into(),
                operation,
                message_seq: message.as_ref().map(|m| m.seq),
                memory_id: if operation == TeamOperation::MemoryPropose {
                    result["id"].as_str().map(str::to_owned)
                } else {
                    None
                },
            },
        )?;
        tx.commit()?;
        Ok((result, message))
    }
}
