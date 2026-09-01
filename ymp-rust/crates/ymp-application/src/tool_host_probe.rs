use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::{ErrorKind, Read, Write};
use std::path::{Path, PathBuf};
use thiserror::Error;
use uuid::Uuid;
use ymp_domain::digest_bytes;
use ymp_runtime_api::{
    CancellationToken, ProbeTransportIdentity, ProviderRequestState, TOOL_HOST_PROBE_ENVIRONMENT,
    TOOL_HOST_PROBE_INTERNAL_ARGUMENTS, TOOL_HOST_PROBE_INTERNAL_SUBCOMMAND,
    TOOL_HOST_PROBE_MCP_PROTOCOL_VERSION, TOOL_HOST_PROBE_SCHEMA_VERSION,
    TOOL_HOST_PROBE_SERVER_VERSION, TOOL_HOST_PROBE_WORKSPACE_SERVER, ToolHostProbeCost,
    ToolHostProbeCostAvailability, ToolHostProbeError, ToolHostProbeFailureEvidence,
    ToolHostProbeFailurePhase, ToolHostProbeFailureStages, ToolHostProbeRequest,
    ToolHostProbeResourceVector, ToolHostProbeRuntimeIdentity, ToolHostProbeTerminal,
    ToolHostProbeTool, ToolHostProbeTrace, ToolHostProbeTrust, Usage, evidence_digest,
    probe_transport_digest, tool_host_probe_tool_schema_digest,
};
use ymp_storage::ObjectStoreError;

use crate::Application;

pub const ATTESTED_TOOL_HOST_PROBE_SCHEMA_VERSION: u32 = 3;
pub const TOOL_HOST_PROBE_FAILURE_SCHEMA_VERSION: u32 = 1;
pub const TOOL_HOST_PROBE_HANDLE_EXPORT: &str = "exports/tool-host-probe.handle.json";

const TOOL_HOST_PROBES_DIRECTORY: &str = "runtime-evidence/tool-host-probes";
const RESERVATION_FILE: &str = "reservation.json";
const FAILURE_FILE: &str = "failure.json";
const ATTESTATION_REFERENCE_FILE: &str = "attestation.ref";
const WORKSPACE_DIRECTORY: &str = "workspace";

#[cfg(test)]
thread_local! {
    static INJECT_ATTESTATION_WRITE_FAILURE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static FAILURE_DURABILITY_STAGES: std::cell::Cell<u8> = const { std::cell::Cell::new(0) };
}

/// The controller request has two phases. Application first creates it with private material and a
/// canonical root but no expected transport; the trusted foreground then consumes it to bind the
/// prelaunch measurement. An unbound value is rejected before reservation or execution.
#[derive(Clone, Debug)]
pub struct ControllerToolHostProbeRequest {
    admission_manifest_digest: String,
    expected_runtime: Option<ToolHostProbeRuntimeIdentity>,
    expected_transport: Option<ProbeTransportIdentity>,
    deadline_ms: u64,
    resource_reservation: ToolHostProbeResourceVector,
    material: ProbeMaterial,
    workspace: PathBuf,
}

impl ControllerToolHostProbeRequest {
    pub fn bind_expected_transport(
        mut self,
        expected_runtime: ToolHostProbeRuntimeIdentity,
        expected_transport: ProbeTransportIdentity,
    ) -> Result<Self, ToolHostProbeAttestationError> {
        validate_expected_transport(&expected_runtime, &expected_transport, &self.workspace)?;
        self.expected_runtime = Some(expected_runtime);
        self.expected_transport = Some(expected_transport);
        Ok(self)
    }

    pub fn admission_manifest_digest(&self) -> &str {
        &self.admission_manifest_digest
    }

    pub fn expected_runtime(&self) -> Option<&ToolHostProbeRuntimeIdentity> {
        self.expected_runtime.as_ref()
    }

    pub fn expected_transport(&self) -> Option<&ProbeTransportIdentity> {
        self.expected_transport.as_ref()
    }

    pub fn deadline_ms(&self) -> u64 {
        self.deadline_ms
    }

    pub fn resource_reservation(&self) -> &ToolHostProbeResourceVector {
        &self.resource_reservation
    }

    pub fn workspace_root(&self) -> &Path {
        &self.workspace
    }

    pub fn relative_path(&self) -> &Path {
        &self.material.relative_path
    }

    pub fn nonce(&self) -> &str {
        &self.material.nonce
    }
}

/// An untrusted locator. Deserializing or copying it grants no authority: the matching
/// Application must still verify the private reservation, reference and object.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AttestedToolHostProbeHandle {
    schema_version: u32,
    store_identity: String,
    probe_id: String,
    record_digest: String,
}

impl AttestedToolHostProbeHandle {
    pub fn schema_version(&self) -> u32 {
        self.schema_version
    }

    pub fn store_identity(&self) -> &str {
        &self.store_identity
    }

    pub fn probe_id(&self) -> &str {
        &self.probe_id
    }

    pub fn record_digest(&self) -> &str {
        &self.record_digest
    }
}

/// Immutable, read-only failure evidence recovered through the owning Application store. It is a
/// diagnostic record only and carries no attestation, retry, refund or readiness authority.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ToolHostProbeFailureRecord {
    schema_version: u32,
    store_identity: String,
    run_id: String,
    admission_manifest_digest: String,
    probe_id: String,
    invocation_id: String,
    nonce_digest: String,
    relative_path: PathBuf,
    runtime: ToolHostProbeRuntimeIdentity,
    probe_transport: ProbeTransportIdentity,
    probe_transport_digest: String,
    reservation_digest: String,
    resource_reservation: ToolHostProbeResourceVector,
    replay_key: String,
    evidence: ToolHostProbeFailureEvidence,
    record_digest: String,
}

impl ToolHostProbeFailureRecord {
    pub fn schema_version(&self) -> u32 {
        self.schema_version
    }

    pub fn store_identity(&self) -> &str {
        &self.store_identity
    }

    pub fn run_id(&self) -> &str {
        &self.run_id
    }

    pub fn admission_manifest_digest(&self) -> &str {
        &self.admission_manifest_digest
    }

    pub fn probe_id(&self) -> &str {
        &self.probe_id
    }

    pub fn invocation_id(&self) -> &str {
        &self.invocation_id
    }

    pub fn nonce_digest(&self) -> &str {
        &self.nonce_digest
    }

    pub fn relative_path(&self) -> &Path {
        &self.relative_path
    }

    pub fn runtime(&self) -> &ToolHostProbeRuntimeIdentity {
        &self.runtime
    }

    pub fn probe_transport(&self) -> &ProbeTransportIdentity {
        &self.probe_transport
    }

    pub fn probe_transport_digest(&self) -> &str {
        &self.probe_transport_digest
    }

    pub fn reservation_digest(&self) -> &str {
        &self.reservation_digest
    }

    pub fn resource_reservation(&self) -> &ToolHostProbeResourceVector {
        &self.resource_reservation
    }

    pub fn replay_key(&self) -> &str {
        &self.replay_key
    }

    pub fn evidence(&self) -> &ToolHostProbeFailureEvidence {
        &self.evidence
    }

    pub fn record_digest(&self) -> &str {
        &self.record_digest
    }
}

/// Controller-attested probe evidence.
///
/// Fields and construction are private, and only serialization is implemented. Raw trace or model
/// JSON therefore cannot be turned into this value:
///
/// ```compile_fail
/// use ymp_application::AttestedToolHostProbe;
/// let _: AttestedToolHostProbe = serde_json::from_str("{}").unwrap();
/// ```
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct AttestedToolHostProbe {
    schema_version: u32,
    store_identity: String,
    run_id: String,
    admission_manifest_digest: String,
    probe_id: String,
    invocation_id: String,
    nonce_digest: String,
    readback_digest: String,
    readback_bytes: u64,
    prelaunch_destination_absent: bool,
    relative_path: PathBuf,
    trace: ToolHostProbeTrace,
    trace_digest: String,
    runtime: ToolHostProbeRuntimeIdentity,
    probe_transport: ProbeTransportIdentity,
    probe_transport_digest: String,
    reservation_digest: String,
    resource_reservation: ToolHostProbeResourceVector,
    charged: ToolHostProbeResourceVector,
    usage: Usage,
    wall_time_ms: u64,
    cost: ToolHostProbeCost,
    terminal: ToolHostProbeTerminal,
    replay_key: String,
}

impl AttestedToolHostProbe {
    pub fn schema_version(&self) -> u32 {
        self.schema_version
    }

    pub fn store_identity(&self) -> &str {
        &self.store_identity
    }

    pub fn run_id(&self) -> &str {
        &self.run_id
    }

    pub fn admission_manifest_digest(&self) -> &str {
        &self.admission_manifest_digest
    }

    pub fn probe_id(&self) -> &str {
        &self.probe_id
    }

    pub fn invocation_id(&self) -> &str {
        &self.invocation_id
    }

    pub fn nonce_digest(&self) -> &str {
        &self.nonce_digest
    }

    pub fn readback_digest(&self) -> &str {
        &self.readback_digest
    }

    pub fn readback_bytes(&self) -> u64 {
        self.readback_bytes
    }

    pub fn prelaunch_destination_absent(&self) -> bool {
        self.prelaunch_destination_absent
    }

    pub fn relative_path(&self) -> &Path {
        &self.relative_path
    }

    pub fn trace(&self) -> &ToolHostProbeTrace {
        &self.trace
    }

    pub fn trace_digest(&self) -> &str {
        &self.trace_digest
    }

    pub fn runtime(&self) -> &ToolHostProbeRuntimeIdentity {
        &self.runtime
    }

    pub fn probe_transport(&self) -> &ProbeTransportIdentity {
        &self.probe_transport
    }

    pub fn probe_transport_digest(&self) -> &str {
        &self.probe_transport_digest
    }

    pub fn reservation_digest(&self) -> &str {
        &self.reservation_digest
    }

    pub fn resource_reservation(&self) -> &ToolHostProbeResourceVector {
        &self.resource_reservation
    }

    pub fn charged(&self) -> &ToolHostProbeResourceVector {
        &self.charged
    }

    pub fn usage(&self) -> &Usage {
        &self.usage
    }

    pub fn wall_time_ms(&self) -> u64 {
        self.wall_time_ms
    }

    pub fn cost(&self) -> &ToolHostProbeCost {
        &self.cost
    }

    pub fn terminal(&self) -> ToolHostProbeTerminal {
        self.terminal
    }

    pub fn replay_key(&self) -> &str {
        &self.replay_key
    }
}

#[derive(Debug, Error)]
pub enum ToolHostProbeAttestationError {
    #[error("tool-host probe I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("tool-host probe JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    ObjectStore(#[from] ObjectStoreError),
    #[error(transparent)]
    Runtime(#[from] ToolHostProbeError),
    #[error("invalid controller tool-host probe request field {field}")]
    InvalidRequest { field: &'static str },
    #[error("tool-host probe destination already existed before launch")]
    DestinationAlreadyExists,
    #[error("tool-host probe reservation is already spent")]
    ReservationSpent,
    #[error("tool-host probe reservation is missing")]
    ReservationMissing,
    #[error("tool-host probe reservation is corrupt or mismatched: {detail}")]
    ReservationInvalid { detail: String },
    #[error("tool-host probe failure record is missing")]
    FailureMissing,
    #[error("tool-host probe failure record is corrupt or mismatched: {detail}")]
    FailureInvalid { detail: String },
    #[error("tool-host probe cannot contain both failure evidence and an attestation")]
    FailureAttestationConflict,
    #[error("tool-host probe could not persist failure evidence after {original}: {persistence}")]
    FailurePersistence {
        original: Box<ToolHostProbeAttestationError>,
        persistence: String,
    },
    #[error("tool-host probe trace differs from controller state in {field}")]
    TraceMismatch { field: &'static str },
    #[error("tool-host probe destination is missing after the runtime trace")]
    DestinationMissing,
    #[error("tool-host probe workspace contains an unexpected effect")]
    UnexpectedWorkspaceEffect,
    #[error("tool-host probe controller read-back differs from its nonce")]
    ReadbackMismatch,
    #[error("tool-host probe charged vector exceeds its reservation in {field}")]
    ReservationExceeded { field: &'static str },
    #[error("tool-host probe attestation reference already exists")]
    AttestationReferenceAlreadyExists,
    #[error("tool-host probe attestation reference is missing")]
    AttestationReferenceMissing,
    #[error("tool-host probe attestation reference is corrupt or mismatched: {detail}")]
    AttestationReferenceInvalid { detail: String },
    #[error("tool-host probe handle is invalid or belongs to another store: {detail}")]
    HandleInvalid { detail: String },
    #[error("tool-host probe attestation object is invalid: {detail}")]
    AttestationObjectInvalid { detail: String },
    #[error("tool-host probe handle export already exists: {0}")]
    HandleExportAlreadyExists(PathBuf),
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct ProbeReservation {
    schema_version: u32,
    store_identity: String,
    run_id: String,
    admission_manifest_digest: String,
    probe_id: String,
    invocation_id: String,
    nonce_digest: String,
    relative_path: PathBuf,
    expected_runtime: ToolHostProbeRuntimeIdentity,
    expected_transport: ProbeTransportIdentity,
    probe_transport_digest: String,
    resource_reservation: ToolHostProbeResourceVector,
    replay_key: String,
}

#[derive(Clone, Debug, Serialize)]
struct ReplayKeyMaterial<'a> {
    schema_version: u32,
    store_identity: &'a str,
    run_id: &'a str,
    admission_manifest_digest: &'a str,
    probe_id: &'a str,
    invocation_id: &'a str,
    nonce_digest: &'a str,
    relative_path: &'a Path,
    expected_runtime: &'a ToolHostProbeRuntimeIdentity,
    expected_transport: &'a ProbeTransportIdentity,
    probe_transport_digest: &'a str,
    resource_reservation: &'a ToolHostProbeResourceVector,
}

#[derive(Clone, Copy)]
struct ExpectedProbeBinding<'a> {
    runtime: &'a ToolHostProbeRuntimeIdentity,
    transport: &'a ProbeTransportIdentity,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct AttestationReference {
    schema_version: u32,
    store_identity: String,
    run_id: String,
    probe_id: String,
    reservation_digest: String,
    object_digest: String,
    record_digest: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct StoredToolHostProbeFailureRecord {
    schema_version: u32,
    store_identity: String,
    run_id: String,
    admission_manifest_digest: String,
    probe_id: String,
    invocation_id: String,
    nonce_digest: String,
    relative_path: PathBuf,
    runtime: ToolHostProbeRuntimeIdentity,
    probe_transport: ProbeTransportIdentity,
    probe_transport_digest: String,
    reservation_digest: String,
    resource_reservation: ToolHostProbeResourceVector,
    replay_key: String,
    evidence: ToolHostProbeFailureEvidence,
    record_digest: String,
}

impl From<StoredToolHostProbeFailureRecord> for ToolHostProbeFailureRecord {
    fn from(stored: StoredToolHostProbeFailureRecord) -> Self {
        Self {
            schema_version: stored.schema_version,
            store_identity: stored.store_identity,
            run_id: stored.run_id,
            admission_manifest_digest: stored.admission_manifest_digest,
            probe_id: stored.probe_id,
            invocation_id: stored.invocation_id,
            nonce_digest: stored.nonce_digest,
            relative_path: stored.relative_path,
            runtime: stored.runtime,
            probe_transport: stored.probe_transport,
            probe_transport_digest: stored.probe_transport_digest,
            reservation_digest: stored.reservation_digest,
            resource_reservation: stored.resource_reservation,
            replay_key: stored.replay_key,
            evidence: stored.evidence,
            record_digest: stored.record_digest,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct StoredAttestedToolHostProbe {
    schema_version: u32,
    store_identity: String,
    run_id: String,
    admission_manifest_digest: String,
    probe_id: String,
    invocation_id: String,
    nonce_digest: String,
    readback_digest: String,
    readback_bytes: u64,
    prelaunch_destination_absent: bool,
    relative_path: PathBuf,
    trace: ToolHostProbeTrace,
    trace_digest: String,
    runtime: ToolHostProbeRuntimeIdentity,
    probe_transport: ProbeTransportIdentity,
    probe_transport_digest: String,
    reservation_digest: String,
    resource_reservation: ToolHostProbeResourceVector,
    charged: ToolHostProbeResourceVector,
    usage: Usage,
    wall_time_ms: u64,
    cost: ToolHostProbeCost,
    terminal: ToolHostProbeTerminal,
    replay_key: String,
}

impl From<StoredAttestedToolHostProbe> for AttestedToolHostProbe {
    fn from(stored: StoredAttestedToolHostProbe) -> Self {
        Self {
            schema_version: stored.schema_version,
            store_identity: stored.store_identity,
            run_id: stored.run_id,
            admission_manifest_digest: stored.admission_manifest_digest,
            probe_id: stored.probe_id,
            invocation_id: stored.invocation_id,
            nonce_digest: stored.nonce_digest,
            readback_digest: stored.readback_digest,
            readback_bytes: stored.readback_bytes,
            prelaunch_destination_absent: stored.prelaunch_destination_absent,
            relative_path: stored.relative_path,
            trace: stored.trace,
            trace_digest: stored.trace_digest,
            runtime: stored.runtime,
            probe_transport: stored.probe_transport,
            probe_transport_digest: stored.probe_transport_digest,
            reservation_digest: stored.reservation_digest,
            resource_reservation: stored.resource_reservation,
            charged: stored.charged,
            usage: stored.usage,
            wall_time_ms: stored.wall_time_ms,
            cost: stored.cost,
            terminal: stored.terminal,
            replay_key: stored.replay_key,
        }
    }
}

#[derive(Clone, Debug)]
struct ProbeMaterial {
    probe_id: String,
    invocation_id: String,
    nonce: String,
    relative_path: PathBuf,
}

impl ProbeMaterial {
    fn random() -> Self {
        let probe_uuid = Uuid::new_v4().simple().to_string();
        let invocation_uuid = Uuid::new_v4().simple().to_string();
        let path_uuid = Uuid::new_v4().simple().to_string();
        Self {
            probe_id: format!("probe-{probe_uuid}"),
            invocation_id: format!("invocation-{invocation_uuid}"),
            nonce: Uuid::new_v4().simple().to_string(),
            relative_path: PathBuf::from(format!("tool-host-probe-{path_uuid}.nonce")),
        }
    }
}

impl Application {
    /// Phase one creates the only workspace root the probe may use. No reservation is written and
    /// no executor is reachable until the trusted foreground binds its prelaunch measurement.
    pub fn prepare_controller_tool_host_probe(
        &self,
        admission_manifest_digest: impl Into<String>,
        deadline_ms: u64,
        resource_reservation: ToolHostProbeResourceVector,
    ) -> Result<ControllerToolHostProbeRequest, ToolHostProbeAttestationError> {
        self.prepare_controller_tool_host_probe_with_material(
            admission_manifest_digest.into(),
            deadline_ms,
            resource_reservation,
            ProbeMaterial::random(),
        )
    }

    fn prepare_controller_tool_host_probe_with_material(
        &self,
        admission_manifest_digest: String,
        deadline_ms: u64,
        resource_reservation: ToolHostProbeResourceVector,
        material: ProbeMaterial,
    ) -> Result<ControllerToolHostProbeRequest, ToolHostProbeAttestationError> {
        validate_controller_request_base(
            &admission_manifest_digest,
            deadline_ms,
            &resource_reservation,
        )?;
        validate_probe_material(&material)?;
        let probe_directory = self
            .data_root
            .join(TOOL_HOST_PROBES_DIRECTORY)
            .join(&material.probe_id);
        if probe_directory.join(RESERVATION_FILE).exists() {
            return Err(ToolHostProbeAttestationError::ReservationSpent);
        }
        let workspace = canonical_probe_workspace(&self.data_root, &material.probe_id, true)?;
        let destination = workspace.join(&material.relative_path);
        match fs::symlink_metadata(&destination) {
            Ok(_) => return Err(ToolHostProbeAttestationError::DestinationAlreadyExists),
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        Ok(ControllerToolHostProbeRequest {
            admission_manifest_digest,
            expected_runtime: None,
            expected_transport: None,
            deadline_ms,
            resource_reservation,
            material,
            workspace,
        })
    }

    /// Spend one controller-owned reservation and produce a private attestation through the
    /// purpose-built runtime seam. The executor receives only the generated runtime request and
    /// the private workspace selected here.
    pub fn controller_tool_host_probe<F>(
        &mut self,
        request: ControllerToolHostProbeRequest,
        executor: F,
    ) -> Result<AttestedToolHostProbeHandle, ToolHostProbeAttestationError>
    where
        F: FnOnce(&Path, ToolHostProbeRequest) -> Result<ToolHostProbeTrace, ToolHostProbeError>,
    {
        validate_controller_request(&request)?;
        let expected_runtime = request
            .expected_runtime
            .clone()
            .ok_or_else(|| invalid_request("expected_runtime"))?;
        let expected_transport = request
            .expected_transport
            .clone()
            .ok_or_else(|| invalid_request("expected_transport"))?;
        let material = request.material.clone();
        validate_probe_material(&material)?;

        let store_identity = configured_store_identity(&self.data_root)?;
        let run_id = self.state.run_id.clone();
        let probe_directory = self
            .data_root
            .join(TOOL_HOST_PROBES_DIRECTORY)
            .join(&material.probe_id);
        let reservation_path = probe_directory.join(RESERVATION_FILE);
        if reservation_path.exists() {
            return Err(ToolHostProbeAttestationError::ReservationSpent);
        }

        let workspace = canonical_probe_workspace(&self.data_root, &material.probe_id, false)?;
        if workspace != request.workspace {
            return Err(invalid_request("workspace_root"));
        }
        let destination = workspace.join(&material.relative_path);
        match fs::symlink_metadata(&destination) {
            Ok(_) => return Err(ToolHostProbeAttestationError::DestinationAlreadyExists),
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }

        let nonce_digest = digest_bytes(material.nonce.as_bytes());
        let replay_key = replay_key(
            &store_identity,
            &run_id,
            &request.admission_manifest_digest,
            &material,
            &nonce_digest,
            ExpectedProbeBinding {
                runtime: &expected_runtime,
                transport: &expected_transport,
            },
            &request.resource_reservation,
        )?;
        let reservation = ProbeReservation {
            schema_version: ATTESTED_TOOL_HOST_PROBE_SCHEMA_VERSION,
            store_identity: store_identity.clone(),
            run_id: run_id.clone(),
            admission_manifest_digest: request.admission_manifest_digest.clone(),
            probe_id: material.probe_id.clone(),
            invocation_id: material.invocation_id.clone(),
            nonce_digest: nonce_digest.clone(),
            relative_path: material.relative_path.clone(),
            expected_runtime: expected_runtime.clone(),
            expected_transport: expected_transport.clone(),
            probe_transport_digest: expected_runtime.probe_transport_digest.clone(),
            resource_reservation: request.resource_reservation.clone(),
            replay_key: replay_key.clone(),
        };
        let reservation_bytes = canonical_json(&reservation)?;
        write_new_synced(&reservation_path, &reservation_bytes).map_err(|error| {
            if error.kind() == ErrorKind::AlreadyExists {
                ToolHostProbeAttestationError::ReservationSpent
            } else {
                error.into()
            }
        })?;
        let reservation_digest = digest_bytes(&reservation_bytes);

        let runtime_request = ToolHostProbeRequest {
            schema_version: TOOL_HOST_PROBE_SCHEMA_VERSION,
            probe_id: material.probe_id.clone(),
            invocation_id: material.invocation_id.clone(),
            nonce: material.nonce.clone(),
            workspace_path: material.relative_path.clone(),
            deadline_ms: request.deadline_ms,
            resource_reservation: request.resource_reservation.clone(),
            expected_runtime: expected_runtime.clone(),
            cancellation: CancellationToken::default(),
        };
        let mut failure_evidence = ToolHostProbeFailureEvidence {
            stages: ToolHostProbeFailureStages {
                controller_readback: Some(false),
                attestation_written: Some(false),
                handle_written: Some(false),
                ..ToolHostProbeFailureStages::default()
            },
            ..ToolHostProbeFailureEvidence::default()
        };
        let operation = (|| {
            let trace = match executor(&workspace, runtime_request) {
                Ok(trace) => trace,
                Err(error) => {
                    let (original, observed) = error.into_original_and_evidence();
                    if let Some(observed) = observed {
                        failure_evidence.merge_observed(observed);
                    }
                    if failure_evidence.phase == ToolHostProbeFailurePhase::Unknown {
                        failure_evidence.phase = ToolHostProbeFailurePhase::RuntimeProcess;
                    }
                    return Err(ToolHostProbeAttestationError::Runtime(original));
                }
            };
            failure_evidence.merge_observed(trace.failure_evidence.clone());
            failure_evidence.phase = ToolHostProbeFailurePhase::ControllerTraceValidation;
            validate_trace(&trace, &request, &material)?;

            failure_evidence.phase = ToolHostProbeFailurePhase::ControllerReadback;
            validate_workspace_effect(&workspace, &material.relative_path)?;
            let readback = read_controller_destination(&workspace, &material.relative_path)?;
            if readback != material.nonce.as_bytes() {
                return Err(ToolHostProbeAttestationError::ReadbackMismatch);
            }
            failure_evidence.stages.controller_readback = Some(true);

            let readback_digest = digest_bytes(&readback);
            let readback_bytes = u64::try_from(readback.len()).map_err(|_| {
                ToolHostProbeAttestationError::TraceMismatch {
                    field: "readback_bytes",
                }
            })?;
            let charged = charged_vector(&trace);
            ensure_charged_within(&charged, &request.resource_reservation)?;
            let trace_digest = digest_bytes(&canonical_json(&trace)?);
            failure_evidence.phase = ToolHostProbeFailurePhase::AttestationPersistence;
            injected_attestation_write_failure()?;
            let attestation = AttestedToolHostProbe {
                schema_version: ATTESTED_TOOL_HOST_PROBE_SCHEMA_VERSION,
                store_identity: store_identity.clone(),
                run_id: run_id.clone(),
                admission_manifest_digest: request.admission_manifest_digest.clone(),
                probe_id: material.probe_id.clone(),
                invocation_id: material.invocation_id.clone(),
                nonce_digest: nonce_digest.clone(),
                readback_digest,
                readback_bytes,
                prelaunch_destination_absent: true,
                relative_path: material.relative_path.clone(),
                trace: trace.clone(),
                trace_digest,
                runtime: trace.runtime.clone(),
                probe_transport: expected_transport.clone(),
                probe_transport_digest: trace.runtime.probe_transport_digest.clone(),
                reservation_digest: reservation_digest.clone(),
                resource_reservation: trace.resource_reservation.clone(),
                charged,
                usage: trace.usage.clone(),
                wall_time_ms: trace.usage.wall_time_ms,
                cost: trace.cost.clone(),
                terminal: trace.terminal,
                replay_key: replay_key.clone(),
            };
            let attestation_bytes = canonical_json(&attestation)?;
            let object_digest = self.object_store.put(&attestation_bytes)?;
            failure_evidence.stages.attestation_written = Some(true);

            let mut reference = AttestationReference {
                schema_version: ATTESTED_TOOL_HOST_PROBE_SCHEMA_VERSION,
                store_identity: store_identity.clone(),
                run_id: run_id.clone(),
                probe_id: material.probe_id.clone(),
                reservation_digest: reservation_digest.clone(),
                object_digest,
                record_digest: String::new(),
            };
            reference.record_digest = reference_digest(&reference)?;
            let reference_bytes = canonical_json(&reference)?;
            let reference_path = probe_directory.join(ATTESTATION_REFERENCE_FILE);
            write_new_synced(&reference_path, &reference_bytes).map_err(|error| {
                if error.kind() == ErrorKind::AlreadyExists {
                    ToolHostProbeAttestationError::AttestationReferenceAlreadyExists
                } else {
                    error.into()
                }
            })?;
            failure_evidence.stages.handle_written = Some(true);

            Ok(AttestedToolHostProbeHandle {
                schema_version: ATTESTED_TOOL_HOST_PROBE_SCHEMA_VERSION,
                store_identity: reference.store_identity,
                probe_id: reference.probe_id,
                record_digest: reference.record_digest,
            })
        })();

        match operation {
            Ok(handle) => Ok(handle),
            Err(original) => {
                let failure_path = probe_directory.join(FAILURE_FILE);
                match write_tool_host_probe_failure(
                    &failure_path,
                    &reservation,
                    &reservation_digest,
                    failure_evidence,
                ) {
                    Ok(()) => Err(original),
                    Err(persistence) => Err(ToolHostProbeAttestationError::FailurePersistence {
                        original: Box::new(original),
                        persistence: persistence.to_string(),
                    }),
                }
            }
        }
    }

    /// Resolve an untrusted handle only through this application's private store and verify every
    /// reservation, reference, object and controller binding before returning the opaque value.
    pub fn attested_tool_host_probe(
        &self,
        handle: &AttestedToolHostProbeHandle,
    ) -> Result<AttestedToolHostProbe, ToolHostProbeAttestationError> {
        if handle.schema_version != ATTESTED_TOOL_HOST_PROBE_SCHEMA_VERSION {
            return Err(handle_invalid("unsupported schema version"));
        }
        validate_generated_identifier("probe", &handle.probe_id)
            .map_err(|_| handle_invalid("invalid probe identifier"))?;
        if !is_sha256(&handle.store_identity) || !is_sha256(&handle.record_digest) {
            return Err(handle_invalid("non-canonical digest"));
        }
        let store_identity = configured_store_identity(&self.data_root)?;
        if handle.store_identity != store_identity {
            return Err(handle_invalid("store identity mismatch"));
        }

        let probe_directory = self
            .data_root
            .join(TOOL_HOST_PROBES_DIRECTORY)
            .join(&handle.probe_id);
        refuse_failure_attestation_conflict(&probe_directory)?;
        let reservation_path = probe_directory.join(RESERVATION_FILE);
        let reservation_bytes = read_required(
            &reservation_path,
            ToolHostProbeAttestationError::ReservationMissing,
        )?;
        let reservation: ProbeReservation =
            decode_canonical(&reservation_bytes).map_err(|error| {
                ToolHostProbeAttestationError::ReservationInvalid {
                    detail: error.to_string(),
                }
            })?;
        let workspace = canonical_probe_workspace(&self.data_root, &handle.probe_id, false)?;
        validate_reservation(
            &reservation,
            &store_identity,
            &self.state.run_id,
            &handle.probe_id,
            &workspace,
        )?;
        let reservation_digest = digest_bytes(&reservation_bytes);

        let reference_path = probe_directory.join(ATTESTATION_REFERENCE_FILE);
        let reference_bytes = read_required(
            &reference_path,
            ToolHostProbeAttestationError::AttestationReferenceMissing,
        )?;
        let reference: AttestationReference =
            decode_canonical(&reference_bytes).map_err(|error| {
                ToolHostProbeAttestationError::AttestationReferenceInvalid {
                    detail: error.to_string(),
                }
            })?;
        validate_reference(
            &reference,
            handle,
            &store_identity,
            &self.state.run_id,
            &reservation_digest,
        )?;

        let object_bytes = self.object_store.read(&reference.object_digest)?;
        let stored: StoredAttestedToolHostProbe =
            decode_canonical(&object_bytes).map_err(|error| {
                ToolHostProbeAttestationError::AttestationObjectInvalid {
                    detail: error.to_string(),
                }
            })?;
        let attestation = AttestedToolHostProbe::from(stored);
        validate_loaded_attestation(
            &attestation,
            &reservation,
            &reservation_digest,
            &reference,
            &workspace,
        )?;
        Ok(attestation)
    }

    /// Reload one immutable failed probe only through this application's private store. The probe
    /// identifier is a locator; every reservation, replay, store and digest binding is rechecked.
    pub fn tool_host_probe_failure(
        &self,
        probe_id: &str,
    ) -> Result<ToolHostProbeFailureRecord, ToolHostProbeAttestationError> {
        validate_generated_identifier("probe", probe_id)
            .map_err(|_| failure_invalid("invalid probe identifier"))?;
        let store_identity = configured_store_identity(&self.data_root)?;
        let probe_directory = self
            .data_root
            .join(TOOL_HOST_PROBES_DIRECTORY)
            .join(probe_id);
        refuse_failure_attestation_conflict(&probe_directory)?;

        let reservation_path = probe_directory.join(RESERVATION_FILE);
        let reservation_bytes = read_required(
            &reservation_path,
            ToolHostProbeAttestationError::ReservationMissing,
        )?;
        let reservation: ProbeReservation =
            decode_canonical(&reservation_bytes).map_err(|error| {
                ToolHostProbeAttestationError::ReservationInvalid {
                    detail: error.to_string(),
                }
            })?;
        let workspace = canonical_probe_workspace(&self.data_root, probe_id, false)?;
        validate_reservation(
            &reservation,
            &store_identity,
            &self.state.run_id,
            probe_id,
            &workspace,
        )?;
        let reservation_digest = digest_bytes(&reservation_bytes);

        let failure_bytes = read_required(
            &probe_directory.join(FAILURE_FILE),
            ToolHostProbeAttestationError::FailureMissing,
        )?;
        let stored: StoredToolHostProbeFailureRecord =
            decode_canonical(&failure_bytes).map_err(|error| failure_invalid(error.to_string()))?;
        let failure = ToolHostProbeFailureRecord::from(stored);
        validate_loaded_failure(&failure, &reservation, &reservation_digest)?;
        Ok(failure)
    }

    /// The one controller-owned export location. No caller-selected path is accepted.
    pub fn attested_tool_host_probe_handle_export_path(&self) -> PathBuf {
        self.data_root.join(TOOL_HOST_PROBE_HANDLE_EXPORT)
    }

    /// Persist a verified locator after its private attestation is durable. Existing bytes are
    /// never overwritten.
    pub fn export_attested_tool_host_probe_handle(
        &self,
        handle: &AttestedToolHostProbeHandle,
    ) -> Result<PathBuf, ToolHostProbeAttestationError> {
        self.attested_tool_host_probe(handle)?;
        let destination = self.attested_tool_host_probe_handle_export_path();
        let bytes = canonical_json(handle)?;
        write_new_synced(&destination, &bytes).map_err(|error| {
            if error.kind() == ErrorKind::AlreadyExists {
                ToolHostProbeAttestationError::HandleExportAlreadyExists(destination.clone())
            } else {
                error.into()
            }
        })?;
        Ok(destination)
    }
}

#[cfg(test)]
fn injected_attestation_write_failure() -> Result<(), ToolHostProbeAttestationError> {
    let injected = INJECT_ATTESTATION_WRITE_FAILURE.with(|flag| flag.replace(false));
    if injected {
        return Err(std::io::Error::other("injected attestation persistence failure").into());
    }
    Ok(())
}

#[cfg(not(test))]
fn injected_attestation_write_failure() -> Result<(), ToolHostProbeAttestationError> {
    Ok(())
}

fn write_tool_host_probe_failure(
    path: &Path,
    reservation: &ProbeReservation,
    reservation_digest: &str,
    mut evidence: ToolHostProbeFailureEvidence,
) -> Result<(), ToolHostProbeAttestationError> {
    normalize_failure_evidence(&mut evidence);
    validate_failure_evidence(&evidence)?;
    let mut stored = StoredToolHostProbeFailureRecord {
        schema_version: TOOL_HOST_PROBE_FAILURE_SCHEMA_VERSION,
        store_identity: reservation.store_identity.clone(),
        run_id: reservation.run_id.clone(),
        admission_manifest_digest: reservation.admission_manifest_digest.clone(),
        probe_id: reservation.probe_id.clone(),
        invocation_id: reservation.invocation_id.clone(),
        nonce_digest: reservation.nonce_digest.clone(),
        relative_path: reservation.relative_path.clone(),
        runtime: reservation.expected_runtime.clone(),
        probe_transport: reservation.expected_transport.clone(),
        probe_transport_digest: reservation.probe_transport_digest.clone(),
        reservation_digest: reservation_digest.to_owned(),
        resource_reservation: reservation.resource_reservation.clone(),
        replay_key: reservation.replay_key.clone(),
        evidence,
        record_digest: String::new(),
    };
    stored.record_digest = failure_record_digest(&stored)?;
    let bytes = canonical_json(&stored)?;
    write_new_failure_synced(path, &bytes).map_err(|error| {
        if error.kind() == ErrorKind::AlreadyExists {
            failure_invalid("failure record already exists")
        } else {
            error.into()
        }
    })
}

fn normalize_failure_evidence(evidence: &mut ToolHostProbeFailureEvidence) {
    if evidence.phase == ToolHostProbeFailurePhase::Unknown {
        evidence.phase = ToolHostProbeFailurePhase::RuntimeProcess;
    }
    let event_identity = [
        evidence.last_event_id.is_some(),
        evidence.last_event_sequence.is_some(),
        evidence.last_event_type.is_some(),
    ];
    let invalid_event = event_identity.iter().any(|present| *present)
        && !event_identity.iter().all(|present| *present)
        || evidence
            .last_event_id
            .as_ref()
            .is_some_and(|value| value.is_empty() || value.len() > 128)
        || evidence.last_event_sequence == Some(0)
        || evidence
            .last_event_type
            .as_ref()
            .is_some_and(|value| value.is_empty() || value.len() > 64);
    if invalid_event {
        evidence.last_event_id = None;
        evidence.last_event_sequence = None;
        evidence.last_event_type = None;
    }
    if evidence.process_exit_code.is_some() && evidence.process_signal.is_some() {
        evidence.process_exit_code = None;
        evidence.process_signal = None;
    }
    match &evidence.usage {
        Some(usage) => evidence.cost = Some(failure_cost_from_usage(usage)),
        None => evidence.cost = None,
    }
    if evidence.usage.is_none()
        && (evidence.runtime_failure_kind.is_some()
            || matches!(
                evidence.last_event_type.as_deref(),
                Some("failed" | "timed_out" | "cancelled")
            ))
    {
        evidence.runtime_failure_kind = None;
        evidence.last_event_id = None;
        evidence.last_event_sequence = None;
        evidence.last_event_type = None;
    }
    if evidence
        .codex_diagnostic
        .as_ref()
        .is_some_and(|diagnostic| !is_sha256(&diagnostic.digest))
    {
        evidence.codex_diagnostic = None;
    }
    if evidence
        .mcp_diagnostic
        .as_ref()
        .is_some_and(|diagnostic| !is_sha256(&diagnostic.digest))
    {
        evidence.mcp_diagnostic = None;
    }
    let provider_is_consistent = match evidence.provider_request_state {
        ProviderRequestState::NotStarted => evidence.stages.turn_started == Some(false),
        ProviderRequestState::TurnStartedUnconfirmed => {
            evidence.stages.turn_started == Some(true)
                && evidence.stages.provider_response != Some(true)
                && evidence.stages.provider_typed_failure != Some(true)
        }
        ProviderRequestState::ProviderResponded => {
            evidence.stages.provider_response == Some(true)
                || evidence.stages.provider_typed_failure == Some(true)
        }
        ProviderRequestState::Unknown => true,
    };
    if !provider_is_consistent {
        evidence.provider_request_state = ProviderRequestState::Unknown;
        evidence.stages.turn_started = None;
        evidence.stages.provider_response = None;
        evidence.stages.provider_typed_failure = None;
    }
    if evidence.stages.process_spawned == Some(false)
        && (evidence.stages.turn_started == Some(true)
            || evidence.stages.mcp_call == Some(true)
            || evidence.stages.mcp_result == Some(true))
    {
        evidence.stages.process_spawned = None;
    }
    if evidence.stages.handle_written == Some(true) {
        evidence.stages.handle_written = Some(false);
    }
}

fn failure_cost_from_usage(usage: &Usage) -> ToolHostProbeCost {
    match usage.cost_microusd {
        Some(amount) => ToolHostProbeCost {
            availability: ToolHostProbeCostAvailability::Reported,
            currency: Some("USD".to_owned()),
            amount_microusd: Some(amount),
        },
        None => ToolHostProbeCost {
            availability: ToolHostProbeCostAvailability::Unavailable,
            currency: None,
            amount_microusd: None,
        },
    }
}

fn validate_loaded_failure(
    failure: &ToolHostProbeFailureRecord,
    reservation: &ProbeReservation,
    reservation_digest: &str,
) -> Result<(), ToolHostProbeAttestationError> {
    for (field, matches) in [
        (
            "schema_version",
            failure.schema_version == TOOL_HOST_PROBE_FAILURE_SCHEMA_VERSION,
        ),
        (
            "store_identity",
            failure.store_identity == reservation.store_identity,
        ),
        ("run_id", failure.run_id == reservation.run_id),
        (
            "manifest_digest",
            failure.admission_manifest_digest == reservation.admission_manifest_digest,
        ),
        ("probe_id", failure.probe_id == reservation.probe_id),
        (
            "invocation_id",
            failure.invocation_id == reservation.invocation_id,
        ),
        (
            "nonce_digest",
            failure.nonce_digest == reservation.nonce_digest,
        ),
        (
            "relative_path",
            failure.relative_path == reservation.relative_path,
        ),
        ("runtime", failure.runtime == reservation.expected_runtime),
        (
            "probe_transport",
            failure.probe_transport == reservation.expected_transport,
        ),
        (
            "probe_transport_digest",
            failure.probe_transport_digest == reservation.probe_transport_digest
                && failure.probe_transport_digest
                    == probe_transport_digest(&failure.probe_transport),
        ),
        (
            "reservation_digest",
            failure.reservation_digest == reservation_digest,
        ),
        (
            "resource_reservation",
            failure.resource_reservation == reservation.resource_reservation,
        ),
        ("replay_key", failure.replay_key == reservation.replay_key),
        ("record_digest", is_sha256(&failure.record_digest)),
    ] {
        if !matches {
            return Err(failure_invalid(field));
        }
    }
    let stored = StoredToolHostProbeFailureRecord {
        schema_version: failure.schema_version,
        store_identity: failure.store_identity.clone(),
        run_id: failure.run_id.clone(),
        admission_manifest_digest: failure.admission_manifest_digest.clone(),
        probe_id: failure.probe_id.clone(),
        invocation_id: failure.invocation_id.clone(),
        nonce_digest: failure.nonce_digest.clone(),
        relative_path: failure.relative_path.clone(),
        runtime: failure.runtime.clone(),
        probe_transport: failure.probe_transport.clone(),
        probe_transport_digest: failure.probe_transport_digest.clone(),
        reservation_digest: failure.reservation_digest.clone(),
        resource_reservation: failure.resource_reservation.clone(),
        replay_key: failure.replay_key.clone(),
        evidence: failure.evidence.clone(),
        record_digest: failure.record_digest.clone(),
    };
    if failure.record_digest != failure_record_digest(&stored)? {
        return Err(failure_invalid("record digest"));
    }
    validate_failure_evidence(&failure.evidence)
}

fn validate_failure_evidence(
    evidence: &ToolHostProbeFailureEvidence,
) -> Result<(), ToolHostProbeAttestationError> {
    if evidence.phase == ToolHostProbeFailurePhase::Unknown {
        return Err(failure_invalid("failure phase is unknown"));
    }
    let event_identity = [
        evidence.last_event_id.is_some(),
        evidence.last_event_sequence.is_some(),
        evidence.last_event_type.is_some(),
    ];
    if event_identity.iter().any(|present| *present)
        && !event_identity.iter().all(|present| *present)
    {
        return Err(failure_invalid("last event identity is incomplete"));
    }
    if evidence
        .last_event_id
        .as_ref()
        .is_some_and(|value| value.is_empty() || value.len() > 128)
        || evidence.last_event_sequence == Some(0)
        || evidence
            .last_event_type
            .as_ref()
            .is_some_and(|value| value.is_empty() || value.len() > 64)
    {
        return Err(failure_invalid("last event identity is invalid"));
    }
    if evidence.process_exit_code.is_some() && evidence.process_signal.is_some() {
        return Err(failure_invalid("process exit and signal are both present"));
    }
    match (&evidence.usage, &evidence.cost) {
        (None, None) => {}
        (Some(usage), Some(cost)) => validate_failure_cost(usage, cost)?,
        _ => return Err(failure_invalid("usage and cost availability differ")),
    }
    if (evidence.runtime_failure_kind.is_some()
        || matches!(
            evidence.last_event_type.as_deref(),
            Some("failed" | "timed_out" | "cancelled")
        ))
        && evidence.usage.is_none()
    {
        return Err(failure_invalid("terminal usage is missing"));
    }
    for diagnostic in [
        evidence.codex_diagnostic.as_ref(),
        evidence.mcp_diagnostic.as_ref(),
    ]
    .into_iter()
    .flatten()
    {
        if !is_sha256(&diagnostic.digest) {
            return Err(failure_invalid("diagnostic digest"));
        }
    }
    match evidence.provider_request_state {
        ProviderRequestState::NotStarted if evidence.stages.turn_started != Some(false) => {
            return Err(failure_invalid("provider not_started observation"));
        }
        ProviderRequestState::TurnStartedUnconfirmed
            if evidence.stages.turn_started != Some(true)
                || evidence.stages.provider_response == Some(true)
                || evidence.stages.provider_typed_failure == Some(true) =>
        {
            return Err(failure_invalid("provider unconfirmed observation"));
        }
        ProviderRequestState::ProviderResponded
            if evidence.stages.provider_response != Some(true)
                && evidence.stages.provider_typed_failure != Some(true) =>
        {
            return Err(failure_invalid("provider response observation"));
        }
        _ => {}
    }
    if evidence.stages.process_spawned == Some(false)
        && (evidence.stages.turn_started == Some(true)
            || evidence.stages.mcp_call == Some(true)
            || evidence.stages.mcp_result == Some(true))
    {
        return Err(failure_invalid("pre-spawn record contains later stages"));
    }
    if evidence.stages.handle_written == Some(true) {
        return Err(failure_invalid("failed probe claims a handle"));
    }
    Ok(())
}

fn validate_failure_cost(
    usage: &Usage,
    cost: &ToolHostProbeCost,
) -> Result<(), ToolHostProbeAttestationError> {
    match usage.cost_microusd {
        Some(amount)
            if cost.availability == ToolHostProbeCostAvailability::Reported
                && cost.currency.as_deref() == Some("USD")
                && cost.amount_microusd == Some(amount) =>
        {
            Ok(())
        }
        None if cost.availability == ToolHostProbeCostAvailability::Unavailable
            && cost.currency.is_none()
            && cost.amount_microusd.is_none() =>
        {
            Ok(())
        }
        _ => Err(failure_invalid("cost availability")),
    }
}

fn failure_record_digest(
    record: &StoredToolHostProbeFailureRecord,
) -> Result<String, serde_json::Error> {
    let mut unsigned = record.clone();
    unsigned.record_digest.clear();
    Ok(digest_bytes(&canonical_json(&unsigned)?))
}

fn refuse_failure_attestation_conflict(
    probe_directory: &Path,
) -> Result<(), ToolHostProbeAttestationError> {
    let exists = |path: &Path| -> Result<bool, std::io::Error> {
        match fs::symlink_metadata(path) {
            Ok(_) => Ok(true),
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(false),
            Err(error) => Err(error),
        }
    };
    if exists(&probe_directory.join(FAILURE_FILE))?
        && exists(&probe_directory.join(ATTESTATION_REFERENCE_FILE))?
    {
        return Err(ToolHostProbeAttestationError::FailureAttestationConflict);
    }
    Ok(())
}

fn validate_controller_request(
    request: &ControllerToolHostProbeRequest,
) -> Result<(), ToolHostProbeAttestationError> {
    validate_controller_request_base(
        &request.admission_manifest_digest,
        request.deadline_ms,
        &request.resource_reservation,
    )?;
    let expected_runtime = request
        .expected_runtime
        .as_ref()
        .ok_or_else(|| invalid_request("expected_runtime"))?;
    let expected_transport = request
        .expected_transport
        .as_ref()
        .ok_or_else(|| invalid_request("expected_transport"))?;
    validate_expected_transport(expected_runtime, expected_transport, &request.workspace)
}

fn validate_controller_request_base(
    admission_manifest_digest: &str,
    deadline_ms: u64,
    resource_reservation: &ToolHostProbeResourceVector,
) -> Result<(), ToolHostProbeAttestationError> {
    if !is_sha256(admission_manifest_digest) {
        return Err(invalid_request("admission_manifest_digest"));
    }
    if deadline_ms == 0 || deadline_ms > 600_000 {
        return Err(invalid_request("deadline_ms"));
    }
    validate_reservation_vector(resource_reservation, deadline_ms)
}

fn validate_expected_transport(
    expected_runtime: &ToolHostProbeRuntimeIdentity,
    expected_transport: &ProbeTransportIdentity,
    workspace: &Path,
) -> Result<(), ToolHostProbeAttestationError> {
    for (field, value) in [
        ("route", expected_runtime.route.as_str()),
        ("profile", expected_runtime.profile.as_str()),
        ("cli", expected_runtime.cli.as_str()),
        ("cli_version", expected_runtime.cli_version.as_str()),
        ("driver", expected_runtime.driver.as_str()),
        ("driver_version", expected_runtime.driver_version.as_str()),
    ] {
        if value.is_empty() || value.len() > 4096 {
            return Err(invalid_request(field));
        }
    }
    if expected_runtime.tool_schema_digest != tool_host_probe_tool_schema_digest() {
        return Err(invalid_request("tool_schema_digest"));
    }
    let transport_digest = probe_transport_digest(expected_transport);
    for (field, matches) in [
        (
            "probe_transport",
            expected_runtime.probe_transport == *expected_transport,
        ),
        (
            "probe_transport_digest",
            expected_runtime.probe_transport_digest == transport_digest,
        ),
    ] {
        if !matches {
            return Err(invalid_request(field));
        }
    }
    validate_probe_transport_identity(expected_transport, workspace)
}

/// Compares the runtime facts that establish compatibility while deliberately excluding the CLI
/// version. The runtime-observed value remains serialized in the trace, but it cannot authorize or
/// refuse behavior that the contract and executable bytes already identify.
fn runtime_compatibility_matches(
    expected: &ToolHostProbeRuntimeIdentity,
    observed: &ToolHostProbeRuntimeIdentity,
) -> bool {
    expected.runtime_kind == observed.runtime_kind
        && expected.route == observed.route
        && expected.profile == observed.profile
        && expected.cli == observed.cli
        && expected.compatibility_contract_digest == observed.compatibility_contract_digest
        && expected.executable_digest == observed.executable_digest
        && expected.driver == observed.driver
        && expected.driver_version == observed.driver_version
        && expected.tool_schema_digest == observed.tool_schema_digest
        && expected.probe_transport == observed.probe_transport
        && expected.probe_transport_digest == observed.probe_transport_digest
}

fn observed_cli_version_is_valid(runtime: &ToolHostProbeRuntimeIdentity) -> bool {
    !runtime.cli_version.is_empty() && runtime.cli_version.len() <= 4096
}

fn validate_probe_transport_identity(
    expected_transport: &ProbeTransportIdentity,
    workspace: &Path,
) -> Result<(), ToolHostProbeAttestationError> {
    for (field, matches) in [
        (
            "mcp_protocol_version",
            expected_transport.mcp_protocol_version == TOOL_HOST_PROBE_MCP_PROTOCOL_VERSION,
        ),
        (
            "server_name",
            expected_transport.server_name == TOOL_HOST_PROBE_WORKSPACE_SERVER,
        ),
        (
            "server_version",
            expected_transport.server_version == TOOL_HOST_PROBE_SERVER_VERSION,
        ),
        (
            "transport_tool_schema_digest",
            expected_transport.tool_schema_digest == tool_host_probe_tool_schema_digest(),
        ),
        (
            "ordered_tools",
            expected_transport.ordered_tools
                == [
                    ToolHostProbeTool::WorkspaceWrite,
                    ToolHostProbeTool::WorkspaceRead,
                ],
        ),
        (
            "server_executable_digest",
            is_sha256(&expected_transport.server_executable_digest),
        ),
        (
            "launcher_executable_digest",
            is_sha256(&expected_transport.launcher_executable_digest),
        ),
        (
            "internal_subcommand",
            expected_transport.internal_subcommand == TOOL_HOST_PROBE_INTERNAL_SUBCOMMAND,
        ),
        (
            "arguments",
            expected_transport.arguments
                == TOOL_HOST_PROBE_INTERNAL_ARGUMENTS
                    .iter()
                    .map(|argument| (*argument).to_owned())
                    .collect::<Vec<_>>(),
        ),
        (
            "inherited_environment",
            expected_transport.inherited_environment
                == TOOL_HOST_PROBE_ENVIRONMENT
                    .iter()
                    .map(|name| (*name).to_owned())
                    .collect::<Vec<_>>(),
        ),
        (
            "canonical_workspace_root_digest",
            expected_transport.canonical_workspace_root_digest
                == digest_bytes(workspace.as_os_str().as_encoded_bytes()),
        ),
    ] {
        if !matches {
            return Err(invalid_request(field));
        }
    }
    Ok(())
}

fn validate_reservation_vector(
    vector: &ToolHostProbeResourceVector,
    deadline_ms: u64,
) -> Result<(), ToolHostProbeAttestationError> {
    for (field, expected, found) in [
        ("model_calls", 1, vector.model_calls),
        ("workspace_reads", 1, vector.workspace_reads),
        ("workspace_writes", 1, vector.workspace_writes),
        ("invocation_starts", 1, vector.invocation_starts),
    ] {
        if found != expected {
            return Err(invalid_request(field));
        }
    }
    if vector.max_input_tokens == 0 || vector.max_output_tokens == 0 {
        return Err(invalid_request("token_limits"));
    }
    if vector.max_wall_time_ms != deadline_ms {
        return Err(invalid_request("max_wall_time_ms"));
    }
    for (field, found) in forbidden_effect_dimensions(vector) {
        if found != 0 {
            return Err(invalid_request(field));
        }
    }
    Ok(())
}

fn validate_probe_material(material: &ProbeMaterial) -> Result<(), ToolHostProbeAttestationError> {
    validate_generated_identifier("probe", &material.probe_id)?;
    validate_generated_identifier("invocation", &material.invocation_id)?;
    if material.nonce.len() != 32 || !material.nonce.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(invalid_request("generated_nonce"));
    }
    let path = material.relative_path.to_string_lossy();
    let path_nonce = path
        .strip_prefix("tool-host-probe-")
        .and_then(|value| value.strip_suffix(".nonce"));
    if !path_nonce.is_some_and(|value| {
        value.len() == 32
            && value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    }) {
        return Err(invalid_request("generated_relative_path"));
    }
    Ok(())
}

fn validate_generated_identifier(
    prefix: &'static str,
    value: &str,
) -> Result<(), ToolHostProbeAttestationError> {
    let Some(suffix) = value.strip_prefix(&format!("{prefix}-")) else {
        return Err(invalid_request("generated_identifier"));
    };
    if suffix.len() != 32
        || !suffix
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(invalid_request("generated_identifier"));
    }
    Ok(())
}

fn validate_trace(
    trace: &ToolHostProbeTrace,
    request: &ControllerToolHostProbeRequest,
    material: &ProbeMaterial,
) -> Result<(), ToolHostProbeAttestationError> {
    for (field, matches) in [
        (
            "schema_version",
            trace.schema_version == TOOL_HOST_PROBE_SCHEMA_VERSION,
        ),
        ("probe_id", trace.probe_id == material.probe_id),
        (
            "invocation_id",
            trace.invocation_id == material.invocation_id,
        ),
        ("nonce_input", trace.nonce_input == material.nonce),
        (
            "runtime_reported_readback",
            trace.runtime_reported_readback == material.nonce,
        ),
        (
            "workspace_path",
            trace.workspace_path == material.relative_path,
        ),
        ("deadline_ms", trace.deadline_ms == request.deadline_ms),
        (
            "resource_reservation",
            trace.resource_reservation == request.resource_reservation,
        ),
        (
            "runtime",
            request
                .expected_runtime
                .as_ref()
                .is_some_and(|expected| runtime_compatibility_matches(expected, &trace.runtime)),
        ),
        (
            "runtime_cli_version",
            observed_cli_version_is_valid(&trace.runtime),
        ),
        (
            "model_calls",
            trace.model_calls == request.resource_reservation.model_calls,
        ),
        (
            "input_digest",
            trace.input_digest == evidence_digest(material.nonce.as_bytes()),
        ),
        (
            "output_digest",
            trace.output_digest == evidence_digest(material.nonce.as_bytes()),
        ),
        (
            "terminal",
            trace.terminal == ToolHostProbeTerminal::Completed,
        ),
        (
            "trust",
            trace.trust == ToolHostProbeTrust::UntrustedRuntimeTrace,
        ),
    ] {
        if !matches {
            return Err(trace_mismatch(field));
        }
    }
    validate_trace_failure_evidence(trace)?;
    validate_usage_and_cost(&trace.usage, &trace.cost, &request.resource_reservation)?;
    validate_tool_event_digests(trace, material)?;
    if !is_sha256(&trace.event_digest) {
        return Err(trace_mismatch("event_digest"));
    }
    Ok(())
}

fn validate_trace_failure_evidence(
    trace: &ToolHostProbeTrace,
) -> Result<(), ToolHostProbeAttestationError> {
    let evidence = &trace.failure_evidence;
    validate_failure_evidence(evidence).map_err(|_| trace_mismatch("failure_evidence"))?;
    for (field, matches) in [
        (
            "failure_evidence.phase",
            evidence.phase == ToolHostProbeFailurePhase::McpTransport,
        ),
        (
            "failure_evidence.runtime_started",
            evidence.stages.runtime_started == Some(true),
        ),
        (
            "failure_evidence.mcp_call",
            evidence.stages.mcp_call == Some(true),
        ),
        (
            "failure_evidence.mcp_result",
            evidence.stages.mcp_result == Some(true),
        ),
        (
            "failure_evidence.controller_readback",
            evidence.stages.controller_readback.is_none(),
        ),
        (
            "failure_evidence.attestation",
            evidence.stages.attestation_written.is_none()
                && evidence.stages.handle_written.is_none(),
        ),
        (
            "failure_evidence.runtime_failure_kind",
            evidence.runtime_failure_kind.is_none(),
        ),
        (
            "failure_evidence.last_event_type",
            evidence.last_event_type.as_deref() == Some("completed"),
        ),
        (
            "failure_evidence.last_event_sequence",
            evidence
                .last_event_sequence
                .is_some_and(|sequence| sequence > trace.tool_event_digests[1].sequence),
        ),
        (
            "failure_evidence.usage",
            evidence.usage.as_ref() == Some(&trace.usage),
        ),
        (
            "failure_evidence.cost",
            evidence.cost.as_ref() == Some(&trace.cost),
        ),
        (
            "failure_evidence.diagnostic",
            evidence.codex_diagnostic.is_none() && evidence.mcp_diagnostic.is_none(),
        ),
    ] {
        if !matches {
            return Err(trace_mismatch(field));
        }
    }
    Ok(())
}

fn validate_tool_event_digests(
    trace: &ToolHostProbeTrace,
    material: &ProbeMaterial,
) -> Result<(), ToolHostProbeAttestationError> {
    let write_arguments = serde_json::json!({
        "path": material.relative_path,
        "content": material.nonce,
    });
    let write_result = serde_json::json!({"bytes_written": material.nonce.len()});
    let read_arguments = serde_json::json!({"path": material.relative_path});
    let read_result = serde_json::json!({"content": material.nonce});
    let expected = [
        (
            ToolHostProbeTool::WorkspaceWrite,
            evidence_digest(&canonical_json(&write_arguments)?),
            evidence_digest(&canonical_json(&write_result)?),
        ),
        (
            ToolHostProbeTool::WorkspaceRead,
            evidence_digest(&canonical_json(&read_arguments)?),
            evidence_digest(&canonical_json(&read_result)?),
        ),
    ];
    for (index, event) in trace.tool_event_digests.iter().enumerate() {
        let (tool, arguments, result) = &expected[index];
        if event.sequence != u64::try_from(index + 2).expect("two probe events")
            || event.tool != *tool
            || event.arguments_digest != *arguments
            || event.result_digest != *result
        {
            return Err(trace_mismatch("tool_event_digests"));
        }
    }
    Ok(())
}

fn validate_usage_and_cost(
    usage: &Usage,
    cost: &ToolHostProbeCost,
    reservation: &ToolHostProbeResourceVector,
) -> Result<(), ToolHostProbeAttestationError> {
    let total_tokens = usage
        .input_tokens
        .saturating_add(usage.cached_input_tokens)
        .saturating_add(usage.output_tokens)
        .saturating_add(usage.reasoning_output_tokens);
    if total_tokens == 0 {
        return Err(trace_mismatch("usage.tokens"));
    }
    if usage.wall_time_ms == 0 {
        return Err(trace_mismatch("usage.wall_time_ms"));
    }
    if usage.protected_queries != 0 || usage.in_flight_excess != Default::default() {
        return Err(trace_mismatch("usage.forbidden"));
    }
    if !usage.cost_is_attributed()
        || usage
            .cost_by_model
            .windows(2)
            .any(|pair| pair[0].model >= pair[1].model)
    {
        return Err(trace_mismatch("usage.cost_by_model"));
    }
    match usage.cost_microusd {
        Some(amount) => {
            if cost.availability != ToolHostProbeCostAvailability::Reported
                || cost.currency.as_deref() != Some("USD")
                || cost.amount_microusd != Some(amount)
            {
                return Err(trace_mismatch("cost"));
            }
        }
        None => {
            if cost.availability != ToolHostProbeCostAvailability::Unavailable
                || cost.currency.is_some()
                || cost.amount_microusd.is_some()
                || !usage.cost_by_model.is_empty()
            {
                return Err(trace_mismatch("cost"));
            }
        }
    }
    ensure_charged_within(&charged_vector_from_usage(usage), reservation)
}

fn charged_vector(trace: &ToolHostProbeTrace) -> ToolHostProbeResourceVector {
    let mut charged = charged_vector_from_usage(&trace.usage);
    charged.model_calls = trace.model_calls;
    charged.workspace_reads = 1;
    charged.workspace_writes = 1;
    charged.invocation_starts = 1;
    charged
}

fn charged_vector_from_usage(usage: &Usage) -> ToolHostProbeResourceVector {
    ToolHostProbeResourceVector {
        model_calls: 0,
        max_input_tokens: usage.input_tokens,
        max_cached_input_tokens: usage.cached_input_tokens,
        max_output_tokens: usage.output_tokens,
        max_reasoning_output_tokens: usage.reasoning_output_tokens,
        max_cost_microusd: usage.cost_microusd,
        max_wall_time_ms: usage.wall_time_ms,
        workspace_reads: 0,
        workspace_writes: 0,
        invocation_starts: 0,
        protected_queries: usage.protected_queries,
        external_actions: 0,
        participant_starts: 0,
        attempt_starts: 0,
        offer_creations: 0,
        obligation_creations: 0,
        board_actions: 0,
        task_actions: 0,
        recruitment_actions: 0,
        candidate_actions: 0,
        communication_actions: 0,
    }
}

fn ensure_charged_within(
    charged: &ToolHostProbeResourceVector,
    reserved: &ToolHostProbeResourceVector,
) -> Result<(), ToolHostProbeAttestationError> {
    for (field, found, limit) in [
        ("model_calls", charged.model_calls, reserved.model_calls),
        (
            "input_tokens",
            charged.max_input_tokens,
            reserved.max_input_tokens,
        ),
        (
            "cached_input_tokens",
            charged.max_cached_input_tokens,
            reserved.max_cached_input_tokens,
        ),
        (
            "output_tokens",
            charged.max_output_tokens,
            reserved.max_output_tokens,
        ),
        (
            "reasoning_output_tokens",
            charged.max_reasoning_output_tokens,
            reserved.max_reasoning_output_tokens,
        ),
        (
            "wall_time_ms",
            charged.max_wall_time_ms,
            reserved.max_wall_time_ms,
        ),
        (
            "workspace_reads",
            charged.workspace_reads,
            reserved.workspace_reads,
        ),
        (
            "workspace_writes",
            charged.workspace_writes,
            reserved.workspace_writes,
        ),
        (
            "invocation_starts",
            charged.invocation_starts,
            reserved.invocation_starts,
        ),
        (
            "protected_queries",
            charged.protected_queries,
            reserved.protected_queries,
        ),
    ] {
        if found > limit {
            return Err(ToolHostProbeAttestationError::ReservationExceeded { field });
        }
    }
    if let Some(found) = charged.max_cost_microusd {
        match reserved.max_cost_microusd {
            Some(limit) if found <= limit => {}
            None if found == 0 => {}
            _ => {
                return Err(ToolHostProbeAttestationError::ReservationExceeded {
                    field: "cost_microusd",
                });
            }
        }
    }
    for (field, found) in forbidden_effect_dimensions(charged) {
        if found != 0 {
            return Err(ToolHostProbeAttestationError::ReservationExceeded { field });
        }
    }
    Ok(())
}

fn forbidden_effect_dimensions(vector: &ToolHostProbeResourceVector) -> [(&'static str, u64); 11] {
    [
        ("external_actions", vector.external_actions),
        ("participant_starts", vector.participant_starts),
        ("attempt_starts", vector.attempt_starts),
        ("offer_creations", vector.offer_creations),
        ("obligation_creations", vector.obligation_creations),
        ("board_actions", vector.board_actions),
        ("task_actions", vector.task_actions),
        ("recruitment_actions", vector.recruitment_actions),
        ("candidate_actions", vector.candidate_actions),
        ("communication_actions", vector.communication_actions),
        ("protected_queries", vector.protected_queries),
    ]
}

fn validate_workspace_effect(
    workspace: &Path,
    relative_path: &Path,
) -> Result<(), ToolHostProbeAttestationError> {
    let mut entries = fs::read_dir(workspace)?;
    let Some(entry) = entries.next().transpose()? else {
        return Err(ToolHostProbeAttestationError::DestinationMissing);
    };
    if entries.next().transpose()?.is_some()
        || entry.file_name() != relative_path.as_os_str()
        || !entry.file_type()?.is_file()
    {
        return Err(ToolHostProbeAttestationError::UnexpectedWorkspaceEffect);
    }
    Ok(())
}

fn read_controller_destination(
    workspace: &Path,
    relative_path: &Path,
) -> Result<Vec<u8>, ToolHostProbeAttestationError> {
    let destination = workspace.join(relative_path);
    let resolved = destination.canonicalize().map_err(|error| {
        if error.kind() == ErrorKind::NotFound {
            ToolHostProbeAttestationError::DestinationMissing
        } else {
            error.into()
        }
    })?;
    if !resolved.starts_with(workspace) {
        return Err(ToolHostProbeAttestationError::UnexpectedWorkspaceEffect);
    }
    let mut file = File::open(&destination)?;
    if !file.metadata()?.is_file() {
        return Err(ToolHostProbeAttestationError::UnexpectedWorkspaceEffect);
    }
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    if destination.canonicalize()? != resolved {
        return Err(ToolHostProbeAttestationError::UnexpectedWorkspaceEffect);
    }
    Ok(bytes)
}

fn validate_reservation(
    reservation: &ProbeReservation,
    store_identity: &str,
    run_id: &str,
    probe_id: &str,
    workspace: &Path,
) -> Result<(), ToolHostProbeAttestationError> {
    let material = ProbeMaterial {
        probe_id: reservation.probe_id.clone(),
        invocation_id: reservation.invocation_id.clone(),
        nonce: "00000000000000000000000000000000".to_owned(),
        relative_path: reservation.relative_path.clone(),
    };
    for (field, matches) in [
        (
            "schema_version",
            reservation.schema_version == ATTESTED_TOOL_HOST_PROBE_SCHEMA_VERSION,
        ),
        (
            "store_identity",
            reservation.store_identity == store_identity,
        ),
        ("run_id", reservation.run_id == run_id),
        ("probe_id", reservation.probe_id == probe_id),
        (
            "manifest_digest",
            is_sha256(&reservation.admission_manifest_digest),
        ),
        ("nonce_digest", is_sha256(&reservation.nonce_digest)),
        (
            "probe_transport_digest",
            reservation.probe_transport_digest
                == probe_transport_digest(&reservation.expected_transport)
                && reservation.probe_transport_digest
                    == reservation.expected_runtime.probe_transport_digest
                && reservation.expected_runtime.probe_transport == reservation.expected_transport,
        ),
    ] {
        if !matches {
            return Err(reservation_invalid(field));
        }
    }
    validate_probe_material(&material).map_err(|_| reservation_invalid("generated fields"))?;
    validate_reservation_vector(
        &reservation.resource_reservation,
        reservation.resource_reservation.max_wall_time_ms,
    )
    .map_err(|_| reservation_invalid("resource reservation"))?;
    validate_probe_transport_identity(&reservation.expected_transport, workspace)
        .map_err(|_| reservation_invalid("expected transport"))?;
    validate_expected_transport(
        &reservation.expected_runtime,
        &reservation.expected_transport,
        workspace,
    )
    .map_err(|_| reservation_invalid("expected runtime"))?;
    let expected_replay_key = replay_key(
        store_identity,
        run_id,
        &reservation.admission_manifest_digest,
        &material,
        &reservation.nonce_digest,
        ExpectedProbeBinding {
            runtime: &reservation.expected_runtime,
            transport: &reservation.expected_transport,
        },
        &reservation.resource_reservation,
    )?;
    if reservation.replay_key != expected_replay_key {
        return Err(reservation_invalid("replay key"));
    }
    Ok(())
}

fn validate_reference(
    reference: &AttestationReference,
    handle: &AttestedToolHostProbeHandle,
    store_identity: &str,
    run_id: &str,
    reservation_digest: &str,
) -> Result<(), ToolHostProbeAttestationError> {
    for (field, matches) in [
        (
            "schema_version",
            reference.schema_version == ATTESTED_TOOL_HOST_PROBE_SCHEMA_VERSION,
        ),
        ("store_identity", reference.store_identity == store_identity),
        ("run_id", reference.run_id == run_id),
        ("probe_id", reference.probe_id == handle.probe_id),
        (
            "reservation_digest",
            reference.reservation_digest == reservation_digest,
        ),
        ("object_digest", is_sha256(&reference.object_digest)),
        (
            "record_digest",
            reference.record_digest == handle.record_digest,
        ),
    ] {
        if !matches {
            return Err(reference_invalid(field));
        }
    }
    if reference.record_digest != reference_digest(reference)? {
        return Err(reference_invalid("record digest"));
    }
    Ok(())
}

fn validate_loaded_attestation(
    attestation: &AttestedToolHostProbe,
    reservation: &ProbeReservation,
    reservation_digest: &str,
    reference: &AttestationReference,
    workspace: &Path,
) -> Result<(), ToolHostProbeAttestationError> {
    for (field, matches) in [
        (
            "schema_version",
            attestation.schema_version == ATTESTED_TOOL_HOST_PROBE_SCHEMA_VERSION,
        ),
        (
            "store_identity",
            attestation.store_identity == reservation.store_identity,
        ),
        ("run_id", attestation.run_id == reservation.run_id),
        (
            "manifest_digest",
            attestation.admission_manifest_digest == reservation.admission_manifest_digest,
        ),
        ("probe_id", attestation.probe_id == reservation.probe_id),
        (
            "invocation_id",
            attestation.invocation_id == reservation.invocation_id,
        ),
        (
            "nonce_digest",
            attestation.nonce_digest == reservation.nonce_digest,
        ),
        (
            "relative_path",
            attestation.relative_path == reservation.relative_path,
        ),
        (
            "probe_transport",
            attestation.probe_transport == reservation.expected_transport,
        ),
        (
            "probe_transport_digest",
            attestation.probe_transport_digest == reservation.probe_transport_digest
                && attestation.probe_transport_digest
                    == probe_transport_digest(&attestation.probe_transport),
        ),
        (
            "reservation_digest",
            attestation.reservation_digest == reservation_digest,
        ),
        (
            "resource_reservation",
            attestation.resource_reservation == reservation.resource_reservation,
        ),
        (
            "replay_key",
            attestation.replay_key == reservation.replay_key,
        ),
        (
            "prelaunch_absence",
            attestation.prelaunch_destination_absent,
        ),
        (
            "trace_digest",
            attestation.trace_digest == digest_bytes(&canonical_json(&attestation.trace)?),
        ),
        (
            "runtime",
            runtime_compatibility_matches(&attestation.runtime, &attestation.trace.runtime)
                && runtime_compatibility_matches(
                    &reservation.expected_runtime,
                    &attestation.runtime,
                ),
        ),
        (
            "runtime_cli_version",
            observed_cli_version_is_valid(&attestation.runtime)
                && observed_cli_version_is_valid(&attestation.trace.runtime),
        ),
        (
            "runtime_transport",
            attestation.runtime.probe_transport == attestation.probe_transport
                && attestation.runtime.probe_transport_digest == attestation.probe_transport_digest,
        ),
        ("usage", attestation.usage == attestation.trace.usage),
        ("cost", attestation.cost == attestation.trace.cost),
        (
            "terminal",
            attestation.terminal == attestation.trace.terminal,
        ),
        (
            "wall_time_ms",
            attestation.wall_time_ms == attestation.trace.usage.wall_time_ms,
        ),
        (
            "readback_digest",
            attestation.readback_digest
                == digest_bytes(attestation.trace.runtime_reported_readback.as_bytes()),
        ),
        (
            "readback_bytes",
            attestation.readback_bytes
                == u64::try_from(attestation.trace.runtime_reported_readback.len())
                    .unwrap_or(u64::MAX),
        ),
        (
            "object_reference",
            digest_bytes(&canonical_json(attestation)?) == reference.object_digest,
        ),
    ] {
        if !matches {
            return Err(attestation_invalid(field));
        }
    }
    let material = ProbeMaterial {
        probe_id: reservation.probe_id.clone(),
        invocation_id: reservation.invocation_id.clone(),
        nonce: attestation.trace.nonce_input.clone(),
        relative_path: reservation.relative_path.clone(),
    };
    let request = ControllerToolHostProbeRequest {
        admission_manifest_digest: reservation.admission_manifest_digest.clone(),
        expected_runtime: Some(attestation.runtime.clone()),
        expected_transport: Some(reservation.expected_transport.clone()),
        deadline_ms: reservation.resource_reservation.max_wall_time_ms,
        resource_reservation: reservation.resource_reservation.clone(),
        material: material.clone(),
        workspace: workspace.to_owned(),
    };
    if digest_bytes(material.nonce.as_bytes()) != reservation.nonce_digest {
        return Err(attestation_invalid("nonce binding"));
    }
    validate_trace(&attestation.trace, &request, &material)
        .map_err(|error| attestation_invalid(error.to_string()))?;
    validate_expected_transport(
        &attestation.runtime,
        &attestation.probe_transport,
        workspace,
    )
    .map_err(|error| attestation_invalid(error.to_string()))?;
    let charged = charged_vector(&attestation.trace);
    if charged != attestation.charged {
        return Err(attestation_invalid("charged vector"));
    }
    ensure_charged_within(&charged, &reservation.resource_reservation)
        .map_err(|error| attestation_invalid(error.to_string()))
}

fn configured_store_identity(data_root: &Path) -> Result<String, std::io::Error> {
    let canonical = data_root.canonicalize()?;
    Ok(digest_bytes(canonical.as_os_str().as_encoded_bytes()))
}

fn canonical_probe_workspace(
    data_root: &Path,
    probe_id: &str,
    create: bool,
) -> Result<PathBuf, ToolHostProbeAttestationError> {
    let workspace = data_root
        .join(TOOL_HOST_PROBES_DIRECTORY)
        .join(probe_id)
        .join(WORKSPACE_DIRECTORY);
    if create {
        fs::create_dir_all(&workspace)?;
    }
    let metadata = fs::symlink_metadata(&workspace)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(ToolHostProbeAttestationError::UnexpectedWorkspaceEffect);
    }
    let canonical = workspace.canonicalize()?;
    let expected = data_root
        .canonicalize()?
        .join(TOOL_HOST_PROBES_DIRECTORY)
        .join(probe_id)
        .join(WORKSPACE_DIRECTORY);
    if canonical != expected {
        return Err(ToolHostProbeAttestationError::UnexpectedWorkspaceEffect);
    }
    Ok(canonical)
}

fn replay_key(
    store_identity: &str,
    run_id: &str,
    admission_manifest_digest: &str,
    material: &ProbeMaterial,
    nonce_digest: &str,
    expected: ExpectedProbeBinding<'_>,
    resource_reservation: &ToolHostProbeResourceVector,
) -> Result<String, serde_json::Error> {
    let probe_transport_digest = probe_transport_digest(expected.transport);
    Ok(digest_bytes(&canonical_json(&ReplayKeyMaterial {
        schema_version: ATTESTED_TOOL_HOST_PROBE_SCHEMA_VERSION,
        store_identity,
        run_id,
        admission_manifest_digest,
        probe_id: &material.probe_id,
        invocation_id: &material.invocation_id,
        nonce_digest,
        relative_path: &material.relative_path,
        expected_runtime: expected.runtime,
        expected_transport: expected.transport,
        probe_transport_digest: &probe_transport_digest,
        resource_reservation,
    })?))
}

fn reference_digest(reference: &AttestationReference) -> Result<String, serde_json::Error> {
    let mut unsigned = reference.clone();
    unsigned.record_digest.clear();
    Ok(digest_bytes(&canonical_json(&unsigned)?))
}

fn canonical_json<T: Serialize + ?Sized>(value: &T) -> Result<Vec<u8>, serde_json::Error> {
    serde_json::to_vec(value)
}

fn decode_canonical<T>(bytes: &[u8]) -> Result<T, serde_json::Error>
where
    T: DeserializeOwned + Serialize,
{
    let value: T = serde_json::from_slice(bytes)?;
    if canonical_json(&value)? != bytes {
        return Err(<serde_json::Error as serde::de::Error>::custom(
            "record is not in canonical encoding",
        ));
    }
    Ok(value)
}

fn write_new_synced(path: &Path, bytes: &[u8]) -> Result<(), std::io::Error> {
    let parent = path
        .parent()
        .ok_or_else(|| std::io::Error::new(ErrorKind::InvalidInput, "record path has no parent"))?;
    fs::create_dir_all(parent)?;
    let mut file = OpenOptions::new().create_new(true).write(true).open(path)?;
    file.write_all(bytes)?;
    file.flush()?;
    file.sync_all()?;
    File::open(parent)?.sync_all()?;
    Ok(())
}

fn write_new_failure_synced(path: &Path, bytes: &[u8]) -> Result<(), std::io::Error> {
    let parent = path
        .parent()
        .ok_or_else(|| std::io::Error::new(ErrorKind::InvalidInput, "record path has no parent"))?;
    fs::create_dir_all(parent)?;
    #[cfg(test)]
    FAILURE_DURABILITY_STAGES.with(|stages| stages.set(0));
    let mut file = OpenOptions::new().create_new(true).write(true).open(path)?;
    #[cfg(test)]
    FAILURE_DURABILITY_STAGES.with(|stages| stages.set(stages.get() | 0b001));
    file.write_all(bytes)?;
    file.flush()?;
    sync_failure_file(&file)?;
    sync_failure_parent(parent)?;
    Ok(())
}

fn sync_failure_file(file: &File) -> Result<(), std::io::Error> {
    file.sync_all()?;
    #[cfg(test)]
    FAILURE_DURABILITY_STAGES.with(|stages| stages.set(stages.get() | 0b010));
    Ok(())
}

fn sync_failure_parent(parent: &Path) -> Result<(), std::io::Error> {
    File::open(parent)?.sync_all()?;
    #[cfg(test)]
    FAILURE_DURABILITY_STAGES.with(|stages| stages.set(stages.get() | 0b100));
    Ok(())
}

fn read_required(
    path: &Path,
    missing: ToolHostProbeAttestationError,
) -> Result<Vec<u8>, ToolHostProbeAttestationError> {
    fs::read(path).map_err(|error| {
        if error.kind() == ErrorKind::NotFound {
            missing
        } else {
            error.into()
        }
    })
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn invalid_request(field: &'static str) -> ToolHostProbeAttestationError {
    ToolHostProbeAttestationError::InvalidRequest { field }
}

fn trace_mismatch(field: &'static str) -> ToolHostProbeAttestationError {
    ToolHostProbeAttestationError::TraceMismatch { field }
}

fn handle_invalid(detail: impl Into<String>) -> ToolHostProbeAttestationError {
    ToolHostProbeAttestationError::HandleInvalid {
        detail: detail.into(),
    }
}

fn reservation_invalid(detail: impl Into<String>) -> ToolHostProbeAttestationError {
    ToolHostProbeAttestationError::ReservationInvalid {
        detail: detail.into(),
    }
}

fn failure_invalid(detail: impl Into<String>) -> ToolHostProbeAttestationError {
    ToolHostProbeAttestationError::FailureInvalid {
        detail: detail.into(),
    }
}

fn reference_invalid(detail: impl Into<String>) -> ToolHostProbeAttestationError {
    ToolHostProbeAttestationError::AttestationReferenceInvalid {
        detail: detail.into(),
    }
}

fn attestation_invalid(detail: impl Into<String>) -> ToolHostProbeAttestationError {
    ToolHostProbeAttestationError::AttestationObjectInvalid {
        detail: detail.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tempfile::{TempDir, tempdir};
    use ymp_domain::Budget;
    use ymp_runtime_api::{
        DiagnosticSummary, InFlightExcess, RuntimeFailureKind, RuntimeKind, ToolHostProbeEffect,
        ToolHostProbeToolEventDigest,
    };

    const DEADLINE_MS: u64 = 1_000;

    fn create_application(root: &TempDir) -> Application {
        Application::create(root.path().join("store"), "run-probe", Budget::new(1, 1))
            .expect("application")
    }

    fn reservation() -> ToolHostProbeResourceVector {
        ToolHostProbeResourceVector {
            model_calls: 1,
            max_input_tokens: 64,
            max_cached_input_tokens: 32,
            max_output_tokens: 32,
            max_reasoning_output_tokens: 16,
            max_cost_microusd: Some(100),
            max_wall_time_ms: DEADLINE_MS,
            workspace_reads: 1,
            workspace_writes: 1,
            invocation_starts: 1,
            protected_queries: 0,
            external_actions: 0,
            participant_starts: 0,
            attempt_starts: 0,
            offer_creations: 0,
            obligation_creations: 0,
            board_actions: 0,
            task_actions: 0,
            recruitment_actions: 0,
            candidate_actions: 0,
            communication_actions: 0,
        }
    }

    fn runtime_identity(transport: ProbeTransportIdentity) -> ToolHostProbeRuntimeIdentity {
        ToolHostProbeRuntimeIdentity {
            runtime_kind: RuntimeKind::Fake,
            route: "fixture/no-network".to_owned(),
            profile: "workspace-read-write-only".to_owned(),
            cli: "ymp-internal-fake".to_owned(),
            cli_version: "fake-cli 1.0.0".to_owned(),
            compatibility_contract_digest: "4".repeat(64),
            executable_digest: "5".repeat(64),
            driver: "fake-process-driver".to_owned(),
            driver_version: "fake-process-driver 1.0.0".to_owned(),
            tool_schema_digest: tool_host_probe_tool_schema_digest(),
            probe_transport_digest: probe_transport_digest(&transport),
            probe_transport: transport,
        }
    }

    fn transport_identity(workspace: &Path) -> ProbeTransportIdentity {
        ProbeTransportIdentity {
            mcp_protocol_version: TOOL_HOST_PROBE_MCP_PROTOCOL_VERSION.to_owned(),
            server_name: TOOL_HOST_PROBE_WORKSPACE_SERVER.to_owned(),
            server_version: TOOL_HOST_PROBE_SERVER_VERSION.to_owned(),
            tool_schema_digest: tool_host_probe_tool_schema_digest(),
            ordered_tools: [
                ToolHostProbeTool::WorkspaceWrite,
                ToolHostProbeTool::WorkspaceRead,
            ],
            server_executable_digest: "1".repeat(64),
            launcher_executable_digest: "1".repeat(64),
            internal_subcommand: TOOL_HOST_PROBE_INTERNAL_SUBCOMMAND.to_owned(),
            arguments: TOOL_HOST_PROBE_INTERNAL_ARGUMENTS
                .iter()
                .map(|argument| (*argument).to_owned())
                .collect(),
            inherited_environment: TOOL_HOST_PROBE_ENVIRONMENT
                .iter()
                .map(|name| (*name).to_owned())
                .collect(),
            canonical_workspace_root_digest: digest_bytes(workspace.as_os_str().as_encoded_bytes()),
        }
    }

    fn execute_prepared<F>(
        application: &mut Application,
        material: ProbeMaterial,
        executor: F,
    ) -> Result<AttestedToolHostProbeHandle, ToolHostProbeAttestationError>
    where
        F: FnOnce(&Path, ToolHostProbeRequest) -> Result<ToolHostProbeTrace, ToolHostProbeError>,
    {
        let prepared = application.prepare_controller_tool_host_probe_with_material(
            "a".repeat(64),
            DEADLINE_MS,
            reservation(),
            material,
        )?;
        let transport = transport_identity(prepared.workspace_root());
        let request =
            prepared.bind_expected_transport(runtime_identity(transport.clone()), transport)?;
        application.controller_tool_host_probe(request, executor)
    }

    fn material(suffix: char) -> ProbeMaterial {
        ProbeMaterial {
            probe_id: format!("probe-{}", suffix.to_string().repeat(32)),
            invocation_id: format!("invocation-{}", suffix.to_string().repeat(32)),
            nonce: "0123456789abcdef0123456789abcdef".to_owned(),
            relative_path: PathBuf::from(format!(
                "tool-host-probe-{}.nonce",
                suffix.to_string().repeat(32)
            )),
        }
    }

    fn complete_usage() -> Usage {
        Usage {
            input_tokens: 11,
            cached_input_tokens: 3,
            output_tokens: 5,
            reasoning_output_tokens: 2,
            cost_microusd: None,
            cost_by_model: Vec::new(),
            wall_time_ms: 7,
            protected_queries: 0,
            in_flight_excess: Default::default(),
        }
    }

    fn json_digest(value: &serde_json::Value) -> String {
        evidence_digest(&serde_json::to_vec(value).expect("canonical fixture JSON"))
    }

    fn successful_trace(request: &ToolHostProbeRequest) -> ToolHostProbeTrace {
        let write_arguments = serde_json::json!({
            "path": request.workspace_path,
            "content": request.nonce,
        });
        let write_result = serde_json::json!({"bytes_written": request.nonce.len()});
        let read_arguments = serde_json::json!({"path": request.workspace_path});
        let read_result = serde_json::json!({"content": request.nonce});
        ToolHostProbeTrace {
            schema_version: TOOL_HOST_PROBE_SCHEMA_VERSION,
            probe_id: request.probe_id.clone(),
            invocation_id: request.invocation_id.clone(),
            nonce_input: request.nonce.clone(),
            runtime_reported_readback: request.nonce.clone(),
            workspace_path: request.workspace_path.clone(),
            deadline_ms: request.deadline_ms,
            resource_reservation: request.resource_reservation.clone(),
            runtime: request.expected_runtime.clone(),
            model_calls: request.resource_reservation.model_calls,
            usage: complete_usage(),
            cost: ToolHostProbeCost {
                availability: ToolHostProbeCostAvailability::Unavailable,
                currency: None,
                amount_microusd: None,
            },
            input_digest: evidence_digest(request.nonce.as_bytes()),
            output_digest: evidence_digest(request.nonce.as_bytes()),
            tool_event_digests: [
                ToolHostProbeToolEventDigest {
                    sequence: 2,
                    tool: ToolHostProbeTool::WorkspaceWrite,
                    arguments_digest: json_digest(&write_arguments),
                    result_digest: json_digest(&write_result),
                },
                ToolHostProbeToolEventDigest {
                    sequence: 3,
                    tool: ToolHostProbeTool::WorkspaceRead,
                    arguments_digest: json_digest(&read_arguments),
                    result_digest: json_digest(&read_result),
                },
            ],
            event_digest: "b".repeat(64),
            failure_evidence: ToolHostProbeFailureEvidence {
                phase: ToolHostProbeFailurePhase::McpTransport,
                stages: ToolHostProbeFailureStages {
                    runtime_started: Some(true),
                    mcp_call: Some(true),
                    mcp_result: Some(true),
                    ..ToolHostProbeFailureStages::default()
                },
                last_event_id: Some(format!("{}.event-4", request.invocation_id)),
                last_event_sequence: Some(4),
                last_event_type: Some("completed".to_owned()),
                usage: Some(complete_usage()),
                cost: Some(ToolHostProbeCost {
                    availability: ToolHostProbeCostAvailability::Unavailable,
                    currency: None,
                    amount_microusd: None,
                }),
                ..ToolHostProbeFailureEvidence::default()
            },
            terminal: ToolHostProbeTerminal::Completed,
            trust: ToolHostProbeTrust::UntrustedRuntimeTrace,
        }
    }

    fn write_nonce_and_trace(
        workspace: &Path,
        request: ToolHostProbeRequest,
    ) -> Result<ToolHostProbeTrace, ToolHostProbeError> {
        fs::write(
            workspace.join(&request.workspace_path),
            request.nonce.as_bytes(),
        )
        .expect("fixture write");
        Ok(successful_trace(&request))
    }

    fn probe_directory(root: &TempDir, probe_id: &str) -> PathBuf {
        root.path()
            .join("store")
            .join(TOOL_HOST_PROBES_DIRECTORY)
            .join(probe_id)
    }

    fn reference(root: &TempDir, handle: &AttestedToolHostProbeHandle) -> AttestationReference {
        serde_json::from_slice(
            &fs::read(probe_directory(root, handle.probe_id()).join(ATTESTATION_REFERENCE_FILE))
                .expect("reference bytes"),
        )
        .expect("reference")
    }

    fn assert_no_attestation(root: &TempDir, probe_id: &str) {
        assert!(
            !probe_directory(root, probe_id)
                .join(ATTESTATION_REFERENCE_FILE)
                .exists(),
            "failed probe persisted a success reference"
        );
    }

    fn assert_one_failure_record(root: &TempDir, probe_id: &str) {
        let directory = probe_directory(root, probe_id);
        assert!(directory.join(RESERVATION_FILE).is_file());
        assert!(directory.join(FAILURE_FILE).is_file());
        assert_no_attestation(root, probe_id);
        let failures = fs::read_dir(&directory)
            .expect("probe directory")
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name() == FAILURE_FILE)
            .count();
        assert_eq!(failures, 1);
    }

    fn terminal_failure_evidence(
        phase: ToolHostProbeFailurePhase,
        provider_request_state: ProviderRequestState,
        stages: ToolHostProbeFailureStages,
        sequence: u64,
    ) -> ToolHostProbeFailureEvidence {
        let mut usage = complete_usage();
        if provider_request_state == ProviderRequestState::TurnStartedUnconfirmed {
            usage.in_flight_excess = InFlightExcess {
                model_requests: 1,
                ..InFlightExcess::default()
            };
        }
        ToolHostProbeFailureEvidence {
            phase,
            provider_request_state,
            stages,
            runtime_failure_kind: Some(RuntimeFailureKind::ProcessExit),
            last_event_id: Some(format!("invocation.event-{sequence}")),
            last_event_sequence: Some(sequence),
            last_event_type: Some("failed".to_owned()),
            process_exit_code: Some(17),
            duration_ms: Some(47),
            usage: Some(usage),
            cost: Some(ToolHostProbeCost {
                availability: ToolHostProbeCostAvailability::Unavailable,
                currency: None,
                amount_microusd: None,
            }),
            codex_diagnostic: Some(DiagnosticSummary::from_bytes(
                b"raw stderr prompt output credential",
                true,
            )),
            ..ToolHostProbeFailureEvidence::default()
        }
    }

    fn persist_terminal_failure(application: &mut Application, material: &ProbeMaterial) {
        let evidence = terminal_failure_evidence(
            ToolHostProbeFailurePhase::RuntimeProcess,
            ProviderRequestState::NotStarted,
            ToolHostProbeFailureStages {
                process_spawned: Some(true),
                runtime_started: Some(true),
                turn_started: Some(false),
                cleanup_completed: Some(true),
                ..ToolHostProbeFailureStages::default()
            },
            1,
        );
        let result = execute_prepared(application, material.clone(), move |_, _| {
            Err(ToolHostProbeError::RuntimeFailed {
                detail: "raw runtime failure".to_owned(),
            }
            .with_failure_evidence(evidence))
        });
        assert!(matches!(
            result,
            Err(ToolHostProbeAttestationError::Runtime(
                ToolHostProbeError::RuntimeFailed { .. }
            ))
        ));
    }

    fn rewrite_failure(
        root: &TempDir,
        probe_id: &str,
        mutate: impl FnOnce(&mut StoredToolHostProbeFailureRecord),
    ) {
        let path = probe_directory(root, probe_id).join(FAILURE_FILE);
        let mut stored: StoredToolHostProbeFailureRecord =
            decode_canonical(&fs::read(&path).expect("failure bytes")).expect("stored failure");
        mutate(&mut stored);
        stored.record_digest = failure_record_digest(&stored).expect("mutated record digest");
        fs::write(
            &path,
            canonical_json(&stored).expect("mutated failure bytes"),
        )
        .expect("replace failure fixture");
    }

    fn copy_directory(source: &Path, destination: &Path) {
        fs::create_dir_all(destination).expect("copy destination");
        for entry in fs::read_dir(source).expect("copy source") {
            let entry = entry.expect("copy entry");
            let target = destination.join(entry.file_name());
            if entry.file_type().expect("copy file type").is_dir() {
                copy_directory(&entry.path(), &target);
            } else {
                fs::copy(entry.path(), target).expect("copy file");
            }
        }
    }

    #[test]
    fn happy_path_spends_one_reservation_and_reloads_only_through_the_store() {
        let root = tempdir().expect("temporary root");
        let mut application = create_application(&root);
        let material = material('1');
        let handle = execute_prepared(&mut application, material.clone(), write_nonce_and_trace)
            .expect("attested probe handle");

        let directory = probe_directory(&root, &material.probe_id);
        assert!(directory.join(RESERVATION_FILE).is_file());
        assert!(directory.join(ATTESTATION_REFERENCE_FILE).is_file());
        let reservation_entries = fs::read_dir(&directory)
            .expect("probe directory")
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name() == RESERVATION_FILE)
            .count();
        assert_eq!(reservation_entries, 1);

        let encoded = serde_json::to_vec(&handle).expect("handle JSON");
        let fields = serde_json::from_slice::<serde_json::Value>(&encoded)
            .expect("handle value")
            .as_object()
            .expect("handle object")
            .clone();
        assert_eq!(fields.len(), 4);
        for forbidden in ["nonce", "path", "object_digest", "attestation"] {
            assert!(!fields.contains_key(forbidden));
        }
        let decoded: AttestedToolHostProbeHandle =
            serde_json::from_slice(&encoded).expect("strict handle round trip");
        let attestation = application
            .attested_tool_host_probe(&decoded)
            .expect("verified private reload");
        assert_eq!(attestation.probe_id(), material.probe_id);
        assert_eq!(attestation.invocation_id(), material.invocation_id);
        assert!(attestation.prelaunch_destination_absent());
        assert_eq!(attestation.nonce_digest(), attestation.readback_digest());
        assert_eq!(attestation.readback_bytes(), 32);
        assert_eq!(attestation.charged().model_calls, 1);
        assert_eq!(attestation.charged().workspace_reads, 1);
        assert_eq!(attestation.charged().workspace_writes, 1);
        assert_eq!(
            attestation.probe_transport_digest(),
            probe_transport_digest(attestation.probe_transport())
        );

        let export = application
            .export_attested_tool_host_probe_handle(&decoded)
            .expect("handle export");
        assert_eq!(
            export,
            root.path()
                .join("store")
                .join(TOOL_HOST_PROBE_HANDLE_EXPORT)
        );
        assert_eq!(fs::read(&export).expect("export bytes"), encoded);
        assert!(matches!(
            application.export_attested_tool_host_probe_handle(&decoded),
            Err(ToolHostProbeAttestationError::HandleExportAlreadyExists(_))
        ));

        drop(application);
        let reopened = Application::open(root.path().join("store")).expect("reopen application");
        assert_eq!(
            reopened
                .attested_tool_host_probe(&decoded)
                .expect("recovered attestation"),
            attestation
        );
    }

    #[test]
    fn observed_runtime_version_is_evidence_not_attestation_authority() {
        const OBSERVED_VERSION: &str = "fake-cli 9.7.3";

        let root = tempdir().expect("temporary root");
        let mut application = create_application(&root);
        let handle = execute_prepared(&mut application, material('c'), |workspace, request| {
            assert_ne!(request.expected_runtime.cli_version, OBSERVED_VERSION);
            fs::write(
                workspace.join(&request.workspace_path),
                request.nonce.as_bytes(),
            )
            .expect("fixture write");
            let mut trace = successful_trace(&request);
            trace.runtime.cli_version = OBSERVED_VERSION.to_owned();
            Ok(trace)
        })
        .expect("version-independent attestation");

        let attestation = application
            .attested_tool_host_probe(&handle)
            .expect("attested observed version");
        assert_eq!(attestation.trace().runtime.cli_version, OBSERVED_VERSION);
        assert_eq!(attestation.runtime().cli_version, OBSERVED_VERSION);
    }

    #[test]
    fn preparation_and_measurement_precede_reservation_and_executor() {
        let root = tempdir().expect("temporary root");
        let mut application = create_application(&root);
        let material = material('9');
        let prepared = application
            .prepare_controller_tool_host_probe_with_material(
                "a".repeat(64),
                DEADLINE_MS,
                reservation(),
                material.clone(),
            )
            .expect("prepared probe");
        let reservation_path = probe_directory(&root, &material.probe_id).join(RESERVATION_FILE);
        assert!(!reservation_path.exists());
        let transport = transport_identity(prepared.workspace_root());
        let unbound = application.controller_tool_host_probe(prepared.clone(), |_, _| {
            panic!("unbound request reached executor")
        });
        assert!(matches!(
            unbound,
            Err(ToolHostProbeAttestationError::InvalidRequest {
                field: "expected_runtime"
            })
        ));
        assert!(!reservation_path.exists());
        let mut default_digest = runtime_identity(transport.clone());
        default_digest.probe_transport_digest.clear();
        assert!(
            prepared
                .clone()
                .bind_expected_transport(default_digest, transport.clone())
                .is_err()
        );
        let request = prepared
            .bind_expected_transport(runtime_identity(transport.clone()), transport)
            .expect("foreground-bound request");
        assert!(!reservation_path.exists());

        application
            .controller_tool_host_probe(request, |workspace, request| {
                assert!(reservation_path.is_file());
                write_nonce_and_trace(workspace, request)
            })
            .expect("attested probe");
    }

    #[test]
    fn altered_actual_transport_and_copied_workspace_root_fail_before_attestation() {
        let root = tempdir().expect("temporary root");
        let mut application = create_application(&root);
        let first_material = material('a');
        let prepared = application
            .prepare_controller_tool_host_probe_with_material(
                "a".repeat(64),
                DEADLINE_MS,
                reservation(),
                first_material.clone(),
            )
            .expect("prepared probe");
        let transport = transport_identity(prepared.workspace_root());
        let request = prepared
            .bind_expected_transport(runtime_identity(transport.clone()), transport)
            .expect("bound request");
        let result = application.controller_tool_host_probe(request, |workspace, request| {
            fs::write(
                workspace.join(&request.workspace_path),
                request.nonce.as_bytes(),
            )
            .expect("nonce destination");
            let mut trace = successful_trace(&request);
            trace.runtime.probe_transport.server_executable_digest = "f".repeat(64);
            trace.runtime.probe_transport_digest =
                probe_transport_digest(&trace.runtime.probe_transport);
            Ok(trace)
        });
        assert!(matches!(
            result,
            Err(ToolHostProbeAttestationError::TraceMismatch { field: "runtime" })
        ));
        assert_no_attestation(&root, &first_material.probe_id);

        let other = tempdir().expect("copied root");
        let other_workspace = other.path().join("workspace");
        fs::create_dir(&other_workspace).expect("other workspace");
        let material = material('b');
        let prepared = application
            .prepare_controller_tool_host_probe_with_material(
                "a".repeat(64),
                DEADLINE_MS,
                reservation(),
                material.clone(),
            )
            .expect("second prepared probe");
        let copied_transport = transport_identity(
            &other_workspace
                .canonicalize()
                .expect("canonical other workspace"),
        );
        assert!(
            prepared
                .bind_expected_transport(
                    runtime_identity(copied_transport.clone()),
                    copied_transport,
                )
                .is_err()
        );
        assert!(
            !probe_directory(&root, &material.probe_id)
                .join(RESERVATION_FILE)
                .exists()
        );
    }

    #[test]
    fn preexisting_destination_is_refused_before_reservation_or_execution() {
        let root = tempdir().expect("temporary root");
        let mut application = create_application(&root);
        let material = material('2');
        let workspace = probe_directory(&root, &material.probe_id).join(WORKSPACE_DIRECTORY);
        fs::create_dir_all(&workspace).expect("workspace");
        fs::write(workspace.join(&material.relative_path), b"planted").expect("planted file");
        let result = execute_prepared(&mut application, material.clone(), |_, _| {
            panic!("executor reached after prelaunch refusal")
        });
        assert!(matches!(
            result,
            Err(ToolHostProbeAttestationError::DestinationAlreadyExists)
        ));
        assert!(
            !probe_directory(&root, &material.probe_id)
                .join(RESERVATION_FILE)
                .exists()
        );
    }

    #[test]
    fn stranded_reservation_is_spent_and_cannot_start_twice() {
        let root = tempdir().expect("temporary root");
        let mut application = create_application(&root);
        let material = material('3');
        let starts = Arc::new(AtomicUsize::new(0));
        let first_starts = Arc::clone(&starts);
        let first = execute_prepared(&mut application, material.clone(), move |_, _| {
            first_starts.fetch_add(1, Ordering::SeqCst);
            Err(ToolHostProbeError::RuntimeFailed {
                detail: "fixture crash".to_owned(),
            })
        });
        assert!(matches!(
            first,
            Err(ToolHostProbeAttestationError::Runtime(
                ToolHostProbeError::RuntimeFailed { .. }
            ))
        ));
        assert!(
            probe_directory(&root, &material.probe_id)
                .join(RESERVATION_FILE)
                .is_file()
        );
        assert_no_attestation(&root, &material.probe_id);

        let second_starts = Arc::clone(&starts);
        let second = execute_prepared(&mut application, material.clone(), move |_, _| {
            second_starts.fetch_add(1, Ordering::SeqCst);
            unreachable!("spent reservation started again")
        });
        assert!(matches!(
            second,
            Err(ToolHostProbeAttestationError::ReservationSpent)
        ));
        assert_eq!(starts.load(Ordering::SeqCst), 1);
        assert_no_attestation(&root, &material.probe_id);
        let stranded_handle = AttestedToolHostProbeHandle {
            schema_version: ATTESTED_TOOL_HOST_PROBE_SCHEMA_VERSION,
            store_identity: configured_store_identity(&root.path().join("store"))
                .expect("store identity"),
            probe_id: material.probe_id,
            record_digest: "0".repeat(64),
        };
        assert!(matches!(
            application.attested_tool_host_probe(&stranded_handle),
            Err(ToolHostProbeAttestationError::AttestationReferenceMissing)
        ));
    }

    #[test]
    fn every_runtime_provider_and_mcp_stage_writes_one_sanitized_failure_record() {
        let root = tempdir().expect("temporary root");
        let mut application = create_application(&root);
        let cases = [
            (
                '0',
                ToolHostProbeFailureEvidence {
                    phase: ToolHostProbeFailurePhase::BeforeProcessSpawn,
                    provider_request_state: ProviderRequestState::NotStarted,
                    stages: ToolHostProbeFailureStages {
                        process_spawned: Some(false),
                        runtime_started: Some(false),
                        turn_started: Some(false),
                        ..ToolHostProbeFailureStages::default()
                    },
                    ..ToolHostProbeFailureEvidence::default()
                },
            ),
            (
                '1',
                terminal_failure_evidence(
                    ToolHostProbeFailurePhase::RuntimeProcess,
                    ProviderRequestState::NotStarted,
                    ToolHostProbeFailureStages {
                        process_spawned: Some(true),
                        runtime_started: Some(true),
                        turn_started: Some(false),
                        cleanup_completed: Some(true),
                        ..ToolHostProbeFailureStages::default()
                    },
                    1,
                ),
            ),
            (
                '2',
                terminal_failure_evidence(
                    ToolHostProbeFailurePhase::ProviderRequest,
                    ProviderRequestState::TurnStartedUnconfirmed,
                    ToolHostProbeFailureStages {
                        process_spawned: Some(true),
                        runtime_started: Some(true),
                        turn_started: Some(true),
                        provider_response: Some(false),
                        provider_typed_failure: Some(false),
                        cleanup_completed: Some(true),
                        ..ToolHostProbeFailureStages::default()
                    },
                    2,
                ),
            ),
            (
                '3',
                terminal_failure_evidence(
                    ToolHostProbeFailurePhase::ProviderRequest,
                    ProviderRequestState::ProviderResponded,
                    ToolHostProbeFailureStages {
                        process_spawned: Some(true),
                        runtime_started: Some(true),
                        turn_started: Some(true),
                        provider_response: Some(true),
                        provider_typed_failure: Some(false),
                        cleanup_completed: Some(true),
                        ..ToolHostProbeFailureStages::default()
                    },
                    3,
                ),
            ),
            (
                '4',
                terminal_failure_evidence(
                    ToolHostProbeFailurePhase::ProviderRequest,
                    ProviderRequestState::ProviderResponded,
                    ToolHostProbeFailureStages {
                        process_spawned: Some(true),
                        runtime_started: Some(true),
                        turn_started: Some(true),
                        provider_response: Some(false),
                        provider_typed_failure: Some(true),
                        cleanup_completed: Some(true),
                        ..ToolHostProbeFailureStages::default()
                    },
                    4,
                ),
            ),
            (
                '5',
                terminal_failure_evidence(
                    ToolHostProbeFailurePhase::McpTransport,
                    ProviderRequestState::ProviderResponded,
                    ToolHostProbeFailureStages {
                        process_spawned: Some(true),
                        runtime_started: Some(true),
                        turn_started: Some(true),
                        provider_response: Some(true),
                        provider_typed_failure: Some(false),
                        mcp_call: Some(false),
                        cleanup_completed: Some(true),
                        ..ToolHostProbeFailureStages::default()
                    },
                    5,
                ),
            ),
            (
                '6',
                terminal_failure_evidence(
                    ToolHostProbeFailurePhase::McpTransport,
                    ProviderRequestState::ProviderResponded,
                    ToolHostProbeFailureStages {
                        process_spawned: Some(true),
                        runtime_started: Some(true),
                        turn_started: Some(true),
                        provider_response: Some(true),
                        provider_typed_failure: Some(false),
                        mcp_call: Some(true),
                        mcp_result: Some(false),
                        cleanup_completed: Some(true),
                        ..ToolHostProbeFailureStages::default()
                    },
                    6,
                ),
            ),
            ('7', {
                let mut evidence = terminal_failure_evidence(
                    ToolHostProbeFailurePhase::McpTransport,
                    ProviderRequestState::ProviderResponded,
                    ToolHostProbeFailureStages {
                        process_spawned: Some(true),
                        runtime_started: Some(true),
                        turn_started: Some(true),
                        provider_response: Some(true),
                        provider_typed_failure: Some(false),
                        mcp_call: Some(true),
                        mcp_result: Some(true),
                        cleanup_completed: Some(true),
                        ..ToolHostProbeFailureStages::default()
                    },
                    7,
                );
                evidence.mcp_diagnostic = Some(DiagnosticSummary::from_bytes(
                    b"raw MCP result must not persist",
                    false,
                ));
                evidence
            }),
        ];

        for (suffix, evidence) in cases {
            let material = material(suffix);
            let mut expected = evidence.clone();
            expected.stages.controller_readback = Some(false);
            expected.stages.attestation_written = Some(false);
            expected.stages.handle_written = Some(false);
            let result = execute_prepared(&mut application, material.clone(), move |_, _| {
                Err(ToolHostProbeError::RuntimeFailed {
                    detail: "raw stderr prompt output credential".to_owned(),
                }
                .with_failure_evidence(evidence))
            });
            assert!(matches!(
                result,
                Err(ToolHostProbeAttestationError::Runtime(
                    ToolHostProbeError::RuntimeFailed { .. }
                ))
            ));
            assert_one_failure_record(&root, &material.probe_id);
            FAILURE_DURABILITY_STAGES.with(|stages| assert_eq!(stages.get(), 0b111));

            let failure = application
                .tool_host_probe_failure(&material.probe_id)
                .expect("verified failure reload");
            assert_eq!(failure.evidence(), &expected);
            assert_eq!(failure.probe_id(), material.probe_id);
            assert_eq!(failure.invocation_id(), material.invocation_id);
            assert_eq!(failure.record_digest().len(), 64);
            assert_eq!(
                failure.probe_transport_digest(),
                probe_transport_digest(failure.probe_transport())
            );
            if suffix == '0' {
                assert_eq!(
                    failure.evidence().provider_request_state,
                    ProviderRequestState::NotStarted
                );
                assert_eq!(failure.evidence().usage, None);
                assert_eq!(failure.evidence().cost, None);
                assert_eq!(failure.evidence().last_event_id, None);
                assert_eq!(failure.evidence().process_exit_code, None);
            }
            if suffix == '2' {
                assert_eq!(
                    failure
                        .evidence()
                        .usage
                        .as_ref()
                        .expect("turn usage")
                        .in_flight_excess
                        .model_requests,
                    1
                );
            }
            let bytes = fs::read(probe_directory(&root, &material.probe_id).join(FAILURE_FILE))
                .expect("failure bytes");
            let text = String::from_utf8(bytes).expect("failure UTF-8");
            for forbidden in [
                "raw stderr",
                "raw MCP result",
                "prompt output",
                "credential",
                material.nonce.as_str(),
            ] {
                assert!(!text.contains(forbidden), "failure exposed {forbidden}");
            }

            let replay = execute_prepared(&mut application, material.clone(), |_, _| {
                panic!("spent failed reservation executed again")
            });
            assert!(matches!(
                replay,
                Err(ToolHostProbeAttestationError::ReservationSpent)
            ));
        }
    }

    #[test]
    fn trace_readback_and_attestation_failures_are_durably_distinct() {
        let root = tempdir().expect("temporary root");
        let mut application = create_application(&root);

        let trace_material = material('8');
        let trace_result =
            execute_prepared(&mut application, trace_material.clone(), |_, request| {
                let mut trace = successful_trace(&request);
                trace.runtime.route = "mutated-route".to_owned();
                Ok(trace)
            });
        assert!(matches!(
            trace_result,
            Err(ToolHostProbeAttestationError::TraceMismatch { field: "runtime" })
        ));

        let readback_material = material('9');
        let readback_result =
            execute_prepared(&mut application, readback_material.clone(), |_, request| {
                Ok(successful_trace(&request))
            });
        assert!(matches!(
            readback_result,
            Err(ToolHostProbeAttestationError::DestinationMissing)
        ));

        let attestation_material = material('a');
        INJECT_ATTESTATION_WRITE_FAILURE.with(|flag| flag.set(true));
        let attestation_result = execute_prepared(
            &mut application,
            attestation_material.clone(),
            write_nonce_and_trace,
        );
        assert!(matches!(
            attestation_result,
            Err(ToolHostProbeAttestationError::Io(_))
        ));

        for (material, phase, readback) in [
            (
                trace_material,
                ToolHostProbeFailurePhase::ControllerTraceValidation,
                Some(false),
            ),
            (
                readback_material,
                ToolHostProbeFailurePhase::ControllerReadback,
                Some(false),
            ),
            (
                attestation_material,
                ToolHostProbeFailurePhase::AttestationPersistence,
                Some(true),
            ),
        ] {
            assert_one_failure_record(&root, &material.probe_id);
            let failure = application
                .tool_host_probe_failure(&material.probe_id)
                .expect("controller failure reload");
            assert_eq!(failure.evidence().phase, phase);
            assert_eq!(failure.evidence().stages.controller_readback, readback);
            assert_eq!(failure.evidence().stages.handle_written, Some(false));
            assert_eq!(failure.evidence().usage, Some(complete_usage()));
            assert_eq!(
                failure.evidence().last_event_type.as_deref(),
                Some("completed")
            );
        }
        FAILURE_DURABILITY_STAGES.with(|stages| assert_eq!(stages.get(), 0b111));
    }

    #[test]
    fn controller_readback_and_workspace_effect_check_reject_a_false_green_trace() {
        let root = tempdir().expect("temporary root");
        let mut application = create_application(&root);
        let wrong_bytes = material('4');
        let result = execute_prepared(
            &mut application,
            wrong_bytes.clone(),
            |workspace, request| {
                fs::write(
                    workspace.join(&request.workspace_path),
                    b"model-authored-value",
                )
                .expect("mutated destination");
                Ok(successful_trace(&request))
            },
        );
        assert!(matches!(
            result,
            Err(ToolHostProbeAttestationError::ReadbackMismatch)
        ));
        assert_no_attestation(&root, &wrong_bytes.probe_id);

        let extra_effect = material('5');
        let result = execute_prepared(
            &mut application,
            extra_effect.clone(),
            |workspace, request| {
                fs::write(
                    workspace.join(&request.workspace_path),
                    request.nonce.as_bytes(),
                )
                .expect("nonce destination");
                fs::write(workspace.join("extra.txt"), b"extra").expect("extra effect");
                Ok(successful_trace(&request))
            },
        );
        assert!(matches!(
            result,
            Err(ToolHostProbeAttestationError::UnexpectedWorkspaceEffect)
        ));
        assert_no_attestation(&root, &extra_effect.probe_id);
    }

    #[test]
    fn wrong_trace_bindings_usage_terminal_and_budget_leave_no_attestation() {
        enum Mutation {
            MissingFile,
            Nonce,
            Path,
            Route,
            Usage,
            Budget,
            Digest,
            Terminal,
            ForbiddenEffect,
            Evidence,
        }
        for (suffix, mutation) in [
            ('0', Mutation::MissingFile),
            ('1', Mutation::Nonce),
            ('2', Mutation::Path),
            ('3', Mutation::Route),
            ('4', Mutation::Usage),
            ('5', Mutation::Budget),
            ('6', Mutation::Digest),
            ('7', Mutation::Terminal),
            ('8', Mutation::ForbiddenEffect),
            ('9', Mutation::Evidence),
        ]
        .into_iter()
        {
            let root = tempdir().expect("temporary root");
            let mut application = create_application(&root);
            let material = material(suffix);
            let result = execute_prepared(
                &mut application,
                material.clone(),
                move |workspace, request| {
                    if matches!(mutation, Mutation::Terminal) {
                        return Err(ToolHostProbeError::AmbiguousTerminal);
                    }
                    if matches!(mutation, Mutation::ForbiddenEffect) {
                        return Err(ToolHostProbeError::ForbiddenEffect {
                            effect: ToolHostProbeEffect::Board,
                            server: "ymp.board".to_owned(),
                            tool: "publish".to_owned(),
                        });
                    }
                    if matches!(mutation, Mutation::MissingFile) {
                        return Ok(successful_trace(&request));
                    }
                    fs::write(
                        workspace.join(&request.workspace_path),
                        request.nonce.as_bytes(),
                    )
                    .expect("nonce destination");
                    let mut trace = successful_trace(&request);
                    match mutation {
                        Mutation::MissingFile => unreachable!(),
                        Mutation::Nonce => trace.nonce_input = "wrong".to_owned(),
                        Mutation::Path => trace.workspace_path = PathBuf::from("runtime-chosen"),
                        Mutation::Route => trace.runtime.route = "wrong-route".to_owned(),
                        Mutation::Usage => trace.usage = Usage::default(),
                        Mutation::Budget => {
                            trace.usage.input_tokens =
                                trace.resource_reservation.max_input_tokens + 1;
                        }
                        Mutation::Digest => trace.output_digest = "c".repeat(63),
                        Mutation::Evidence => {
                            trace.failure_evidence.provider_request_state =
                                ProviderRequestState::ProviderResponded;
                            trace.failure_evidence.stages.provider_response = None;
                            trace.failure_evidence.stages.provider_typed_failure = None;
                        }
                        Mutation::Terminal | Mutation::ForbiddenEffect => unreachable!(),
                    }
                    Ok(trace)
                },
            );
            assert!(result.is_err(), "mutation {suffix} reached an attestation");
            assert!(
                probe_directory(&root, &material.probe_id)
                    .join(RESERVATION_FILE)
                    .is_file(),
                "mutation {suffix} failed before spending its start reservation"
            );
            assert!(
                probe_directory(&root, &material.probe_id)
                    .join(FAILURE_FILE)
                    .is_file(),
                "mutation {suffix} did not persist failure evidence"
            );
            application
                .tool_host_probe_failure(&material.probe_id)
                .expect("mutated trace has reloadable sanitized failure");
            assert_no_attestation(&root, &material.probe_id);
        }
    }

    #[test]
    fn failure_recovery_rejects_missing_corrupt_mutated_replayed_and_conflicting_records() {
        #[derive(Clone, Copy)]
        enum Mutation {
            Missing,
            Corrupt,
            MissingUsage,
            UnknownPhase,
            ReplayKey,
        }
        for (index, mutation) in [
            Mutation::Missing,
            Mutation::Corrupt,
            Mutation::MissingUsage,
            Mutation::UnknownPhase,
            Mutation::ReplayKey,
        ]
        .into_iter()
        .enumerate()
        {
            let root = tempdir().expect("temporary root");
            let mut application = create_application(&root);
            let suffix = char::from(b'b' + u8::try_from(index).expect("bounded index"));
            let material = material(suffix);
            persist_terminal_failure(&mut application, &material);
            let path = probe_directory(&root, &material.probe_id).join(FAILURE_FILE);
            match mutation {
                Mutation::Missing => fs::remove_file(path).expect("remove failure"),
                Mutation::Corrupt => fs::write(path, b"not-json").expect("corrupt failure"),
                Mutation::MissingUsage => rewrite_failure(&root, &material.probe_id, |stored| {
                    stored.evidence.usage = None;
                    stored.evidence.cost = None;
                }),
                Mutation::UnknownPhase => rewrite_failure(&root, &material.probe_id, |stored| {
                    stored.evidence.phase = ToolHostProbeFailurePhase::Unknown;
                }),
                Mutation::ReplayKey => rewrite_failure(&root, &material.probe_id, |stored| {
                    stored.replay_key = "f".repeat(64);
                }),
            }
            let error = application
                .tool_host_probe_failure(&material.probe_id)
                .expect_err("failure mutation must be rejected");
            match mutation {
                Mutation::Missing => assert!(matches!(
                    error,
                    ToolHostProbeAttestationError::FailureMissing
                )),
                Mutation::Corrupt
                | Mutation::MissingUsage
                | Mutation::UnknownPhase
                | Mutation::ReplayKey => assert!(matches!(
                    error,
                    ToolHostProbeAttestationError::FailureInvalid { .. }
                )),
            }
            assert_no_attestation(&root, &material.probe_id);
            let replay = execute_prepared(&mut application, material.clone(), |_, _| {
                panic!("corrupt failed reservation executed again")
            });
            assert!(matches!(
                replay,
                Err(ToolHostProbeAttestationError::ReservationSpent)
            ));
        }

        let root = tempdir().expect("conflict root");
        let mut application = create_application(&root);
        let material = material('f');
        persist_terminal_failure(&mut application, &material);
        fs::write(
            probe_directory(&root, &material.probe_id).join(ATTESTATION_REFERENCE_FILE),
            b"fabricated simultaneous attestation",
        )
        .expect("conflicting attestation reference");
        assert!(matches!(
            application.tool_host_probe_failure(&material.probe_id),
            Err(ToolHostProbeAttestationError::FailureAttestationConflict)
        ));
        let handle = AttestedToolHostProbeHandle {
            schema_version: ATTESTED_TOOL_HOST_PROBE_SCHEMA_VERSION,
            store_identity: configured_store_identity(&root.path().join("store"))
                .expect("store identity"),
            probe_id: material.probe_id,
            record_digest: "0".repeat(64),
        };
        assert!(matches!(
            application.attested_tool_host_probe(&handle),
            Err(ToolHostProbeAttestationError::FailureAttestationConflict)
        ));
    }

    #[test]
    fn failure_record_copied_to_another_store_is_not_recoverable() {
        let source = tempdir().expect("source root");
        let mut application = create_application(&source);
        let material = material('d');
        persist_terminal_failure(&mut application, &material);
        assert_no_attestation(&source, &material.probe_id);
        assert!(
            application
                .tool_host_probe_failure(&material.probe_id)
                .is_ok()
        );
        let replay = execute_prepared(&mut application, material.clone(), |_, _| {
            panic!("source failed reservation executed again")
        });
        assert!(matches!(
            replay,
            Err(ToolHostProbeAttestationError::ReservationSpent)
        ));
        drop(application);

        let copied = tempdir().expect("copied root");
        copy_directory(&source.path().join("store"), &copied.path().join("store"));
        let mut copied_application =
            Application::open(copied.path().join("store")).expect("open copied store bytes");
        assert!(matches!(
            copied_application.tool_host_probe_failure(&material.probe_id),
            Err(ToolHostProbeAttestationError::ReservationInvalid { .. })
                | Err(ToolHostProbeAttestationError::UnexpectedWorkspaceEffect)
        ));
        assert_no_attestation(&copied, &material.probe_id);
        let replay = execute_prepared(&mut copied_application, material, |_, _| {
            panic!("copied failed reservation executed again")
        });
        assert!(matches!(
            replay,
            Err(ToolHostProbeAttestationError::ReservationSpent)
        ));
    }

    #[test]
    fn mutated_fabricated_and_copied_handles_are_not_authority() {
        let root = tempdir().expect("temporary root");
        let mut application = create_application(&root);
        let handle = execute_prepared(&mut application, material('6'), write_nonce_and_trace)
            .expect("handle");
        let valid = serde_json::to_value(&handle).expect("handle value");

        for (field, value) in [
            ("store_identity", serde_json::json!("d".repeat(64))),
            (
                "probe_id",
                serde_json::json!(format!("probe-{}", "7".repeat(32))),
            ),
            ("record_digest", serde_json::json!("e".repeat(64))),
        ] {
            let mut forged = valid.clone();
            forged[field] = value;
            let forged: AttestedToolHostProbeHandle =
                serde_json::from_value(forged).expect("strict-shaped forged handle");
            assert!(application.attested_tool_host_probe(&forged).is_err());
        }
        let mut unknown = valid.clone();
        unknown["nonce"] = serde_json::json!("caller-controlled");
        assert!(serde_json::from_value::<AttestedToolHostProbeHandle>(unknown).is_err());

        let other_root = tempdir().expect("other root");
        let other_application = create_application(&other_root);
        assert!(other_application.attested_tool_host_probe(&handle).is_err());

        let fabricated: AttestedToolHostProbeHandle = serde_json::from_value(serde_json::json!({
            "schema_version": ATTESTED_TOOL_HOST_PROBE_SCHEMA_VERSION,
            "store_identity": handle.store_identity(),
            "probe_id": format!("probe-{}", "8".repeat(32)),
            "record_digest": "f".repeat(64),
        }))
        .expect("fabricated strict handle");
        assert!(application.attested_tool_host_probe(&fabricated).is_err());
    }

    #[test]
    fn missing_corrupt_or_raw_object_and_reference_fail_closed() {
        enum Corruption {
            MissingReference,
            CorruptReference,
            MissingObject,
            CorruptObject,
            RawTraceObject,
            CorruptReservation,
            MissingTransportReservation,
            AlteredTransportObject,
        }
        for (index, corruption) in [
            Corruption::MissingReference,
            Corruption::CorruptReference,
            Corruption::MissingObject,
            Corruption::CorruptObject,
            Corruption::RawTraceObject,
            Corruption::CorruptReservation,
            Corruption::MissingTransportReservation,
            Corruption::AlteredTransportObject,
        ]
        .into_iter()
        .enumerate()
        {
            let root = tempdir().expect("temporary root");
            let mut application = create_application(&root);
            let suffix = char::from(b'0' + u8::try_from(index).expect("bounded index"));
            let handle =
                execute_prepared(&mut application, material(suffix), write_nonce_and_trace)
                    .expect("handle");
            let directory = probe_directory(&root, handle.probe_id());
            let reference_path = directory.join(ATTESTATION_REFERENCE_FILE);
            let reservation_path = directory.join(RESERVATION_FILE);
            let mut stored_reference = reference(&root, &handle);
            let object_path = application
                .object_store
                .path_for(&stored_reference.object_digest)
                .expect("object path");
            match corruption {
                Corruption::MissingReference => {
                    fs::remove_file(&reference_path).expect("remove reference");
                }
                Corruption::CorruptReference => {
                    fs::write(&reference_path, b"not-json").expect("corrupt reference");
                }
                Corruption::MissingObject => {
                    fs::remove_file(object_path).expect("remove object");
                }
                Corruption::CorruptObject => {
                    fs::write(object_path, b"corrupt-object").expect("corrupt object");
                }
                Corruption::RawTraceObject => {
                    let attestation = application
                        .attested_tool_host_probe(&handle)
                        .expect("attestation before mutation");
                    let raw_trace = canonical_json(attestation.trace()).expect("raw trace bytes");
                    stored_reference.object_digest = application
                        .object_store
                        .put(&raw_trace)
                        .expect("raw trace object");
                    stored_reference.record_digest =
                        reference_digest(&stored_reference).expect("mutated reference digest");
                    fs::write(
                        &reference_path,
                        canonical_json(&stored_reference).expect("reference bytes"),
                    )
                    .expect("replace reference");
                    let mut forged = handle.clone();
                    forged.record_digest = stored_reference.record_digest;
                    assert!(matches!(
                        application.attested_tool_host_probe(&forged),
                        Err(ToolHostProbeAttestationError::AttestationObjectInvalid { .. })
                    ));
                    continue;
                }
                Corruption::CorruptReservation => {
                    fs::write(reservation_path, b"{}").expect("corrupt reservation");
                }
                Corruption::MissingTransportReservation => {
                    let mut value: serde_json::Value = serde_json::from_slice(
                        &fs::read(&reservation_path).expect("reservation bytes"),
                    )
                    .expect("reservation value");
                    value
                        .as_object_mut()
                        .expect("reservation object")
                        .remove("expected_transport");
                    fs::write(
                        reservation_path,
                        serde_json::to_vec(&value).expect("missing-transport bytes"),
                    )
                    .expect("replace reservation");
                }
                Corruption::AlteredTransportObject => {
                    let object_bytes = application
                        .object_store
                        .read(&stored_reference.object_digest)
                        .expect("attestation object");
                    let mut stored: StoredAttestedToolHostProbe =
                        decode_canonical(&object_bytes).expect("stored attestation");
                    stored.probe_transport.server_name =
                        "ymp.schema-identical-substitute".to_owned();
                    stored.probe_transport_digest = probe_transport_digest(&stored.probe_transport);
                    stored.runtime.probe_transport = stored.probe_transport.clone();
                    stored.runtime.probe_transport_digest = stored.probe_transport_digest.clone();
                    stored.trace.runtime = stored.runtime.clone();
                    stored.trace_digest =
                        digest_bytes(&canonical_json(&stored.trace).expect("trace bytes"));
                    stored_reference.object_digest = application
                        .object_store
                        .put(&canonical_json(&stored).expect("altered attestation bytes"))
                        .expect("altered attestation object");
                    stored_reference.record_digest =
                        reference_digest(&stored_reference).expect("altered reference digest");
                    fs::write(
                        &reference_path,
                        canonical_json(&stored_reference).expect("reference bytes"),
                    )
                    .expect("replace reference");
                    let mut forged = handle.clone();
                    forged.record_digest = stored_reference.record_digest;
                    assert!(matches!(
                        application.attested_tool_host_probe(&forged),
                        Err(ToolHostProbeAttestationError::AttestationObjectInvalid { .. })
                    ));
                    continue;
                }
            }
            assert!(
                application.attested_tool_host_probe(&handle).is_err(),
                "corruption {index} reloaded an attestation"
            );
        }
    }
}
