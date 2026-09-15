//! Strict payload codec for the durable journal's version-1 entry payloads.
//!
//! The two payload forms are fixed by the durable Journal contract:
//!
//! ```json
//! {"type":"session_opened","session_id":S,"task":{"id":T,"goal":{"request":G},"acceptance_contract":{"criteria":[{"id":C,"description":D}]},"constraints":{"conditions":[K]}}}
//! ```
//!
//! ```json
//! {"type":"session_cancelled","session_id":S}
//! ```
//!
//! The codec is a private DTO layer: every field is required for its type,
//! unknown fields, duplicate fields and an unknown `type` are rejected, and a
//! `task` on `session_cancelled` is rejected. Key order is insignificant on
//! decode; the encoder produces one byte-stable form. Strings are restored
//! exactly, with no normalization, and domain constructors revalidate every
//! restored value.

use serde::{Deserialize, Serialize};

use ymp_domain::{
    AcceptanceContract, Constraints, Criterion, CriterionId, Goal, SessionId, Task, TaskId,
};
use ymp_kernel::{JournalError, SessionEvent};

/// The version of the payload encoding this build writes and reads.
pub(super) const PAYLOAD_VERSION: u32 = 1;

/// A single payload string is at most 2^20 UTF-8 bytes.
pub(super) const MAX_STRING_BYTES: usize = 1 << 20;
/// A record payload is at most 2^20 bytes.
pub(super) const MAX_PAYLOAD_BYTES: usize = 1 << 20;

/// Root DTO of both payload forms. `task` distinguishes an absent key from an
/// explicit `null` through the nested `Option`, so a `task` key in any form on
/// `session_cancelled` is detectable.
#[derive(Serialize)]
struct PayloadDto<'a> {
    r#type: &'a str,
    session_id: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    task: Option<TaskDto<'a>>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PayloadDtoOwned {
    r#type: String,
    session_id: String,
    #[serde(default, deserialize_with = "present_field")]
    task: Option<Option<TaskDtoOwned<String>>>,
}

/// Distinguishes an absent key (`None`) from an explicit `null`
/// (`Some(None)`) or a value (`Some(Some(..))`).
fn present_field<'de, D, T>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Ok(Some(Option::<T>::deserialize(deserializer)?))
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct TaskDtoOwned<T> {
    id: T,
    goal: GoalDto<T>,
    acceptance_contract: AcceptanceContractDto<T>,
    constraints: ConstraintsDto<T>,
}

type TaskDto<'a> = TaskDtoOwned<&'a str>;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct GoalDto<T> {
    request: T,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct AcceptanceContractDto<T> {
    criteria: Vec<CriterionDto<T>>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CriterionDto<T> {
    id: T,
    description: T,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ConstraintsDto<T> {
    conditions: Vec<T>,
}

/// Encodes one event into its exact version-1 payload form. Input limits are
/// validated before serialization; a violation is an input-limit failure
/// (`AdapterFailure`) raised before any storage access.
pub(super) fn encode_event(event: &SessionEvent) -> Result<Vec<u8>, JournalError> {
    let input_limit = |message: String| JournalError::AdapterFailure { message };
    let require_limit = |value: &str| {
        if value.len() > MAX_STRING_BYTES {
            Err(input_limit(format!(
                "payload string exceeds the {MAX_STRING_BYTES}-byte limit"
            )))
        } else {
            Ok(())
        }
    };

    match event {
        SessionEvent::SessionOpened { session_id, task } => {
            require_limit(session_id.as_str())?;
            require_limit(task.id().as_str())?;
            require_limit(task.goal().request())?;
            for criterion in task.acceptance_contract().criteria() {
                require_limit(criterion.id().as_str())?;
                require_limit(criterion.description())?;
            }
            for condition in task.constraints().conditions() {
                require_limit(condition)?;
            }
            let dto = PayloadDto {
                r#type: "session_opened",
                session_id: session_id.as_str(),
                task: Some(TaskDto {
                    id: task.id().as_str(),
                    goal: GoalDto {
                        request: task.goal().request(),
                    },
                    acceptance_contract: AcceptanceContractDto {
                        criteria: task
                            .acceptance_contract()
                            .criteria()
                            .iter()
                            .map(|criterion| CriterionDto {
                                id: criterion.id().as_str(),
                                description: criterion.description(),
                            })
                            .collect(),
                    },
                    constraints: ConstraintsDto {
                        conditions: task
                            .constraints()
                            .conditions()
                            .iter()
                            .map(String::as_str)
                            .collect(),
                    },
                }),
            };
            serialize(dto)
        }
        SessionEvent::SessionCancelled { session_id } => {
            require_limit(session_id.as_str())?;
            let dto = PayloadDto {
                r#type: "session_cancelled",
                session_id: session_id.as_str(),
                task: None,
            };
            serialize(dto)
        }
    }
}

fn serialize<T: Serialize>(dto: T) -> Result<Vec<u8>, JournalError> {
    let bytes = serde_json::to_vec(&dto).map_err(|error| {
        JournalError::AdapterFailure {
            message: format!("payload encoding failed: {error}"),
        }
    })?;
    if bytes.len() > MAX_PAYLOAD_BYTES {
        return Err(JournalError::AdapterFailure {
            message: format!("encoded record payload exceeds the {MAX_PAYLOAD_BYTES}-byte limit"),
        });
    }
    Ok(bytes)
}

/// Decodes one version-1 payload. Every violation of the fixed forms —
/// malformed JSON, unknown or duplicate fields, an unknown type, a `task` on
/// `session_cancelled`, a limit violation, or a value the domain constructors
/// reject — is returned as a human-readable reason the caller maps to
/// `Corruption`.
pub(super) fn decode_payload(bytes: &[u8]) -> Result<SessionEvent, String> {
    if bytes.len() > MAX_PAYLOAD_BYTES {
        return Err(format!(
            "stored payload exceeds the {MAX_PAYLOAD_BYTES}-byte limit"
        ));
    }
    let dto: PayloadDtoOwned = serde_json::from_slice(bytes).map_err(|error| {
        format!("stored payload is not a valid version-1 payload: {error}")
    })?;
    require_stored_limit(&dto.session_id)?;
    let session_id =
        SessionId::new(dto.session_id).map_err(|error| domain_error("session_id", &error))?;
    match dto.r#type.as_str() {
        "session_opened" => {
            let task = match dto.task {
                Some(Some(task)) => task,
                Some(None) => return Err("field 'task' must not be null".to_owned()),
                None => return Err("missing required field 'task'".to_owned()),
            };
            Ok(SessionEvent::SessionOpened {
                session_id,
                task: build_task(task)?,
            })
        }
        "session_cancelled" => {
            if dto.task.is_some() {
                return Err("unknown field 'task' for type 'session_cancelled'".to_owned());
            }
            Ok(SessionEvent::SessionCancelled { session_id })
        }
        other => Err(format!("unknown payload type '{other}'")),
    }
}

fn require_stored_limit(value: &str) -> Result<(), String> {
    if value.len() > MAX_STRING_BYTES {
        Err(format!(
            "stored payload string exceeds the {MAX_STRING_BYTES}-byte limit"
        ))
    } else {
        Ok(())
    }
}

fn build_task(dto: TaskDtoOwned<String>) -> Result<Task, String> {
    require_stored_limit(&dto.id)?;
    require_stored_limit(&dto.goal.request)?;
    for criterion in &dto.acceptance_contract.criteria {
        require_stored_limit(&criterion.id)?;
        require_stored_limit(&criterion.description)?;
    }
    for condition in &dto.constraints.conditions {
        require_stored_limit(condition)?;
    }

    let mut criteria = Vec::with_capacity(dto.acceptance_contract.criteria.len());
    for criterion in dto.acceptance_contract.criteria {
        let id = CriterionId::new(criterion.id).map_err(|error| domain_error("id", &error))?;
        criteria.push(
            Criterion::new(id, criterion.description)
                .map_err(|error| domain_error("description", &error))?,
        );
    }

    Ok(Task::new(
        TaskId::new(dto.id).map_err(|error| domain_error("id", &error))?,
        Goal::new(dto.goal.request).map_err(|error| domain_error("request", &error))?,
        AcceptanceContract::new(criteria).map_err(|error| domain_error("criteria", &error))?,
        Constraints::new(dto.constraints.conditions)
            .map_err(|error| domain_error("conditions", &error))?,
    ))
}

fn domain_error(field: &str, error: &ymp_domain::DomainError) -> String {
    format!("payload field '{field}' is not a valid domain value: {error}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opened_event() -> SessionEvent {
        let task = Task::new(
            TaskId::new("task-1").expect("valid task ID"),
            Goal::new("Produce a \"quoted\" goal\\path\nwith lines\tand émoji 🚀")
                .expect("valid goal"),
            AcceptanceContract::new(vec![
                Criterion::new(
                    CriterionId::new("c1").expect("valid criterion ID"),
                    "Matches exactly.",
                )
                .expect("valid criterion"),
                Criterion::new(
                    CriterionId::new("c2").expect("valid criterion ID"),
                    "Second criterion.",
                )
                .expect("valid criterion"),
            ])
            .expect("valid acceptance contract"),
            Constraints::new(vec!["no network".to_owned(), "local only".to_owned()])
                .expect("valid constraints"),
        );
        SessionEvent::SessionOpened {
            session_id: SessionId::new("session \"one\"/\\✔").expect("valid session ID"),
            task,
        }
    }

    #[test]
    fn encode_decode_round_trips_exactly() {
        for event in [
            opened_event(),
            SessionEvent::SessionCancelled {
                session_id: SessionId::new("cancel-me").expect("valid session ID"),
            },
        ] {
            let payload = encode_event(&event).expect("payload encodes");
            assert_eq!(decode_payload(&payload).expect("payload decodes"), event);
        }
    }

    #[test]
    fn encode_is_byte_stable_and_matches_the_fixed_form() {
        let payload = encode_event(&SessionEvent::SessionCancelled {
            session_id: SessionId::new("s1").expect("valid session ID"),
        })
        .expect("payload encodes");
        assert_eq!(payload, br#"{"type":"session_cancelled","session_id":"s1"}"#);
        let again = encode_event(&SessionEvent::SessionCancelled {
            session_id: SessionId::new("s1").expect("valid session ID"),
        })
        .expect("payload encodes");
        assert_eq!(payload, again);
    }

    #[test]
    fn decode_ignores_key_order() {
        let payload = br#" {"session_id":"s1","type":"session_cancelled"} "#;
        let expected = SessionEvent::SessionCancelled {
            session_id: SessionId::new("s1").expect("valid session ID"),
        };
        assert_eq!(decode_payload(payload).expect("payload decodes"), expected);
    }

    #[test]
    fn decode_rejects_malformed_payloads() {
        let cases: Vec<(&str, Vec<u8>)> = vec![
            ("unknown type", br#"{"type":"session_paused","session_id":"s1"}"#.to_vec()),
            (
                "unknown field",
                br#"{"type":"session_cancelled","session_id":"s1","extra":1}"#.to_vec(),
            ),
            (
                "nested unknown field",
                br#"{"type":"session_opened","session_id":"s1","task":{"id":"t","goal":{"request":"g","x":1},"acceptance_contract":{"criteria":[{"id":"c","description":"d"}]},"constraints":{"conditions":[]}}}"#.to_vec(),
            ),
            (
                "duplicate field",
                br#"{"type":"session_cancelled","session_id":"s1","session_id":"s2"}"#.to_vec(),
            ),
            (
                "nested duplicate field",
                br#"{"type":"session_cancelled","session_id":"s1","session_id":"s1"}"#.to_vec(),
            ),
            ("missing field", br#"{"type":"session_cancelled"}"#.to_vec()),
            (
                "missing task on opened",
                br#"{"type":"session_opened","session_id":"s1"}"#.to_vec(),
            ),
            (
                "null task on opened",
                br#"{"type":"session_opened","session_id":"s1","task":null}"#.to_vec(),
            ),
            (
                "task on cancelled",
                br#"{"type":"session_cancelled","session_id":"s1","task":null}"#.to_vec(),
            ),
            ("trailing bytes", br#"{"type":"session_cancelled","session_id":"s1"}tail"#.to_vec()),
            (
                "non-string value",
                br#"{"type":"session_cancelled","session_id":7}"#.to_vec(),
            ),
            ("malformed json", b"{not json".to_vec()),
            ("empty payload", Vec::new()),
            (
                "blank domain value",
                br#"{"type":"session_cancelled","session_id":"  "}"#.to_vec(),
            ),
            (
                "empty acceptance contract",
                br#"{"type":"session_opened","session_id":"s1","task":{"id":"t","goal":{"request":"g"},"acceptance_contract":{"criteria":[]},"constraints":{"conditions":[]}}}"#.to_vec(),
            ),
        ];
        for (name, payload) in cases {
            assert!(decode_payload(&payload).is_err(), "case '{name}' should fail");
        }
    }

    #[test]
    fn encode_rejects_oversized_strings_before_building() {
        let oversized = "x".repeat(MAX_STRING_BYTES + 1);
        let event = SessionEvent::SessionCancelled {
            session_id: SessionId::new(oversized).expect("valid session ID"),
        };
        match encode_event(&event) {
            Err(JournalError::AdapterFailure { .. }) => {}
            other => panic!("expected an input-limit AdapterFailure, got {other:?}"),
        }
    }

    #[test]
    fn decode_rejects_oversized_stored_strings() {
        let oversized = "x".repeat(MAX_STRING_BYTES + 1);
        let payload = format!(
            "{{\"type\":\"session_cancelled\",\"session_id\":\"{oversized}\"}}"
        );
        assert!(decode_payload(payload.as_bytes()).is_err());
    }
}
