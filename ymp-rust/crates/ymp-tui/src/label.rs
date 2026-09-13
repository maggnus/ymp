//! What an agent is called: the concrete model it runs as.
//!
//! One rule names agents in the terminal interface and in the headless command alike. While
//! agents are discovered or chosen, an agent is the concrete model identifier native metadata
//! resolves it to. While an agent works, it is the model and effort of the one invocation doing
//! that work, as the invocation reported them. Where no effort was reported, or what was reported
//! only switches thinking on or off, the label is the model alone; a reported `none` is a native
//! level and stays. A caption an installation returned stays in the
//! stored metadata, and a provider or actor identifier stays a field of its own: neither becomes
//! the name. `default` is an internal alias rather than a model, so it is shown only as the model
//! native metadata or the invocation resolves it to. A value nothing recorded stays unknown and
//! says so. A local fixture has no native model to be named by and keeps its configured name.

use crate::text;
use ymp_core::{
    AgentAttribution, AgentIdentity, AgentIdentityStatus, AgentProfile, AssignmentRecord, Config,
    ExecutionSettings, InvocationRecord, Message, ProviderKind, SessionTrace,
};

/// The native alias for whatever model an installation chooses. It names no model itself.
const DEFAULT_ALIAS: &str = "default";
/// A label is one line. An identifier longer than this is cut rather than filling a terminal.
const MAX_CELLS: usize = 120;

/// The label of an agent whose concrete model nothing recorded or resolved.
pub const UNKNOWN_MODEL: &str = "unknown model";
/// Reported values that switch a native thinking control on or off rather than grade an effort.
/// Such a value stays in the details exactly as reported and is never mapped to a level.
const SWITCHES: &[&str] = &["on", "off", "enabled", "disabled", "true", "false"];

/// A native identifier that names a model rather than the internal default alias.
fn concrete(id: &str) -> Option<&str> {
    let id = id.trim();
    (!id.is_empty() && id != DEFAULT_ALIAS).then_some(id)
}

/// The concrete model native metadata resolves an identity to, if it resolves to one.
pub fn resolved_model(identity: &AgentIdentity) -> Option<&str> {
    identity
        .resolved_model
        .as_deref()
        .and_then(concrete)
        .or_else(|| identity.model.as_deref().and_then(concrete))
}

/// Whether an identity is only the internal default alias, with no concrete model resolved.
///
/// Such an agent is not a choice of model. A profile configured that way remains a profile, and
/// an existing member remains a member in an explicitly unknown state.
pub fn unresolved_alias(identity: &AgentIdentity) -> bool {
    identity.status != AgentIdentityStatus::Local
        && identity.model.as_deref().map(str::trim) == Some(DEFAULT_ALIAS)
        && resolved_model(identity).is_none()
}

/// Whether a native identifier is the internal default alias rather than a model.
pub fn is_default_alias(id: &str) -> bool {
    id.trim() == DEFAULT_ALIAS
}

/// The concrete model one recorded assignment ran, with the latest invocation it produced.
pub fn assignment_model(
    assignment: &AssignmentRecord,
    invocation: Option<&InvocationRecord>,
) -> Option<String> {
    Turn::recorded(assignment, invocation).model().map(clean)
}

/// The label of an agent while agents are discovered or chosen: its concrete model alone.
///
/// `None` means native metadata resolves no concrete model for it. A caller states that in its
/// own words rather than substituting a caption or an identifier.
pub fn offering(identity: &AgentIdentity) -> Option<String> {
    match identity.status {
        AgentIdentityStatus::Local => Some(clean(&identity.name)),
        _ => resolved_model(identity).map(clean),
    }
}

/// The label of a captured profile that no record or reading speaks for, such as a member of a
/// session whose records were not read.
pub fn profile(profile: &AgentProfile, config: &Config) -> String {
    if provider_is_local(config, &profile.provider) {
        return clean(&profile.name);
    }
    profile
        .model
        .as_deref()
        .and_then(concrete)
        .map_or_else(|| UNKNOWN_MODEL.to_owned(), clean)
}

/// The label for the work one linked invocation did: its model and the effort it reported.
///
/// Whether that work was a local fixture's is read from the identity captured with it, never
/// from what its provider is configured as now.
pub fn invocation(attribution: &AgentAttribution) -> String {
    recorded(attribution.identity.as_ref(), Turn::linked(attribution))
}

/// The label for one recorded assignment, with the latest invocation it produced.
pub fn assignment(assignment: &AssignmentRecord, invocation: Option<&InvocationRecord>) -> String {
    recorded(
        assignment.agent_identity.as_ref(),
        Turn::recorded(assignment, invocation),
    )
}

fn recorded(identity: Option<&AgentIdentity>, turn: Turn<'_>) -> String {
    match identity {
        // A local fixture has no native model. The identity captured with its turn carries the
        // name it was configured with then.
        Some(identity) if identity.status == AgentIdentityStatus::Local => clean(&identity.name),
        _ => turn.label(),
    }
}

/// Who wrote a stored message.
///
/// The user and ymp keep their own names. An agent message is named by the invocation linked to
/// it, which the caller reads with `SessionTrace::message_attribution`. Nothing else says what
/// ran a message no invocation is linked to: neither its author's other turns nor what its
/// provider is configured as now. Its model is unknown.
pub fn author(message: &Message, linked: Option<&AgentAttribution>) -> String {
    match (message.author.as_str(), linked) {
        ("you" | "ymp", _) => message.author.clone(),
        (_, Some(attribution)) => invocation(attribution),
        (_, None) => UNKNOWN_MODEL.to_owned(),
    }
}

/// A message heading, `author · kind`, as one terminal-safe line.
pub fn heading(message: &Message, linked: Option<&AgentAttribution>) -> String {
    format!("{} · {}", author(message, linked), clean(&message.kind))
}

/// The label for text an agent is streaming: the one invocation the records show it running.
pub fn streaming(agent: &str, trace: Option<&SessionTrace>) -> String {
    trace
        .and_then(|trace| trace.active_agent_attribution(agent))
        .map_or_else(
            || UNKNOWN_MODEL.to_owned(),
            |attribution| invocation(&attribution),
        )
}

/// The label for an agent where no single invocation is meant: a roster row, an assignee, a total.
///
/// A session's records speak for the agents it ran. The latest turn recorded for an agent names
/// its concrete model, and a turn that recorded none leaves the agent unknown rather than named
/// from a later reading. Only an agent the records do not mention is named from the pool, as the
/// pool was last read.
pub fn agent(
    agent: &str,
    trace: Option<&SessionTrace>,
    pool: Option<&AgentIdentity>,
    config: &Config,
) -> String {
    if local_actor(agent, trace, pool, config) {
        return local_name(agent, trace, config);
    }
    let latest = trace.and_then(|trace| {
        let assignment = trace
            .assignments
            .iter()
            .rev()
            .find(|assignment| assignment.agent_id == agent)?;
        let invocation = trace
            .invocations
            .iter()
            .rev()
            .find(|invocation| invocation.assignment_id == assignment.id);
        Some(Turn::recorded(assignment, invocation))
    });
    match latest {
        Some(turn) => turn.model().map_or_else(|| UNKNOWN_MODEL.to_owned(), clean),
        None => pool
            .and_then(offering)
            .unwrap_or_else(|| UNKNOWN_MODEL.to_owned()),
    }
}

/// One line of terminal-safe text: escape sequences and control characters removed, whitespace
/// collapsed, and an overlong value cut.
pub fn clean(value: &str) -> String {
    text::truncate(&text::one_line(value), MAX_CELLS)
}

/// What one invocation asked for, was sent and reported, with the identity captured for it.
struct Turn<'a> {
    identity: Option<&'a AgentIdentity>,
    requested: &'a ExecutionSettings,
    sent: Option<&'a ExecutionSettings>,
    reported: Option<&'a ExecutionSettings>,
}

impl<'a> Turn<'a> {
    fn linked(attribution: &'a AgentAttribution) -> Self {
        Self {
            identity: attribution.identity.as_ref(),
            requested: &attribution.requested,
            sent: Some(&attribution.sent),
            reported: Some(&attribution.reported),
        }
    }

    fn recorded(
        assignment: &'a AssignmentRecord,
        invocation: Option<&'a InvocationRecord>,
    ) -> Self {
        Self {
            identity: assignment.agent_identity.as_ref(),
            requested: invocation.map_or(&assignment.requested, |invocation| &invocation.requested),
            sent: invocation.map(|invocation| &invocation.sent),
            reported: invocation.map(|invocation| &invocation.reported),
        }
    }

    /// The model the turn ran: what the installation reported, or else what was sent or asked
    /// for, resolved through the identity captured for exactly those settings.
    fn model(&self) -> Option<&'a str> {
        if let Some(model) = self
            .reported
            .and_then(|settings| settings.model.as_deref())
            .and_then(concrete)
        {
            return Some(model);
        }
        let asked = self
            .sent
            .and_then(|settings| settings.model.as_deref())
            .or(self.requested.model.as_deref());
        match asked {
            Some(asked) => self
                .identity
                .filter(|identity| identity.model.as_deref() == Some(asked))
                .and_then(|identity| identity.resolved_model.as_deref())
                .and_then(concrete)
                .or_else(|| concrete(asked)),
            None => self.identity.and_then(resolved_model),
        }
    }

    /// The graded effort the installation reported. A requested or sent effort is not evidence
    /// of the effort that applied, so it is never presented as one, and a switch that only turns
    /// thinking on or off grades nothing.
    fn effort(&self) -> Option<&'a str> {
        self.reported
            .and_then(|settings| settings.effort.as_deref())
            .map(str::trim)
            .filter(|effort| {
                !effort.is_empty()
                    && !SWITCHES
                        .iter()
                        .any(|switch| effort.eq_ignore_ascii_case(switch))
            })
    }

    /// The model, with the graded effort beside it only where one was reported.
    fn label(&self) -> String {
        let model = self.model().map_or_else(|| UNKNOWN_MODEL.to_owned(), clean);
        match self.effort() {
            Some(effort) => format!("{model} {}", clean(effort)),
            None => model,
        }
    }
}

/// Whether a roster names a local fixture rather than an agent of a native installation.
///
/// A roster describes the agent as it stands, so this may read its latest turn and its
/// configuration. A message is never named this way; it is named by its own invocation.
fn local_actor(
    agent: &str,
    trace: Option<&SessionTrace>,
    pool: Option<&AgentIdentity>,
    config: &Config,
) -> bool {
    if let Some(trace) = trace {
        if let Some(assignment) = trace
            .assignments
            .iter()
            .rev()
            .find(|assignment| assignment.agent_id == agent)
        {
            // A turn that captured no identity recorded nothing that makes it a fixture's. What
            // its provider is configured as now is not evidence about that turn.
            return assignment
                .agent_identity
                .as_ref()
                .is_some_and(|identity| identity.status == AgentIdentityStatus::Local);
        }
        if let Some(profile) = trace
            .session
            .team
            .iter()
            .find(|profile| profile.id == agent)
        {
            return provider_is_local(config, &profile.provider);
        }
    }
    match pool {
        Some(identity) => identity.status == AgentIdentityStatus::Local,
        None => config
            .agent(agent)
            .is_ok_and(|profile| provider_is_local(config, &profile.provider)),
    }
}

/// A local fixture's name in a roster: captured with its session, or configured.
fn local_name(agent: &str, trace: Option<&SessionTrace>, config: &Config) -> String {
    let captured = trace
        .and_then(|trace| {
            trace
                .session
                .team
                .iter()
                .find(|profile| profile.id == agent)
        })
        .map(|profile| profile.name.as_str());
    let configured = config
        .agent(agent)
        .ok()
        .map(|profile| profile.name.as_str());
    clean(captured.or(configured).unwrap_or(agent))
}

fn provider_is_local(config: &Config, provider: &str) -> bool {
    config
        .provider(provider)
        .is_ok_and(|provider| provider.kind == ProviderKind::Mock)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ymp_core::ProviderConfig;

    fn config() -> Config {
        let native = |id: &str, kind| ProviderConfig {
            id: id.into(),
            kind,
            command: "/ymp-test/never-launched".into(),
            args: vec![],
            env_refs: Default::default(),
            enabled: true,
        };
        let profile = |id: &str, name: &str, provider: &str| AgentProfile {
            id: id.into(),
            name: name.into(),
            provider: provider.into(),
            model: None,
            instructions: String::new(),
            enabled: true,
        };
        Config {
            providers: vec![
                native("native", ProviderKind::Acp),
                native("fixture", ProviderKind::Mock),
            ],
            agents: vec![
                profile("native", "Recommended caption", "native"),
                profile("atlas", "Atlas", "fixture"),
            ],
            team: vec!["native".into(), "atlas".into()],
            ..Config::default()
        }
    }

    fn identity(model: Option<&str>, resolved: Option<&str>) -> AgentIdentity {
        AgentIdentity {
            name: "Default (recommended)".into(),
            configured_name: "Recommended caption".into(),
            model: model.map(str::to_owned),
            effort: None,
            resolved_model: resolved.map(str::to_owned),
            source: None,
            status: AgentIdentityStatus::Native,
        }
    }

    fn settings(model: Option<&str>, effort: Option<&str>) -> ExecutionSettings {
        ExecutionSettings {
            model: model.map(str::to_owned),
            effort: effort.map(str::to_owned),
            permission_mode: None,
        }
    }

    fn linked(
        identity: Option<AgentIdentity>,
        requested: ExecutionSettings,
        sent: ExecutionSettings,
        reported: ExecutionSettings,
    ) -> AgentAttribution {
        AgentAttribution {
            agent_id: "native".into(),
            provider_id: "native".into(),
            assignment_id: "assignment".into(),
            invocation_id: "invocation".into(),
            identity,
            requested,
            sent,
            reported,
        }
    }

    fn message(author: &str) -> Message {
        Message {
            seq: 1,
            session_id: "session".into(),
            author: author.into(),
            recipient: None,
            kind: "chat".into(),
            text: "body".into(),
            created_at: ymp_core::now(),
        }
    }

    #[test]
    fn a_choice_is_the_raw_concrete_model_and_never_its_caption() {
        assert_eq!(
            offering(&identity(Some("native-model-z"), None)).as_deref(),
            Some("native-model-z")
        );
        // Native metadata resolves an alias, and the resolution is the name.
        assert_eq!(
            offering(&identity(Some("opus[1m]"), Some("native-long-version[1m]"))).as_deref(),
            Some("native-long-version[1m]")
        );
        let resolved = identity(Some("default"), Some("native-long-version"));
        assert_eq!(offering(&resolved).as_deref(), Some("native-long-version"));
        assert!(!unresolved_alias(&resolved));
        // The alias alone resolves to nothing, so it is no choice of model at all.
        let alias = identity(Some("default"), None);
        assert_eq!(offering(&alias), None);
        assert!(unresolved_alias(&alias));
        // No model is set: explicitly unknown, and not an alias either.
        assert_eq!(offering(&identity(None, None)), None);
        assert!(!unresolved_alias(&identity(None, None)));
    }

    #[test]
    fn work_is_named_by_the_invocations_own_model_and_reported_effort() {
        let reported = linked(
            Some(identity(Some("default"), Some("native-long-version[1m]"))),
            settings(Some("default"), None),
            settings(Some("default"), None),
            settings(Some("native-short-version"), Some("max")),
        );
        assert_eq!(invocation(&reported), "native-short-version max");
        // Nothing reported yet: the captured identity resolves the alias that was sent, and an
        // effort that was only sent is not presented as the one that applied.
        let unreported = linked(
            Some(identity(Some("default"), Some("native-long-version[1m]"))),
            settings(Some("default"), Some("max")),
            settings(Some("default"), Some("max")),
            settings(None, None),
        );
        assert_eq!(invocation(&unreported), "native-long-version[1m]");
        // A model sent explicitly is acknowledged rather than reported, and it is still the model.
        let explicit = linked(
            None,
            settings(Some("native-model-z"), None),
            settings(Some("native-model-z"), None),
            settings(None, Some("low")),
        );
        assert_eq!(invocation(&explicit), "native-model-z low");
        // The alias with nothing to resolve it is never shown as a model.
        let alias = linked(
            Some(identity(Some("default"), None)),
            settings(Some("default"), None),
            settings(Some("default"), None),
            settings(None, None),
        );
        assert_eq!(invocation(&alias), "unknown model");
    }

    #[test]
    fn a_thinking_switch_or_a_missing_effort_leaves_the_model_alone() {
        // Nothing requested or sent an effort; the installation reported what it did.
        let turn = |effort: Option<&str>| {
            invocation(&linked(
                None,
                settings(Some("glm-4.7"), None),
                settings(Some("glm-4.7"), None),
                settings(Some("glm-4.7"), effort),
            ))
        };
        assert_eq!(turn(None), "glm-4.7");
        assert_eq!(turn(Some(" ")), "glm-4.7");
        // A binary thought control is not a level, and it is not mapped to one.
        for switch in ["on", "off", "enabled", "disabled", "On"] {
            assert_eq!(
                turn(Some(switch)),
                "glm-4.7",
                "{switch} was shown as an effort"
            );
        }
        // A native level stays, including a reported none, which is not missing metadata.
        assert_eq!(turn(Some("none")), "glm-4.7 none");
        assert_eq!(turn(Some("high")), "glm-4.7 high");
    }

    #[test]
    fn an_unlinked_message_stays_unknown_and_a_linked_fixture_keeps_its_captured_name() {
        assert_eq!(author(&message("you"), None), "you");
        assert_eq!(author(&message("ymp"), None), "ymp");
        // This author's provider is a local fixture now. Nothing links the message to a turn,
        // so that says nothing about what wrote it.
        assert_eq!(author(&message("atlas"), None), "unknown model");
        assert_eq!(heading(&message("native"), None), "unknown model · chat");
        let mut fixture = identity(None, None);
        fixture.name = "Atlas as captured".into();
        fixture.status = AgentIdentityStatus::Local;
        let bound = AgentAttribution {
            agent_id: "atlas".into(),
            provider_id: "fixture".into(),
            ..linked(
                Some(fixture),
                settings(None, None),
                settings(None, None),
                settings(None, None),
            )
        };
        assert_eq!(author(&message("atlas"), Some(&bound)), "Atlas as captured");
    }

    #[test]
    fn a_roster_names_a_fixture_by_its_name_and_an_unread_agent_as_unknown() {
        let config = config();
        assert_eq!(agent("atlas", None, None, &config), "Atlas");
        assert_eq!(agent("native", None, None, &config), "unknown model");
        let read = identity(Some("default"), Some("native-long-version"));
        assert_eq!(
            agent("native", None, Some(&read), &config),
            "native-long-version"
        );
    }

    #[test]
    fn a_label_cannot_carry_terminal_control() {
        let hostile = identity(Some("native\u{1b}[31m-model\u{7}\nnext"), None);
        let label = offering(&hostile).unwrap();
        assert_eq!(label, "native-model next");
        assert!(!label.chars().any(char::is_control));
        let long = identity(Some(&"m".repeat(500)), None);
        assert!(text::width(&offering(&long).unwrap()) <= MAX_CELLS);
    }
}
