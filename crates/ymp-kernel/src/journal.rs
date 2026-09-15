//! Journal adapter contract and shared validation of complete, atomic appends.

use crate::{events::Event, view::SessionView};
use std::collections::BTreeMap;
use ymp_domain::{
    Denial, Digest, Id, Result,
    journal::{Envelope, MethodParameters, PolicySelection, encode},
    require_text,
};

type ParameterValidator = fn(&PolicySelection) -> Result<()>;

/// Explicit, immutable-at-runtime bindings of implementation/version to parameter schema.
/// This validates configuration data only; validators cannot authorize kernel operations.
#[derive(Clone)]
pub struct ParameterSchemas {
    validators: BTreeMap<(String, String, String), ParameterValidator>,
}

impl Default for ParameterSchemas {
    fn default() -> Self {
        Self {
            validators: BTreeMap::from([
                (
                    ("CostModel".into(), "PriceWeighted".into(), "1".into()),
                    validate_price_weighted as ParameterValidator,
                ),
                (
                    ("ResourcePolicy".into(), "PurposeBounded".into(), "1".into()),
                    validate_purpose_bounded as ParameterValidator,
                ),
                (
                    ("MethodRouter".into(), "FixedMethod".into(), "1".into()),
                    validate_fixed as ParameterValidator,
                ),
                (
                    (
                        "ReadinessProbe".into(),
                        "StaticDependencyProbe".into(),
                        "1".into(),
                    ),
                    validate_static as ParameterValidator,
                ),
            ]),
        }
    }
}
fn validate_price_weighted(selection: &PolicySelection) -> Result<()> {
    let parameters: ymp_domain::resources::PriceWeightedParameters =
        ymp_domain::journal::decode(&encode(&selection.parameters)?)?;
    parameters.validate()
}
fn validate_purpose_bounded(selection: &PolicySelection) -> Result<()> {
    let parameters: ymp_domain::resources::PurposeBoundedParameters =
        ymp_domain::journal::decode(&encode(&selection.parameters)?)?;
    parameters.validate()
}
fn validate_static(selection: &PolicySelection) -> Result<()> {
    if selection.parameters != serde_json::json!({}) {
        return Err(Denial::new(
            "policy_parameters",
            "StaticDependencyProbe version 1 has no parameters",
        ));
    }
    Ok(())
}
fn validate_fixed(selection: &PolicySelection) -> Result<()> {
    MethodParameters::from_selection(selection)?;
    Ok(())
}
impl ParameterSchemas {
    pub fn register(
        &mut self,
        port: &str,
        implementation: &str,
        version: &str,
        validator: ParameterValidator,
    ) -> Result<()> {
        for text in [port, implementation, version] {
            require_text(text, 256)?;
        }
        let key = (port.into(), implementation.into(), version.into());
        if self.validators.contains_key(&key) {
            return Err(Denial::new(
                "policy_schema",
                "An existing implementation/version schema cannot be replaced",
            ));
        }
        self.validators.insert(key, validator);
        Ok(())
    }
    pub fn validate(&self, selection: &PolicySelection) -> Result<()> {
        selection.validate()?;
        let policy = &selection.policy;
        let key = (
            policy.port.clone(),
            policy.implementation.clone(),
            policy.version.clone(),
        );
        let validator = self.validators.get(&key).ok_or_else(|| {
            Denial::new("policy_schema", "Unknown policy implementation or version")
        })?;
        validator(selection)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JournalRead {
    pub revision: u64,
    pub events: Vec<Envelope<Event>>,
}

impl JournalRead {
    pub fn view(&self, session: &Id, through: Option<u64>) -> Result<SessionView> {
        self.view_with_schemas(session, through, &ParameterSchemas::default())
    }
    pub fn view_with_schemas(
        &self,
        session: &Id,
        through: Option<u64>,
        schemas: &ParameterSchemas,
    ) -> Result<SessionView> {
        if self.revision != self.events.len() as u64 {
            return Err(Denial::new(
                "journal_head",
                "Journal head and retained events disagree",
            ));
        }
        let through = through.unwrap_or(self.revision);
        if through > self.revision {
            return Err(Denial::new(
                "revision_missing",
                "Requested journal revision is not available",
            ));
        }
        let count = usize::try_from(through)
            .map_err(|_| Denial::new("revision_overflow", "Revision is not addressable"))?;
        SessionView::replay_with_schemas(session.clone(), &self.events[..count], schemas)
    }
}

/// A trusted adapter is owned by kernel services. Strategies receive SessionView only.
/// `append` validates the whole batch while holding the same lock/transaction used
/// for commit. A denial must leave both the events and the revision unchanged.
pub trait Journal: Send + Sync {
    fn schemas(&self) -> &ParameterSchemas;
    fn read(&self, session: &Id) -> Result<JournalRead>;
    fn append(&self, session: &Id, expected: u64, events: &[Envelope<Event>]) -> Result<u64>;

    fn view(&self, session: &Id, through: Option<u64>) -> Result<SessionView> {
        self.read(session)?
            .view_with_schemas(session, through, self.schemas())
    }

    /// Read-only resolution of a possibly lost acknowledgement. Never resubmits.
    fn resolve_append(
        &self,
        session: &Id,
        expected: u64,
        events: &[Envelope<Event>],
    ) -> Result<AppendResolution> {
        resolve_append(
            &self.read(session)?,
            session,
            expected,
            events,
            self.schemas(),
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AppendResolution {
    Committed(u64),
    Absent,
    Conflict { revision: u64 },
}

pub fn resolve_append(
    current: &JournalRead,
    session: &Id,
    expected: u64,
    events: &[Envelope<Event>],
    schemas: &ParameterSchemas,
) -> Result<AppendResolution> {
    current.view_with_schemas(session, None, schemas)?;
    if events.is_empty() {
        return Err(Denial::new("empty_append", "An append must contain events"));
    }
    let end = expected
        .checked_add(events.len() as u64)
        .ok_or_else(|| Denial::new("revision_overflow", "Journal sequence exhausted"))?;
    if current.revision < expected {
        return Ok(AppendResolution::Conflict {
            revision: current.revision,
        });
    }
    let offset = usize::try_from(expected)
        .map_err(|_| Denial::new("revision_overflow", "Revision is not addressable"))?;
    let prefix = JournalRead {
        revision: expected,
        events: current.events[..offset].to_vec(),
    };
    validate_append(&prefix, session, expected, events, schemas)?;
    if current.revision == expected {
        return Ok(AppendResolution::Absent);
    }
    if current.revision >= end {
        let stored = &current.events[offset..offset + events.len()];
        if encode(&stored)? == encode(&events)? {
            return Ok(AppendResolution::Committed(end));
        }
    }
    Ok(AppendResolution::Conflict {
        revision: current.revision,
    })
}

/// Immutable bytes addressed by SHA-256. Readers provide an allocation bound.
pub trait ContentStore: Send + Sync {
    fn put(&self, bytes: &[u8]) -> Result<Digest>;
    fn get(&self, digest: &Digest, limit: usize) -> Result<Vec<u8>>;
}

pub fn validate_append(
    current: &JournalRead,
    session: &Id,
    expected: u64,
    events: &[Envelope<Event>],
    schemas: &ParameterSchemas,
) -> Result<SessionView> {
    if current.revision != expected {
        return Err(Denial::new(
            "stale_revision",
            "The journal changed since the proposal was prepared",
        ));
    }
    if events.is_empty() {
        return Err(Denial::new("empty_append", "An append must contain events"));
    }
    let mut view = current.view_with_schemas(session, None, schemas)?;
    for event in events {
        view.apply(event, schemas)?;
    }
    view.validate_complete()?;
    Ok(view)
}
