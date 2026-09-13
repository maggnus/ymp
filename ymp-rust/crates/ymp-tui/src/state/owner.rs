//! Owner control of a loaded session from the team page.
//!
//! A command is built from the read model on display when the owner chooses it, with that
//! model's revisions, and is sent unchanged. When the backend refuses it, the view is read again
//! and the owner chooses again: nothing is reinterpreted against a newer model on its own. The
//! filters here only decide what is offered; the owner API validates every command itself.

use super::*;
use crate::control::{Outcome, Read, Reply, Request, Snapshot};
use crate::control_page::{self, Standing};
use ymp_core::{
    new_id, ContinueWithCurrentFilesCommand, CurrentFilesContext, FreshPlanReviewCommand,
    OwnerRunControl, OwnerTeamAction, OwnerTeamActionKind, OwnerTeamCommand, OwnerTeamReceipt,
    PlanVersion, RecoveryControl, RecoveryControlCommand, RecoveryControlKind,
    RecoveryInspectionCommand, RecoveryStage, RecoveryStatus, RecoveryWaitReason,
    TerminationEvidence,
};

/// One entry of an owner chooser.
#[derive(Clone, Debug)]
pub struct ChoiceOption {
    pub label: String,
    /// What choosing it does.
    pub detail: String,
    /// Why it cannot be chosen now, when it cannot.
    pub reason: Option<String>,
    pub pick: Pick,
}

/// What an owner chooser entry does. Each one carries the revision it was offered under, so the
/// command it becomes is bound to what the owner saw.
#[derive(Clone, Debug)]
pub enum Pick {
    Review {
        stage: String,
        revision: u64,
        proposal: Box<PlanVersion>,
    },
    Reviewer {
        stage: String,
        revision: u64,
        proposal: Box<PlanVersion>,
        reviewer: String,
    },
    Inspect {
        stage: String,
        revision: u64,
    },
    CurrentFiles {
        stage: String,
    },
    Control {
        stage: String,
        revision: u64,
        kind: RecoveryControlKind,
    },
    Team {
        revision: u64,
        action: OwnerTeamAction,
    },
    Replace {
        agent: String,
        revision: u64,
    },
    SessionWait {
        revision: u64,
    },
}

fn option(
    label: impl Into<String>,
    detail: impl Into<String>,
    reason: Option<String>,
    pick: Pick,
) -> ChoiceOption {
    ChoiceOption {
        label: label.into(),
        detail: detail.into(),
        reason,
        pick,
    }
}

fn team_kind(action: &OwnerTeamAction) -> OwnerTeamActionKind {
    match action {
        OwnerTeamAction::Add { .. } => OwnerTeamActionKind::Add,
        OwnerTeamAction::Remove { .. } => OwnerTeamActionKind::Remove,
        OwnerTeamAction::Replace { .. } => OwnerTeamActionKind::Replace,
        OwnerTeamAction::Pause => OwnerTeamActionKind::Pause,
        OwnerTeamAction::Continue => OwnerTeamActionKind::Continue,
        OwnerTeamAction::Wait { .. } => OwnerTeamActionKind::Wait,
    }
}

/// Why the session's typed action list leaves an action out.
fn not_permitted(kind: OwnerTeamActionKind) -> &'static str {
    match kind {
        OwnerTeamActionKind::Add => {
            "This session's team constraints do not permit another member now: its size is fixed or at its limit."
        }
        OwnerTeamActionKind::Remove => {
            "This session's team constraints do not permit removing a member now."
        }
        OwnerTeamActionKind::Replace => "No eligible agent outside the team can replace a member.",
        OwnerTeamActionKind::Pause => "The session is already held.",
        OwnerTeamActionKind::Continue => "The session is not held.",
        OwnerTeamActionKind::Wait => "The session cannot wait now.",
    }
}

impl App {
    /// Why no run may start now: a branch switch, or owner work that holds the directory.
    pub fn run_refusal(&self) -> Option<String> {
        if let Some(branch) = self.git.switching() {
            return Some(format!(
                "A switch to {} is in progress. Runs can start once it has finished.",
                text::sanitize(branch)
            ));
        }
        let pending = self.control.pending().filter(|p| p.holds_workspace())?;
        Some(format!(
            "{} for session {} is using its directory. Runs can start once it has finished.",
            pending.activity(),
            text::short_id(pending.session())
        ))
    }

    /// Why an owner command cannot be admitted now. `workspace` is set for work that reads or
    /// holds the session's directory, which no run or branch switch may share.
    fn owner_blocker(&self, workspace: bool) -> Option<String> {
        if let Some(pending) = self.control.pending() {
            return Some(format!("{} is still in progress.", pending.activity()));
        }
        if !workspace {
            return None;
        }
        if self.active {
            return Some("A run is active in this window. Stop it with /stop first.".into());
        }
        self.git.switching().map(|branch| {
            format!(
                "A switch to {} is in progress. Try again once it has finished.",
                text::sanitize(branch)
            )
        })
    }

    /// The model of the displayed session, or why there is none to act on.
    fn shown_snapshot(&self) -> Result<&Snapshot, String> {
        if let Some(snapshot) = self.control.ready() {
            return Ok(snapshot);
        }
        Err(match &self.control.read {
            Read::NotInitialized => "This session has no initialized team, so its membership and recovery cannot be changed. Starting preferences are not edited for a loaded session.".into(),
            Read::Failed(error) => format!("The session team could not be read: {error}"),
            _ => "The session team is still being read. Try again in a moment.".into(),
        })
    }

    fn open_choice(&mut self, title: String, badge: String, options: Vec<ChoiceOption>) {
        let selected = options
            .iter()
            .position(|option| option.reason.is_none())
            .unwrap_or(0);
        self.overlay = Some(Overlay::Choose {
            title,
            badge,
            options,
            selected,
        });
    }

    /// Enter and Space on the team page of a loaded session.
    pub(super) fn team_session_key(
        &mut self,
        item: &views::Item,
        code: KeyCode,
        width: u16,
    ) -> Vec<Action> {
        let key = item.key.as_str();
        match code {
            KeyCode::Enter if key == control_page::SESSION_KEY => self.open_session_actions(),
            KeyCode::Enter if key == control_page::RETRY_KEY => self.repeat_command(),
            KeyCode::Enter if key == control_page::CONTINUE_KEY => {
                return self.start_continuation()
            }
            KeyCode::Enter => {
                if let Some(stage) = key.strip_prefix(control_page::STAGE_PREFIX) {
                    self.open_stage_actions(stage);
                } else if let Some(agent) = key.strip_prefix(control_page::MEMBER_PREFIX) {
                    self.open_member_actions(agent);
                } else {
                    self.inspect_selected_row(width);
                }
            }
            KeyCode::Char(' ') => {
                let agent = key.strip_prefix(control_page::MEMBER_PREFIX).unwrap_or(key);
                self.toggle_session_member(agent);
            }
            _ => {}
        }
        Vec::new()
    }

    /// Space on an agent row: add it to the session team, or remove it, after a confirmation.
    fn toggle_session_member(&mut self, agent: &str) {
        let decided = match self.shown_snapshot() {
            Err(reason) => Err(reason),
            Ok(snapshot) => {
                let view = &snapshot.view;
                let member = view.effective.current_members.iter().any(|id| id == agent)
                    || view.desired_members.iter().any(|id| id == agent);
                let known = member
                    || view
                        .eligible_candidates
                        .iter()
                        .any(|candidate| candidate.profile.id == agent)
                    || self.config.agents.iter().any(|profile| profile.id == agent)
                    || self.pool.agent(agent).is_some();
                // Rows that name no agent, such as the roster rules, have nothing to toggle.
                if !known {
                    return;
                }
                let agent_id = agent.to_owned();
                Ok((
                    view.revision,
                    if member {
                        OwnerTeamAction::Remove { agent_id }
                    } else {
                        OwnerTeamAction::Add { agent_id }
                    },
                ))
            }
        };
        match decided {
            Ok((revision, action)) => self.propose_team(action, revision, false),
            Err(reason) => self.fail(reason),
        }
    }

    /// `/team add ID` or `/team remove ID` with a session loaded.
    pub(super) fn team_command_typed(&mut self, action: OwnerTeamAction) {
        let Some(session) = self.session.clone() else {
            return;
        };
        if let Some(revision) = self.control.ready().map(|s| s.view.revision) {
            return self.propose_team(action, revision, true);
        }
        match &self.control.read {
            Read::NotInitialized | Read::Failed(_) => {
                if let Err(reason) = self.shown_snapshot() {
                    self.fail(reason);
                }
            }
            _ => {
                self.control.deferred = Some((session, action));
                self.notice("Reading the session team. The change is sent once it has been read.");
            }
        }
    }

    /// Build a team command at `revision`. A membership change is confirmed first, except a
    /// typed one that does not amend a fixed roster, since typing it was the explicit request.
    fn propose_team(&mut self, action: OwnerTeamAction, revision: u64, typed: bool) {
        let prepared = self.shown_snapshot().and_then(|snapshot| {
            let view = &snapshot.view;
            let kind = team_kind(&action);
            if !view.permitted_actions.contains(&kind) {
                return Err(not_permitted(kind).to_owned());
            }
            if let OwnerTeamAction::Remove { agent_id }
            | OwnerTeamAction::Replace { agent_id, .. } = &action
            {
                if view
                    .pending_departures
                    .iter()
                    .any(|d| &d.agent_id == agent_id)
                {
                    return Err(format!("{agent_id} is already leaving this session."));
                }
            }
            let membership = matches!(
                kind,
                OwnerTeamActionKind::Add
                    | OwnerTeamActionKind::Remove
                    | OwnerTeamActionKind::Replace
            );
            let busy = match &action {
                OwnerTeamAction::Remove { agent_id }
                | OwnerTeamAction::Replace { agent_id, .. } => view
                    .responsibilities
                    .iter()
                    .any(|r| &r.agent_id == agent_id),
                _ => false,
            };
            let pinned = membership && view.constraints.fixed_roster.is_some();
            let command = OwnerTeamCommand {
                session_id: view.session_id.clone(),
                expected_revision: revision,
                command_id: new_id(),
                revise_pinned_roster: pinned,
                action: action.clone(),
            };
            Ok((command, busy, membership && (!typed || pinned)))
        });
        let (command, busy, confirm) = match prepared {
            Ok(prepared) => prepared,
            Err(reason) => return self.fail(reason),
        };
        if let Some(reason) = self.owner_blocker(false) {
            return self.fail(reason);
        }
        if confirm {
            self.overlay = Some(Overlay::Confirm {
                question: team_question(&command, busy),
                target: Confirm::Owner {
                    request: Box::new(Request::Team(command)),
                    badge: "changes the session",
                },
            });
        } else {
            self.submit_owner(Request::Team(command));
        }
    }

    /// Queue one exact owner command.
    pub(super) fn submit_owner(&mut self, request: Request) {
        let workspace = request.holds_workspace() || matches!(request, Request::Context { .. });
        if let Some(reason) = self.owner_blocker(workspace) {
            return self.fail(reason);
        }
        let activity = request.activity();
        match self.control.submit(request) {
            Ok(()) => {
                self.status = activity.into();
                self.invalidate_page();
                self.dirty = true;
            }
            Err(reason) => self.fail(reason),
        }
    }

    fn open_session_actions(&mut self) {
        let built = self.shown_snapshot().map(|snapshot| {
            let view = &snapshot.view;
            let revision = view.revision;
            let busy = self.owner_blocker(false);
            let mut options = Vec::new();
            if view.permitted_actions.contains(&OwnerTeamActionKind::Pause) {
                options.push(option(
                    "Pause session",
                    "Current work drains and no new work starts until you continue.",
                    busy.clone(),
                    Pick::Team {
                        revision,
                        action: OwnerTeamAction::Pause,
                    },
                ));
            }
            if view
                .permitted_actions
                .contains(&OwnerTeamActionKind::Continue)
            {
                options.push(option(
                    "Continue session",
                    "Releases the session hold. No run starts; /resume starts it.",
                    busy.clone(),
                    Pick::Team {
                        revision,
                        action: OwnerTeamAction::Continue,
                    },
                ));
            }
            if view.permitted_actions.contains(&OwnerTeamActionKind::Wait) {
                options.push(option(
                    "Wait for a condition",
                    "Holds the session until you continue it, recording the condition you name.",
                    busy,
                    Pick::SessionWait { revision },
                ));
            }
            (
                format!("Session {}", text::short_id(&view.session_id)),
                options,
            )
        });
        match built {
            Ok((title, options)) => self.open_choice(title, "session hold".into(), options),
            Err(reason) => self.fail(reason),
        }
    }

    fn open_member_actions(&mut self, agent: &str) {
        let built = self.shown_snapshot().map(|snapshot| {
            let view = &snapshot.view;
            let revision = view.revision;
            let standing = control_page::team_rows(snapshot)
                .into_iter()
                .find(|(id, _)| id == agent)
                .map(|(_, standing)| standing);
            let busy = self.owner_blocker(false);
            let working = view.responsibilities.iter().any(|r| r.agent_id == agent);
            let permitted = |kind| {
                (!view.permitted_actions.contains(&kind)).then(|| not_permitted(kind).to_owned())
            };
            let agent_id = agent.to_owned();
            let mut options = Vec::new();
            match standing {
                Some(Standing::Removed) | None => options.push(option(
                    "Add to the session",
                    "Selects it for this session again. Starting preferences are unchanged.",
                    permitted(OwnerTeamActionKind::Add).or(busy),
                    Pick::Team {
                        revision,
                        action: OwnerTeamAction::Add { agent_id },
                    },
                )),
                Some(standing) => {
                    let leaving = (standing == Standing::Leaving)
                        .then(|| "Already leaving this session.".to_owned());
                    let outside = view.eligible_candidates.iter().any(|candidate| {
                        !view
                            .effective
                            .current_members
                            .contains(&candidate.profile.id)
                            && !view.desired_members.contains(&candidate.profile.id)
                    });
                    let drains = if working {
                        " It keeps its current work until that ends and is given no new work."
                    } else {
                        ""
                    };
                    options.push(option(
                        "Replace",
                        format!("Choose an eligible agent to take its place.{drains}"),
                        leaving
                            .clone()
                            .or_else(|| permitted(OwnerTeamActionKind::Replace))
                            .or_else(|| {
                                (!outside)
                                    .then(|| not_permitted(OwnerTeamActionKind::Replace).to_owned())
                            })
                            .or(busy.clone()),
                        Pick::Replace {
                            agent: agent_id.clone(),
                            revision,
                        },
                    ));
                    options.push(option(
                        "Remove from the session",
                        format!("Removes it from this session only.{drains}"),
                        leaving
                            .or_else(|| permitted(OwnerTeamActionKind::Remove))
                            .or(busy),
                        Pick::Team {
                            revision,
                            action: OwnerTeamAction::Remove { agent_id },
                        },
                    ));
                }
            }
            options
        });
        match built {
            Ok(options) => self.open_choice(agent.to_owned(), "session team".into(), options),
            Err(reason) => self.fail(reason),
        }
    }

    fn open_replacements(&mut self, agent: &str, revision: u64) {
        let built = self.shown_snapshot().map(|snapshot| {
            let view = &snapshot.view;
            view.eligible_candidates
                .iter()
                .filter(|candidate| {
                    !view
                        .effective
                        .current_members
                        .contains(&candidate.profile.id)
                        && !view.desired_members.contains(&candidate.profile.id)
                })
                .map(|candidate| {
                    let id = &candidate.profile.id;
                    let removed = if snapshot.excluded.contains(id) {
                        " · removed earlier; choosing it selects it again"
                    } else {
                        ""
                    };
                    option(
                        self.agent_choice_label(id),
                        format!("agent {id} · {}{removed}", candidate.profile.provider),
                        None,
                        Pick::Team {
                            revision,
                            action: OwnerTeamAction::Replace {
                                agent_id: agent.to_owned(),
                                replacement_id: id.clone(),
                            },
                        },
                    )
                })
                .collect::<Vec<_>>()
        });
        match built {
            Ok(options) if options.is_empty() => {
                self.fail(not_permitted(OwnerTeamActionKind::Replace))
            }
            Ok(options) => self.open_choice(
                format!("Replace {agent}"),
                "eligible agents".into(),
                options,
            ),
            Err(reason) => self.fail(reason),
        }
    }

    fn agent_choice_label(&self, id: &str) -> String {
        let name = label::agent(
            id,
            self.records.trace.as_ref(),
            self.pool.agent(id).map(|agent| &agent.identity),
            &self.config,
        );
        if name == id {
            name
        } else {
            format!("{name}  {id}")
        }
    }

    fn open_stage_actions(&mut self, stage_id: &str) {
        let built = self.shown_snapshot().and_then(|snapshot| {
            let stage = snapshot
                .stage(stage_id)
                .ok_or_else(|| "This stage is no longer in the session records.".to_owned())?;
            Ok((self.stage_options(snapshot, stage), stage.clone()))
        });
        match built {
            Ok((options, _)) if options.is_empty() => {
                self.fail("This stage offers no action now.");
            }
            Ok((options, stage)) => self.open_choice(
                control_page::purpose_word(&stage.purpose),
                control_page::reason(&stage),
                options,
            ),
            Err(reason) => {
                self.control.want_read();
                self.fail(reason);
            }
        }
    }

    /// The actions a stage supports, from its typed action list, each with the concrete reason
    /// it cannot be chosen now where the typed state gives one.
    fn stage_options(&self, snapshot: &Snapshot, stage: &RecoveryStage) -> Vec<ChoiceOption> {
        let view = &snapshot.view;
        let controls = view
            .recovery_actions
            .iter()
            .find(|actions| actions.stage_id == stage.id)
            .map(|actions| actions.controls.clone())
            .unwrap_or_else(|| stage.manual_actions().controls);
        let busy = self.owner_blocker(false);
        let native = self.owner_blocker(true);
        let session_hold = (view.control != OwnerRunControl::Continue)
            .then(|| "The session is held. Continue the session first.".to_owned());
        let stage_hold = matches!(
            stage.wait_reason,
            Some(RecoveryWaitReason::OwnerWait | RecoveryWaitReason::OwnerPause)
        )
        .then(|| "This stage is held. Release the hold first.".to_owned());
        let first = |reasons: [&Option<String>; 5]| reasons.into_iter().find_map(Clone::clone);
        let none = None;
        let revision = stage.revision;
        let mut options = Vec::new();
        if controls.contains(&RecoveryControlKind::ReviewSavedPlanFresh) {
            match &stage.plan {
                Some(plan) => options.push(option(
                    "Review saved plan again",
                    "A new independent read-only review of the exact saved plan. No task starts.",
                    first([&native, &session_hold, &stage_hold, &none, &none]),
                    Pick::Review {
                        stage: stage.id.clone(),
                        revision,
                        proposal: Box::new(plan.clone()),
                    },
                )),
                None => options.push(option(
                    "Review saved plan again",
                    "A new independent read-only review of the exact saved plan.",
                    Some("No saved plan is recorded for this stage.".into()),
                    Pick::Inspect {
                        stage: stage.id.clone(),
                        revision,
                    },
                )),
            }
        }
        if controls.contains(&RecoveryControlKind::InspectEffects) {
            options.push(option(
                "Inspect effects",
                "An independent inspection of the recorded effects. The runtime chooses the inspector and refuses when the evidence is insufficient.",
                first([&native, &session_hold, &none, &none, &none]),
                Pick::Inspect {
                    stage: stage.id.clone(),
                    revision,
                },
            ));
        }
        if stage.status != RecoveryStatus::Complete
            && !stage.failures.is_empty()
            && !stage.effects_resolved()
        {
            let running = (stage.status == RecoveryStatus::Running)
                .then(|| "Work on this stage is still running.".to_owned());
            let unverified = stage
                .failures
                .iter()
                .any(|f| f.termination != TerminationEvidence::BackendEnded)
                .then(|| "Earlier execution has not verifiably ended.".to_owned());
            options.push(option(
                "Continue with current files",
                if snapshot.authorization(stage).is_some() {
                    "Already authorized for these failed calls. Starts the ordinary run of this session."
                } else {
                    "New work starts from the files as they are now. Effects of the failed calls stay unverified."
                },
                first([&native, &session_hold, &stage_hold, &running, &unverified]),
                Pick::CurrentFiles {
                    stage: stage.id.clone(),
                },
            ));
        }
        for (kind, label, detail) in [
            (
                RecoveryControlKind::Continue,
                "Continue",
                "Records the decision to continue this stage. /resume starts the run.",
            ),
            (
                RecoveryControlKind::Retry,
                "Retry",
                "Records one new attempt of this stage. /resume starts the run.",
            ),
            (
                RecoveryControlKind::ReleaseHold,
                "Release hold",
                "Restores the condition before the hold. Nothing starts.",
            ),
            (
                RecoveryControlKind::Wait,
                "Wait",
                "Holds this stage, recording the condition you name.",
            ),
            (RecoveryControlKind::Pause, "Pause", "Holds this stage."),
        ] {
            if controls.contains(&kind) {
                options.push(option(
                    label,
                    detail,
                    busy.clone(),
                    Pick::Control {
                        stage: stage.id.clone(),
                        revision,
                        kind,
                    },
                ));
            }
        }
        options
    }

    /// Reviewers for a saved plan. Agents that cannot be independent of it are shown with the
    /// reason; the owner API makes the final decision about the rest.
    fn open_reviewers(&mut self, stage_id: &str, revision: u64, proposal: Box<PlanVersion>) {
        let built = self.shown_snapshot().map(|snapshot| {
            let view = &snapshot.view;
            let author = self.records.trace.as_ref().and_then(|trace| {
                trace
                    .assignments
                    .iter()
                    .find(|a| a.id == proposal.producer_assignment_id)
                    .map(|a| a.agent_id.clone())
            });
            let failed: Vec<String> = snapshot
                .stage(stage_id)
                .map(|stage| stage.failures.iter().map(|f| f.agent_id.clone()).collect())
                .unwrap_or_default();
            let native = self.owner_blocker(true);
            let mut options: Vec<ChoiceOption> = view
                .eligible_candidates
                .iter()
                .map(|candidate| {
                    let id = candidate.profile.id.clone();
                    let reason = if author.as_ref() == Some(&id) {
                        Some("Wrote the saved plan, so it cannot review it.".to_owned())
                    } else if failed.contains(&id) {
                        Some("Failed on this stage, so it cannot review it.".to_owned())
                    } else if snapshot.excluded.contains(&id) {
                        Some("Removed from this session by the owner.".to_owned())
                    } else if view.pending_departures.iter().any(|d| d.agent_id == id) {
                        Some("Leaving this session.".to_owned())
                    } else {
                        native.clone()
                    };
                    let member = view.effective.current_members.contains(&id);
                    option(
                        self.agent_choice_label(&id),
                        format!(
                            "agent {id} · {}{}",
                            candidate.profile.provider,
                            if member { " · session member" } else { "" }
                        ),
                        reason,
                        Pick::Reviewer {
                            stage: stage_id.to_owned(),
                            revision,
                            proposal: proposal.clone(),
                            reviewer: id,
                        },
                    )
                })
                .collect();
            options.sort_by_key(|option| option.reason.is_some());
            options
        });
        match built {
            Ok(options) if options.iter().all(|option| option.reason.is_some()) => {
                self.fail("No eligible independent reviewer: the plan author and agents that failed on this stage cannot review it. Add another eligible agent to the session team.");
                if !options.is_empty() {
                    self.open_choice("Choose a reviewer".into(), "none available".into(), options);
                }
            }
            Ok(options) => self.open_choice(
                "Choose a reviewer".into(),
                "read-only review".into(),
                options,
            ),
            Err(reason) => self.fail(reason),
        }
    }

    pub(super) fn choose_key(
        &mut self,
        key: KeyEvent,
        title: String,
        badge: String,
        options: Vec<ChoiceOption>,
        selected: usize,
    ) -> Vec<Action> {
        let last = options.len().saturating_sub(1);
        let next = match key.code {
            KeyCode::Esc => return Vec::new(),
            KeyCode::Enter => {
                let Some(chosen) = options.get(selected).cloned() else {
                    return Vec::new();
                };
                if let Some(reason) = chosen.reason {
                    self.fail(reason);
                    self.overlay = Some(Overlay::Choose {
                        title,
                        badge,
                        options,
                        selected,
                    });
                    return Vec::new();
                }
                return self.pick(chosen.pick);
            }
            KeyCode::Up | KeyCode::Char('k') => selected.saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') => (selected + 1).min(last),
            KeyCode::Home => 0,
            KeyCode::End => last,
            _ => selected,
        };
        self.overlay = Some(Overlay::Choose {
            title,
            badge,
            options,
            selected: next,
        });
        Vec::new()
    }

    fn pick(&mut self, pick: Pick) -> Vec<Action> {
        let Some(session) = self.session.clone() else {
            return Vec::new();
        };
        match pick {
            Pick::Review {
                stage,
                revision,
                proposal,
            } => self.open_reviewers(&stage, revision, proposal),
            Pick::Reviewer {
                stage,
                revision,
                proposal,
                reviewer,
            } => self.submit_owner(Request::Review(Box::new(FreshPlanReviewCommand {
                session_id: session,
                stage_id: stage,
                expected_revision: revision,
                command_id: new_id(),
                proposal: *proposal,
                reviewer_id: reviewer,
            }))),
            Pick::Inspect { stage, revision } => {
                self.submit_owner(Request::Inspect(RecoveryInspectionCommand {
                    session_id: session,
                    stage_id: stage,
                    expected_revision: revision,
                    command_id: new_id(),
                }))
            }
            Pick::CurrentFiles { stage } => {
                // Failures a recorded decision already acknowledges are not acknowledged again:
                // the ordinary run of the session starts in the directory that decision names.
                let recorded = self.control.ready().and_then(|snapshot| {
                    let stage = snapshot.stage(&stage)?;
                    snapshot
                        .authorization(stage)
                        .map(|authorization| authorization.command.context.directory.clone())
                });
                match recorded {
                    Some(directory) => return self.start_run_for(session, directory),
                    None => self.submit_owner(Request::Context { session, stage }),
                }
            }
            Pick::Control {
                stage,
                revision,
                kind: RecoveryControlKind::Wait,
            } => {
                self.overlay = Some(Overlay::Prompt {
                    target: PromptTarget::StageWait {
                        session,
                        stage,
                        revision,
                    },
                    label: "Wait for a condition".into(),
                    help: "Name what this stage waits for. It stays held until you release it."
                        .into(),
                    field: Field::default(),
                });
            }
            Pick::Control {
                stage,
                revision,
                kind,
            } => {
                let action = match kind {
                    RecoveryControlKind::Retry => RecoveryControl::Retry,
                    RecoveryControlKind::Continue => RecoveryControl::Continue,
                    RecoveryControlKind::Pause => RecoveryControl::Pause,
                    RecoveryControlKind::ReleaseHold => RecoveryControl::ReleaseHold,
                    _ => return Vec::new(),
                };
                self.submit_owner(Request::Stage(RecoveryControlCommand {
                    session_id: session,
                    stage_id: stage,
                    expected_revision: revision,
                    command_id: new_id(),
                    action,
                }));
            }
            Pick::Team { revision, action } => self.propose_team(action, revision, false),
            Pick::Replace { agent, revision } => self.open_replacements(&agent, revision),
            Pick::SessionWait { revision } => {
                self.overlay = Some(Overlay::Prompt {
                    target: PromptTarget::SessionWait { session, revision },
                    label: "Session waits for a condition".into(),
                    help:
                        "Name what the session waits for. No new work starts until you continue it."
                            .into(),
                    field: Field::default(),
                });
            }
        }
        Vec::new()
    }

    pub(super) fn commit_wait(&mut self, target: PromptTarget, condition: String) {
        if condition.trim().is_empty() {
            return self.fail("Name the condition to wait for.");
        }
        match target {
            PromptTarget::StageWait {
                session,
                stage,
                revision,
            } => self.submit_owner(Request::Stage(RecoveryControlCommand {
                session_id: session,
                stage_id: stage,
                expected_revision: revision,
                command_id: new_id(),
                action: RecoveryControl::Wait { condition },
            })),
            PromptTarget::SessionWait { session, revision } => {
                self.submit_owner(Request::Team(OwnerTeamCommand {
                    session_id: session,
                    expected_revision: revision,
                    command_id: new_id(),
                    revise_pinned_roster: false,
                    action: OwnerTeamAction::Wait { condition },
                }))
            }
            _ => {}
        }
    }

    /// Send the command whose outcome did not arrive again, exactly as it was.
    fn repeat_command(&mut self) {
        let Some(request) = self.control.retry.clone() else {
            return;
        };
        if self.session.as_deref() != Some(request.session()) {
            return;
        }
        let workspace = request.holds_workspace();
        if let Some(reason) = self.owner_blocker(workspace) {
            return self.fail(reason);
        }
        self.control.retry = None;
        self.submit_owner(request);
    }

    fn start_continuation(&mut self) -> Vec<Action> {
        let Some(receipt) = self.control.continuation.clone() else {
            return Vec::new();
        };
        let context = receipt.command.context;
        self.start_run_for(context.session_id, context.directory)
    }

    fn start_run_for(&mut self, session: String, directory: std::path::PathBuf) -> Vec<Action> {
        if self.session.as_deref() != Some(session.as_str()) {
            return Vec::new();
        }
        if let Some(reason) = self.owner_blocker(true).or_else(|| self.run_refusal()) {
            self.continuation_not_started(&session, &reason);
            return Vec::new();
        }
        vec![Action::ContinueRun { session, directory }]
    }

    /// The run of an authorized continuation has started.
    pub fn continuation_started(&mut self, session: &str) {
        if self
            .control
            .continuation
            .as_ref()
            .is_some_and(|receipt| receipt.command.context.session_id == session)
        {
            self.control.continuation = None;
        }
        self.invalidate_page();
    }

    /// The run of an authorized continuation did not start. The authorization stays recorded and
    /// the owner can start the run again without being asked about the failures again.
    pub fn continuation_not_started(&mut self, session: &str, reason: &str) {
        let reason = reason.trim_end_matches('.');
        let message = format!(
            "Continuation with current files is authorized for session {}, but its run did not start: {reason}. Choose it again on /team to start the run; the authorization is not requested again.",
            text::short_id(session)
        );
        self.status = message.clone();
        self.fail(message);
        self.invalidate_page();
    }

    /// A read of the session team ended without a reply.
    pub fn control_read_abandoned(&mut self, reason: String) {
        self.control.read_abandoned(reason);
        self.invalidate_page();
    }

    /// An owner command ended without a reply. Its exact command is kept for sending again.
    pub fn control_abandoned(&mut self, reason: String) {
        if let Some(request) = self.control.pending().cloned() {
            self.control.finish();
            if !matches!(request, Request::Context { .. }) {
                self.control.retry = Some(request);
            }
        }
        self.status = "Ready".into();
        self.fail(format!(
            "The owner action ended without an outcome: {reason}. Its exact command is kept on /team to send again."
        ));
        self.control.want_read();
        self.invalidate_page();
    }

    /// Take the reply to a read or a command.
    pub fn control_reply(&mut self, reply: Reply) -> Vec<Action> {
        let Reply { request, outcome } = reply;
        let outcome = match (outcome, &request) {
            (
                Outcome::Read(result),
                Request::Read {
                    generation,
                    session,
                },
            ) => {
                if self.control.receive_read(*generation, session, result) {
                    if let Some((wanted, action)) = self.control.deferred.take() {
                        if &wanted == session {
                            match self.control.ready().map(|s| s.view.revision) {
                                Some(revision) => self.propose_team(action, revision, true),
                                None => {
                                    if let Err(reason) = self.shown_snapshot() {
                                        self.fail(reason);
                                    }
                                }
                            }
                        }
                    }
                    self.invalidate_page();
                    self.dirty = true;
                }
                return Vec::new();
            }
            (outcome, _) => outcome,
        };
        self.control.finish();
        self.status = "Ready".into();
        let session = request.session().to_owned();
        let shown = self.session.as_deref() == Some(session.as_str());
        let short = text::short_id(&session);
        let subject = if shown {
            String::new()
        } else {
            format!(" for session {short}")
        };
        let refused = |what: &str, error: &str| {
            format!(
                "{what}{subject}: {}. The session was read again; choose again if the situation changed.",
                error.trim_end_matches('.')
            )
        };
        let mut actions = Vec::new();
        match outcome {
            Outcome::Read(_) => {}
            Outcome::Team(Ok(receipt)) => {
                self.control.retry = None;
                self.notice(team_outcome(&receipt, &short));
            }
            Outcome::Team(Err(error)) => {
                self.control.retry = Some(request.clone());
                self.fail(refused("The team change was not applied", &error));
            }
            Outcome::Stage(Ok(receipt)) => {
                self.control.retry = None;
                self.notice(format!(
                    "{}{subject}",
                    stage_outcome(&receipt.command.action)
                ));
            }
            Outcome::Stage(Err(error)) => {
                self.control.retry = Some(request.clone());
                self.fail(refused("The stage decision was not recorded", &error));
            }
            Outcome::Review(Ok(receipt)) => {
                self.control.retry = None;
                let record = &receipt.record;
                self.notice(format!(
                    "New review of the saved plan{subject}: {} by {}. {} {}No task was started.",
                    if record.approved {
                        "approved"
                    } else {
                        "rejected"
                    },
                    record.response.agent_id,
                    text::one_line(&record.reason),
                    if record.approved {
                        ""
                    } else {
                        "The objection stays and the plan needs revision. "
                    }
                ));
            }
            Outcome::Review(Err(error)) => {
                self.control.retry = Some(request.clone());
                self.fail(refused("The saved plan was not reviewed", &error));
            }
            Outcome::Inspect(Ok(_)) => {
                self.control.retry = None;
                self.notice(format!(
                    "The recorded effects were inspected{subject}. Choose Continue on the stage to proceed; nothing was started."
                ));
            }
            Outcome::Inspect(Err(error)) => {
                self.control.retry = Some(request.clone());
                self.fail(refused("The effects were not inspected", &error));
            }
            Outcome::Context(Ok(context)) => {
                if shown && self.view == View::Team && self.overlay.is_none() {
                    let stage = self
                        .control
                        .ready()
                        .and_then(|snapshot| snapshot.stage(&context.stage_id))
                        .cloned();
                    let question = current_files_question(&context, stage.as_ref());
                    self.overlay = Some(Overlay::Confirm {
                        question,
                        target: Confirm::Owner {
                            request: Box::new(Request::Authorize(Box::new(
                                ContinueWithCurrentFilesCommand {
                                    command_id: new_id(),
                                    context,
                                },
                            ))),
                            badge: "starts the run",
                        },
                    });
                } else {
                    self.notice(format!(
                        "Current files{subject} were read, but that session is no longer on display. Choose Continue with current files again to review them."
                    ));
                }
            }
            Outcome::Context(Err(error)) => {
                self.fail(refused("Current files could not be read", &error));
            }
            Outcome::Authorize(Ok(receipt)) => {
                self.control.retry = None;
                let directory = receipt.command.context.directory.clone();
                self.control.continuation = Some(receipt);
                self.notice(format!(
                    "Continuation with current files is authorized for session {short}. Effects of the failed calls stay unverified."
                ));
                if shown {
                    actions.push(Action::ContinueRun {
                        session: session.clone(),
                        directory,
                    });
                } else {
                    self.continuation_not_started(&session, "another session is displayed");
                }
            }
            Outcome::Authorize(Err(error)) => {
                self.control.retry = Some(request.clone());
                self.fail(refused("Continuation was not authorized", &error));
            }
        }
        if shown {
            self.refresh_records();
            self.control.want_read();
        }
        // The outcome is also the status, so it can be read on the page where it was chosen.
        if let Some(notice) = self.notices.last() {
            self.status = text::one_line(&notice.text);
        }
        self.invalidate_page();
        actions
    }
}

fn team_question(command: &OwnerTeamCommand, busy: bool) -> String {
    let session = text::short_id(&command.session_id);
    let mut lines = vec![match &command.action {
        OwnerTeamAction::Add { agent_id } => format!("Add {agent_id} to session {session}?"),
        OwnerTeamAction::Remove { agent_id } => {
            format!("Remove {agent_id} from session {session}?")
        }
        OwnerTeamAction::Replace {
            agent_id,
            replacement_id,
        } => format!("Replace {agent_id} with {replacement_id} in session {session}?"),
        OwnerTeamAction::Pause => format!("Pause session {session}?"),
        OwnerTeamAction::Continue => format!("Release the hold on session {session}?"),
        OwnerTeamAction::Wait { condition } => {
            format!("Hold session {session} until: {condition}?")
        }
    }];
    if busy {
        lines.push(match &command.action {
            OwnerTeamAction::Replace {
                agent_id,
                replacement_id,
            } => format!(
                "{agent_id} keeps its current work until that ends and is given no new work; {replacement_id} joins then."
            ),
            _ => "It keeps its current work until that ends and is given no new work.".into(),
        });
    }
    if command.revise_pinned_roster {
        lines.push("This session has a fixed roster; confirming also amends it.".into());
    }
    lines.push("Starting preferences are not changed.".into());
    lines.join("\n")
}

fn team_outcome(receipt: &OwnerTeamReceipt, session: &str) -> String {
    let leaving = |agent: &str| {
        receipt
            .pending_departures
            .iter()
            .any(|d| d.agent_id == agent)
    };
    let preferences = "Starting preferences are unchanged.";
    match &receipt.command.action {
        OwnerTeamAction::Add { agent_id } => {
            format!("Added {agent_id} to session {session}. {preferences}")
        }
        OwnerTeamAction::Remove { agent_id } if leaving(agent_id) => format!(
            "{agent_id} is leaving session {session}: it keeps its current work until that ends and is given no new work. {preferences}"
        ),
        OwnerTeamAction::Remove { agent_id } => {
            format!("Removed {agent_id} from session {session}. {preferences}")
        }
        OwnerTeamAction::Replace {
            agent_id,
            replacement_id,
        } if leaving(agent_id) => format!(
            "{agent_id} is leaving session {session} and {replacement_id} joins once its current work ends. {preferences}"
        ),
        OwnerTeamAction::Replace {
            agent_id,
            replacement_id,
        } => format!(
            "Replaced {agent_id} with {replacement_id} in session {session}. {preferences}"
        ),
        OwnerTeamAction::Pause => format!(
            "Session {session} is paused. Current work drains and no new work starts."
        ),
        OwnerTeamAction::Continue => format!(
            "The hold on session {session} is released. No run started; /resume starts it."
        ),
        OwnerTeamAction::Wait { condition } => {
            format!("Session {session} is held until: {condition}.")
        }
    }
}

fn stage_outcome(action: &RecoveryControl) -> String {
    match action {
        RecoveryControl::Retry => {
            "One new attempt of the stage is recorded. /resume starts the run".into()
        }
        RecoveryControl::Continue => {
            "Continuation of the stage is recorded. /resume starts the run".into()
        }
        RecoveryControl::Wait { condition } => format!("The stage is held until: {condition}"),
        RecoveryControl::Pause => "The stage is paused".into(),
        RecoveryControl::ReleaseHold => {
            "The hold on the stage is released and its previous condition restored. Nothing started"
                .into()
        }
    }
}

fn current_files_question(context: &CurrentFilesContext, stage: Option<&RecoveryStage>) -> String {
    let mut lines = vec![format!(
        "Continue session {} with the current files?",
        text::short_id(&context.session_id)
    )];
    lines.push(format!("Directory: {}", context.directory.display()));
    if let Some(stage) = stage {
        lines.push(format!(
            "Stage: {} · {}",
            control_page::purpose_word(&stage.purpose),
            control_page::reason(stage)
        ));
    }
    lines.push(if context.failures.is_empty() {
        "Failed calls: none recorded".into()
    } else {
        format!(
            "Failed calls: {}",
            context
                .failures
                .iter()
                .map(|f| format!("{} ({})", f.agent_id, text::short_id(&f.invocation_id)))
                .collect::<Vec<_>>()
                .join(", ")
        )
    });
    lines.push(format!("Files listed: {}", context.files.len()));
    lines.push("Effects of the failed calls stay unverified, and nothing is rolled back.".into());
    lines.push(
        "Confirming records this decision, then starts the ordinary run of this session.".into(),
    );
    lines.join("\n")
}
