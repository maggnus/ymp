//! Filesystem prerequisite observations and a no-inference readiness proposal.
use std::{fs, path::Path};
use ymp_domain::{
    Denial, Id, Proposal, Result,
    identity::{DependencyKind, DependencyObservation, DependencyState, Provider, Readiness},
    journal::PolicySelection,
};
use ymp_kernel::{
    ports::checks::ReadinessProbe,
    registry::{ReadinessView, dependency_exclusion},
};

pub struct StaticDependencyProbe {
    selection: PolicySelection,
}
impl StaticDependencyProbe {
    pub fn new() -> Result<Self> {
        Ok(Self {
            selection: PolicySelection::new(
                "ReadinessProbe",
                "StaticDependencyProbe",
                "1",
                serde_json::json!({}),
            )?,
        })
    }
    /// Inspect a declared path without spawning it or reading credentials.
    /// An executable bit is a static prerequisite, not proof that a future spawn will succeed.
    pub fn capture(
        provider: Id<Provider>,
        model: Option<String>,
        path: &Path,
        kind: DependencyKind,
        at: u64,
    ) -> Result<DependencyObservation> {
        if !path.is_absolute() {
            return Err(Denial::new(
                "dependency_path",
                "A dependency requires an explicit absolute path",
            ));
        }
        let state = match fs::metadata(path) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => DependencyState::Missing,
            Err(_) => DependencyState::Unreadable,
            Ok(metadata) => match kind {
                DependencyKind::Directory if metadata.is_dir() => DependencyState::Available,
                DependencyKind::File if metadata.is_file() => DependencyState::Available,
                DependencyKind::Executable if metadata.is_file() => {
                    #[cfg(unix)]
                    {
                        use std::os::unix::fs::PermissionsExt;
                        if metadata.permissions().mode() & 0o111 != 0 {
                            DependencyState::Available
                        } else {
                            DependencyState::NotExecutable
                        }
                    }
                    #[cfg(not(unix))]
                    {
                        DependencyState::NotExecutable
                    }
                }
                _ => DependencyState::WrongKind,
            },
        };
        Ok(DependencyObservation {
            provider,
            model,
            path: path.to_owned(),
            kind,
            state,
            observed_at: at,
        })
    }
}
impl ReadinessProbe for StaticDependencyProbe {
    fn selection(&self) -> &PolicySelection {
        &self.selection
    }
    fn probe(&self, view: &ReadinessView) -> Proposal<Readiness> {
        Proposal {
            value: dependency_exclusion(&view.dependencies)
                .map(Readiness::NotReady)
                .unwrap_or(Readiness::Ready),
            rationale: "Evaluate recorded static dependencies without invoking a model".into(),
            basis: vec![],
            policy: self.selection.policy.clone(),
        }
    }
}

/// Capture the input identity before invoking a replaceable probe.
pub fn readiness_response(
    probe: &dyn ReadinessProbe,
    view: &ReadinessView,
) -> Result<ymp_kernel::registry::ReadinessResponse> {
    let profile = view.profile.clone();
    let input = ymp_domain::Digest::of_value(view)?;
    let proposal = probe.probe(view);
    Ok(ymp_kernel::registry::ReadinessResponse {
        profile,
        input,
        proposal,
    })
}
