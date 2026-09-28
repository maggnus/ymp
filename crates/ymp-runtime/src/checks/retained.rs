//! Process-free ExactBytes runner over immutable content-addressed storage.
use std::collections::BTreeMap;
use ymp_domain::{
    Denial, Digest, Result, assignment::ErrorClass, journal::PolicySelection, verification::*,
};
use ymp_kernel::{
    journal::ContentStore,
    ports::checks::{CheckExecution, CheckObservation, CheckRunner},
};

pub struct RetainedBytes;
impl CheckRunner for RetainedBytes {
    fn environment(&self) -> Result<CheckEnvironment> {
        Ok(CheckEnvironment {
            runner: PolicySelection::new(
                "CheckRunner",
                "RetainedBytes",
                "1",
                serde_json::json!({}),
            )?,
            platform: "content-addressed storage; no host execution".into(),
            identity: BTreeMap::from([(
                "algorithm".into(),
                "SHA-256 over retained file bytes; missing file is an observation".into(),
            )]),
            limits: CheckLimits {
                timeout_ms: 1000,
                output_bytes: 1,
            },
        })
    }
    fn run(
        &self,
        execution: &CheckExecution<'_>,
        store: &dyn ContentStore,
    ) -> Result<CheckObservation> {
        if *execution.environment != self.environment()? {
            return Err(Denial::new(
                "check_environment",
                "RetainedBytes environment differs",
            ));
        }
        let kind = match &execution.check.spec {
            CheckSpec::ExactBytes { path, .. } => {
                let digest = execution
                    .target
                    .tree
                    .files
                    .get(path)
                    .map(|file| {
                        let bytes = store.get(&file.digest, file.bytes as usize)?;
                        if bytes.len() as u64 != file.bytes || Digest::of(&bytes) != file.digest {
                            return Err(Denial::new(
                                "check_content",
                                "Retained file content disagrees",
                            ));
                        }
                        Ok(Digest::of(bytes))
                    })
                    .transpose()?;
                CheckObservationKind::ExactBytes(digest)
            }
            CheckSpec::Command { .. } => CheckObservationKind::Error {
                class: ErrorClass::Environment,
                reason: "RetainedBytes does not execute commands".into(),
            },
        };
        Ok(CheckObservation {
            check: execution.check.reference(),
            target: execution.target.reference()?,
            environment: Digest::of_value(execution.environment)?,
            kind,
            stdout: vec![],
            stderr: vec![],
        })
    }
}
