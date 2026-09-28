//! Replaceable execution observations. Adapters cannot supply an acceptance verdict.
use crate::journal::ContentStore;
use ymp_domain::{Digest, Ref, Result, verification::*, workspace::Snapshot};

pub struct CheckExecution<'a> {
    pub check: &'a Check,
    pub target: &'a Snapshot,
    pub verifier: Option<&'a Snapshot>,
    pub environment: &'a CheckEnvironment,
}
pub struct CheckObservation {
    pub check: Ref,
    pub target: Ref,
    pub environment: Digest,
    pub kind: CheckObservationKind,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}
pub trait CheckRunner: Send + Sync {
    fn environment(&self) -> Result<CheckEnvironment>;
    fn run(
        &self,
        execution: &CheckExecution<'_>,
        store: &dyn ContentStore,
    ) -> Result<CheckObservation>;
}

/// Readiness proposals never carry a journal writer or execution authority.
pub trait ReadinessProbe {
    fn selection(&self) -> &ymp_domain::journal::PolicySelection;
    fn probe(
        &self,
        view: &crate::registry::ReadinessView,
    ) -> ymp_domain::Proposal<ymp_domain::identity::Readiness>;
}
/// Metadata-only native discovery; no prompts, authentication copies or inference.
pub trait NativeDiscovery {
    fn discover(&self) -> Result<ymp_domain::identity::Discovery>;
}
