use super::{
    confirmation,
    provenance::{record, records},
    Store,
};
use anyhow::{ensure, Context, Result};
use rusqlite::{params, Connection, OptionalExtension, Transaction};
use std::collections::BTreeMap;
use ymp_core::*;

pub(super) fn write(tx: &Transaction<'_>, entry: &MemoryEntry) -> Result<()> {
    tx.execute("INSERT INTO memory(id,project_id,status,data) VALUES (?,?,?,?) ON CONFLICT(id) DO UPDATE SET project_id=excluded.project_id,status=excluded.status,data=excluded.data", params![entry.id, entry.project_id, entry.status, serde_json::to_string(entry)?])?;
    tx.execute("DELETE FROM memory_search WHERE id=?", [&entry.id])?;
    tx.execute(
        "INSERT INTO memory_search(id,title,content) VALUES (?,?,?)",
        params![entry.id, entry.title, entry.content],
    )?;
    Ok(())
}

pub(super) fn applicable(
    db: &Connection,
    entry: &MemoryEntry,
    project: Option<&str>,
    scope: &BTreeMap<String, String>,
    mode: KnowledgeRetrievalMode,
) -> Result<bool> {
    if !["active", "proposed"].contains(&entry.status.as_str())
        || entry
            .project_id
            .as_deref()
            .is_some_and(|p| Some(p) != project)
    {
        return Ok(false);
    }
    let Some(provenance) = &entry.provenance else {
        // Historical active records are context with unknown confirmation, never
        // upgraded into evidence. Runtime prompt assembly labels the distinction.
        return Ok(mode == KnowledgeRetrievalMode::IncludeUnconfirmed);
    };
    if !provenance
        .applicability
        .iter()
        .all(|(key, value)| scope.get(key) == Some(value))
    {
        return Ok(false);
    }
    if provenance.confirmation != ConfirmationStatus::Confirmed {
        return Ok(mode == KnowledgeRetrievalMode::IncludeUnconfirmed);
    }
    if entry.status != "active" {
        return Ok(false);
    }
    let Some(source) = &provenance.source else {
        return Ok(false);
    };
    let acceptance: DecisionRecord = record(db, "decisions", &source.acceptance_id)?;
    let Some(result) = &acceptance.links.result else {
        return Ok(false);
    };
    Ok(acceptance.session_id == entry.source_session
        && acceptance.kind == "task_accepted"
        && acceptance.outcome
            == Some(DecisionOutcome::Accepted {
                confirmation: ConfirmationStatus::Confirmed,
            })
        && source.result_id == result.id
        && source.result_version == result.version
        && source.criteria_version == result.criteria_version
        && source.confirmation_ids == acceptance.links.confirmation_ids
        && confirmation::current_task(db, result)?
        && confirmation::current_files(db, &entry.source_session, result)?
        && confirmation::grade(db, &entry.source_session, result)?.0
            == ConfirmationStatus::Confirmed)
}

impl Store {
    /// All lifecycle states for inspection; these are not automatically prompt context.
    pub fn memory_inventory(&self, project: Option<&str>) -> Result<Vec<MemoryEntry>> {
        let db = self.db()?;
        let mut q = db.prepare(
            "SELECT data FROM memory WHERE project_id IS NULL OR project_id=? ORDER BY rowid DESC",
        )?;
        let values = q
            .query_map([project.unwrap_or("")], |r| r.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        values
            .into_iter()
            .map(|s| Ok(serde_json::from_str(&s)?))
            .collect()
    }

    /// Lexical search with an explicit confirmation mode. Candidate inspection
    /// never changes status or qualifies the returned material as evidence.
    pub fn search_memory(
        &self,
        project: Option<&str>,
        query: &str,
        scope: &BTreeMap<String, String>,
        mode: KnowledgeRetrievalMode,
    ) -> Result<Vec<MemoryEntry>> {
        let db = self.db()?;
        super::read_memory_candidates(&db, project, query)?
            .into_iter()
            .filter_map(
                |entry| match applicable(&db, &entry, project, scope, mode) {
                    Ok(true) => Some(Ok(entry)),
                    Ok(false) => None,
                    Err(error) => Some(Err(error)),
                },
            )
            .collect()
    }

    /// Resolve the actual stored source after a retrieval policy nominates an ID.
    /// A stale version hint, foreign scope, retired entry or changed source is omitted.
    pub fn resolve_memory(
        &self,
        project: Option<&str>,
        id: &str,
        version: Option<&str>,
        scope: &BTreeMap<String, String>,
        mode: KnowledgeRetrievalMode,
    ) -> Result<Option<MemoryEntry>> {
        let db = self.db()?;
        let raw: Option<String> = db
            .query_row("SELECT data FROM memory WHERE id=?", [id], |r| r.get(0))
            .optional()?;
        let Some(raw) = raw else { return Ok(None) };
        let entry: MemoryEntry = serde_json::from_str(&raw)?;
        let actual_version = content_digest(&serde_json::to_string(&entry)?);
        if version.is_some_and(|v| v != actual_version)
            || !applicable(&db, &entry, project, scope, mode)?
        {
            return Ok(None);
        }
        Ok(Some(entry))
    }

    /// Persist one bounded proposal against an actual accepted source. Free text
    /// remains proposed even when the source result is confirmed.
    pub fn retain_knowledge(
        &self,
        acceptance_id: &str,
        proposal: &KnowledgeProposal,
        applicability: &BTreeMap<String, String>,
        policy: &KnowledgePolicyIdentity,
    ) -> Result<MemoryEntry> {
        ensure!(
            !policy.id.is_empty() && !policy.version.is_empty(),
            "Knowledge policy identity is required"
        );
        let mut db = self.db()?;
        let tx = db.transaction()?;
        let acceptance: DecisionRecord = record(&tx, "decisions", acceptance_id)?;
        ensure!(
            acceptance.kind == "task_accepted"
                && matches!(acceptance.outcome, Some(DecisionOutcome::Accepted { .. })),
            "Knowledge source must be an accepted task result"
        );
        let result = acceptance
            .links
            .result
            .as_ref()
            .context("Knowledge source has no result version")?;
        let session: Session = record(&tx, "sessions", &acceptance.session_id)?;
        let producer: AssignmentRecord = record(
            &tx,
            "assignments",
            result
                .producer_assignment_ids
                .first()
                .context("Knowledge source has no producer")?,
        )?;
        let confirmed = acceptance.outcome
            == Some(DecisionOutcome::Accepted {
                confirmation: ConfirmationStatus::Confirmed,
            })
            && confirmation::current_task(&tx, result)?
            && confirmation::current_files(&tx, &acceptance.session_id, result)?
            && confirmation::grade(&tx, &acceptance.session_id, result)?.0
                == ConfirmationStatus::Confirmed;
        let (kind, title, content, global, projected) = match proposal {
            KnowledgeProposal::ProjectOutcome => {
                let title = result
                    .task_definition
                    .as_ref()
                    .map(|d| d.title.clone())
                    .unwrap_or_else(|| "Accepted outcome".into());
                let content = if confirmed {
                    let mut text = format!("Confirmed criteria: {}. Scope: this exact result and its captured input/artifact versions.", result.criteria.iter().map(|c| c.description.as_str()).collect::<Vec<_>>().join("; "));
                    for artifact in &result.artifacts {
                        text.push_str(&format!(
                            "\nArtifact: {} (sha256 {}).",
                            artifact.path.display(),
                            artifact.sha256.as_deref().unwrap_or("missing")
                        ));
                        if let Some(bytes) = &artifact.bytes {
                            if let Ok(value) = std::str::from_utf8(bytes) {
                                text.push_str("\nCaptured artifact excerpt: ");
                                text.extend(value.chars().take(4000));
                            }
                        }
                    }
                    text
                } else {
                    format!("Accepted, unconfirmed result report: {}", result.summary)
                };
                ("outcome", title, content, false, true)
            }
            KnowledgeProposal::CheckProcedure => {
                let contract: Option<DecisionRecord> = result
                    .contract_id
                    .as_ref()
                    .map(|id| record(&tx, "decisions", id))
                    .transpose()?;
                let observed = acceptance
                    .links
                    .confirmation_ids
                    .iter()
                    .map(|id| record::<DecisionRecord>(&tx, "decisions", id))
                    .collect::<Result<Vec<_>>>()?;
                let passed_checks = observed
                    .iter()
                    .filter_map(|d| d.links.check.as_ref())
                    .filter(|check| check.outcome == ConfirmationCheckOutcome::Passed)
                    .map(|check| check.check_id.as_str())
                    .collect::<std::collections::HashSet<_>>();
                let mut procedures = Vec::new();
                if let Some(captured) = contract
                    .as_ref()
                    .and_then(|d| d.links.acceptance_contract.as_ref())
                {
                    for check in captured
                        .contract
                        .checks
                        .iter()
                        .filter(|check| passed_checks.contains(check.id.as_str()))
                    {
                        let description = match check.assertion {
                            CheckAssertion::ExactBytes { .. } => "For an exact-byte artifact contract, compare the actual file bytes with the declared expected bytes, including newlines.",
                            CheckAssertion::MatchesInput { .. } => "For an input-copy contract, compare the artifact with the actual supplied input bytes and preserve the input version.",
                            CheckAssertion::Command { .. } => "For a declared validator contract, pin the validator implementation and bind its observed result to the declared criteria and actual input/artifact versions.",
                        };
                        if !procedures.contains(&description) {
                            procedures.push(description);
                        }
                    }
                }
                ensure!(
                    !procedures.is_empty(),
                    "Reusable check experience needs a declared trusted check"
                );
                ("check_procedure", "Verify file-producing tasks with declared checks".into(), format!("{}\nApplicability: only the named check contract types. Source confirmation applies only to this recorded run; this does not confirm other task quality or any agent-written lesson.", procedures.join("\n")), true, true)
            }
            KnowledgeProposal::Candidate { title, content } => {
                ensure!(
                    !title.trim().is_empty()
                        && !content.trim().is_empty()
                        && title.len() <= 1000
                        && content.len() <= 32000,
                    "Invalid knowledge candidate size"
                );
                ("procedure", title.clone(), content.clone(), false, false)
            }
        };
        let key = content_digest(&serde_json::to_string(&(
            acceptance_id,
            proposal,
            applicability,
        ))?);
        let id = format!("knowledge:{key}");
        // Replaying acceptance cannot revive a retired or superseded entry.
        if let Some(raw) = tx
            .query_row("SELECT data FROM memory WHERE id=?", [&id], |r| {
                r.get::<_, String>(0)
            })
            .optional()?
        {
            return Ok(serde_json::from_str(&raw)?);
        }
        let entry = MemoryEntry {
            id,
            project_id: if global {
                None
            } else {
                Some(session.project_id)
            },
            kind: kind.into(),
            title,
            content,
            source_session: acceptance.session_id.clone(),
            author: if projected {
                producer.agent_id
            } else {
                policy.id.clone()
            },
            reviewer: if projected {
                acceptance.actor.clone()
            } else {
                None
            },
            status: if confirmed && projected {
                "active"
            } else {
                "proposed"
            }
            .into(),
            created_at: now(),
            supersedes: None,
            provenance: Some(KnowledgeProvenance {
                confirmation: if confirmed && projected {
                    ConfirmationStatus::Confirmed
                } else {
                    ConfirmationStatus::Unconfirmed
                },
                applicability: applicability.clone(),
                source: Some(KnowledgeSource {
                    acceptance_id: acceptance.id.clone(),
                    result_id: result.id.clone(),
                    result_version: result.version,
                    criteria_version: result.criteria_version.clone(),
                    confirmation_ids: acceptance.links.confirmation_ids.clone(),
                }),
                assignment_id: if projected {
                    Some(producer.id.clone())
                } else {
                    None
                },
                invocation_id: if projected {
                    records::<InvocationRecord>(&tx, "invocations", &acceptance.session_id)?
                        .into_iter()
                        .find(|i| i.assignment_id == producer.id)
                        .map(|i| i.id)
                } else {
                    None
                },
                policy: policy.clone(),
            }),
        };
        write(&tx, &entry)?;
        tx.execute("INSERT INTO events(session_id,kind,data,created_at) VALUES (?,'knowledge_retained',?,?)", params![acceptance.session_id, serde_json::to_string(&entry)?, entry.created_at])?;
        tx.commit()?;
        Ok(entry)
    }

    /// Inspect stable outcome locations, including after a later sibling fails.
    /// No provider, planning or artifact production is started.
    pub fn outcomes(&self, session_id: &str) -> Result<Vec<StoredOutcome>> {
        let db = self.db()?;
        let _: Session = record(&db, "sessions", session_id)?;
        records::<DecisionRecord>(&db, "decisions", session_id)?
            .into_iter()
            .filter(|d| d.kind == "task_accepted" && d.links.result.is_some())
            .map(|d| {
                let result = d.links.result.context("Accepted outcome has no result")?;
                let directory = confirmation::source_directory(&db, session_id, &result)?.with_context(|| format!("outcome_location_unknown: captured directory is missing for result {}:{} in session {session_id}", result.id, result.version))?;
                let current = confirmation::current_task(&db, &result)?
                    && confirmation::current_files(&db, session_id, &result)?;
                let confirmation = if current {
                    confirmation::grade(&db, session_id, &result)?.0
                } else {
                    ConfirmationStatus::Unconfirmed
                };
                Ok(StoredOutcome {
                    source_session: session_id.into(),
                    acceptance_id: d.id,
                    result_id: result.id,
                    result_version: result.version,
                    directory: directory.clone(),
                    summary: result.summary,
                    artifacts: result
                        .artifacts
                        .into_iter()
                        .map(|a| OutcomeArtifact {
                            path: directory.join(a.path),
                            sha256: a.sha256,
                        })
                        .collect(),
                    confirmation,
                    current,
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_active_memory_is_inspectable_but_cannot_be_used_as_supported_evidence() {
        let temp = tempfile::tempdir().unwrap();
        let store = Store::open(temp.path()).unwrap();
        let entry = MemoryEntry {
            provenance: None,
            id: "legacy".into(),
            project_id: None,
            kind: "procedure".into(),
            title: "Legacy procedure".into(),
            content: "An unsupported historic claim".into(),
            source_session: "historical-session".into(),
            author: "one".into(),
            reviewer: Some("two".into()),
            status: "active".into(),
            created_at: now(),
            supersedes: None,
        };
        assert!(
            store.save_memory(&entry).is_err(),
            "Normal saves cannot manufacture activation"
        );
        {
            // Historical storage predates evidence provenance. Preserve its bytes
            // and lifecycle value while the current consumer denies authority.
            let mut db = store.db().unwrap();
            let tx = db.transaction().unwrap();
            write(&tx, &entry).unwrap();
            tx.commit().unwrap();
        }
        assert!(store.memory(None, "legacy").unwrap().is_empty());
        let found = store
            .search_memory(
                None,
                "legacy",
                &Default::default(),
                KnowledgeRetrievalMode::IncludeUnconfirmed,
            )
            .unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].status, "active");
        assert!(found[0].provenance.is_none());
        store.forget_memory("legacy").unwrap();
        assert!(store
            .search_memory(
                None,
                "legacy",
                &Default::default(),
                KnowledgeRetrievalMode::IncludeUnconfirmed
            )
            .unwrap()
            .is_empty());
        assert_eq!(store.memory_inventory(None).unwrap()[0].status, "retired");
    }

    #[test]
    fn legacy_outcome_without_captured_directory_does_not_invent_a_location() {
        let temp = tempfile::tempdir().unwrap();
        let store = Store::open(&temp.path().join("state")).unwrap();
        let project = store.project(temp.path()).unwrap();
        let session = Session {
            id: new_id(),
            project_id: project.id,
            title: "Legacy outcome".into(),
            status: "completed".into(),
            created_at: now(),
            team: vec![],
            turns_used: 0,
        };
        store.save_session(&session).unwrap();
        std::fs::write(temp.path().join("greeting.txt"), b"Hello from ymp\n").unwrap();
        let result = ResultVersion {
            id: "legacy-result".into(),
            version: 1,
            task: None,
            summary: "A historical file was created".into(),
            task_definition: None,
            criteria: vec![],
            criteria_version: "legacy".into(),
            contract_id: None,
            producer_assignment_ids: vec![],
            artifacts: vec![FileSnapshot::capture(
                temp.path(),
                std::path::Path::new("greeting.txt"),
            )
            .unwrap()],
            component_ids: vec![],
        };
        let acceptance = DecisionRecord {
            id: new_id(),
            session_id: session.id.clone(),
            kind: "task_accepted".into(),
            actor: None,
            reason: "Historical acceptance without a directory capture".into(),
            outcome: Some(DecisionOutcome::Accepted {
                confirmation: ConfirmationStatus::Unknown,
            }),
            links: RecordLinks {
                result: Some(result),
                ..Default::default()
            },
            created_at: now(),
        };
        // Imported legacy data can lack provenance required by today's writers.
        store
            .db()
            .unwrap()
            .execute(
                "INSERT INTO decisions(id,session_id,data) VALUES (?,?,?)",
                params![
                    acceptance.id,
                    session.id,
                    serde_json::to_string(&acceptance).unwrap()
                ],
            )
            .unwrap();
        let error = store.outcomes(&session.id).unwrap_err();
        assert!(error.to_string().contains("outcome_location_unknown"));
        assert!(error.to_string().contains("legacy-result:1"));
        assert!(temp.path().join("greeting.txt").exists());
        assert!(store.trace(&session.id).unwrap().invocations.is_empty());
    }
}
