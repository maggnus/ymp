use super::*;
use anyhow::ensure;

impl Engine {
    /// Merge only owner-accepted native identities whose enabled provider binding still matches.
    /// This lets an already running Engine observe a newly scanned model on an existing provider.
    pub(super) fn session_config(&self, session: &str) -> Result<Config> {
        let mut config = self.config.clone();
        if let Some(owner) = self.store.owner_team_state(session)? {
            for accepted in owner.accepted_candidates {
                let profile = &accepted.candidate.profile;
                let Some(provider) = config.providers.iter().find(|p| {
                    p.id == profile.provider
                        && p.enabled
                        && provider_fingerprint(p) == accepted.provider_fingerprint
                }) else {
                    continue;
                };
                let provider_id = provider.id.clone();
                if !config.agents.iter().any(|a| a.id == profile.id) {
                    config.agents.push(profile.clone());
                }
                if let Some(snapshot) = accepted.native_snapshot {
                    let newer = config
                        .native_catalog
                        .providers
                        .get(&provider_id)
                        .is_none_or(|current| {
                            chrono::DateTime::parse_from_rfc3339(&snapshot.last_attempt).ok()
                                > chrono::DateTime::parse_from_rfc3339(&current.last_attempt).ok()
                        });
                    if newer {
                        config
                            .native_catalog
                            .providers
                            .insert(provider_id, snapshot);
                    }
                }
            }
        }
        Ok(config)
    }
    pub(super) fn owner_boundary(&self, session: &str) -> Result<()> {
        self.store.settle_owner_team(session)?;
        if let Some(owner) = self.store.owner_team_state(session)? {
            ensure!(
                owner.control == OwnerRunControl::Continue,
                "owner_paused: session is waiting for explicit owner continuation"
            );
        }
        Ok(())
    }
    /// Resolve a local selection against native scan metadata; aliases and editable captions
    /// are not alternative IDs. No native discovery or model call occurs here.
    pub fn owner_team_command(&self, command: &OwnerTeamCommand) -> Result<OwnerTeamReceipt> {
        if let Some(receipt) = self.store.owner_team_receipt(command)? {
            return Ok(receipt);
        }
        let id = match &command.action {
            OwnerTeamAction::Add { agent_id } => Some(agent_id),
            OwnerTeamAction::Replace { replacement_id, .. } => Some(replacement_id),
            _ => None,
        };
        let config = self.session_config(&command.session_id)?;
        let selected = if let Some(id) = id {
            let pool = ymp_providers::discovery::inspect_pool(&config)?;
            let candidate = pool
                .agents
                .into_iter()
                .find(|a| &a.profile.id == id)
                .context("owner_command: unknown native agent ID")?;
            ensure!(
                candidate.exclusions.is_empty()
                    && matches!(
                        candidate.identity.status,
                        AgentIdentityStatus::Native
                            | AgentIdentityStatus::Stale
                            | AgentIdentityStatus::Local
                    ),
                "owner_command: candidate needs verified native metadata and current eligibility"
            );
            let provider = config.provider(&candidate.profile.provider)?;
            let mut constraints = self
                .store
                .owner_team_state(&command.session_id)?
                .map(|s| s.constraints)
                .or(self
                    .store
                    .session_policy(&command.session_id)?
                    .and_then(|p| p.team_constraints))
                .unwrap_or_default();
            // A pin amendment is explicit; the eligibility restriction remains enforced.
            if command.revise_pinned_roster {
                constraints.fixed_roster = None;
            }
            ensure!(
                constraints
                    .eligible_agents
                    .as_ref()
                    .is_none_or(|allowed| allowed.contains(id)),
                "owner_command: explicit eligibility restriction excludes candidate"
            );
            let settings = self
                .store
                .session_policy(&command.session_id)?
                .and_then(|p| p.execution.get(id).cloned())
                .unwrap_or_default()
                .resolve(&candidate.profile, &ModelEffort::default())?;
            let mut validation = self.clone();
            validation.config = config.clone();
            validation.validate_native_settings(&candidate.profile, &settings)?;
            let selected = VerifiedTeamCandidate {
                provider_fingerprint: provider_fingerprint(provider),
                native_snapshot: config.native_provider_snapshot(&provider.id).cloned(),
                candidate,
            };
            Some(selected)
        } else {
            None
        };
        self.store.commit_owner_team(command, selected.as_ref())
    }
    /// Single typed backend model for the later UI, using stored native metadata only.
    pub fn team_control(&self, session: &str) -> Result<TeamControlView> {
        let effective = self
            .store
            .team_state(session)?
            .context("Team is not initialized")?;
        let startup_policy = self.store.session_policy(session)?;
        let owner = self.store.owner_team_state(session)?;
        let constraints = owner
            .as_ref()
            .map(|s| s.constraints.clone())
            .or_else(|| {
                startup_policy
                    .as_ref()
                    .and_then(|p| p.team_constraints.clone())
            })
            .unwrap_or_default();
        let config = self.session_config(session)?;
        let pool = ymp_providers::discovery::inspect_pool(&config)?;
        let eligible_candidates: Vec<PoolAgent> = pool
            .agents
            .into_iter()
            .filter(|a| {
                a.exclusions.is_empty()
                    && matches!(
                        a.identity.status,
                        AgentIdentityStatus::Native
                            | AgentIdentityStatus::Stale
                            | AgentIdentityStatus::Local
                    )
                    && constraints
                        .eligible_agents
                        .as_ref()
                        .is_none_or(|ids| ids.contains(&a.profile.id))
            })
            .collect();
        let mut permitted_actions = vec![OwnerTeamActionKind::Wait];
        if owner
            .as_ref()
            .is_none_or(|s| s.control == OwnerRunControl::Continue)
        {
            permitted_actions.push(OwnerTeamActionKind::Pause);
        } else {
            permitted_actions.push(OwnerTeamActionKind::Continue);
        }
        if constraints.fixed_size.is_none()
            && effective.current_members.len() < constraints.max_members
        {
            permitted_actions.push(OwnerTeamActionKind::Add);
        }
        if constraints.fixed_size.is_none()
            && !effective.current_members.is_empty()
            && (constraints.fixed_roster.is_none() || effective.current_members.len() > 1)
        {
            permitted_actions.push(OwnerTeamActionKind::Remove);
        }
        if !effective.current_members.is_empty()
            && eligible_candidates
                .iter()
                .any(|a| !effective.current_members.contains(&a.profile.id))
        {
            permitted_actions.push(OwnerTeamActionKind::Replace);
        }
        Ok(TeamControlView {
            schema_version: 1,
            session_id: session.into(),
            revision: effective.revision,
            policy_revision: owner.as_ref().map_or(0, |s| s.policy_revision),
            desired_members: owner
                .as_ref()
                .filter(|s| !s.pending_departures.is_empty())
                .map_or_else(
                    || effective.current_members.clone(),
                    |s| s.desired_members.clone(),
                ),
            pending_departures: owner
                .as_ref()
                .map_or_else(Vec::new, |s| s.pending_departures.clone()),
            control: owner
                .as_ref()
                .map_or(OwnerRunControl::Continue, |s| s.control.clone()),
            startup_policy,
            effective,
            constraints,
            eligible_candidates,
            responsibilities: self.store.active_responsibilities(session)?,
            board: self.store.board(session)?,
            invocation_failures: self
                .store
                .decisions(session)?
                .into_iter()
                .filter_map(|d| d.links.failure)
                .collect(),
            recovery_stages: self.store.recovery_stages(session)?,
            recovery_actions: self
                .store
                .recovery_stages(session)?
                .iter()
                .map(RecoveryStage::manual_actions)
                .collect(),
            permitted_actions,
        })
    }
}
