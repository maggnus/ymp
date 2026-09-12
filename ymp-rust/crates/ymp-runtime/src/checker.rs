//! Replaceable execution boundary. Implementations return raw outcomes; the
//! runtime owns all evidence capture, scope, freshness, acceptance and credit.
use anyhow::Result;
use std::{future::Future, path::Path, pin::Pin};
use tokio_util::sync::CancellationToken;
use ymp_core::*;

#[derive(Debug)]
pub struct CheckExecution {
    pub exit_code: Option<i32>,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

pub type CheckFuture<'a> = Pin<Box<dyn Future<Output = Result<CheckExecution>> + Send + 'a>>;

pub trait ConfirmationChecker: Send + Sync {
    fn identity(&self) -> CheckerIdentity;
    fn execute<'a>(
        &'a self,
        check: &'a TrustedCheck,
        directory: &'a Path,
        artifacts: &'a [FileSnapshot],
        inputs: &'a [FileSnapshot],
        cancel: CancellationToken,
    ) -> CheckFuture<'a>;
}

pub struct BuiltinConfirmationChecker;
impl ConfirmationChecker for BuiltinConfirmationChecker {
    fn identity(&self) -> CheckerIdentity {
        CheckerIdentity {
            id: "ymp.builtin-confirmation".into(),
            version: "1".into(),
        }
    }
    fn execute<'a>(
        &'a self,
        check: &'a TrustedCheck,
        directory: &'a Path,
        artifacts: &'a [FileSnapshot],
        inputs: &'a [FileSnapshot],
        cancel: CancellationToken,
    ) -> CheckFuture<'a> {
        Box::pin(async move {
            let success = match &check.assertion {
                CheckAssertion::ExactBytes { artifact, expected } => {
                    artifacts
                        .iter()
                        .find(|s| &s.path == artifact)
                        .and_then(|s| s.bytes.as_ref())
                        == Some(expected)
                }
                CheckAssertion::MatchesInput { artifact, input } => {
                    let actual = artifacts
                        .iter()
                        .find(|s| &s.path == artifact)
                        .and_then(|s| s.bytes.as_ref());
                    actual.is_some()
                        && actual
                            == inputs
                                .iter()
                                .find(|s| &s.path == input)
                                .and_then(|s| s.bytes.as_ref())
                }
                CheckAssertion::Command { program, args, .. } => {
                    let mut command = tokio::process::Command::new(program);
                    command
                        .args(
                            args.iter()
                                .map(|a| a.replace("{workdir}", &directory.to_string_lossy())),
                        )
                        .current_dir(directory)
                        .kill_on_drop(true);
                    let output = tokio::select! { _=cancel.cancelled()=>anyhow::bail!("Check cancelled"), output=command.output()=>output? };
                    return Ok(CheckExecution {
                        exit_code: output.status.code(),
                        stdout: output.stdout,
                        stderr: output.stderr,
                    });
                }
            };
            Ok(CheckExecution {
                exit_code: Some(if success { 0 } else { 1 }),
                stdout: format!("Typed assertion matched: {success}").into_bytes(),
                stderr: vec![],
            })
        })
    }
}
