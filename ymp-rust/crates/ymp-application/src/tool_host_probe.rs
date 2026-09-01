use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::{ErrorKind, Read, Write};
use std::path::{Path, PathBuf};
use thiserror::Error;
use uuid::Uuid;
use ymp_domain::digest_bytes;
use ymp_runtime_api::{
    CancellationToken, ProbeTransportIdentity, TOOL_HOST_PROBE_ENVIRONMENT,
    TOOL_HOST_PROBE_INTERNAL_ARGUMENTS, TOOL_HOST_PROBE_INTERNAL_SUBCOMMAND,
    TOOL_HOST_PROBE_MCP_PROTOCOL_VERSION, TOOL_HOST_PROBE_SCHEMA_VERSION,
    TOOL_HOST_PROBE_SERVER_VERSION, TOOL_HOST_PROBE_WORKSPACE_SERVER, ToolHostProbeCost,
    ToolHostProbeCostAvailability, ToolHostProbeError, ToolHostProbeRequest,
    ToolHostProbeResourceVector, ToolHostProbeRuntimeIdentity, ToolHostProbeTerminal,
    ToolHostProbeTool, ToolHostProbeTrace, ToolHostProbeTrust, Usage, evidence_digest,
    probe_transport_digest, tool_host_probe_tool_schema_digest,
};
use ymp_storage::ObjectStoreError;

use crate::Application;

pub const ATTESTED_TOOL_HOST_PROBE_SCHEMA_VERSION: u32 = 2;
pub const TOOL_HOST_PROBE_HANDLE_EXPORT: &str = "exports/tool-host-probe.handle.json";

const TOOL_HOST_PROBES_DIRECTORY: &str = "runtime-evidence/tool-host-probes";
const RESERVATION_FILE: &str = "reservation.json";
const ATTESTATION_REFERENCE_FILE: &str = "attestation.ref";
const WORKSPACE_DIRECTORY: &str = "workspace";

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
    expected_transport: &'a ProbeTransportIdentity,
    probe_transport_digest: &'a str,
    resource_reservation: &'a ToolHostProbeResourceVector,
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
            &expected_transport,
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
            expected_runtime,
            cancellation: CancellationToken::default(),
        };
        let trace = executor(&workspace, runtime_request)?;
        validate_trace(&trace, &request, &material)?;
        validate_workspace_effect(&workspace, &material.relative_path)?;
        let readback = read_controller_destination(&workspace, &material.relative_path)?;
        if readback != material.nonce.as_bytes() {
            return Err(ToolHostProbeAttestationError::ReadbackMismatch);
        }

        let readback_digest = digest_bytes(&readback);
        let readback_bytes = u64::try_from(readback.len()).map_err(|_| {
            ToolHostProbeAttestationError::TraceMismatch {
                field: "readback_bytes",
            }
        })?;
        let charged = charged_vector(&trace);
        ensure_charged_within(&charged, &request.resource_reservation)?;
        let trace_digest = digest_bytes(&canonical_json(&trace)?);
        let attestation = AttestedToolHostProbe {
            schema_version: ATTESTED_TOOL_HOST_PROBE_SCHEMA_VERSION,
            store_identity: store_identity.clone(),
            run_id: run_id.clone(),
            admission_manifest_digest: request.admission_manifest_digest,
            probe_id: material.probe_id.clone(),
            invocation_id: material.invocation_id,
            nonce_digest,
            readback_digest,
            readback_bytes,
            prelaunch_destination_absent: true,
            relative_path: material.relative_path,
            trace: trace.clone(),
            trace_digest,
            runtime: trace.runtime.clone(),
            probe_transport: expected_transport,
            probe_transport_digest: trace.runtime.probe_transport_digest.clone(),
            reservation_digest: reservation_digest.clone(),
            resource_reservation: trace.resource_reservation.clone(),
            charged,
            usage: trace.usage.clone(),
            wall_time_ms: trace.usage.wall_time_ms,
            cost: trace.cost.clone(),
            terminal: trace.terminal,
            replay_key,
        };
        let attestation_bytes = canonical_json(&attestation)?;
        let object_digest = self.object_store.put(&attestation_bytes)?;

        let mut reference = AttestationReference {
            schema_version: ATTESTED_TOOL_HOST_PROBE_SCHEMA_VERSION,
            store_identity,
            run_id,
            probe_id: material.probe_id.clone(),
            reservation_digest,
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

        Ok(AttestedToolHostProbeHandle {
            schema_version: ATTESTED_TOOL_HOST_PROBE_SCHEMA_VERSION,
            store_identity: reference.store_identity,
            probe_id: reference.probe_id,
            record_digest: reference.record_digest,
        })
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
                .is_some_and(|expected| trace.runtime == *expected),
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
    validate_usage_and_cost(&trace.usage, &trace.cost, &request.resource_reservation)?;
    validate_tool_event_digests(trace, material)?;
    if !is_sha256(&trace.event_digest) {
        return Err(trace_mismatch("event_digest"));
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
                == probe_transport_digest(&reservation.expected_transport),
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
    let expected_replay_key = replay_key(
        store_identity,
        run_id,
        &reservation.admission_manifest_digest,
        &material,
        &reservation.nonce_digest,
        &reservation.expected_transport,
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
        ("runtime", attestation.runtime == attestation.trace.runtime),
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
    expected_transport: &ProbeTransportIdentity,
    resource_reservation: &ToolHostProbeResourceVector,
) -> Result<String, serde_json::Error> {
    let probe_transport_digest = probe_transport_digest(expected_transport);
    Ok(digest_bytes(&canonical_json(&ReplayKeyMaterial {
        schema_version: ATTESTED_TOOL_HOST_PROBE_SCHEMA_VERSION,
        store_identity,
        run_id,
        admission_manifest_digest,
        probe_id: &material.probe_id,
        invocation_id: &material.invocation_id,
        nonce_digest,
        relative_path: &material.relative_path,
        expected_transport,
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
    use ymp_runtime_api::{RuntimeKind, ToolHostProbeEffect, ToolHostProbeToolEventDigest};

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
            assert_no_attestation(&root, &material.probe_id);
        }
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
