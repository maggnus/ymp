//! ymp-domain: pure values and validation, no I/O.
//! Values validate at construction and at serialized input boundaries.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use sha2::{Digest as _, Sha256};
use std::{fmt, hash::Hash, marker::PhantomData};

pub type Result<T> = std::result::Result<T, Denial>;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Denial {
    pub code: String,
    pub message: String,
    pub refs: Vec<Ref>,
}

impl Denial {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            refs: Vec::new(),
        }
    }

    pub fn with_ref(mut self, reference: Ref) -> Self {
        self.refs.push(reference);
        self
    }
}

impl fmt::Display for Denial {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for Denial {}

/// An opaque identifier. The marker distinguishes domain types without changing wire bytes.
pub struct Id<T = ()> {
    text: String,
    marker: PhantomData<fn() -> T>,
}

impl<T> Id<T> {
    pub fn new(text: impl Into<String>) -> Result<Self> {
        let text = text.into();
        if text.is_empty()
            || text.len() > 128
            || !text
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"-_.:".contains(&b))
            || !text.as_bytes()[0].is_ascii_alphanumeric()
        {
            return Err(Denial::new(
                "invalid_id",
                "IDs must contain 1–128 ASCII letters, digits, dots, colons, underscores or hyphens, beginning with a letter or digit",
            ));
        }
        Ok(Self {
            text,
            marker: PhantomData,
        })
    }

    pub fn as_str(&self) -> &str {
        &self.text
    }

    pub fn erased(&self) -> Id {
        Id {
            text: self.text.clone(),
            marker: PhantomData,
        }
    }
}

impl<T> Clone for Id<T> {
    fn clone(&self) -> Self {
        Self {
            text: self.text.clone(),
            marker: PhantomData,
        }
    }
}
impl<T> PartialEq for Id<T> {
    fn eq(&self, rhs: &Self) -> bool {
        self.text == rhs.text
    }
}
impl<T> Eq for Id<T> {}
impl<T> PartialOrd for Id<T> {
    fn partial_cmp(&self, rhs: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(rhs))
    }
}
impl<T> Ord for Id<T> {
    fn cmp(&self, rhs: &Self) -> std::cmp::Ordering {
        self.text.cmp(&rhs.text)
    }
}
impl<T> Hash for Id<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.text.hash(state);
    }
}
impl<T> fmt::Debug for Id<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Id").field(&self.text).finish()
    }
}
impl<T> fmt::Display for Id<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.text.fmt(f)
    }
}
impl<T> Serialize for Id<T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.text)
    }
}
impl<'de, T> Deserialize<'de> for Id<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        Self::new(String::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}

/// A finite probability validated at construction and deserialization.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Serialize, Deserialize)]
#[serde(try_from = "f64", into = "f64")]
pub struct Prob(task::Real);
impl Prob {
    pub fn new(value: f64) -> Result<Self> {
        if !(0.0..=1.0).contains(&value) {
            return Err(Denial::new(
                "probability",
                "Probability must be finite and within zero and one",
            ));
        }
        Ok(Self(task::Real::new(value)?))
    }
    pub fn get(self) -> f64 {
        self.0.get()
    }
}
impl TryFrom<f64> for Prob {
    type Error = Denial;
    fn try_from(value: f64) -> Result<Self> {
        Self::new(value)
    }
}
impl From<Prob> for f64 {
    fn from(value: Prob) -> Self {
        value.get()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Digest(String);

impl Digest {
    pub fn of(bytes: impl AsRef<[u8]>) -> Self {
        let bytes = Sha256::digest(bytes.as_ref());
        let mut text = String::with_capacity(64);
        for byte in bytes {
            const HEX: &[u8; 16] = b"0123456789abcdef";
            text.push(HEX[usize::from(byte >> 4)] as char);
            text.push(HEX[usize::from(byte & 15)] as char);
        }
        Self(text)
    }

    pub fn of_value<T: Serialize>(value: &T) -> Result<Self> {
        Ok(Self::of(journal::encode(value)?))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl TryFrom<String> for Digest {
    type Error = Denial;
    fn try_from(text: String) -> Result<Self> {
        if text.len() != 64
            || !text
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(Denial::new(
                "invalid_digest",
                "A SHA-256 digest is exactly 64 lowercase hexadecimal digits",
            ));
        }
        Ok(Self(text))
    }
}
impl From<Digest> for String {
    fn from(value: Digest) -> Self {
        value.0
    }
}
impl fmt::Display for Digest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ref {
    pub id: Id,
    pub version: Digest,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PolicyRef {
    pub port: String,
    #[serde(rename = "impl")]
    pub implementation: String,
    pub version: String,
    pub params: Digest,
}

impl PolicyRef {
    pub fn validate(&self) -> Result<()> {
        for value in [&self.port, &self.implementation, &self.version] {
            require_text(value, 256)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Proposal<T> {
    pub value: T,
    pub rationale: String,
    pub basis: Vec<Ref>,
    pub policy: PolicyRef,
}

impl<T> Proposal<T> {
    pub fn validate(&self) -> Result<()> {
        self.policy.validate()?;
        require_text(&self.rationale, 16_384)
    }
}

pub fn require_text(value: &str, maximum: usize) -> Result<()> {
    if value.trim().is_empty() || value.len() > maximum || value.contains('\0') {
        Err(Denial::new(
            "invalid_text",
            "Text is empty, too long or contains a null character",
        ))
    } else {
        Ok(())
    }
}

pub mod assignment;
pub mod coordination;
pub mod experience;
pub mod identity;
pub mod journal;
pub mod plan;
pub mod report;
pub mod resources;
pub mod result;
pub mod task;
pub mod verification;
pub mod workspace;
