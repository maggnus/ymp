//! Read-only snapshots of what a session recorded.
//!
//! The controller takes these at moments a reader can name: when the window opens, when a
//! session is loaded, when a run reports that it finished or stopped, and when a page that
//! presents them is opened. A page that presents a snapshot reads nothing else while
//! painting, neither the store nor the filesystem nor `PATH`. That is what keeps browsing
//! those pages a pure read, and it is also why each of them states when its records were
//! read: what it shows was true then.
//!
//! The claim is about these snapshots and not about every page. `/sessions`, `/diff`,
//! `/checks`, `/memory`, `/reputation` and `/files` read the store or the working directory
//! inside their own build, so a read time stated here does not describe them.
//!
//! Nothing here grades or infers. A field the records do not carry stays missing, a status
//! the runtime did not classify stays unknown, and an accepted result is never presented as
//! confirmed because it looks finished.

use std::collections::{BTreeMap, BTreeSet};
use ymp_core::{
    now, AgentPool, AssignmentRecord, BoardCommitment, BoardDecision, BoardProposal,
    BoardProposalStatus, BoardSnapshot, BoardTask, Config, ConfirmationStatus, DecisionOutcome,
    DecisionRecord, GrantRecord, InvocationRecord, InvocationState, PoolAgent, ResultVersion,
    SessionPolicy, SessionTrace, StoredOutcome, TeamConstraints, TeamState,
    WorkspaceAccessDecision, WorkspaceWait,
};
use ymp_providers::discovery::ProviderHealth;
use ymp_storage::Store;

/// Whether the files an accepted result named are still the ones it was accepted with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResultState {
    /// The inputs, artifacts and task binding the result named are unchanged.
    Current,
    /// Something the result was accepted against has changed since.
    Stale,
    /// The question could not be answered, which is not the same as an answer.
    Unread,
}

impl ResultState {
    pub fn word(self) -> &'static str {
        match self {
            ResultState::Current => "still current",
            ResultState::Stale => "superseded by later changes",
            ResultState::Unread => "not checked",
        }
    }
}

/// The records of one session, as they were at `read_at`.
#[derive(Default)]
pub struct Records {
    pub session: Option<String>,
    /// When the controller read them. Empty before the first read.
    pub read_at: String,
    pub trace: Option<SessionTrace>,
    /// Grants reached through the assignments that hold them; a session with no assignment
    /// has none to reach.
    pub grants: Vec<GrantRecord>,
    /// Keyed by the id of the decision that carries the result version.
    pub results: BTreeMap<String, ResultState>,
    /// Observations this session recorded competence credit for.
    pub credited: BTreeSet<String>,
    /// Accepted results with the directory each was recorded in. Built from the captured
    /// location, so it stays what the run recorded even after the project path changes.
    pub outcomes: Vec<StoredOutcome>,
    /// Why the recorded locations could not be listed, which is not the same as there
    /// being none: one unreadable location leaves the whole answer unavailable.
    pub outcomes_unreadable: Option<String>,
    /// Why there is nothing to show, when the reason is a failed read.
    pub unreadable: Option<String>,
    /// The shared plan as the store holds it: its version, every task's own version and
    /// commitment, and the proposals agents made against it. `None` where no session is open
    /// or the read failed, which is not the same as a session with no proposals.
    pub board: Option<BoardSnapshot>,
    pub board_unreadable: Option<String>,
}

impl Records {
    /// No session has been opened. Distinct from a session that recorded nothing.
    pub fn unopened() -> Self {
        Self::default()
    }

    /// Read everything one session wrote. Every call is a read; none of them writes.
    pub fn read(store: &Store, session: Option<&str>) -> Self {
        let read_at = now();
        let Some(id) = session else {
            return Self {
                read_at,
                ..Self::default()
            };
        };
        let trace = match store.trace(id) {
            Ok(trace) => trace,
            Err(error) => {
                return Self {
                    session: Some(id.to_owned()),
                    read_at,
                    unreadable: Some(format!("{error:#}")),
                    ..Self::default()
                };
            }
        };
        let mut grants = Vec::new();
        for assignment in &trace.assignments {
            for grant_id in &assignment.grant_ids {
                if let Ok(grant) = store.team_grant(id, grant_id) {
                    grants.push(grant);
                }
            }
        }
        let mut results = BTreeMap::new();
        for decision in &trace.decisions {
            if let Some(result) = &decision.links.result {
                // A digest comparison against the working directory, which is why it is
                // taken here and not while a page is being painted.
                let state = match store.result_is_current(id, result) {
                    Ok(true) => ResultState::Current,
                    Ok(false) => ResultState::Stale,
                    Err(_) => ResultState::Unread,
                };
                results.insert(decision.id.clone(), state);
            }
        }
        let credited = trace
            .decisions
            .iter()
            .filter(|decision| decision.kind == "reputation_observed")
            .filter_map(|decision| decision.links.observation_id.clone())
            .collect();
        // Locations are read here, with the rest: the call compares recorded digests against
        // the working directory, which is not something a page may do while painting.
        let (outcomes, outcomes_unreadable) = match store.outcomes(id) {
            Ok(outcomes) => (outcomes, None),
            Err(error) => (Vec::new(), Some(format!("{error:#}"))),
        };
        // The plan's own versions and the proposals against it are a read of the same session,
        // taken here so the page that presents them paints from a snapshot like every other.
        let (board, board_unreadable) = match store.board(id) {
            Ok(board) => (Some(board), None),
            Err(error) => (None, Some(format!("{error:#}"))),
        };
        Self {
            session: Some(id.to_owned()),
            read_at,
            trace: Some(trace),
            grants,
            results,
            credited,
            outcomes,
            outcomes_unreadable,
            unreadable: None,
            board,
            board_unreadable,
        }
    }

    /// The version of the plan the proposals were made against, where a board was read.
    ///
    /// A version is a digest of the plan's own definitions. It changes when a commitment
    /// changes the plan, which is why a proposal that names an older one cannot be committed.
    pub fn plan_version(&self) -> Option<&str> {
        self.board.as_ref().map(|board| board.plan_version.as_str())
    }

    /// What the plan holds for one task: its own version and the commitment on it.
    pub fn board_task(&self, task_id: &str) -> Option<&BoardTask> {
        self.board
            .as_ref()?
            .tasks
            .iter()
            .find(|entry| entry.task.id == task_id)
    }

    /// Every proposal the session recorded, in the order it recorded them.
    pub fn proposals(&self) -> &[BoardProposal] {
        self.board
            .as_ref()
            .map(|board| board.proposals.as_slice())
            .unwrap_or_default()
    }

    /// The decision that answered one proposal, where the runtime recorded one.
    ///
    /// The decision is the authority, not the proposal's stored status: the runtime writes the
    /// decision and the status in one transaction, and only the decision carries the reason.
    pub fn board_decision(&self, proposal: &str) -> Option<(&DecisionRecord, &BoardDecision)> {
        self.decisions().iter().find_map(|decision| {
            decision
                .links
                .board
                .as_deref()
                .filter(|board| board.proposal.id == proposal)
                .map(|board| (decision, board))
        })
    }

    /// The commitment one agent currently holds, where the plan holds one for it.
    pub fn commitment_of(&self, agent: &str) -> Option<(&BoardTask, &BoardCommitment)> {
        self.board.as_ref()?.tasks.iter().find_map(|entry| {
            entry
                .commitment
                .as_ref()
                .filter(|commitment| commitment.agent_id == agent)
                .map(|commitment| (entry, commitment))
        })
    }

    /// Turns the runtime put off rather than started, for one task.
    ///
    /// A deferral is a recorded wait and never a silent skip: the runtime writes why it left a
    /// task for the next work boundary, and `commitment_busy` is the one that means the agent
    /// responsible for it was already working.
    pub fn deferrals_for_task(&self, task_id: &str) -> Vec<(&DecisionRecord, &WorkspaceWait)> {
        self.waits_for_task(task_id)
            .into_iter()
            .filter(|(_, wait)| wait.code == "commitment_busy" || wait.code == "agent_busy")
            .collect()
    }

    /// Proposals nothing has answered yet, which is what a reader can still expect.
    ///
    /// Waiting means two things at once: the stored status is still pending, and no decision
    /// this session read answers it. A proposal whose status moved without a decision here is
    /// not waiting either, and its row says that what the runtime decided was not read.
    pub fn pending_proposals(&self) -> usize {
        self.proposals()
            .iter()
            .filter(|proposal| {
                proposal.status == BoardProposalStatus::Pending
                    && self.board_decision(&proposal.id).is_none()
            })
            .count()
    }

    /// The roster the runtime holds for this session: who a turn may be given to now.
    ///
    /// A session that recorded no roster has none, which is not the same as an empty one.
    /// Membership is appended to the captured team, so an identity the session captured can
    /// be absent from this list after a run replaced it.
    pub fn roster(&self) -> Option<&TeamState> {
        self.trace
            .as_ref()
            .and_then(|trace| trace.team_state.as_ref())
    }

    /// Whether the roster lists this agent: `None` where no roster was recorded at all.
    pub fn in_roster(&self, agent: &str) -> Option<bool> {
        self.roster()
            .map(|state| state.current_members.iter().any(|id| id == agent))
    }

    /// The bounds a roster was formed under, as the session captured them. A session that
    /// captured none would use the configuration of the run that resumes it.
    pub fn captured_constraints(&self) -> Option<&TeamConstraints> {
        self.policy()
            .and_then(|policy| policy.team_constraints.as_ref())
    }

    pub fn policy(&self) -> Option<&SessionPolicy> {
        self.trace.as_ref().and_then(|trace| trace.policy.as_ref())
    }

    /// What the run recorded about one reservation of the working directory.
    ///
    /// A turn's reservation carries the assignment's own id, which is how the two records
    /// meet. A session that recorded no reservation for an assignment has none, which is
    /// not the same as a turn that held nothing.
    pub fn reservation(&self, reservation_id: &str) -> Option<Reservation<'_>> {
        let of_kind = |kind: &'static str| {
            self.decisions().iter().find(|decision| {
                decision.kind == kind
                    && decision
                        .links
                        .workspace_access
                        .as_ref()
                        .is_some_and(|access| access.reservation_id == reservation_id)
            })
        };
        let acquired = of_kind("workspace_access_acquired")?;
        Some(Reservation {
            access: acquired.links.workspace_access.as_ref()?,
            acquired,
            admitted: of_kind("workspace_access_admitted"),
            released: of_kind("workspace_access_released"),
        })
    }

    /// Every wait the run recorded, in the order it recorded them.
    pub fn waits(&self) -> Vec<(&DecisionRecord, &WorkspaceWait)> {
        self.decisions()
            .iter()
            .filter_map(|decision| {
                decision
                    .links
                    .workspace_wait
                    .as_ref()
                    .map(|wait| (decision, wait))
            })
            .collect()
    }

    /// Waits recorded for one agent, which is what a turn waited through before it ran.
    pub fn waits_of(&self, agent: &str) -> Vec<(&DecisionRecord, &WorkspaceWait)> {
        self.waits()
            .into_iter()
            .filter(|(decision, _)| decision.actor.as_deref() == Some(agent))
            .collect()
    }

    /// Waits recorded against one task, including the ones no agent was named for.
    pub fn waits_for_task(&self, task_id: &str) -> Vec<(&DecisionRecord, &WorkspaceWait)> {
        self.waits()
            .into_iter()
            .filter(|(decision, _)| {
                decision
                    .links
                    .task
                    .as_ref()
                    .is_some_and(|task| task.task_id == task_id)
            })
            .collect()
    }

    pub fn assignments(&self) -> &[AssignmentRecord] {
        self.trace
            .as_ref()
            .map(|trace| trace.assignments.as_slice())
            .unwrap_or_default()
    }

    pub fn invocations(&self) -> &[InvocationRecord] {
        self.trace
            .as_ref()
            .map(|trace| trace.invocations.as_slice())
            .unwrap_or_default()
    }

    pub fn decisions(&self) -> &[DecisionRecord] {
        self.trace
            .as_ref()
            .map(|trace| trace.decisions.as_slice())
            .unwrap_or_default()
    }

    pub fn decision(&self, id: &str) -> Option<&DecisionRecord> {
        self.decisions().iter().find(|d| d.id == id)
    }

    /// The invocations one assignment produced, oldest first. A retried assignment has
    /// more than one, and the latest is the one a state word describes.
    pub fn invocations_of(&self, assignment: &str) -> Vec<&InvocationRecord> {
        self.invocations()
            .iter()
            .filter(|invocation| invocation.assignment_id == assignment)
            .collect()
    }

    pub fn last_invocation(&self, assignment: &str) -> Option<&InvocationRecord> {
        self.invocations_of(assignment).into_iter().next_back()
    }

    /// Assignments whose latest invocation is still running, by the record alone.
    pub fn open(&self) -> Vec<&AssignmentRecord> {
        self.assignments()
            .iter()
            .filter(|assignment| {
                self.last_invocation(&assignment.id)
                    .is_some_and(|invocation| invocation.state == InvocationState::Running)
            })
            .collect()
    }

    /// Every agent id an assignment names, in the order the records first name them.
    pub fn agents(&self) -> Vec<String> {
        let mut seen = Vec::new();
        for assignment in self.assignments() {
            if !seen.contains(&assignment.agent_id) {
                seen.push(assignment.agent_id.clone());
            }
        }
        seen
    }

    /// Grants an assignment holds, whether or not they are still in force.
    pub fn grants_of(&self, assignment: &str) -> Vec<&GrantRecord> {
        self.grants
            .iter()
            .filter(|grant| grant.assignment_id == assignment)
            .collect()
    }

    /// Grants that are in force for an agent right now: issued by an assignment whose
    /// invocation is still running, and not revoked.
    pub fn live_grants(&self, agent: &str) -> Vec<&GrantRecord> {
        self.open()
            .into_iter()
            .filter(|assignment| assignment.agent_id == agent)
            .flat_map(|assignment| self.grants_of(&assignment.id))
            .filter(|grant| grant.revoked_at.is_none())
            .collect()
    }

    pub fn result_state(&self, decision: &str) -> ResultState {
        self.results
            .get(decision)
            .copied()
            .unwrap_or(ResultState::Unread)
    }

    /// The acceptance of one task, if a decision recorded one.
    ///
    /// The latest matching decision wins, because acceptance is appended: a reopened task
    /// accepted again has two, and the current state is the later one.
    pub fn acceptance(&self, task_id: &str) -> Option<Acceptance<'_>> {
        self.decisions()
            .iter()
            .filter(|decision| decision.kind == "task_accepted")
            .filter(|decision| {
                decision
                    .links
                    .task
                    .as_ref()
                    .is_some_and(|task| task.task_id == task_id)
            })
            .next_back()
            .map(|decision| self.describe(decision))
    }

    /// What one acceptance or rejection decision says, without interpreting it.
    pub fn describe<'a>(&'a self, decision: &'a DecisionRecord) -> Acceptance<'a> {
        let (accepted, confirmation) = match decision.outcome {
            Some(DecisionOutcome::Accepted { confirmation }) => (Some(true), Some(confirmation)),
            Some(DecisionOutcome::Rejected) => (Some(false), None),
            None => (None, None),
        };
        Acceptance {
            decision,
            accepted,
            confirmation,
            result: decision.links.result.as_ref(),
            evidence: decision.links.confirmation_ids.len(),
            state: self.result_state(&decision.id),
            reviewers: self.reviewers(decision),
        }
    }

    /// Who independently reviewed the result this decision accepted. The acceptance links
    /// the review decisions; each review names its own actor.
    fn reviewers(&self, decision: &DecisionRecord) -> Vec<String> {
        decision
            .links
            .review_ids
            .iter()
            .filter_map(|id| self.decision(id))
            .filter_map(|review| review.actor.clone())
            .collect()
    }
}

/// One acceptance or rejection, with the basis the record carries for it.
pub struct Acceptance<'a> {
    pub decision: &'a DecisionRecord,
    /// `None` where the runtime recorded the decision without classifying its outcome.
    pub accepted: Option<bool>,
    /// Only an accepted decision carries a confirmation grade.
    pub confirmation: Option<ConfirmationStatus>,
    pub result: Option<&'a ResultVersion>,
    /// How many pieces of applicable passing check evidence the acceptance bound.
    pub evidence: usize,
    pub state: ResultState,
    pub reviewers: Vec<String>,
}

impl Acceptance<'_> {
    /// The word for the acceptance state, which is never "confirmed" unless the record
    /// says so. An unclassified outcome stays unclassified.
    pub fn word(&self) -> &'static str {
        match (self.accepted, self.confirmation) {
            (Some(true), Some(ConfirmationStatus::Confirmed)) => "accepted, confirmed",
            (Some(true), Some(ConfirmationStatus::Unconfirmed)) => "accepted, unconfirmed",
            (Some(true), _) => "accepted, confirmation unknown",
            (Some(false), _) => "rejected",
            (None, _) => "recorded without an outcome",
        }
    }

    pub fn confirmed(&self) -> bool {
        self.confirmation == Some(ConfirmationStatus::Confirmed)
    }
}

/// One recorded reservation of the working directory, as the run wrote it.
pub struct Reservation<'a> {
    pub access: &'a WorkspaceAccessDecision,
    pub acquired: &'a DecisionRecord,
    /// The record that ties the reservation to the turn that actually ran under it.
    pub admitted: Option<&'a DecisionRecord>,
    /// A reservation with no release record was still held when the records were read.
    pub released: Option<&'a DecisionRecord>,
}

/// The locally eligible pool, as the controller last inspected it.
///
/// Inspection reads the configuration and looks for the provider executables on `PATH`. It
/// launches nothing, asks no provider anything and checks no credential, so the answer is
/// about this machine's installation and never about whether an account can run a model.
#[derive(Default)]
pub struct Pool {
    pub read_at: String,
    pub pool: Option<AgentPool>,
    /// One entry per configured provider: whether its program was found, and where.
    pub health: Vec<ProviderHealth>,
    pub unreadable: Option<String>,
}

impl Pool {
    pub fn read(config: &Config) -> Self {
        let read_at = now();
        let health = ymp_providers::discovery::inspect(config);
        match ymp_providers::discovery::inspect_pool(config) {
            Ok(pool) => Self {
                read_at,
                pool: Some(pool),
                health,
                unreadable: None,
            },
            Err(error) => Self {
                read_at,
                pool: None,
                health,
                unreadable: Some(format!("{error:#}")),
            },
        }
    }

    pub fn agents(&self) -> &[PoolAgent] {
        self.pool
            .as_ref()
            .map(|pool| pool.agents.as_slice())
            .unwrap_or_default()
    }

    pub fn agent(&self, id: &str) -> Option<&PoolAgent> {
        self.agents().iter().find(|agent| agent.profile.id == id)
    }

    pub fn provider(&self, id: &str) -> Option<&ProviderHealth> {
        self.health.iter().find(|health| health.id == id)
    }
}

impl Records {
    /// Whether this session recorded competence credit for a task, and for whom.
    ///
    /// Credit is a record of its own, written only where confirmed evidence supported a
    /// single producer. Its absence is not a judgement about the work.
    pub fn credit_for(&self, task_id: &str) -> Option<&str> {
        self.decisions()
            .iter()
            .filter(|decision| decision.kind == "reputation_observed")
            .find(|decision| {
                decision
                    .links
                    .task
                    .as_ref()
                    .is_some_and(|task| task.task_id == task_id)
            })
            .and_then(|decision| decision.actor.as_deref())
    }
}
