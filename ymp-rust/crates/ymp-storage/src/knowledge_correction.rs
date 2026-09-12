//! Explicit correction authority reuses the captured contract and decision journal.
use super::{knowledge, provenance::record, Store};
use anyhow::{ensure, Context, Result};
use rusqlite::{params, Connection, OptionalExtension};
use ymp_core::*;

fn reference(entry: &MemoryEntry) -> Result<KnowledgeRef> {
    Ok(KnowledgeRef {
        id: entry.id.clone(),
        version: content_digest(&serde_json::to_string(entry)?),
    })
}

fn target(
    db: &Connection,
    binding: &KnowledgeCorrectionBinding,
    project: &str,
) -> Result<MemoryEntry> {
    let entry: MemoryEntry = record(db, "memory", &binding.target.id)?;
    ensure!(
        reference(&entry)? == binding.target,
        "Correction target version changed"
    );
    ensure!(
        ["active", "proposed"].contains(&entry.status.as_str()),
        "Correction target is retired or superseded"
    );
    ensure!(
        entry.project_id.as_deref().is_none_or(|p| p == project),
        "Correction belongs to another project"
    );
    let applicability = entry
        .provenance
        .as_ref()
        .map(|p| p.applicability.clone())
        .unwrap_or_default();
    ensure!(
        applicability == binding.applicability,
        "Correction applicability differs from the original claim"
    );
    ensure!(
        entry.project_id.is_none() == (binding.projection == KnowledgeProjection::CheckProcedure),
        "Correction cannot change project/global applicability"
    );
    Ok(entry)
}

/// Called in the existing atomic session/contract capture before any invocation.
pub(super) fn validate_capture(
    db: &Connection,
    session_id: &str,
    captured: &CapturedAcceptanceContract,
) -> Result<()> {
    let Some(binding) = &captured.contract.knowledge_correction else {
        return Ok(());
    };
    let session: Session = record(db, "sessions", session_id)?;
    let old = target(db, binding, &session.project_id)?;
    if let Some(replacement) = &binding.source_replacement {
        let source = old
            .provenance
            .as_ref()
            .and_then(|p| p.source.as_ref())
            .context("Source replacement requires the original captured source")?;
        let acceptance: DecisionRecord = record(db, "decisions", &source.acceptance_id)?;
        let result = acceptance
            .links
            .result
            .context("Original source has no result")?;
        ensure!(
            acceptance.kind == "task_accepted"
                && acceptance.session_id == old.source_session
                && result.id == source.result_id
                && result.version == source.result_version
                && result.criteria_version == source.criteria_version
                && acceptance.links.confirmation_ids == source.confirmation_ids,
            "Original source binding mismatch"
        );
        let contract: DecisionRecord = record(
            db,
            "decisions",
            result
                .contract_id
                .as_deref()
                .context("Original source has no captured input contract")?,
        )?;
        let contract = contract
            .links
            .acceptance_contract
            .context("Original source contract missing")?;
        let previous = contract
            .inputs
            .iter()
            .find(|s| s.path == replacement.previous_input)
            .context("Previous input was not part of the original captured source")?;
        let next = captured
            .inputs
            .iter()
            .find(|s| s.path == replacement.replacement_input)
            .context("Replacement input was not captured")?;
        ensure!(
            previous.sha256.is_some() && next.sha256.is_some() && previous.sha256 != next.sha256,
            "Source replacement requires actually changed captured bytes"
        );
    }
    Ok(())
}

pub(super) fn validate_review_context(
    db: &Connection,
    result: &ResultVersion,
    assignment: &AssignmentRecord,
) -> Result<()> {
    let Some(id) = &result.contract_id else {
        return Ok(());
    };
    let contract: DecisionRecord = record(db, "decisions", id)?;
    let Some(binding) = contract
        .links
        .acceptance_contract
        .as_ref()
        .and_then(|c| c.contract.knowledge_correction.as_ref())
    else {
        return Ok(());
    };
    let digest = content_digest(&serde_json::to_string(binding)?);
    ensure!(
        assignment
            .context
            .iter()
            .any(|c| c.kind == ContextKind::KnowledgeCorrection
                && c.id == *id
                && c.session_id.as_deref() == Some(&assignment.session_id)
                && c.digest.as_ref() == Some(&digest)),
        "Independent review did not include this exact correction relation"
    );
    Ok(())
}

/// An explicitly captured source transition invalidates the old claim even when
/// the old file remains intact. Capture does not confirm the replacement value.
pub(super) fn source_changed(db: &Connection, entry: &MemoryEntry) -> Result<bool> {
    let mut query = db.prepare("SELECT data FROM decisions WHERE json_extract(data,'$.kind')='acceptance_contract_captured' AND json_extract(data,'$.links.acceptance_contract.contract.knowledge_correction.target.id')=?")?;
    let values = query
        .query_map([&entry.id], |r| r.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let reference = reference(entry)?;
    for value in values {
        let decision: DecisionRecord = serde_json::from_str(&value)?;
        if decision
            .links
            .acceptance_contract
            .and_then(|c| c.contract.knowledge_correction)
            .is_some_and(|b| b.target == reference && b.source_replacement.is_some())
        {
            return Ok(true);
        }
    }
    Ok(false)
}

impl Store {
    /// The exact predecessor captured with the contract, available to independent
    /// review even after its current-knowledge status changes. Never activation.
    pub fn correction_predecessor(&self, contract_id: &str) -> Result<MemoryEntry> {
        let db = self.db()?;
        let contract: DecisionRecord = record(&db, "decisions", contract_id)?;
        let binding = contract
            .links
            .acceptance_contract
            .as_ref()
            .and_then(|c| c.contract.knowledge_correction.as_ref())
            .context("Contract has no correction binding")?;
        let raw: String = db.query_row("SELECT data FROM events WHERE session_id=? AND kind='knowledge_correction_target' AND json_extract(data,'$.contract_id')=? ORDER BY seq LIMIT 1", params![contract.session_id, contract_id], |r| r.get(0))?;
        let value: serde_json::Value = serde_json::from_str(&raw)?;
        let predecessor: MemoryEntry = serde_json::from_value(value["entry"].clone())?;
        ensure!(
            reference(&predecessor)? == binding.target,
            "Captured correction predecessor version mismatch"
        );
        Ok(predecessor)
    }

    /// The replacement projection must already be retained from the accepted
    /// result. Only this transaction may attach its replacement link and retire
    /// the predecessor. Replays return the original receipt without new credit.
    pub fn commit_knowledge_correction(
        &self,
        proposal: &KnowledgeCorrectionProposal,
        policy: &KnowledgePolicyIdentity,
    ) -> Result<KnowledgeCorrectionOutcome> {
        ensure!(
            !policy.id.is_empty() && !policy.version.is_empty(),
            "Correction policy identity is required"
        );
        let mut db = self.db()?;
        let tx = db.transaction()?;
        let receipt_id = format!(
            "correction:{}",
            content_digest(&serde_json::to_string(proposal)?)
        );
        let previous: Option<String> = tx
            .query_row(
                "SELECT data FROM decisions WHERE id=?",
                [&receipt_id],
                |r| r.get(0),
            )
            .optional()?;
        if let Some(previous) = previous {
            let decision: DecisionRecord = serde_json::from_str(&previous)?;
            let correction = decision
                .links
                .knowledge_correction
                .context("Missing correction receipt")?;
            ensure!(
                correction.target == proposal.target
                    && correction.acceptance_id == proposal.acceptance_id,
                "Correction replay mismatch"
            );
            return Ok(KnowledgeCorrectionOutcome::AlreadyApplied { correction });
        }
        let acceptance: DecisionRecord = record(&tx, "decisions", &proposal.acceptance_id)?;
        ensure!(
            acceptance.kind == "task_accepted"
                && acceptance.outcome
                    == Some(DecisionOutcome::Accepted {
                        confirmation: ConfirmationStatus::Confirmed
                    }),
            "Correction requires confirmed independent acceptance"
        );
        let result = acceptance
            .links
            .result
            .as_ref()
            .context("Correction acceptance has no result")?;
        let contract_id = result
            .contract_id
            .as_ref()
            .context("Correction requires a trusted bound contract")?;
        let contract: DecisionRecord = record(&tx, "decisions", contract_id)?;
        let binding = contract
            .links
            .acceptance_contract
            .as_ref()
            .and_then(|c| c.contract.knowledge_correction.as_ref())
            .context("Confirmed result has no authority to correct this claim")?;
        ensure!(
            binding.target == proposal.target,
            "Correction proposal differs from captured authority"
        );
        let session: Session = record(&tx, "sessions", &acceptance.session_id)?;
        let mut old = target(&tx, binding, &session.project_id)?;
        let replacement_id = knowledge::entry_id(
            &acceptance.id,
            &binding.projection.proposal(),
            &binding.applicability,
        )?;
        ensure!(
            replacement_id != old.id,
            "Knowledge cannot supersede itself"
        );
        let mut replacement: MemoryEntry = record(&tx, "memory", &replacement_id)?;
        ensure!(
            replacement.status == "proposed",
            "Correction replacement is no longer pending"
        );
        // Evaluate evidence using the prospective lifecycle value; activation is
        // written only together with the predecessor and correction receipt.
        replacement.status = "active".into();
        ensure!(
            replacement.supersedes.is_none()
                && replacement.project_id == old.project_id
                && knowledge::applicable(
                    &tx,
                    &replacement,
                    Some(&session.project_id),
                    &binding.applicability,
                    KnowledgeRetrievalMode::Supported
                )?,
            "Replacement is stale, already linked or outside the original scope"
        );
        // An already superseded target cannot create cycles or acquire another
        // successor. Validate the stored chain as well, preserving legacy data.
        let mut ancestor = old.supersedes.clone();
        let mut seen = std::collections::HashSet::from([old.id.clone(), replacement.id.clone()]);
        while let Some(id) = ancestor {
            ensure!(seen.insert(id.clone()), "Knowledge supersession cycle");
            ancestor = record::<MemoryEntry>(&tx, "memory", &id)?.supersedes;
        }
        let review: DecisionRecord = record(
            &tx,
            "decisions",
            acceptance
                .links
                .review_ids
                .last()
                .context("Correction has no independent review")?,
        )?;
        ensure!(
            review.links.result.as_ref() == Some(result)
                && review.actor == acceptance.actor
                && matches!(review.outcome, Some(DecisionOutcome::Accepted { .. })),
            "Correction review binding mismatch"
        );
        let invocation: InvocationRecord = record(
            &tx,
            "invocations",
            review
                .links
                .invocation_id
                .as_deref()
                .context("Correction reviewer invocation missing")?,
        )?;
        let reviewer: AssignmentRecord = record(&tx, "assignments", &invocation.assignment_id)?;
        ensure!(
            invocation.state == InvocationState::Completed
                && review.actor.as_ref() == Some(&reviewer.agent_id),
            "Correction reviewer is not a completed bound invocation"
        );
        for id in &result.producer_assignment_ids {
            let producer: AssignmentRecord = record(&tx, "assignments", id)?;
            ensure!(
                producer.agent_id != reviewer.agent_id,
                "Correction cannot be self-reviewed"
            );
        }
        validate_review_context(&tx, result, &reviewer)?;
        let (grade, confirmations, failed) =
            super::confirmation::grade(&tx, &acceptance.session_id, result)?;
        ensure!(
            grade == ConfirmationStatus::Confirmed
                && !failed
                && confirmations == acceptance.links.confirmation_ids,
            "Correction evidence is stale or incomplete"
        );
        let checks = confirmations
            .iter()
            .map(|id| record::<DecisionRecord>(&tx, "decisions", id))
            .collect::<Result<Vec<_>>>()?;
        ensure!(
            binding
                .criterion_ids
                .iter()
                .all(|criterion| checks.iter().any(|d| d
                    .links
                    .check
                    .as_ref()
                    .is_some_and(|c| c.criterion_ids.contains(criterion)))),
            "Correction criteria lack applicable passing evidence"
        );
        let correction = KnowledgeCorrectionCommit {
            target: proposal.target.clone(),
            replacement_id: replacement.id.clone(),
            acceptance_id: acceptance.id.clone(),
            contract_id: contract_id.clone(),
            policy: policy.clone(),
        };
        old.status = "superseded".into();
        replacement.supersedes = Some(old.id.clone());
        knowledge::write(&tx, &old)?;
        knowledge::write(&tx, &replacement)?;
        super::provenance::decision(&tx, &DecisionRecord {
            id: receipt_id, session_id: acceptance.session_id.clone(), kind: "knowledge_superseded".into(), actor: None,
            reason: "Confirmed correction criteria and independent review authorize this exact replacement".into(), outcome: None,
            links: RecordLinks { knowledge_correction: Some(correction.clone()), result: Some(result.clone()), task: result.task.clone(), review_ids: acceptance.links.review_ids.clone(), confirmation_ids: confirmations, ..Default::default() }, created_at: now(),
        })?;
        tx.execute("INSERT INTO events(session_id,kind,data,created_at) VALUES (?,'knowledge_superseded',?,?)", params![acceptance.session_id, serde_json::to_string(&correction)?, now()])?;
        tx.commit()?;
        Ok(KnowledgeCorrectionOutcome::Applied { correction })
    }

    /// History with explicit current availability; never automatic prompt context.
    pub fn inspect_knowledge(
        &self,
        project: Option<&str>,
        scope: &std::collections::BTreeMap<String, String>,
    ) -> Result<Vec<KnowledgeInspection>> {
        validate_knowledge_scope(scope)?;
        let entries = self.memory_inventory(project)?;
        let db = self.db()?;
        let mut q = db.prepare(
            "SELECT data FROM decisions WHERE json_extract(data,'$.kind')='knowledge_superseded'",
        )?;
        let decisions = q
            .query_map([], |r| r.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let corrections = decisions
            .into_iter()
            .map(|s| serde_json::from_str::<DecisionRecord>(&s))
            .collect::<serde_json::Result<Vec<_>>>()?
            .into_iter()
            .filter_map(|d| d.links.knowledge_correction)
            .collect::<Vec<_>>();
        entries
            .into_iter()
            .map(|entry| {
                let correction = corrections
                    .iter()
                    .find(|c| c.target.id == entry.id)
                    .or_else(|| corrections.iter().find(|c| c.replacement_id == entry.id))
                    .cloned();
                let replaced_by = corrections
                    .iter()
                    .find(|c| c.target.id == entry.id)
                    .map(|c| c.replacement_id.clone());
                let availability =
                    if entry.status == "superseded" {
                        KnowledgeAvailability::Superseded
                    } else if entry.status == "rejected" {
                        KnowledgeAvailability::Rejected
                    } else if !["active", "proposed"].contains(&entry.status.as_str()) {
                        KnowledgeAvailability::Retired
                    } else if entry.provenance.as_ref().is_some_and(|p| {
                        !p.applicability.iter().all(|(k, v)| scope.get(k) == Some(v))
                    }) {
                        KnowledgeAvailability::ScopeMismatch
                    } else if source_changed(&db, &entry)? {
                        KnowledgeAvailability::SourceVersionChanged
                    } else if entry
                        .provenance
                        .as_ref()
                        .is_none_or(|p| p.confirmation != ConfirmationStatus::Confirmed)
                    {
                        KnowledgeAvailability::Unconfirmed
                    } else if entry.status == "proposed" {
                        KnowledgeAvailability::PendingCorrection
                    } else if knowledge::applicable(
                        &db,
                        &entry,
                        project,
                        scope,
                        KnowledgeRetrievalMode::Supported,
                    )? {
                        KnowledgeAvailability::Available
                    } else {
                        KnowledgeAvailability::SourceUnavailable
                    };
                Ok(KnowledgeInspection {
                    id: entry.id.clone(),
                    version: reference(&entry)?.version,
                    entry,
                    availability,
                    replaced_by,
                    correction,
                })
            })
            .collect()
    }
}
