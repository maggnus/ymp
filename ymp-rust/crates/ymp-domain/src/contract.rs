//! The durable contract record a run is started against.
//!
//! A contract is a bounded JSON document naming the work to be done (the prompt), the source
//! directory it is done in, and the acceptance condition that decides whether a candidate is
//! accepted. The acceptance condition is required. A package without one describes work no
//! mechanical check can judge, so it is rejected when it is loaded instead of being discovered
//! at verification time, when budget has already been spent.
//!
//! The record carries no filesystem authority: it validates structure, bounds and digests only.
//! Resolving a path, reading a program or storing the document belongs to the application.

use std::fmt;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{MAX_IDENTIFIER_CHARS, digest_bytes};

/// The schema version this binary reads and writes. Version 1 made the verifier optional.
pub const CONTRACT_SCHEMA_VERSION: u32 = 2;
pub const MAX_CONTRACT_BYTES: usize = 1024 * 1024;
pub const MAX_PROMPT_BYTES: usize = 64 * 1024;
pub const MAX_VERIFIER_ARGUMENTS: usize = 32;
pub const MAX_VERIFIER_ARGUMENT_BYTES: usize = 4096;
pub const MAX_VERIFIER_WALL_TIME_MS: u64 = 10 * 60 * 1000;
pub const MAX_VERIFIER_OUTPUT_BYTES: usize = 16 * 1024 * 1024;

/// A part of a contract the operator must supply because the kernel cannot infer it.
///
/// The label is the exact wording the interface and the command surface report, so a refusal
/// names the missing part identically wherever it is stated.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MissingPart {
    AcceptanceCondition,
    VerifierProgram,
    NegativeControl,
    OracleDigest,
    Source,
    Prompt,
    ContractId,
}

impl MissingPart {
    pub const fn label(self) -> &'static str {
        match self {
            Self::AcceptanceCondition => {
                "acceptance condition — a verifier that decides whether a candidate is accepted"
            }
            Self::VerifierProgram => "verifier program",
            Self::NegativeControl => {
                "negative control — a deliberately wrong candidate the verifier must reject"
            }
            Self::OracleDigest => "oracle digest",
            Self::Source => "source directory",
            Self::Prompt => "request text",
            Self::ContractId => "contract identifier",
        }
    }
}

impl fmt::Display for MissingPart {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.label())
    }
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum ContractError {
    #[error("the contract package states no {0}")]
    Missing(MissingPart),
    #[error("the contract package exceeds its {MAX_CONTRACT_BYTES}-byte limit: {actual} bytes")]
    TooLarge { actual: usize },
    #[error("the contract package is not readable as JSON: {0}")]
    Malformed(String),
    #[error("unsupported contract schema version {0}")]
    UnsupportedSchemaVersion(u32),
    #[error("contract_id must contain between 1 and {MAX_IDENTIFIER_CHARS} characters")]
    InvalidContractId,
    #[error("the request text must contain between 1 and {MAX_PROMPT_BYTES} bytes")]
    InvalidPrompt,
    #[error("the oracle digest is not a canonical lowercase SHA-256 digest")]
    InvalidOracleDigest,
    #[error("the verifier wall-time limit must be between 1 and {MAX_VERIFIER_WALL_TIME_MS} ms")]
    InvalidWallTime,
    #[error("the verifier output limit must be between 1 and {MAX_VERIFIER_OUTPUT_BYTES} bytes")]
    InvalidOutputLimit,
    #[error("the verifier arguments exceed the declared count or size limit")]
    InvalidArguments,
    #[error("a capture exclusion must be a relative path without traversal: {0}")]
    InvalidCaptureExclusion(String),
    #[error("contract serialization failed: {0}")]
    Serialization(String),
}

/// What decides a candidate, and what proves the decision discriminates.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct VerifierRecord {
    pub program: PathBuf,
    #[serde(default)]
    pub arguments: Vec<String>,
    pub negative_control: PathBuf,
    pub oracle_digest: String,
    pub wall_time_ms: u64,
    pub output_limit_bytes: usize,
}

/// The contract as it is stored: identity, work, source and acceptance condition.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ContractDocument {
    pub schema_version: u32,
    pub contract_id: String,
    pub source: PathBuf,
    pub prompt: String,
    #[serde(default)]
    pub capture_exclusions: Vec<String>,
    /// Required. A contract whose verifier is absent could not reject a wrong candidate.
    pub verifier: VerifierRecord,
}

/// A contract package as it was read, with the digest of the exact bytes that carried it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParsedContract {
    pub document: ContractDocument,
    pub digest: String,
}

/// The lenient shape used for reading, so a package missing a required part is reported by the
/// part it lacks rather than by a serde field name.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ContractFile {
    schema_version: u32,
    contract_id: Option<String>,
    source: Option<PathBuf>,
    prompt: Option<String>,
    #[serde(default)]
    capture_exclusions: Vec<String>,
    verifier: Option<VerifierFile>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct VerifierFile {
    program: Option<PathBuf>,
    #[serde(default)]
    arguments: Vec<String>,
    negative_control: Option<PathBuf>,
    oracle_digest: Option<String>,
    #[serde(default = "default_wall_time_ms")]
    wall_time_ms: u64,
    #[serde(default = "default_output_limit_bytes")]
    output_limit_bytes: usize,
}

pub const fn default_wall_time_ms() -> u64 {
    60_000
}

pub const fn default_output_limit_bytes() -> usize {
    1024 * 1024
}

impl ContractDocument {
    /// Read a contract package. A package without a verifier reference, its oracle digest or its
    /// negative control is rejected here, before any budget can be reserved against it.
    pub fn parse(bytes: &[u8]) -> Result<ParsedContract, ContractError> {
        if bytes.len() > MAX_CONTRACT_BYTES {
            return Err(ContractError::TooLarge {
                actual: bytes.len(),
            });
        }
        let file: ContractFile = serde_json::from_slice(bytes)
            .map_err(|error| ContractError::Malformed(error.to_string()))?;
        if file.schema_version != CONTRACT_SCHEMA_VERSION {
            return Err(ContractError::UnsupportedSchemaVersion(file.schema_version));
        }
        let verifier = file
            .verifier
            .ok_or(ContractError::Missing(MissingPart::AcceptanceCondition))?;
        let document = Self {
            schema_version: file.schema_version,
            contract_id: file
                .contract_id
                .ok_or(ContractError::Missing(MissingPart::ContractId))?,
            source: file
                .source
                .ok_or(ContractError::Missing(MissingPart::Source))?,
            prompt: file
                .prompt
                .ok_or(ContractError::Missing(MissingPart::Prompt))?,
            capture_exclusions: file.capture_exclusions,
            verifier: VerifierRecord {
                program: verifier
                    .program
                    .ok_or(ContractError::Missing(MissingPart::VerifierProgram))?,
                arguments: verifier.arguments,
                negative_control: verifier
                    .negative_control
                    .ok_or(ContractError::Missing(MissingPart::NegativeControl))?,
                oracle_digest: verifier
                    .oracle_digest
                    .ok_or(ContractError::Missing(MissingPart::OracleDigest))?,
                wall_time_ms: verifier.wall_time_ms,
                output_limit_bytes: verifier.output_limit_bytes,
            },
        };
        document.validate()?;
        Ok(ParsedContract {
            digest: digest_bytes(bytes),
            document,
        })
    }

    /// The exact bytes this document is stored and digested as.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, ContractError> {
        self.validate()?;
        serde_json::to_vec(self).map_err(|error| ContractError::Serialization(error.to_string()))
    }

    pub fn validate(&self) -> Result<(), ContractError> {
        if self.schema_version != CONTRACT_SCHEMA_VERSION {
            return Err(ContractError::UnsupportedSchemaVersion(self.schema_version));
        }
        let identifier_length = self.contract_id.chars().count();
        if !(1..=MAX_IDENTIFIER_CHARS).contains(&identifier_length) {
            return Err(ContractError::InvalidContractId);
        }
        if self.prompt.trim().is_empty() {
            return Err(ContractError::Missing(MissingPart::Prompt));
        }
        if self.prompt.len() > MAX_PROMPT_BYTES {
            return Err(ContractError::InvalidPrompt);
        }
        if self.source.as_os_str().is_empty() {
            return Err(ContractError::Missing(MissingPart::Source));
        }
        for exclusion in &self.capture_exclusions {
            validate_capture_exclusion(exclusion)?;
        }
        self.verifier.validate()
    }
}

impl VerifierRecord {
    pub fn validate(&self) -> Result<(), ContractError> {
        if self.program.as_os_str().is_empty() {
            return Err(ContractError::Missing(MissingPart::VerifierProgram));
        }
        if self.negative_control.as_os_str().is_empty() {
            return Err(ContractError::Missing(MissingPart::NegativeControl));
        }
        if self.oracle_digest.is_empty() {
            return Err(ContractError::Missing(MissingPart::OracleDigest));
        }
        if !is_canonical_digest(&self.oracle_digest) {
            return Err(ContractError::InvalidOracleDigest);
        }
        if self.arguments.len() > MAX_VERIFIER_ARGUMENTS
            || self
                .arguments
                .iter()
                .any(|argument| argument.len() > MAX_VERIFIER_ARGUMENT_BYTES)
        {
            return Err(ContractError::InvalidArguments);
        }
        if self.wall_time_ms == 0 || self.wall_time_ms > MAX_VERIFIER_WALL_TIME_MS {
            return Err(ContractError::InvalidWallTime);
        }
        if self.output_limit_bytes == 0 || self.output_limit_bytes > MAX_VERIFIER_OUTPUT_BYTES {
            return Err(ContractError::InvalidOutputLimit);
        }
        Ok(())
    }
}

fn validate_capture_exclusion(path: &str) -> Result<(), ContractError> {
    let candidate = std::path::Path::new(path);
    let traverses = candidate.components().any(|component| {
        matches!(
            component,
            std::path::Component::ParentDir
                | std::path::Component::RootDir
                | std::path::Component::Prefix(_)
        )
    });
    if path.is_empty() || traverses {
        return Err(ContractError::InvalidCaptureExclusion(path.to_owned()));
    }
    Ok(())
}

fn is_canonical_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[cfg(test)]
mod tests {
    use super::{
        CONTRACT_SCHEMA_VERSION, ContractDocument, ContractError, MissingPart, VerifierRecord,
    };
    use std::path::PathBuf;

    fn document() -> ContractDocument {
        ContractDocument {
            schema_version: CONTRACT_SCHEMA_VERSION,
            contract_id: "contract-000000000000".to_owned(),
            source: PathBuf::from("/tmp/source"),
            prompt: "keep the replay path idempotent".to_owned(),
            capture_exclusions: vec!["target".to_owned()],
            verifier: VerifierRecord {
                program: PathBuf::from("/tmp/verify.sh"),
                arguments: Vec::new(),
                negative_control: PathBuf::from("/tmp/negative"),
                oracle_digest: "a".repeat(64),
                wall_time_ms: 60_000,
                output_limit_bytes: 1024,
            },
        }
    }

    #[test]
    fn a_package_without_an_acceptance_condition_is_rejected_at_load() {
        let bytes = serde_json::to_vec(&serde_json::json!({
            "schema_version": CONTRACT_SCHEMA_VERSION,
            "contract_id": "contract-000000000000",
            "source": "/tmp/source",
            "prompt": "keep the replay path idempotent"
        }))
        .expect("package bytes");
        assert_eq!(
            ContractDocument::parse(&bytes),
            Err(ContractError::Missing(MissingPart::AcceptanceCondition))
        );
    }

    #[test]
    fn a_package_without_a_negative_control_or_oracle_digest_is_rejected_at_load() {
        for (removed, expected) in [
            ("negative_control", MissingPart::NegativeControl),
            ("oracle_digest", MissingPart::OracleDigest),
        ] {
            let mut verifier = serde_json::json!({
                "program": "/tmp/verify.sh",
                "negative_control": "/tmp/negative",
                "oracle_digest": "a".repeat(64)
            });
            verifier
                .as_object_mut()
                .expect("verifier object")
                .remove(removed);
            let bytes = serde_json::to_vec(&serde_json::json!({
                "schema_version": CONTRACT_SCHEMA_VERSION,
                "contract_id": "contract-000000000000",
                "source": "/tmp/source",
                "prompt": "keep the replay path idempotent",
                "verifier": verifier
            }))
            .expect("package bytes");
            assert_eq!(
                ContractDocument::parse(&bytes),
                Err(ContractError::Missing(expected)),
                "removing {removed} was accepted"
            );
        }
    }

    #[test]
    fn a_version_one_package_is_not_read_as_version_two() {
        let bytes = serde_json::to_vec(&serde_json::json!({
            "schema_version": 1,
            "contract_id": "contract-000000000000",
            "source": "/tmp/source",
            "prompt": "keep the replay path idempotent"
        }))
        .expect("package bytes");
        assert_eq!(
            ContractDocument::parse(&bytes),
            Err(ContractError::UnsupportedSchemaVersion(1))
        );
    }

    #[test]
    fn canonical_bytes_round_trip_through_the_reader_with_a_stable_digest() {
        let document = document();
        let bytes = document.canonical_bytes().expect("canonical bytes");
        let parsed = ContractDocument::parse(&bytes).expect("parsed package");
        assert_eq!(parsed.document, document);
        assert_eq!(
            parsed.digest,
            ContractDocument::parse(&document.canonical_bytes().expect("bytes"))
                .expect("parsed")
                .digest
        );
    }

    #[test]
    fn an_oracle_digest_that_is_not_a_digest_is_rejected() {
        let mut document = document();
        document.verifier.oracle_digest = "not-a-digest".to_owned();
        assert_eq!(
            document.canonical_bytes(),
            Err(ContractError::InvalidOracleDigest)
        );
    }
}
