//! Compiled execution extensions. The caller retains admission and turn guards.
use crate::{ProviderEvent, TurnRequest, TurnResult};
use anyhow::Result;
use std::{future::Future, pin::Pin};
use tokio::sync::mpsc;
pub use ymp_core::ExecutionBackendIdentity;

pub type ExecutionFuture<'a> = Pin<Box<dyn Future<Output = Result<TurnResult>> + Send + 'a>>;

/// A trusted implementation of one already-admitted turn, without store access.
///
/// Keep identity stable for the implementation's lifetime; change its version
/// when execution semantics, private settings or continuation compatibility change.
/// Cancellation and timeout drop this future. Own subprocesses/resources within
/// the future and clean them up on drop; do not detach work. Emit only observed
/// settings and usage, never inferred native acknowledgements.
pub trait ExecutionBackend: Send + Sync {
    fn identity(&self) -> ExecutionBackendIdentity;

    /// Actual access enforced by this trusted backend for this request. The
    /// conservative default also covers custom backends that ignore read_only.
    fn workspace_access(&self, _request: &TurnRequest) -> ymp_core::WorkspaceAccess {
        ymp_core::WorkspaceAccess::WriteAll
    }

    fn execute(
        &self,
        request: TurnRequest,
        events: mpsc::UnboundedSender<ProviderEvent>,
    ) -> ExecutionFuture<'_>;
}

/// Existing native adapters, including the explicit offline mock provider.
#[derive(Debug, Default)]
pub struct NativeExecutionBackend;

impl ExecutionBackend for NativeExecutionBackend {
    fn identity(&self) -> ExecutionBackendIdentity {
        ExecutionBackendIdentity {
            id: "ymp.native".into(),
            version: env!("CARGO_PKG_VERSION").into(),
        }
    }

    fn workspace_access(&self, request: &TurnRequest) -> ymp_core::WorkspaceAccess {
        // ACP mode names are not a filesystem enforcement guarantee.
        if request.read_only && request.provider.kind != ymp_core::ProviderKind::Acp {
            ymp_core::WorkspaceAccess::ReadAll
        } else {
            ymp_core::WorkspaceAccess::WriteAll
        }
    }

    fn execute(
        &self,
        request: TurnRequest,
        events: mpsc::UnboundedSender<ProviderEvent>,
    ) -> ExecutionFuture<'_> {
        Box::pin(crate::run_native(request, events))
    }
}
