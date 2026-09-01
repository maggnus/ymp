#![forbid(unsafe_code)]

use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::TempDir;
use ymp_application::{Application, AttestedToolHostProbeHandle};
use ymp_domain::Budget;
use ymp_runtime_registry::{Engine, EngineRecord, RegistryAddress};

const ADMISSION_V2_DIGEST: &str =
    "d354f20c8482cd5df7e33fab70dcd267befb621ef09647d430dc40f3924b8ea2";

fn required_path(name: &str) -> PathBuf {
    std::env::var_os(name)
        .map(PathBuf::from)
        .unwrap_or_else(|| panic!("{name} must name the isolated live-probe input"))
}

fn create_directory(path: &Path) {
    fs::create_dir_all(path)
        .unwrap_or_else(|error| panic!("create isolated directory {}: {error}", path.display()));
}

/// This test is the only admitted live path for W1-EVL-04n. It is ignored in every ordinary test
/// run and is executed once, by exact name, only after the deterministic matrix is green.
#[test]
#[ignore = "single owner-admitted live Codex compatibility probe"]
fn one_live_controller_attested_codex_tool_host_probe() {
    assert_eq!(
        std::env::var("YMP_ADMIT_ONE_LIVE_TOOL_HOST_PROBE").as_deref(),
        Ok("W1-EVL-04n"),
        "the explicit one-call admission marker is required"
    );
    let project = required_path("YMP_LIVE_PROJECT");
    let home = required_path("HOME");
    let ymp_home = required_path("YMP_HOME");
    let temporary = required_path("TMPDIR");
    let build = required_path("CARGO_TARGET_DIR");
    let export = required_path("YMP_LIVE_EXPORT");
    let codex = required_path("YMP_LIVE_CODEX_BIN")
        .canonicalize()
        .expect("canonical live Codex executable");
    let auth_source = required_path("YMP_LIVE_CODEX_AUTH_SOURCE");
    for directory in [&project, &home, &ymp_home, &temporary, &build, &export] {
        create_directory(directory);
    }
    let isolated_codex_home = home.join(".codex");
    create_directory(&isolated_codex_home);
    let isolated_auth = isolated_codex_home.join("auth.json");
    fs::copy(&auth_source, &isolated_auth)
        .expect("copy the live credential into the isolated synthetic home");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        fs::set_permissions(&isolated_auth, fs::Permissions::from_mode(0o600))
            .expect("make the isolated credential private");
    }

    let ephemeral = TempDir::new_in(&temporary).expect("short disposable live-probe root");
    let data_root = ephemeral.path().join("application-store");
    let application = Application::create(
        &data_root,
        "run-w1-evl-04n-live-tool-host-probe",
        Budget::new(1, 0),
    )
    .expect("create isolated Application store");
    drop(application);
    let registry = RegistryAddress::Store(data_root.clone()).registry();
    let mut record = EngineRecord::seeded(Engine::Codex);
    record.enabled = true;
    record.disabled_reason = None;
    registry
        .write(Engine::Codex, &record)
        .expect("admit Codex only in the isolated registry");

    let codex_directory = codex.parent().expect("Codex executable directory");
    let runtime_path = format!(
        "{}:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin",
        codex_directory.display()
    );
    let output = Command::new(env!("CARGO_BIN_EXE_ymp"))
        .args([
            "--data-root",
            data_root.to_str().expect("UTF-8 data root"),
            "internal",
            "tool-host-probe",
            "--admission-manifest-digest",
            ADMISSION_V2_DIGEST,
        ])
        .current_dir(&project)
        .env_clear()
        .env("HOME", &home)
        .env("CODEX_HOME", &isolated_codex_home)
        .env("YMP_HOME", &ymp_home)
        .env("TMPDIR", &temporary)
        .env("PATH", runtime_path)
        .env("NO_COLOR", "1")
        .output()
        .expect("spawn the actual built ymp binary");
    assert!(
        output.status.success(),
        "STOP: live tool-host probe failed: status={} stderr={}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );

    let line = String::from_utf8(output.stdout).expect("UTF-8 controller output");
    let exported: Value = serde_json::from_str(line.trim()).expect("controller handle JSON");
    assert_eq!(exported["type"], "tool_host_probe_handle");
    let handle: AttestedToolHostProbeHandle =
        serde_json::from_value(exported["handle"].clone()).expect("strict untrusted handle");
    let application = Application::open(&data_root).expect("reopen isolated Application store");
    let attestation = application
        .attested_tool_host_probe(&handle)
        .expect("controller read-back produced a reloadable attestation");
    let trace = attestation.trace();
    assert_eq!(attestation.admission_manifest_digest(), ADMISSION_V2_DIGEST);
    assert_eq!(trace.model_calls, 1);
    assert_eq!(trace.tool_event_digests.len(), 2);
    assert_eq!(
        trace.runtime.runtime_kind,
        ymp_runtime_api::RuntimeKind::Codex
    );
    assert_eq!(trace.runtime.route, "openai_responses_chatgpt");
    assert_eq!(trace.runtime.profile, "ymp-codex-low-v2");
    assert!(!trace.runtime.cli_version.is_empty());
    assert!(attestation.prelaunch_destination_absent());
    assert_eq!(attestation.nonce_digest(), attestation.readback_digest());
    assert_eq!(trace.input_digest, trace.output_digest);
    assert_eq!(trace.usage.in_flight_excess, Default::default());

    println!(
        "{}",
        serde_json::to_string(&serde_json::json!({
            "kind": "w1_evl_04n_live_tool_host_probe",
            "candidate_executable": env!("CARGO_BIN_EXE_ymp"),
            "admission_manifest_digest": attestation.admission_manifest_digest(),
            "runtime": attestation.runtime(),
            "probe_transport_digest": attestation.probe_transport_digest(),
            "tool_events": trace.tool_event_digests,
            "nonce_digest": attestation.nonce_digest(),
            "readback_digest": attestation.readback_digest(),
            "readback_bytes": attestation.readback_bytes(),
            "usage": attestation.usage(),
            "cost": attestation.cost(),
            "wall_time_ms": attestation.wall_time_ms(),
            "terminal": attestation.terminal(),
            "trust": trace.trust,
            "handle": handle,
            "attestation": {
                "schema_version": attestation.schema_version(),
                "store_identity": attestation.store_identity(),
                "run_id": attestation.run_id(),
                "probe_id": attestation.probe_id(),
                "invocation_id": attestation.invocation_id(),
                "trace_digest": attestation.trace_digest(),
                "reservation_digest": attestation.reservation_digest(),
                "replay_key": attestation.replay_key(),
            },
            "model_ready": false,
            "task_output": null,
            "arm_output": null,
        }))
        .expect("serialize live evidence")
    );
}
