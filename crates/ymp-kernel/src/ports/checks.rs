//! Explicit observation and proposal contracts; neither carries a Journal writer.
use crate::registry::ReadinessView;
use ymp_domain::{
    Proposal, Result,
    identity::{Discovery, Readiness},
    journal::PolicySelection,
};

pub trait ReadinessProbe {
    fn selection(&self) -> &PolicySelection;
    fn probe(&self, view: &ReadinessView) -> Proposal<Readiness>;
}

/// A metadata-only native adapter contract. Implementations must not release a user
/// prompt, copy authentication or perform inference. Concrete adapters have their own tasks.
pub trait NativeDiscovery {
    fn discover(&self) -> Result<Discovery>;
}
