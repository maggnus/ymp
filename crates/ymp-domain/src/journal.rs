//! Pure journal values and the first concrete decision payload.

use crate::{Denial, Digest, Id, PolicyRef, Proposal, Ref, Result, require_text};
use serde::{
    Deserialize, Serialize,
    de::{DeserializeOwned, MapAccess, SeqAccess, Visitor},
};
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
};

/// Canonical JSON v1: sorted object keys, compact encoding, no insignificant whitespace.
/// Array order and JSON number representation remain significant.
pub fn encode<T: Serialize>(value: &T) -> Result<Vec<u8>> {
    fn sorted(value: Value) -> Value {
        match value {
            Value::Object(map) => {
                let map: BTreeMap<_, _> = map.into_iter().map(|(k, v)| (k, sorted(v))).collect();
                Value::Object(map.into_iter().collect())
            }
            Value::Array(items) => Value::Array(items.into_iter().map(sorted).collect()),
            other => other,
        }
    }
    serde_json::to_value(value)
        .and_then(|value| serde_json::to_vec(&sorted(value)))
        .map_err(|_| {
            Denial::new(
                "encoding",
                "The value cannot be represented as canonical JSON",
            )
        })
}

/// Reject duplicate keys at every depth, before conversion to a typed payload.
pub fn decode<T: DeserializeOwned>(bytes: &[u8]) -> Result<T> {
    let value: UniqueValue = serde_json::from_slice(bytes).map_err(|_| {
        Denial::new(
            "decoding",
            "Invalid JSON, duplicate fields or excessive nesting",
        )
    })?;
    serde_json::from_value(value.0).map_err(|_| {
        Denial::new(
            "decoding",
            "Invalid fields or unsupported value in the versioned payload",
        )
    })
}

struct UniqueValue(Value);
impl<'de> Deserialize<'de> for UniqueValue {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        struct JsonVisitor;
        impl<'de> Visitor<'de> for JsonVisitor {
            type Value = UniqueValue;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("JSON without duplicate keys")
            }
            fn visit_bool<E: serde::de::Error>(
                self,
                v: bool,
            ) -> std::result::Result<Self::Value, E> {
                Ok(UniqueValue(v.into()))
            }
            fn visit_i64<E: serde::de::Error>(self, v: i64) -> std::result::Result<Self::Value, E> {
                Ok(UniqueValue(v.into()))
            }
            fn visit_u64<E: serde::de::Error>(self, v: u64) -> std::result::Result<Self::Value, E> {
                Ok(UniqueValue(v.into()))
            }
            fn visit_f64<E: serde::de::Error>(self, v: f64) -> std::result::Result<Self::Value, E> {
                serde_json::Number::from_f64(v)
                    .map(|n| UniqueValue(Value::Number(n)))
                    .ok_or_else(|| E::custom("nonfinite number"))
            }
            fn visit_str<E: serde::de::Error>(
                self,
                v: &str,
            ) -> std::result::Result<Self::Value, E> {
                Ok(UniqueValue(v.into()))
            }
            fn visit_unit<E: serde::de::Error>(self) -> std::result::Result<Self::Value, E> {
                Ok(UniqueValue(Value::Null))
            }
            fn visit_seq<A: SeqAccess<'de>>(
                self,
                mut input: A,
            ) -> std::result::Result<Self::Value, A::Error> {
                let mut values = Vec::new();
                while let Some(UniqueValue(value)) = input.next_element()? {
                    values.push(value);
                }
                Ok(UniqueValue(Value::Array(values)))
            }
            fn visit_map<A: MapAccess<'de>>(
                self,
                mut input: A,
            ) -> std::result::Result<Self::Value, A::Error> {
                let mut values = serde_json::Map::new();
                while let Some(key) = input.next_key::<String>()? {
                    if values.contains_key(&key) {
                        return Err(serde::de::Error::custom("duplicate JSON field"));
                    }
                    let value: UniqueValue = input.next_value()?;
                    values.insert(key, value.0);
                }
                Ok(UniqueValue(Value::Object(values)))
            }
        }
        deserializer.deserialize_any(JsonVisitor)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Actor {
    Runtime,
    Agent(Id),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Envelope<P> {
    pub seq: u64,
    pub session: Id,
    /// Milliseconds since the Unix epoch; supplied explicitly by the caller.
    pub at: u64,
    pub actor: Actor,
    pub policy: Option<PolicyRef>,
    pub input: Option<Digest>,
    pub refs: Vec<Ref>,
    pub payload: P,
}

impl<P: Serialize> Envelope<P> {
    pub fn reference(&self) -> Result<Ref> {
        Ok(Ref {
            id: Id::new(format!(
                "event:{}",
                Digest::of_value(&(&self.session, self.seq))?
            ))?,
            version: Digest::of_value(self)?,
        })
    }
}

/// Recoverable effective parameters, not merely a hash or a current configuration pointer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PolicySelection {
    pub policy: PolicyRef,
    pub parameters: Value,
}

impl PolicySelection {
    pub fn new(port: &str, implementation: &str, version: &str, parameters: Value) -> Result<Self> {
        let selection = Self {
            policy: PolicyRef {
                port: port.into(),
                implementation: implementation.into(),
                version: version.into(),
                params: Digest::of_value(&parameters)?,
            },
            parameters,
        };
        selection.validate()?;
        Ok(selection)
    }

    pub fn validate(&self) -> Result<()> {
        self.policy.validate()?;
        if !self.parameters.is_object() || Digest::of_value(&self.parameters)? != self.policy.params
        {
            return Err(Denial::new(
                "policy_parameters",
                "Effective parameters must be an object matching PolicyRef.params",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum MethodKind {
    Solo,
    SoloWithVerifier,
    AsNeededDecomposition,
    IndependentAttempts(u32),
    BreadthResearch(u32),
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Capability {
    ReadFiles,
    WriteFiles,
    RunProcess,
    TempFiles,
    Sockets,
    Network,
    Browser,
    VcsRead,
    VcsWrite,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum EscalationStep {
    FixEnvironment,
    ReplaceCheck,
    Reassign(BTreeSet<Capability>),
    Retry,
    Decompose(Id),
    AddVerifier,
    AlternativeAttempts(u32),
    StrongerProfile,
    Clarify,
    Replan,
    StopPreserving,
}

/// Data describing a method; recording this value does not execute the method.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Method {
    pub id: Id<Method>,
    pub kind: MethodKind,
    pub ladder: Vec<EscalationStep>,
    pub params: BTreeMap<String, String>,
    pub policy: PolicyRef,
}

impl Method {
    pub fn validate(&self) -> Result<()> {
        self.policy.validate()?;
        if matches!(
            self.kind,
            MethodKind::IndependentAttempts(0) | MethodKind::BreadthResearch(0)
        ) {
            return Err(Denial::new(
                "method",
                "A method must have at least one participant",
            ));
        }
        for step in &self.ladder {
            if matches!(step, EscalationStep::AlternativeAttempts(0))
                || matches!(step, EscalationStep::Reassign(needs) if needs.is_empty())
            {
                return Err(Denial::new(
                    "method",
                    "An escalation must name participants or required capabilities",
                ));
            }
        }
        for (key, value) in &self.params {
            require_text(key, 256)?;
            require_text(value, 4096)?;
        }
        Ok(())
    }
}

/// Parameters for the foundation's two local MethodRouter demonstrations.
/// Feature owners add their own parameter schemas through their real consumers.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MethodParameters {
    pub kind: MethodKind,
    pub ladder: Vec<EscalationStep>,
}

impl MethodParameters {
    pub fn from_selection(selection: &PolicySelection) -> Result<Self> {
        selection.validate()?;
        if selection.policy.port != "MethodRouter"
            || selection.policy.implementation != "FixedMethod"
            || selection.policy.version != "1"
        {
            return Err(Denial::new(
                "policy_port",
                "This parameter schema belongs to MethodRouter/FixedMethod version 1",
            ));
        }
        let value: Self = decode(&encode(&selection.parameters)?)?;
        Method {
            id: Id::new("parameter-validation")?,
            kind: value.kind.clone(),
            ladder: value.ladder.clone(),
            params: BTreeMap::new(),
            policy: selection.policy.clone(),
        }
        .validate()?;
        Ok(value)
    }
}

/// Metadata for an applied decision. Refused proposals return Denial without an event.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Decision<T> {
    pub proposal: Proposal<T>,
    pub effective: PolicySelection,
    pub input: Digest,
    pub outcome: T,
    /// A selection applied by the trusted owner at this exact previous revision.
    pub selection_change: Option<SelectionChange>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SelectionChange {
    pub previous: PolicyRef,
    pub boundary: u64,
}
