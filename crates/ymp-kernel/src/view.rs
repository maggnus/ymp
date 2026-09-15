//! Deterministic, read-only projections. No clock, provider or strategy is used by replay.

use crate::{events::Event, journal::ParameterSchemas};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use ymp_domain::{
    Denial, Digest, Id, Ref, Result,
    journal::{Actor, Decision, Envelope, Method, PolicySelection},
};

/// The implemented journal portion of a session, not a claim that a runnable Session exists.
/// A strategy cannot change the projection or obtain a journal writer from it.
///
/// ```compile_fail
/// use ymp_kernel::view::SessionView;
/// fn change_state(view: &SessionView) {
///     view.revision = 20;
/// }
/// ```
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct SessionView {
    session: Id,
    revision: u64,
    opened: bool,
    policies: BTreeMap<String, PolicySelection>,
    method: Option<Method>,
    decisions: Vec<Decision<Method>>,
    references: BTreeSet<Ref>,
}

impl SessionView {
    pub fn empty(session: Id) -> Self {
        Self {
            session,
            revision: 0,
            opened: false,
            policies: BTreeMap::new(),
            method: None,
            decisions: Vec::new(),
            references: BTreeSet::new(),
        }
    }
    pub fn session(&self) -> &Id {
        &self.session
    }
    pub fn revision(&self) -> u64 {
        self.revision
    }
    pub fn opened(&self) -> bool {
        self.opened
    }
    pub fn policies(&self) -> &BTreeMap<String, PolicySelection> {
        &self.policies
    }
    pub fn method(&self) -> Option<&Method> {
        self.method.as_ref()
    }
    pub fn decisions(&self) -> &[Decision<Method>] {
        &self.decisions
    }
    pub fn digest(&self) -> Result<Digest> {
        // Stable v1 commitment to the complete input journal prefix. Adding a
        // derived view field must not change earlier decision input digests.
        Digest::of_value(&(
            "SessionView",
            1_u32,
            &self.session,
            self.revision,
            &self.references,
        ))
    }
    pub fn resolve(&self, reference: &Ref) -> Result<()> {
        if self.references.contains(reference) {
            Ok(())
        } else {
            Err(Denial::new(
                "missing_ref",
                "The versioned reference is absent from this session view",
            )
            .with_ref(reference.clone()))
        }
    }

    pub fn replay(session: Id, events: &[Envelope<Event>]) -> Result<Self> {
        Self::replay_with_schemas(session, events, &ParameterSchemas::default())
    }

    pub fn replay_with_schemas(
        session: Id,
        events: &[Envelope<Event>],
        schemas: &ParameterSchemas,
    ) -> Result<Self> {
        let mut view = Self::empty(session);
        for event in events {
            view.apply(event, schemas)?;
        }
        Ok(view)
    }

    pub(crate) fn apply(
        &mut self,
        event: &Envelope<Event>,
        schemas: &ParameterSchemas,
    ) -> Result<()> {
        if event.payload.version() != 1 {
            return Err(Denial::new(
                "event_version",
                "Unsupported event payload version",
            ));
        }
        if event.session != self.session
            || event.seq
                != self
                    .revision
                    .checked_add(1)
                    .ok_or_else(|| Denial::new("revision_overflow", "Journal sequence exhausted"))?
        {
            return Err(Denial::new(
                "event_order",
                "Event belongs to another session or is not the next sequence",
            ));
        }
        if event.actor != Actor::Runtime {
            return Err(Denial::new(
                "actor",
                "Session opening and method decisions must be committed by the runtime",
            ));
        }
        for reference in &event.refs {
            self.resolve(reference)?;
        }
        match &event.payload {
            Event::SessionOpened { selections, .. } => {
                if self.opened
                    || self.revision != 0
                    || event.policy.is_some()
                    || event.input.is_some()
                    || !event.refs.is_empty()
                {
                    return Err(Denial::new(
                        "session_open",
                        "A session can only be opened once at its initial revision",
                    ));
                }
                if selections.is_empty() {
                    return Err(Denial::new(
                        "policy_selection",
                        "Explicit session policy selections are required",
                    ));
                }
                let mut policies = BTreeMap::new();
                for selection in selections {
                    schemas.validate(selection)?;
                    if policies
                        .insert(selection.policy.port.clone(), selection.clone())
                        .is_some()
                    {
                        return Err(Denial::new(
                            "policy_selection",
                            "A session selects exactly one implementation per port",
                        ));
                    }
                }
                self.policies = policies;
                self.opened = true;
            }
            Event::MethodChosen { decision, .. } => {
                if decision.effective.policy.port != "MethodRouter" {
                    return Err(Denial::new(
                        "policy_port",
                        "MethodChosen requires a MethodRouter policy",
                    ));
                }
                if !self.opened {
                    return Err(Denial::new(
                        "session_missing",
                        "Open the session before recording a method",
                    ));
                }
                decision.proposal.validate()?;
                decision.proposal.value.validate()?;
                schemas.validate(&decision.effective)?;
                let current = self.policies.get("MethodRouter").ok_or_else(|| {
                    Denial::new("policy_selection", "No MethodRouter is selected")
                })?;
                match &decision.selection_change {
                    None if current != &decision.effective => {
                        return Err(Denial::new(
                            "policy_selection",
                            "The proposal does not use the selected effective policy",
                        ));
                    }
                    Some(change)
                        if change.previous != current.policy
                            || change.boundary != self.revision
                            || current == &decision.effective =>
                    {
                        return Err(Denial::new(
                            "policy_change",
                            "Policy change is stale, redundant or bound to another work boundary",
                        ));
                    }
                    _ => {}
                }
                if decision.proposal.policy != decision.effective.policy
                    || decision.outcome.policy != decision.effective.policy
                    || decision.outcome != decision.proposal.value
                    || event.policy.as_ref() != Some(&decision.effective.policy)
                    || event.input.as_ref() != Some(&decision.input)
                    || decision.input != self.digest()?
                    || event.refs != decision.proposal.basis
                {
                    return Err(Denial::new(
                        "decision",
                        "Decision attribution, input view, proposal or kernel outcome disagrees",
                    ));
                }
                for reference in &decision.proposal.basis {
                    self.resolve(reference)?;
                }
                self.references.insert(Ref {
                    id: decision.outcome.id.erased(),
                    version: Digest::of_value(&decision.outcome)?,
                });
                self.method = Some(decision.outcome.clone());
                self.policies
                    .insert("MethodRouter".into(), decision.effective.clone());
                self.decisions.push((**decision).clone());
            }
        }
        self.references.insert(event.reference()?);
        self.revision = event.seq;
        Ok(())
    }
}
