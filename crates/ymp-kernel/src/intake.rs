//! Trusted operations for explicit user intake and attributable refinements.

use crate::{
    decision::{DecisionConsumer, SessionControl},
    events::{CriteriaCommitted, Event},
    journal::{Journal, validate_append},
};
use std::sync::Arc;
use ymp_domain::{
    Denial, Id, Result,
    journal::{Actor, Envelope, PolicySelection},
    require_text,
    task::{
        AcceptanceContract, Assumption, Clarification, Constraints, Criterion, CriterionOrigin,
        SessionStatus, Task,
    },
};

pub struct IntakeRequest {
    pub task: Task,
    pub criteria: Vec<Criterion>,
}

pub enum IntakeNote {
    Clarification(Clarification),
    Assumption(Assumption),
}

pub struct IntakeRefinement {
    pub expected_revision: u64,
    pub at: u64,
    pub constraints: Constraints,
    pub criteria: Vec<Criterion>,
    pub reason: String,
    pub note: IntakeNote,
}

pub struct Intake<J: Journal> {
    journal: Arc<J>,
    decisions: DecisionConsumer<J>,
}
impl<J: Journal> Intake<J> {
    pub fn acceptance<C: crate::journal::ContentStore>(
        &self,
        content: Arc<C>,
    ) -> crate::acceptance::AcceptanceAuthority<J, C> {
        crate::acceptance::AcceptanceAuthority::new(
            self.journal.clone(),
            content,
            self.decisions.shared(),
        )
    }
    pub fn new(journal: Arc<J>) -> Self {
        Self {
            decisions: DecisionConsumer::new(journal.clone()),
            journal,
        }
    }

    pub fn open(
        &self,
        session: Id,
        at: u64,
        request: IntakeRequest,
        selections: Vec<PolicySelection>,
    ) -> Result<SessionControl> {
        let contract = AcceptanceContract::new(&request.task, &request.criteria, vec![])?;
        let data = CriteriaCommitted {
            task: request.task,
            contract,
            criteria: request.criteria,
            previous: None,
            reason: "Explicit user task".into(),
        };
        validate_commit(None, None, &data)?;
        let events = [
            Envelope {
                seq: 1,
                session: session.clone(),
                at,
                actor: Actor::Runtime,
                policy: None,
                input: None,
                refs: vec![],
                payload: Event::SessionOpened {
                    version: 1,
                    selections,
                },
            },
            Envelope {
                seq: 2,
                session: session.clone(),
                at,
                actor: Actor::Runtime,
                policy: None,
                input: None,
                refs: vec![],
                payload: Event::CriteriaCommitted {
                    version: 1,
                    data: Box::new(data),
                },
            },
        ];
        self.decisions.open_events(session, &events)
    }

    pub fn refine(&self, control: &SessionControl, refinement: IntakeRefinement) -> Result<u64> {
        let session = self.decisions.authorize(control)?;
        let current = self.journal.read(&session)?;
        let view = current.view_with_schemas(&session, None, self.journal.schemas())?;
        if view.revision() != refinement.expected_revision {
            return Err(Denial::new(
                "stale_revision",
                "Intake changed since the refinement was prepared",
            ));
        }
        if view.status() != Some(SessionStatus::Intake) {
            return Err(Denial::new(
                "intake_state",
                "Only an intake task can be refined here",
            ));
        }
        let mut task = view
            .task()
            .cloned()
            .ok_or_else(|| Denial::new("task_missing", "No task is open"))?;
        let previous = view
            .contract()
            .ok_or_else(|| Denial::new("contract_missing", "No acceptance contract is open"))?
            .reference();
        let note = match refinement.note {
            IntakeNote::Clarification(clarification) => {
                clarification.validate()?;
                task.goal.clarifications.push(clarification.clone());
                Event::ClarificationRecorded {
                    version: 1,
                    clarification,
                }
            }
            IntakeNote::Assumption(assumption) => {
                assumption.validate()?;
                task.goal.assumptions.push(assumption.clone());
                Event::AssumptionRecorded {
                    version: 1,
                    assumption,
                }
            }
        };
        task.constraints = refinement.constraints;
        let checks = view
            .contract()
            .expect("intake contract")
            .checks
            .iter()
            .filter(|id| {
                view.checks()
                    .values()
                    .find(|check| check.id.erased() == **id)
                    .is_some_and(|check| {
                        refinement.criteria.iter().any(|criterion| {
                            criterion.id == check.criterion
                                && criterion.reference().is_ok_and(|reference| {
                                    reference.version == check.criterion_version
                                })
                        })
                    })
            })
            .cloned()
            .collect();
        let contract = AcceptanceContract::new(&task, &refinement.criteria, checks)?;
        let data = CriteriaCommitted {
            task,
            contract,
            criteria: refinement.criteria,
            previous: Some(previous.clone()),
            reason: refinement.reason,
        };
        let first = refinement
            .expected_revision
            .checked_add(1)
            .ok_or_else(|| Denial::new("revision_overflow", "Journal sequence exhausted"))?;
        let second = first
            .checked_add(1)
            .ok_or_else(|| Denial::new("revision_overflow", "Journal sequence exhausted"))?;
        let events = [
            Envelope {
                seq: first,
                session: session.clone(),
                at: refinement.at,
                actor: Actor::Runtime,
                policy: None,
                input: None,
                refs: vec![previous.clone()],
                payload: note,
            },
            Envelope {
                seq: second,
                session: session.clone(),
                at: refinement.at,
                actor: Actor::Runtime,
                policy: None,
                input: None,
                refs: vec![previous],
                payload: Event::CriteriaCommitted {
                    version: 1,
                    data: Box::new(data),
                },
            },
        ];
        let validated = validate_append(
            &current,
            &session,
            refinement.expected_revision,
            &events,
            self.journal.schemas(),
        )?;
        let committed = self
            .journal
            .append(&session, refinement.expected_revision, &events)?;
        if committed != validated.revision() {
            return Err(Denial::new(
                "journal_append",
                "Journal returned an unexpected committed revision",
            ));
        }
        Ok(committed)
    }
}

/// Shared by application preflight and event replay. User input cannot claim an admitted Planner.
pub fn validate_commit(
    prior_task: Option<&Task>,
    prior_contract: Option<&AcceptanceContract>,
    data: &CriteriaCommitted,
) -> Result<()> {
    data.contract.validate(&data.task, &data.criteria)?;
    require_text(&data.reason, 16_384)?;
    if data
        .criteria
        .iter()
        .any(|c| !matches!(c.origin, CriterionOrigin::User))
    {
        return Err(Denial::new(
            "criterion_origin",
            "Explicit user intake cannot claim Derived assignment provenance",
        ));
    }
    if data
        .contract
        .checks
        .iter()
        .any(|id| prior_contract.is_none_or(|prior| !prior.checks.contains(id)))
    {
        return Err(Denial::new(
            "check_ref",
            "Checks must be registered by their owning kernel operation",
        ));
    }
    match (prior_task, prior_contract) {
        (None, None) if data.previous.is_none() => Ok(()),
        (Some(task), Some(contract)) => {
            if data.previous.as_ref() != Some(&contract.reference())
                || data.task.id != task.id
                || data.task.contract != task.contract
            {
                return Err(Denial::new(
                    "prior_contract",
                    "An intake revision must preserve task identity and name the current contract version",
                ));
            }
            if data.task.goal != task.goal {
                return Err(Denial::new(
                    "goal_change",
                    "Preserve the original request and record clarification or assumption before revising criteria",
                ));
            }
            if data.contract.version == contract.version {
                return Err(Denial::new(
                    "contract_version",
                    "An intake revision must change its content version",
                ));
            }
            Ok(())
        }
        _ => Err(Denial::new(
            "prior_contract",
            "Initial intake cannot name a previous contract",
        )),
    }
}
