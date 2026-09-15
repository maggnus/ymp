//! Strict payload codec for the durable journal's version-1 entry payloads.
//!
//! The payload forms are fixed by the durable Journal contract. Version 1
//! originally defined the two session-lifecycle forms; later executable
//! slices added execution and criterion-evaluation forms within the same
//! version, preserving the original two byte-for-byte:
//!
//! ```json
//! {"type":"session_opened","session_id":S,"task":{"id":T,"goal":{"request":G},"acceptance_contract":{"criteria":[{"id":C,"description":D}]},"constraints":{"conditions":[K]}}}
//! {"type":"session_cancelled","session_id":S}
//! {"type":"assignment_admitted","session_id":S,"invocation":I,"assignment":{"invocation":I,"agent":A,"role":R,"requested_settings":[{"key":K,"value":V}],"sent_settings":[{"key":K,"value":V}],"allowance":{"reservation":N,"reservation_purpose":"production"|"verification"|"coordination","limits":{"max_turns":N,"max_output_chars":N,"max_wall_clock_ms":N}},"grant":{"id":G,"invocation":I,"reservation":N,"reservation_purpose":"production"|"verification"|"coordination"},"workspace_accesses":[{"scope":W,"operations":["read"|"write"]}]}}
//! {"type":"invocation_start_attempted","session_id":S,"invocation":I}
//! {"type":"invocation_started","session_id":S,"invocation":I}
//! {"type":"invocation_cancellation_requested","session_id":S,"invocation":I}
//! {"type":"invocation_observed","session_id":S,"invocation":I,"termination":{"outcome":"completed"|"failed"|"cancelled"|"timed_out"[,"error_class":C]},"reported_settings":[...]|null,"usage":{"turns":N|null,"output_chars":N|null,"wall_clock_ms":N|null}}
//! {"type":"invocation_uncertain","session_id":S,"invocation":I,"cause":"bounded_wait_expired"|"start_outcome_unknown"}
//! {"type":"invocation_failed_at_start","session_id":S,"invocation":I,"error_class":C}
//! {"type":"effect_evidence_recorded","session_id":S,"invocation":I}
//! {"type":"invocation_accounted","session_id":S,"invocation":I,"usage":{...},"reservation":N}
//! {"type":"criterion_evaluated","session_id":S,"invocation":I,"evidence":{"criteria":[C],"workspace":W,"method":{"kind":"command","command":C},"exit_code":N,"elapsed_ms":N,"files":[],"verifiers":[{"path":P,"sha256":D}]}}
//! ```
//!
//! The codec is a private DTO layer: every field is required for its type,
//! unknown fields, duplicate fields and an unknown `type` are rejected, and
//! a field of one type appearing on another type is rejected. Key order is
//! insignificant on decode; the encoder produces one byte-stable form.
//! Strings are restored exactly, with no normalization, and domain
//! constructors revalidate every restored value. A `null` is meaningful
//! only where the form states it (unreported usage components and
//! unreported settings); elsewhere it is rejected.
//!
//! Wall-clock durations are transported as whole milliseconds: the values
//! are host-side limits and observations, and sub-millisecond precision is
//! not representable in this form.

use std::time::Duration;

use serde::{Deserialize, Serialize};

use ymp_domain::{
    AcceptanceContract, Check, CheckMethod, Constraints, Criterion, CriterionId, Evidence,
    EvidenceFile, Goal, SessionId, Task, TaskId, VerifierDigest,
};
use ymp_kernel::execution::{
    AgentId, Allowance, Assignment, ErrorClass, Grant, GrantId, InvocationId, InvocationLimits,
    ObservedUsage, ReservationPurpose, ResourceAmount, Role, SettingKey, SettingValue, Settings,
    Termination, UncertaintyCause, WorkspaceAccess, WorkspaceOperation, WorkspaceScope,
};
use ymp_kernel::{JournalError, SessionEvent};

/// The version of the payload encoding this build writes and reads.
pub(super) const PAYLOAD_VERSION: u32 = 1;

/// A single payload string is at most 2^20 UTF-8 bytes.
pub(super) const MAX_STRING_BYTES: usize = 1 << 20;
/// A record payload is at most 2^20 bytes.
pub(super) const MAX_PAYLOAD_BYTES: usize = 1 << 20;

// ---------------------------------------------------------------------------
// DTOs
// ---------------------------------------------------------------------------

type SettingPairs<'a> = Vec<SettingPairDto<&'a str>>;

/// Encode DTO: one flat struct whose per-type field presence produces the
/// byte-stable forms. `reported_settings` uses the nested option so an
/// unknown report serializes as an explicit `null`, not an omitted key.
#[derive(Serialize)]
struct PayloadDto<'a> {
    r#type: &'a str,
    session_id: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    task: Option<TaskDto<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    invocation: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    assignment: Option<AssignmentDto<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sent_settings: Option<SettingPairs<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    termination: Option<TerminationDto<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reported_settings: Option<Option<SettingPairs<'a>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    usage: Option<UsageDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cause: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error_class: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reservation: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    evidence: Option<EvidenceDto>,
}

/// Decode DTO: every non-envelope field is presence-tracked so that
/// `decode_payload` can require exactly the fields of the declared type and
/// reject absent, null and foreign fields precisely.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PayloadDtoOwned {
    r#type: String,
    session_id: String,
    #[serde(default, deserialize_with = "present_field")]
    task: Option<Option<TaskDtoOwned<String>>>,
    #[serde(default, deserialize_with = "present_field")]
    invocation: Option<Option<String>>,
    #[serde(default, deserialize_with = "present_field")]
    assignment: Option<Option<AssignmentDtoOwned<String>>>,
    #[serde(default, deserialize_with = "present_field")]
    sent_settings: Option<Option<Vec<SettingPairDto<String>>>>,
    #[serde(default, deserialize_with = "present_field")]
    termination: Option<Option<TerminationDtoOwned<String>>>,
    #[serde(default, deserialize_with = "present_field")]
    reported_settings: Option<Option<Vec<SettingPairDto<String>>>>,
    #[serde(default, deserialize_with = "present_field")]
    usage: Option<Option<UsageDto>>,
    #[serde(default, deserialize_with = "present_field")]
    cause: Option<Option<String>>,
    #[serde(default, deserialize_with = "present_field")]
    error_class: Option<Option<String>>,
    #[serde(default, deserialize_with = "present_field")]
    reservation: Option<Option<u64>>,
    #[serde(default, deserialize_with = "present_field")]
    evidence: Option<Option<EvidenceDto>>,
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
struct SettingPairDto<T> {
    key: T,
    value: T,
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

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct EvidenceDto {
    criteria: Vec<String>,
    workspace: String,
    method: CheckMethodDto,
    #[serde(default, deserialize_with = "present_field")]
    exit_code: Option<Option<i32>>,
    elapsed_ms: u64,
    files: Vec<EvidenceFileDto>,
    verifiers: Vec<VerifierDigestDto>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum CheckMethodDto {
    Command { command: String },
    ExactBytes { path: String, expected: Vec<u8> },
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct EvidenceFileDto {
    path: String,
    #[serde(default, deserialize_with = "present_field")]
    bytes: Option<Option<Vec<u8>>>,
    #[serde(default, deserialize_with = "present_field")]
    sha256: Option<Option<String>>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct VerifierDigestDto {
    path: String,
    sha256: String,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ConstraintsDto<T> {
    conditions: Vec<T>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct InvocationLimitsDto {
    max_turns: u32,
    max_output_chars: u64,
    max_wall_clock_ms: u64,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct AllowanceDto {
    reservation: u64,
    reservation_purpose: String,
    limits: InvocationLimitsDto,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct GrantDto<T> {
    id: T,
    invocation: T,
    reservation: u64,
    reservation_purpose: T,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WorkspaceAccessDto<T> {
    scope: T,
    operations: Vec<T>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct AssignmentDtoOwned<T> {
    invocation: T,
    agent: T,
    role: T,
    requested_settings: Vec<SettingPairDto<T>>,
    sent_settings: Vec<SettingPairDto<T>>,
    allowance: AllowanceDto,
    grant: GrantDto<T>,
    workspace_accesses: Vec<WorkspaceAccessDto<T>>,
}

type AssignmentDto<'a> = AssignmentDtoOwned<&'a str>;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct TerminationDtoOwned<T> {
    outcome: T,
    #[serde(
        default,
        deserialize_with = "present_field",
        skip_serializing_if = "Option::is_none"
    )]
    error_class: Option<Option<T>>,
}

type TerminationDto<'a> = TerminationDtoOwned<&'a str>;

/// Usage components are presence-tracked: every key is required, and an
/// explicit `null` is the meaningful unknown.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct UsageDto {
    #[serde(default, deserialize_with = "present_field")]
    turns: Option<Option<u64>>,
    #[serde(default, deserialize_with = "present_field")]
    output_chars: Option<Option<u64>>,
    #[serde(default, deserialize_with = "present_field")]
    wall_clock_ms: Option<Option<u64>>,
}

// ---------------------------------------------------------------------------
// Decode field checks
// ---------------------------------------------------------------------------

/// A present field carrying a value.
fn required<'a, T>(field: &'a Option<Option<T>>, name: &str) -> Result<&'a T, String> {
    match field {
        Some(Some(value)) => Ok(value),
        Some(None) => Err(format!("field '{name}' must not be null")),
        None => Err(format!("missing required field '{name}'")),
    }
}

/// A present field whose explicit `null` is a meaningful unknown.
fn required_nullable<'a, T>(
    field: &'a Option<Option<T>>,
    name: &str,
) -> Result<&'a Option<T>, String> {
    match field {
        Some(value) => Ok(value),
        None => Err(format!("missing required field '{name}'")),
    }
}

// ---------------------------------------------------------------------------
// Kernel <-> DTO conversions
// ---------------------------------------------------------------------------

fn identifier<T: AsRef<str> + Into<String>>(value: T, name: &str) -> Result<String, String> {
    if value.as_ref().trim().is_empty() {
        Err(format!("field '{name}' must not be blank"))
    } else {
        Ok(value.into())
    }
}

fn settings_pairs(settings: &Settings) -> Vec<SettingPairDto<&str>> {
    settings
        .iter()
        .map(|(key, value)| SettingPairDto {
            key: key.as_str(),
            value: value.as_str(),
        })
        .collect()
}

fn build_settings(pairs: &[SettingPairDto<String>]) -> Result<Settings, String> {
    let mut settings = Settings::new();
    for pair in pairs {
        let key = SettingKey::new(identifier(pair.key.clone(), "setting key")?)
            .map_err(|error| format!("setting key is not valid: {error}"))?;
        let value = SettingValue::new(identifier(pair.value.clone(), "setting value")?)
            .map_err(|error| format!("setting value is not valid: {error}"))?;
        if settings.contains_key(&key) {
            return Err(format!("duplicate setting key '{}'", key.as_str()));
        }
        settings.insert(key, value);
    }
    Ok(settings)
}

fn millis(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

fn limits_dto(limits: &InvocationLimits) -> InvocationLimitsDto {
    InvocationLimitsDto {
        max_turns: limits.max_turns(),
        max_output_chars: limits.max_output_chars(),
        max_wall_clock_ms: millis(limits.max_wall_clock()),
    }
}

fn build_limits(dto: &InvocationLimitsDto) -> Result<InvocationLimits, String> {
    InvocationLimits::new(
        dto.max_turns,
        dto.max_output_chars,
        Duration::from_millis(dto.max_wall_clock_ms),
    )
    .map_err(|error| format!("allowance limits are not valid: {error}"))
}

fn allowance_dto(allowance: &Allowance) -> AllowanceDto {
    AllowanceDto {
        reservation: allowance.reservation().value(),
        reservation_purpose: allowance.reservation_purpose().as_str().to_owned(),
        limits: limits_dto(allowance.limits()),
    }
}

fn build_allowance(dto: &AllowanceDto) -> Result<Allowance, String> {
    Allowance::new(
        ResourceAmount::new(dto.reservation),
        build_reservation_purpose(&dto.reservation_purpose)?,
        build_limits(&dto.limits)?,
    )
    .map_err(|error| format!("allowance is not valid: {error}"))
}

fn assignment_dto(assignment: &Assignment) -> AssignmentDto<'_> {
    AssignmentDtoOwned {
        invocation: assignment.invocation().as_str(),
        agent: assignment.agent().as_str(),
        role: assignment.role().as_str(),
        requested_settings: settings_pairs(assignment.requested_settings()),
        sent_settings: settings_pairs(assignment.sent_settings()),
        allowance: allowance_dto(assignment.allowance()),
        grant: GrantDto {
            id: assignment.grant().id().as_str(),
            invocation: assignment.grant().invocation().as_str(),
            reservation: assignment.grant().reservation().value(),
            reservation_purpose: assignment.grant().reservation_purpose().as_str(),
        },
        workspace_accesses: assignment
            .workspace_accesses()
            .iter()
            .map(|access| WorkspaceAccessDto {
                scope: access.scope().as_str(),
                operations: access
                    .operations()
                    .iter()
                    .map(|operation| operation.as_str())
                    .collect(),
            })
            .collect(),
    }
}

fn build_assignment(dto: &AssignmentDtoOwned<String>) -> Result<Assignment, String> {
    let invocation = InvocationId::new(identifier(dto.invocation.clone(), "invocation")?)
        .map_err(|error| format!("invocation ID is not valid: {error}"))?;
    let grant_invocation = InvocationId::new(identifier(
        dto.grant.invocation.clone(),
        "grant invocation",
    )?)
    .map_err(|error| format!("grant invocation ID is not valid: {error}"))?;
    if grant_invocation != invocation {
        return Err("grant invocation does not match the assignment invocation".to_owned());
    }
    let allowance = build_allowance(&dto.allowance)?;
    let grant_reservation = ResourceAmount::new(dto.grant.reservation);
    if grant_reservation != allowance.reservation() {
        return Err("grant reservation does not match the allowance".to_owned());
    }
    let grant_purpose = build_reservation_purpose(&dto.grant.reservation_purpose)?;
    if grant_purpose != allowance.reservation_purpose() {
        return Err("grant reservation purpose does not match the allowance".to_owned());
    }
    let workspace_accesses = dto
        .workspace_accesses
        .iter()
        .map(|access| {
            let scope = WorkspaceScope::new(identifier(access.scope.clone(), "workspace scope")?)
                .map_err(|error| format!("workspace scope is not valid: {error}"))?;
            let operations = access
                .operations
                .iter()
                .map(|operation| build_workspace_operation(operation))
                .collect::<Result<Vec<_>, _>>()?;
            WorkspaceAccess::new(scope, operations)
                .map_err(|error| format!("workspace access is not valid: {error}"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Assignment::new(
        invocation,
        AgentId::new(identifier(dto.agent.clone(), "agent")?)
            .map_err(|error| format!("agent ID is not valid: {error}"))?,
        Role::new(identifier(dto.role.clone(), "role")?)
            .map_err(|error| format!("role is not valid: {error}"))?,
        build_settings(&dto.requested_settings)?,
        build_settings(&dto.sent_settings)?,
        allowance,
        Grant::new(
            GrantId::new(identifier(dto.grant.id.clone(), "grant ID")?)
                .map_err(|error| format!("grant ID is not valid: {error}"))?,
            grant_invocation,
            grant_reservation,
            grant_purpose,
        ),
        workspace_accesses,
    )
    .map_err(|error| format!("assignment is not valid: {error}"))
}

fn build_reservation_purpose(name: &str) -> Result<ReservationPurpose, String> {
    match name {
        "production" => Ok(ReservationPurpose::Production),
        "verification" => Ok(ReservationPurpose::Verification),
        "coordination" => Ok(ReservationPurpose::Coordination),
        other => Err(format!("unknown reservation purpose '{other}'")),
    }
}

fn build_workspace_operation(name: &str) -> Result<WorkspaceOperation, String> {
    match name {
        "read" => Ok(WorkspaceOperation::Read),
        "write" => Ok(WorkspaceOperation::Write),
        other => Err(format!("unknown workspace operation '{other}'")),
    }
}

fn cause_name(cause: &UncertaintyCause) -> &'static str {
    match cause {
        UncertaintyCause::BoundedWaitExpired => "bounded_wait_expired",
        UncertaintyCause::StartOutcomeUnknown => "start_outcome_unknown",
    }
}

fn build_cause(name: &str) -> Result<UncertaintyCause, String> {
    match name {
        "bounded_wait_expired" => Ok(UncertaintyCause::BoundedWaitExpired),
        "start_outcome_unknown" => Ok(UncertaintyCause::StartOutcomeUnknown),
        other => Err(format!("unknown uncertainty cause '{other}'")),
    }
}

fn termination_dto(termination: &Termination) -> TerminationDto<'_> {
    match termination {
        Termination::Completed => TerminationDtoOwned {
            outcome: "completed",
            error_class: None,
        },
        Termination::Failed { class } => TerminationDtoOwned {
            outcome: "failed",
            error_class: Some(Some(class.as_str())),
        },
        Termination::Cancelled => TerminationDtoOwned {
            outcome: "cancelled",
            error_class: None,
        },
        Termination::TimedOut => TerminationDtoOwned {
            outcome: "timed_out",
            error_class: None,
        },
    }
}

fn build_termination(dto: &TerminationDtoOwned<String>) -> Result<Termination, String> {
    match dto.outcome.as_str() {
        "completed" => {
            forbid_termination_error_class(dto)?;
            Ok(Termination::Completed)
        }
        "cancelled" => {
            forbid_termination_error_class(dto)?;
            Ok(Termination::Cancelled)
        }
        "timed_out" => {
            forbid_termination_error_class(dto)?;
            Ok(Termination::TimedOut)
        }
        "failed" => {
            let class = required(&dto.error_class, "termination.error_class")?.clone();
            Ok(Termination::Failed {
                class: ErrorClass::new(identifier(class, "termination.error_class")?)
                    .map_err(|error| format!("error class is not valid: {error}"))?,
            })
        }
        other => Err(format!("unknown termination outcome '{other}'")),
    }
}

fn forbid_termination_error_class(dto: &TerminationDtoOwned<String>) -> Result<(), String> {
    if dto.error_class.is_some() {
        Err(format!(
            "field 'termination.error_class' is not valid for termination outcome '{}'",
            dto.outcome
        ))
    } else {
        Ok(())
    }
}

fn usage_dto(usage: &ObservedUsage) -> UsageDto {
    UsageDto {
        turns: usage.turns().map(Some),
        output_chars: usage.output_chars().map(Some),
        wall_clock_ms: usage.wall_clock().map(millis).map(Some),
    }
}

fn build_usage(dto: &UsageDto) -> Result<ObservedUsage, String> {
    let mut usage = ObservedUsage::unknown();
    if let Some(turns) = *required_nullable(&dto.turns, "usage.turns")? {
        usage = usage.with_turns(turns);
    }
    if let Some(output_chars) = *required_nullable(&dto.output_chars, "usage.output_chars")? {
        usage = usage.with_output_chars(output_chars);
    }
    if let Some(wall_clock_ms) = *required_nullable(&dto.wall_clock_ms, "usage.wall_clock_ms")? {
        usage = usage.with_wall_clock(Duration::from_millis(wall_clock_ms));
    }
    Ok(usage)
}

fn evidence_path_text(path: &std::path::Path) -> Result<&str, JournalError> {
    path.to_str().ok_or_else(|| JournalError::AdapterFailure {
        message: format!("evidence path '{}' is not valid UTF-8", path.display()),
    })
}

fn evidence_dto(evidence: &Evidence) -> Result<EvidenceDto, JournalError> {
    let method = match evidence.check().method() {
        CheckMethod::Command { command } => CheckMethodDto::Command {
            command: command.clone(),
        },
        CheckMethod::ExactBytes { path, expected } => CheckMethodDto::ExactBytes {
            path: evidence_path_text(path)?.to_owned(),
            expected: expected.clone(),
        },
    };
    let files = evidence
        .files()
        .iter()
        .map(|file| {
            Ok(EvidenceFileDto {
                path: evidence_path_text(file.path())?.to_owned(),
                bytes: Some(file.bytes().map(<[u8]>::to_vec)),
                sha256: Some(file.sha256().map(str::to_owned)),
            })
        })
        .collect::<Result<Vec<_>, JournalError>>()?;
    let verifiers = evidence
        .verifiers()
        .iter()
        .map(|verifier| {
            Ok(VerifierDigestDto {
                path: evidence_path_text(verifier.path())?.to_owned(),
                sha256: verifier.sha256().to_owned(),
            })
        })
        .collect::<Result<Vec<_>, JournalError>>()?;
    Ok(EvidenceDto {
        criteria: evidence
            .check()
            .criteria()
            .iter()
            .map(|criterion| criterion.as_str().to_owned())
            .collect(),
        workspace: evidence.workspace().to_owned(),
        method,
        exit_code: Some(evidence.exit_code()),
        elapsed_ms: evidence.elapsed_ms(),
        files,
        verifiers,
    })
}

fn build_evidence(dto: EvidenceDto) -> Result<Evidence, String> {
    for criterion in &dto.criteria {
        require_stored_limit(criterion)?;
    }
    let criteria = dto
        .criteria
        .into_iter()
        .map(|criterion| {
            CriterionId::new(criterion).map_err(|error| domain_error("evidence.criteria", &error))
        })
        .collect::<Result<Vec<_>, _>>()?;
    require_stored_limit(&dto.workspace)?;

    match dto.method {
        CheckMethodDto::Command { command } => {
            require_stored_limit(&command)?;
            if !dto.files.is_empty() {
                return Err("command evidence must not contain file captures".to_owned());
            }
            let exit_code = *required(&dto.exit_code, "evidence.exit_code")?;
            let mut verifiers = Vec::with_capacity(dto.verifiers.len());
            for verifier in dto.verifiers {
                require_stored_limit(&verifier.path)?;
                require_stored_limit(&verifier.sha256)?;
                verifiers.push(
                    VerifierDigest::new(verifier.path, verifier.sha256)
                        .map_err(|error| domain_error("evidence.verifiers", &error))?,
                );
            }
            let check = Check::command(criteria, command)
                .map_err(|error| domain_error("evidence.method", &error))?;
            Evidence::command(check, dto.workspace, exit_code, dto.elapsed_ms, verifiers)
                .map_err(|error| domain_error("evidence", &error))
        }
        CheckMethodDto::ExactBytes { path, expected } => {
            require_stored_limit(&path)?;
            if required_nullable(&dto.exit_code, "evidence.exit_code")?.is_some() {
                return Err("exact-byte evidence must not contain an exit code".to_owned());
            }
            if !dto.verifiers.is_empty() || dto.files.len() != 1 {
                return Err(
                    "exact-byte evidence requires one file capture and no verifiers".to_owned(),
                );
            }
            let file = dto.files.into_iter().next().expect("length checked");
            require_stored_limit(&file.path)?;
            let bytes = required_nullable(&file.bytes, "evidence.files.bytes")?.clone();
            let digest = required_nullable(&file.sha256, "evidence.files.sha256")?.clone();
            if let Some(digest) = &digest {
                require_stored_limit(digest)?;
            }
            let snapshot = EvidenceFile::new(file.path, bytes)
                .map_err(|error| domain_error("evidence.files", &error))?;
            if snapshot.sha256() != digest.as_deref() {
                return Err("evidence file SHA-256 does not match its exact bytes".to_owned());
            }
            let check = Check::exact_bytes(criteria, path, expected)
                .map_err(|error| domain_error("evidence.method", &error))?;
            Evidence::exact_bytes(check, dto.workspace, dto.elapsed_ms, snapshot)
                .map_err(|error| domain_error("evidence", &error))
        }
    }
}

// ---------------------------------------------------------------------------
// Encode / decode
// ---------------------------------------------------------------------------

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
    let require_settings_limit = |settings: &Settings| {
        for (key, value) in settings.iter() {
            require_limit(key.as_str())?;
            require_limit(value.as_str())?;
        }
        Ok(())
    };

    fn base<'a>(kind: &'a str, session_id: &'a str) -> PayloadDto<'a> {
        PayloadDto {
            r#type: kind,
            session_id,
            task: None,
            invocation: None,
            assignment: None,
            sent_settings: None,
            termination: None,
            reported_settings: None,
            usage: None,
            cause: None,
            error_class: None,
            reservation: None,
            evidence: None,
        }
    }

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
            let mut dto = base("session_opened", session_id.as_str());
            dto.task = Some(TaskDto {
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
            });
            serialize(dto)
        }
        SessionEvent::SessionCancelled { session_id } => {
            require_limit(session_id.as_str())?;
            serialize(base("session_cancelled", session_id.as_str()))
        }
        SessionEvent::AssignmentAdmitted {
            session_id,
            invocation,
            assignment,
        } => {
            require_limit(session_id.as_str())?;
            require_limit(invocation.as_str())?;
            require_limit(assignment.invocation().as_str())?;
            require_limit(assignment.agent().as_str())?;
            require_limit(assignment.role().as_str())?;
            require_limit(assignment.grant().id().as_str())?;
            require_limit(assignment.grant().invocation().as_str())?;
            for access in assignment.workspace_accesses() {
                require_limit(access.scope().as_str())?;
            }
            require_settings_limit(assignment.requested_settings())?;
            require_settings_limit(assignment.sent_settings())?;
            let mut dto = base("assignment_admitted", session_id.as_str());
            dto.invocation = Some(invocation.as_str());
            dto.assignment = Some(assignment_dto(assignment));
            serialize(dto)
        }
        SessionEvent::InvocationStartAttempted {
            session_id,
            invocation,
        } => {
            require_limit(session_id.as_str())?;
            require_limit(invocation.as_str())?;
            let mut dto = base("invocation_start_attempted", session_id.as_str());
            dto.invocation = Some(invocation.as_str());
            serialize(dto)
        }
        SessionEvent::InvocationStarted {
            session_id,
            invocation,
        } => {
            require_limit(session_id.as_str())?;
            require_limit(invocation.as_str())?;
            let mut dto = base("invocation_started", session_id.as_str());
            dto.invocation = Some(invocation.as_str());
            serialize(dto)
        }
        SessionEvent::InvocationUncertain {
            session_id,
            invocation,
            cause,
        } => {
            require_limit(session_id.as_str())?;
            require_limit(invocation.as_str())?;
            let mut dto = base("invocation_uncertain", session_id.as_str());
            dto.invocation = Some(invocation.as_str());
            dto.cause = Some(cause_name(cause));
            serialize(dto)
        }
        SessionEvent::InvocationFailedAtStart {
            session_id,
            invocation,
            class,
        } => {
            require_limit(session_id.as_str())?;
            require_limit(invocation.as_str())?;
            require_limit(class.as_str())?;
            let mut dto = base("invocation_failed_at_start", session_id.as_str());
            dto.invocation = Some(invocation.as_str());
            dto.error_class = Some(class.as_str());
            serialize(dto)
        }
        SessionEvent::EffectEvidenceRecorded {
            session_id,
            invocation,
        } => {
            require_limit(session_id.as_str())?;
            require_limit(invocation.as_str())?;
            let mut dto = base("effect_evidence_recorded", session_id.as_str());
            dto.invocation = Some(invocation.as_str());
            serialize(dto)
        }
        SessionEvent::InvocationCancellationRequested {
            session_id,
            invocation,
        } => {
            require_limit(session_id.as_str())?;
            require_limit(invocation.as_str())?;
            let mut dto = base("invocation_cancellation_requested", session_id.as_str());
            dto.invocation = Some(invocation.as_str());
            serialize(dto)
        }
        SessionEvent::InvocationObserved {
            session_id,
            invocation,
            termination,
            reported_settings,
            usage,
        } => {
            require_limit(session_id.as_str())?;
            require_limit(invocation.as_str())?;
            if let Termination::Failed { class } = termination {
                require_limit(class.as_str())?;
            }
            if let Some(reported) = reported_settings.as_ref() {
                require_settings_limit(reported)?;
            }
            let mut dto = base("invocation_observed", session_id.as_str());
            dto.invocation = Some(invocation.as_str());
            dto.termination = Some(termination_dto(termination));
            dto.reported_settings = Some(reported_settings.as_ref().map(settings_pairs));
            dto.usage = Some(usage_dto(usage));
            serialize(dto)
        }
        SessionEvent::InvocationAccounted {
            session_id,
            invocation,
            usage,
            reservation,
        } => {
            require_limit(session_id.as_str())?;
            require_limit(invocation.as_str())?;
            let mut dto = base("invocation_accounted", session_id.as_str());
            dto.invocation = Some(invocation.as_str());
            dto.usage = Some(usage_dto(usage));
            dto.reservation = Some(reservation.value());
            serialize(dto)
        }
        SessionEvent::CriterionEvaluated {
            session_id,
            invocation,
            evidence,
        } => {
            require_limit(session_id.as_str())?;
            require_limit(invocation.as_str())?;
            for criterion in evidence.check().criteria() {
                require_limit(criterion.as_str())?;
            }
            require_limit(evidence.workspace())?;
            if let CheckMethod::Command { command } = evidence.check().method() {
                require_limit(command)?;
            }
            if let CheckMethod::ExactBytes { path, .. } = evidence.check().method() {
                require_limit(evidence_path_text(path)?)?;
            }
            for file in evidence.files() {
                require_limit(evidence_path_text(file.path())?)?;
                if let Some(digest) = file.sha256() {
                    require_limit(digest)?;
                }
            }
            for verifier in evidence.verifiers() {
                require_limit(evidence_path_text(verifier.path())?)?;
                require_limit(verifier.sha256())?;
            }
            let mut dto = base("criterion_evaluated", session_id.as_str());
            dto.invocation = Some(invocation.as_str());
            dto.evidence = Some(evidence_dto(evidence)?);
            serialize(dto)
        }
    }
}

fn serialize<T: Serialize>(dto: T) -> Result<Vec<u8>, JournalError> {
    let bytes = serde_json::to_vec(&dto).map_err(|error| JournalError::AdapterFailure {
        message: format!("payload encoding failed: {error}"),
    })?;
    if bytes.len() > MAX_PAYLOAD_BYTES {
        return Err(JournalError::AdapterFailure {
            message: format!("encoded record payload exceeds the {MAX_PAYLOAD_BYTES}-byte limit"),
        });
    }
    Ok(bytes)
}

/// Decodes one version-1 payload. Every violation of the fixed forms —
/// malformed JSON, unknown or duplicate fields, an unknown type, a field of
/// one type appearing on another, a limit violation, or a value the domain
/// constructors reject — is returned as a human-readable reason the caller
/// maps to `Corruption`.
pub(super) fn decode_payload(bytes: &[u8]) -> Result<SessionEvent, String> {
    if bytes.len() > MAX_PAYLOAD_BYTES {
        return Err(format!(
            "stored payload exceeds the {MAX_PAYLOAD_BYTES}-byte limit"
        ));
    }
    let dto: PayloadDtoOwned = serde_json::from_slice(bytes)
        .map_err(|error| format!("stored payload is not a valid version-1 payload: {error}"))?;
    require_stored_limit(&dto.session_id)?;
    let session_id = SessionId::new(dto.session_id.clone())
        .map_err(|error| domain_error("session_id", &error))?;
    match dto.r#type.as_str() {
        "session_opened" => {
            forbid_all(
                &dto,
                &[
                    "invocation",
                    "assignment",
                    "sent_settings",
                    "termination",
                    "reported_settings",
                    "usage",
                    "cause",
                    "error_class",
                    "reservation",
                ],
            )?;
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
            forbid_all(
                &dto,
                &[
                    "task",
                    "invocation",
                    "assignment",
                    "sent_settings",
                    "termination",
                    "reported_settings",
                    "usage",
                    "cause",
                    "error_class",
                    "reservation",
                ],
            )?;
            Ok(SessionEvent::SessionCancelled { session_id })
        }
        "assignment_admitted" => {
            forbid_all(
                &dto,
                &[
                    "task",
                    "sent_settings",
                    "termination",
                    "reported_settings",
                    "usage",
                    "cause",
                    "error_class",
                    "reservation",
                ],
            )?;
            let invocation = build_invocation(&dto.invocation)?;
            let assignment = build_assignment(required(&dto.assignment, "assignment")?)?;
            if assignment.invocation() != &invocation {
                return Err("assignment invocation does not match the event invocation".to_owned());
            }
            Ok(SessionEvent::AssignmentAdmitted {
                session_id,
                invocation,
                assignment,
            })
        }
        "invocation_start_attempted" => {
            forbid_all(
                &dto,
                &[
                    "task",
                    "assignment",
                    "sent_settings",
                    "termination",
                    "reported_settings",
                    "usage",
                    "cause",
                    "error_class",
                    "reservation",
                ],
            )?;
            Ok(SessionEvent::InvocationStartAttempted {
                session_id,
                invocation: build_invocation(&dto.invocation)?,
            })
        }
        "invocation_started" => {
            forbid_all(
                &dto,
                &[
                    "task",
                    "assignment",
                    "sent_settings",
                    "termination",
                    "reported_settings",
                    "usage",
                    "cause",
                    "error_class",
                    "reservation",
                ],
            )?;
            Ok(SessionEvent::InvocationStarted {
                session_id,
                invocation: build_invocation(&dto.invocation)?,
            })
        }
        "invocation_uncertain" => {
            forbid_all(
                &dto,
                &[
                    "task",
                    "assignment",
                    "sent_settings",
                    "termination",
                    "reported_settings",
                    "usage",
                    "error_class",
                    "reservation",
                ],
            )?;
            Ok(SessionEvent::InvocationUncertain {
                session_id,
                invocation: build_invocation(&dto.invocation)?,
                cause: build_cause(required(&dto.cause, "cause")?)?,
            })
        }
        "invocation_failed_at_start" => {
            forbid_all(
                &dto,
                &[
                    "task",
                    "assignment",
                    "sent_settings",
                    "termination",
                    "reported_settings",
                    "usage",
                    "cause",
                    "reservation",
                ],
            )?;
            Ok(SessionEvent::InvocationFailedAtStart {
                session_id,
                invocation: build_invocation(&dto.invocation)?,
                class: ErrorClass::new(identifier(
                    required(&dto.error_class, "error_class")?.clone(),
                    "error_class",
                )?)
                .map_err(|error| format!("error class is not valid: {error}"))?,
            })
        }
        "effect_evidence_recorded" => {
            forbid_all(
                &dto,
                &[
                    "task",
                    "assignment",
                    "sent_settings",
                    "termination",
                    "reported_settings",
                    "usage",
                    "cause",
                    "error_class",
                    "reservation",
                ],
            )?;
            Ok(SessionEvent::EffectEvidenceRecorded {
                session_id,
                invocation: build_invocation(&dto.invocation)?,
            })
        }
        "invocation_cancellation_requested" => {
            forbid_all(
                &dto,
                &[
                    "task",
                    "assignment",
                    "sent_settings",
                    "termination",
                    "reported_settings",
                    "usage",
                    "cause",
                    "error_class",
                    "reservation",
                ],
            )?;
            Ok(SessionEvent::InvocationCancellationRequested {
                session_id,
                invocation: build_invocation(&dto.invocation)?,
            })
        }
        "invocation_observed" => {
            forbid_all(
                &dto,
                &[
                    "task",
                    "assignment",
                    "sent_settings",
                    "cause",
                    "error_class",
                    "reservation",
                ],
            )?;
            let reported = match required_nullable(&dto.reported_settings, "reported_settings")? {
                None => None,
                Some(pairs) => Some(build_settings(pairs)?),
            };
            Ok(SessionEvent::InvocationObserved {
                session_id,
                invocation: build_invocation(&dto.invocation)?,
                termination: build_termination(required(&dto.termination, "termination")?)?,
                reported_settings: reported,
                usage: build_usage(required(&dto.usage, "usage")?)?,
            })
        }
        "invocation_accounted" => {
            forbid_all(
                &dto,
                &[
                    "task",
                    "assignment",
                    "sent_settings",
                    "termination",
                    "reported_settings",
                    "cause",
                    "error_class",
                ],
            )?;
            Ok(SessionEvent::InvocationAccounted {
                session_id,
                invocation: build_invocation(&dto.invocation)?,
                usage: build_usage(required(&dto.usage, "usage")?)?,
                reservation: ResourceAmount::new(*required(&dto.reservation, "reservation")?),
            })
        }
        "criterion_evaluated" => {
            forbid_all(
                &dto,
                &[
                    "task",
                    "assignment",
                    "sent_settings",
                    "termination",
                    "reported_settings",
                    "usage",
                    "cause",
                    "error_class",
                    "reservation",
                ],
            )?;
            Ok(SessionEvent::CriterionEvaluated {
                session_id,
                invocation: build_invocation(&dto.invocation)?,
                evidence: build_evidence(required(&dto.evidence, "evidence")?.clone())?,
            })
        }
        other => Err(format!("unknown payload type '{other}'")),
    }
}

/// Rejects every named field that is present for this payload type.
fn forbid_all(dto: &PayloadDtoOwned, names: &[&str]) -> Result<(), String> {
    if dto.evidence.is_some() && dto.r#type != "criterion_evaluated" {
        return Err("unknown field 'evidence' for this payload type".to_owned());
    }
    for name in names {
        let present = match *name {
            "task" => dto.task.is_some(),
            "invocation" => dto.invocation.is_some(),
            "assignment" => dto.assignment.is_some(),
            "cause" => dto.cause.is_some(),
            "error_class" => dto.error_class.is_some(),
            "sent_settings" => dto.sent_settings.is_some(),
            "termination" => dto.termination.is_some(),
            "reported_settings" => dto.reported_settings.is_some(),
            "usage" => dto.usage.is_some(),
            "reservation" => dto.reservation.is_some(),
            "evidence" => dto.evidence.is_some(),
            _ => false,
        };
        if present {
            return Err(format!("unknown field '{name}' for this payload type"));
        }
    }
    Ok(())
}

fn build_invocation(field: &Option<Option<String>>) -> Result<InvocationId, String> {
    InvocationId::new(identifier(
        required(field, "invocation")?.clone(),
        "invocation",
    )?)
    .map_err(|error| format!("invocation ID is not valid: {error}"))
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
    use serde_json::{Value, json};

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

    fn key(name: &str) -> SettingKey {
        SettingKey::new(name).expect("valid setting key")
    }

    fn value(name: &str) -> SettingValue {
        SettingValue::new(name).expect("valid setting value")
    }

    fn settings(pairs: &[(&str, &str)]) -> Settings {
        Settings::from_pairs(pairs.iter().map(|(k, v)| (key(k), value(v)))).expect("valid settings")
    }

    fn limits() -> InvocationLimits {
        InvocationLimits::new(12, 4096, Duration::from_millis(90_000)).expect("valid limits")
    }

    fn allowance() -> Allowance {
        Allowance::new(
            ResourceAmount::new(5),
            ReservationPurpose::Coordination,
            limits(),
        )
        .expect("valid allowance")
    }

    fn assignment() -> Assignment {
        let invocation = InvocationId::new("invocation-1").expect("valid invocation ID");
        Assignment::new(
            invocation.clone(),
            AgentId::new("claude-opus-5").expect("valid agent ID"),
            Role::new("implementer").expect("valid role"),
            settings(&[("effort", "high"), ("thinking", "on")]),
            settings(&[("effort", "high")]),
            allowance(),
            Grant::new(
                GrantId::new("grant-1").expect("valid grant ID"),
                invocation,
                ResourceAmount::new(5),
                ReservationPurpose::Coordination,
            ),
            vec![
                WorkspaceAccess::new(
                    WorkspaceScope::new("session-primary").expect("valid scope"),
                    [WorkspaceOperation::Read, WorkspaceOperation::Write],
                )
                .expect("valid workspace access"),
                WorkspaceAccess::new(
                    WorkspaceScope::new("session-review").expect("valid scope"),
                    [WorkspaceOperation::Read],
                )
                .expect("valid workspace access"),
            ],
        )
        .expect("valid assignment")
    }

    fn valid_assignment_payload() -> Value {
        json!({
            "type": "assignment_admitted",
            "session_id": "s1",
            "invocation": "invocation-1",
            "assignment": {
                "invocation": "invocation-1",
                "agent": "a",
                "role": "r",
                "requested_settings": [],
                "sent_settings": [],
                "allowance": {
                    "reservation": 1,
                    "reservation_purpose": "production",
                    "limits": {
                        "max_turns": 1,
                        "max_output_chars": 1,
                        "max_wall_clock_ms": 1
                    }
                },
                "grant": {
                    "id": "grant-1",
                    "invocation": "invocation-1",
                    "reservation": 1,
                    "reservation_purpose": "production"
                },
                "workspace_accesses": [{"scope": "w", "operations": ["read"]}]
            }
        })
    }

    fn valid_observed_payload(outcome: &str) -> Value {
        json!({
            "type": "invocation_observed",
            "session_id": "s1",
            "invocation": "invocation-1",
            "termination": {"outcome": outcome},
            "reported_settings": null,
            "usage": {"turns": null, "output_chars": null, "wall_clock_ms": null}
        })
    }

    fn decode_json_payload(payload: &Value) -> Result<SessionEvent, String> {
        let bytes = serde_json::to_vec(payload).expect("test payload serializes");
        decode_payload(&bytes)
    }

    fn assert_json_payload_error(payload: &Value, expected_fragments: &[&str]) {
        let error = decode_json_payload(payload).expect_err("test payload should be rejected");
        for fragment in expected_fragments {
            assert!(
                error.contains(fragment),
                "expected error containing '{fragment}', got '{error}'"
            );
        }
    }

    #[test]
    fn encode_decode_round_trips_exactly() {
        let events = vec![
            opened_event(),
            SessionEvent::SessionCancelled {
                session_id: SessionId::new("cancel-me").expect("valid session ID"),
            },
            SessionEvent::AssignmentAdmitted {
                session_id: SessionId::new("s1").expect("valid session ID"),
                invocation: InvocationId::new("invocation-1").expect("valid invocation ID"),
                assignment: assignment(),
            },
            SessionEvent::InvocationStartAttempted {
                session_id: SessionId::new("s1").expect("valid session ID"),
                invocation: InvocationId::new("invocation-1").expect("valid invocation ID"),
            },
            SessionEvent::InvocationStarted {
                session_id: SessionId::new("s1").expect("valid session ID"),
                invocation: InvocationId::new("invocation-1").expect("valid invocation ID"),
            },
            SessionEvent::InvocationUncertain {
                session_id: SessionId::new("s1").expect("valid session ID"),
                invocation: InvocationId::new("invocation-2").expect("valid invocation ID"),
                cause: UncertaintyCause::BoundedWaitExpired,
            },
            SessionEvent::InvocationUncertain {
                session_id: SessionId::new("s1").expect("valid session ID"),
                invocation: InvocationId::new("invocation-3").expect("valid invocation ID"),
                cause: UncertaintyCause::StartOutcomeUnknown,
            },
            SessionEvent::InvocationFailedAtStart {
                session_id: SessionId::new("s1").expect("valid session ID"),
                invocation: InvocationId::new("invocation-4").expect("valid invocation ID"),
                class: ErrorClass::new("adapter_unavailable").expect("valid error class"),
            },
            SessionEvent::EffectEvidenceRecorded {
                session_id: SessionId::new("s1").expect("valid session ID"),
                invocation: InvocationId::new("invocation-5").expect("valid invocation ID"),
            },
            SessionEvent::InvocationCancellationRequested {
                session_id: SessionId::new("s1").expect("valid session ID"),
                invocation: InvocationId::new("invocation-1").expect("valid invocation ID"),
            },
            SessionEvent::InvocationObserved {
                session_id: SessionId::new("s1").expect("valid session ID"),
                invocation: InvocationId::new("invocation-1").expect("valid invocation ID"),
                termination: Termination::Failed {
                    class: ErrorClass::new("provider_overloaded").expect("valid error class"),
                },
                reported_settings: Some(settings(&[("effort", "low")])),
                usage: ObservedUsage::unknown()
                    .with_turns(3)
                    .with_wall_clock(Duration::from_millis(1200)),
            },
            SessionEvent::InvocationObserved {
                session_id: SessionId::new("s1").expect("valid session ID"),
                invocation: InvocationId::new("invocation-2").expect("valid invocation ID"),
                termination: Termination::TimedOut,
                reported_settings: None,
                usage: ObservedUsage::unknown(),
            },
            SessionEvent::InvocationAccounted {
                session_id: SessionId::new("s1").expect("valid session ID"),
                invocation: InvocationId::new("invocation-1").expect("valid invocation ID"),
                usage: ObservedUsage::unknown().with_turns(3),
                reservation: ResourceAmount::new(5),
            },
            SessionEvent::CriterionEvaluated {
                session_id: SessionId::new("s1").expect("valid session ID"),
                invocation: InvocationId::new("invocation-1").expect("valid invocation ID"),
                evidence: Evidence::command(
                    Check::command(
                        vec![CriterionId::new("c1").expect("valid criterion ID")],
                        "echo ok",
                    )
                    .expect("valid check"),
                    "/workspace",
                    0,
                    14,
                    vec![
                        VerifierDigest::new("/bin/sh", "a".repeat(64))
                            .expect("valid verifier digest"),
                    ],
                )
                .expect("valid evidence"),
            },
            SessionEvent::CriterionEvaluated {
                session_id: SessionId::new("s1").expect("valid session ID"),
                invocation: InvocationId::new("invocation-1").expect("valid invocation ID"),
                evidence: Evidence::exact_bytes(
                    Check::exact_bytes(
                        vec![CriterionId::new("c2").expect("valid criterion ID")],
                        "missing.txt",
                        Vec::new(),
                    )
                    .expect("valid check"),
                    "/workspace",
                    2,
                    EvidenceFile::new("missing.txt", None).expect("valid missing evidence file"),
                )
                .expect("valid evidence"),
            },
            SessionEvent::CriterionEvaluated {
                session_id: SessionId::new("s1").expect("valid session ID"),
                invocation: InvocationId::new("invocation-1").expect("valid invocation ID"),
                evidence: Evidence::exact_bytes(
                    Check::exact_bytes(
                        vec![CriterionId::new("c2").expect("valid criterion ID")],
                        "result.txt",
                        b"exact".to_vec(),
                    )
                    .expect("valid check"),
                    "/workspace",
                    2,
                    EvidenceFile::new("result.txt", Some(b"exact".to_vec()))
                        .expect("valid evidence file"),
                )
                .expect("valid evidence"),
            },
        ];
        for event in events {
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
        assert_eq!(
            payload,
            br#"{"type":"session_cancelled","session_id":"s1"}"#
        );
        let again = encode_event(&SessionEvent::SessionCancelled {
            session_id: SessionId::new("s1").expect("valid session ID"),
        })
        .expect("payload encodes");
        assert_eq!(payload, again);

        let admitted = encode_event(&SessionEvent::AssignmentAdmitted {
            session_id: SessionId::new("s1").expect("valid session ID"),
            invocation: InvocationId::new("invocation-1").expect("valid invocation ID"),
            assignment: assignment(),
        })
        .expect("payload encodes");
        assert_eq!(
            admitted,
            br#"{"type":"assignment_admitted","session_id":"s1","invocation":"invocation-1","assignment":{"invocation":"invocation-1","agent":"claude-opus-5","role":"implementer","requested_settings":[{"key":"effort","value":"high"},{"key":"thinking","value":"on"}],"sent_settings":[{"key":"effort","value":"high"}],"allowance":{"reservation":5,"reservation_purpose":"coordination","limits":{"max_turns":12,"max_output_chars":4096,"max_wall_clock_ms":90000}},"grant":{"id":"grant-1","invocation":"invocation-1","reservation":5,"reservation_purpose":"coordination"},"workspace_accesses":[{"scope":"session-primary","operations":["read","write"]},{"scope":"session-review","operations":["read"]}]}}"#
        );

        let observed = encode_event(&SessionEvent::InvocationObserved {
            session_id: SessionId::new("s1").expect("valid session ID"),
            invocation: InvocationId::new("invocation-2").expect("valid invocation ID"),
            termination: Termination::TimedOut,
            reported_settings: None,
            usage: ObservedUsage::unknown(),
        })
        .expect("payload encodes");
        assert_eq!(
            observed,
            br#"{"type":"invocation_observed","session_id":"s1","invocation":"invocation-2","termination":{"outcome":"timed_out"},"reported_settings":null,"usage":{"turns":null,"output_chars":null,"wall_clock_ms":null}}"#
        );
        // The unknown report is an explicit null, not an omitted key.
        assert!(String::from_utf8_lossy(&observed).contains("\"reported_settings\":null"));

        let uncertain = encode_event(&SessionEvent::InvocationUncertain {
            session_id: SessionId::new("s1").expect("valid session ID"),
            invocation: InvocationId::new("invocation-2").expect("valid invocation ID"),
            cause: UncertaintyCause::StartOutcomeUnknown,
        })
        .expect("payload encodes");
        assert_eq!(
            uncertain,
            br#"{"type":"invocation_uncertain","session_id":"s1","invocation":"invocation-2","cause":"start_outcome_unknown"}"#
        );
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
    fn decode_rejects_nested_duplicate_field() {
        let payload = br#"{"type":"session_opened","session_id":"s1","task":{"id":"t","goal":{"request":"g","request":"g"},"acceptance_contract":{"criteria":[{"id":"c","description":"d"}]},"constraints":{"conditions":[]}}}"#;
        let error = decode_payload(payload).expect_err("nested duplicate should be rejected");
        assert!(
            error.contains("duplicate field"),
            "unexpected error: {error}"
        );
        assert!(error.contains("request"), "unexpected error: {error}");
    }

    #[test]
    fn decode_rejects_termination_error_class_mismatch() {
        for outcome in ["completed", "cancelled", "timed_out"] {
            let valid = valid_observed_payload(outcome);
            decode_json_payload(&valid).expect("non-error termination without error class decodes");

            let mut with_error_class = valid;
            with_error_class["termination"]["error_class"] = json!("provider_error");
            assert_json_payload_error(&with_error_class, &["termination.error_class", outcome]);
        }

        let mut with_null_error_class = valid_observed_payload("completed");
        with_null_error_class["termination"]["error_class"] = Value::Null;
        assert_json_payload_error(
            &with_null_error_class,
            &["termination.error_class", "completed"],
        );

        let mut failed_without_error_class = valid_observed_payload("failed");
        assert_json_payload_error(
            &failed_without_error_class,
            &["missing required field", "termination.error_class"],
        );

        failed_without_error_class["termination"]["error_class"] = json!("provider_error");
        decode_json_payload(&failed_without_error_class)
            .expect("failed termination with error class decodes");

        failed_without_error_class["termination"]["error_class"] = Value::Null;
        assert_json_payload_error(
            &failed_without_error_class,
            &["termination.error_class", "must not be null"],
        );
    }

    #[test]
    fn decode_rejects_top_level_error_class_on_non_failure_execution_forms() {
        for event_type in ["invocation_started", "invocation_cancellation_requested"] {
            let payload = json!({
                "type": event_type,
                "session_id": "s1",
                "invocation": "invocation-1",
                "error_class": "provider_error"
            });
            assert_json_payload_error(&payload, &["unknown field 'error_class'", "payload type"]);
        }
    }

    #[test]
    fn decode_rejects_inconsistent_assignment_facts() {
        let valid = valid_assignment_payload();
        decode_json_payload(&valid).expect("consistent admission decodes");

        let mut event_invocation_mismatch = valid.clone();
        event_invocation_mismatch["invocation"] = json!("invocation-9");
        assert_json_payload_error(
            &event_invocation_mismatch,
            &["assignment invocation does not match the event invocation"],
        );

        let mut grant_invocation_mismatch = valid.clone();
        grant_invocation_mismatch["assignment"]["grant"]["invocation"] = json!("invocation-9");
        assert_json_payload_error(
            &grant_invocation_mismatch,
            &["grant invocation does not match the assignment invocation"],
        );

        let mut grant_reservation_mismatch = valid.clone();
        grant_reservation_mismatch["assignment"]["grant"]["reservation"] = json!(2);
        assert_json_payload_error(
            &grant_reservation_mismatch,
            &["grant reservation does not match the allowance"],
        );

        let mut grant_purpose_mismatch = valid.clone();
        grant_purpose_mismatch["assignment"]["grant"]["reservation_purpose"] =
            json!("verification");
        assert_json_payload_error(
            &grant_purpose_mismatch,
            &["grant reservation purpose does not match the allowance"],
        );
    }

    #[test]
    fn malformed_assignment_cases_reach_the_intended_validation() {
        let valid = valid_assignment_payload();

        let mut missing_sent_settings = valid.clone();
        missing_sent_settings["assignment"]
            .as_object_mut()
            .expect("assignment is an object")
            .remove("sent_settings");
        assert_json_payload_error(&missing_sent_settings, &["missing field", "sent_settings"]);

        let mut zero_allowance_reservation = valid.clone();
        zero_allowance_reservation["assignment"]["allowance"]["reservation"] = json!(0);
        zero_allowance_reservation["assignment"]["grant"]["reservation"] = json!(0);
        assert_json_payload_error(
            &zero_allowance_reservation,
            &["allowance is not valid", "allowance reservation"],
        );

        let mut duplicate_setting = valid;
        duplicate_setting["assignment"]["requested_settings"] = json!([
            {"key": "effort", "value": "high"},
            {"key": "effort", "value": "low"}
        ]);
        assert_json_payload_error(&duplicate_setting, &["duplicate setting key", "effort"]);
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
            // Execution forms.
            (
                "invocation field on cancelled",
                br#"{"type":"session_cancelled","session_id":"s1","invocation":"invocation-1"}"#.to_vec(),
            ),
            (
                "error class on start attempted",
                br#"{"type":"invocation_start_attempted","session_id":"s1","invocation":"invocation-1","error_class":"c"}"#.to_vec(),
            ),
            (
                "missing assignment on admitted",
                br#"{"type":"assignment_admitted","session_id":"s1","invocation":"invocation-1"}"#.to_vec(),
            ),
            (
                "null usage on observed",
                br#"{"type":"invocation_observed","session_id":"s1","invocation":"invocation-1","termination":{"outcome":"completed"},"reported_settings":null,"usage":null}"#.to_vec(),
            ),
            (
                "absent reported settings on observed",
                br#"{"type":"invocation_observed","session_id":"s1","invocation":"invocation-1","termination":{"outcome":"completed"},"usage":{"turns":null,"output_chars":null,"wall_clock_ms":null}}"#.to_vec(),
            ),
            (
                "unknown termination outcome",
                br#"{"type":"invocation_observed","session_id":"s1","invocation":"invocation-1","termination":{"outcome":"vanished"},"reported_settings":null,"usage":{"turns":null,"output_chars":null,"wall_clock_ms":null}}"#.to_vec(),
            ),
            (
                "usage missing component",
                br#"{"type":"invocation_accounted","session_id":"s1","invocation":"invocation-1","usage":{"turns":1,"output_chars":2},"reservation":5}"#.to_vec(),
            ),
            (
                "reservation on started",
                br#"{"type":"invocation_started","session_id":"s1","invocation":"invocation-1","reservation":5}"#.to_vec(),
            ),
            (
                "unknown uncertainty cause",
                br#"{"type":"invocation_uncertain","session_id":"s1","invocation":"invocation-1","cause":"mood_based"}"#.to_vec(),
            ),
            (
                "missing uncertainty cause",
                br#"{"type":"invocation_uncertain","session_id":"s1","invocation":"invocation-1"}"#.to_vec(),
            ),
            (
                "missing criterion evidence",
                br#"{"type":"criterion_evaluated","session_id":"s1","invocation":"invocation-1"}"#.to_vec(),
            ),
            (
                "command evidence without exit code",
                br#"{"type":"criterion_evaluated","session_id":"s1","invocation":"invocation-1","evidence":{"criteria":["c1"],"workspace":"/workspace","method":{"kind":"command","command":"true"},"exit_code":null,"elapsed_ms":1,"files":[],"verifiers":[{"path":"/bin/sh","sha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}]}}"#.to_vec(),
            ),
            (
                "command evidence with absent exit-code field",
                br#"{"type":"criterion_evaluated","session_id":"s1","invocation":"invocation-1","evidence":{"criteria":["c1"],"workspace":"/workspace","method":{"kind":"command","command":"true"},"elapsed_ms":1,"files":[],"verifiers":[{"path":"/bin/sh","sha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}]}}"#.to_vec(),
            ),
            (
                "exact-byte evidence with absent bytes field",
                br#"{"type":"criterion_evaluated","session_id":"s1","invocation":"invocation-1","evidence":{"criteria":["c1"],"workspace":"/workspace","method":{"kind":"exact_bytes","path":"missing.txt","expected":[]},"exit_code":null,"elapsed_ms":1,"files":[{"path":"missing.txt","sha256":null}],"verifiers":[]}}"#.to_vec(),
            ),
            (
                "exact-byte evidence with absent digest field",
                br#"{"type":"criterion_evaluated","session_id":"s1","invocation":"invocation-1","evidence":{"criteria":["c1"],"workspace":"/workspace","method":{"kind":"exact_bytes","path":"missing.txt","expected":[]},"exit_code":null,"elapsed_ms":1,"files":[{"path":"missing.txt","bytes":null}],"verifiers":[]}}"#.to_vec(),
            ),
            (
                "evidence on another event type",
                br#"{"type":"session_cancelled","session_id":"s1","evidence":null}"#.to_vec(),
            ),
        ];
        for (name, payload) in cases {
            assert!(
                decode_payload(&payload).is_err(),
                "case '{name}' should fail"
            );
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
        let payload = format!("{{\"type\":\"session_cancelled\",\"session_id\":\"{oversized}\"}}");
        assert!(decode_payload(payload.as_bytes()).is_err());
    }
}
