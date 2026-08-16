//! The host a run's origin participant is actually started on.
//!
//! The run decides *what* to start: its frozen record names one provider, engine and model, and
//! nothing on this level may move it. This module answers the two questions that record cannot —
//! which program on this host serves that route, and which program establishes the private
//! baseline the participant works over — and answers nothing else.
//!
//! **The route is matched, not approximated.** A frozen entry names an engine and a model. The
//! engine has to be one this build manages and one the operator admits; the model has to be the
//! model the managed profile is pinned to. A route naming a model the profile does not run is
//! refused by name rather than served by the profile's own model, because a participant started on
//! a model other than the one its run froze is a run doing work nobody authorized. The runtime
//! profiles refuse such a model themselves; refusing it here means the operator is told which
//! model was asked for and which one this host runs, before a process exists.
//!
//! The same two admissions that stand in front of every attempt stand here — the engine record and
//! the controller's own gate — and they are reached through [`crate::attempt`], which is the only
//! place this crate builds a driver.

use std::path::Path;

use ymp_application::{
    ParticipantRuntimes, PrivateWorkspaces, RouteUnavailable, WorkspaceNotEstablished,
};
use ymp_domain::pool::EntryIdentity;
use ymp_runtime_api::RuntimeDriver;
use ymp_runtime_claude::PINNED_CLAUDE_MODEL;
use ymp_runtime_codex::PINNED_CODEX_MODEL;
use ymp_runtime_registry::RegistryAddress;
use ymp_runtime_supervisor::{admit_workspace_program, initialize_private_git};

use crate::attempt::Route;

/// The managed runtime profiles of one product root, offered to the frozen routes of the runs that
/// stand under it.
#[derive(Clone, Debug)]
pub struct ManagedRuntimes {
    registry: RegistryAddress,
}

impl ManagedRuntimes {
    pub const fn under(registry: RegistryAddress) -> Self {
        Self { registry }
    }

    /// The model the managed profile of this route is pinned to. It is read from the runtime crates
    /// rather than restated here, so a profile that moves to another model moves this with it.
    const fn pinned_model(route: Route) -> &'static str {
        match route {
            Route::Codex => PINNED_CODEX_MODEL,
            Route::ClaudeCode => PINNED_CLAUDE_MODEL,
        }
    }
}

impl ParticipantRuntimes for ManagedRuntimes {
    fn driver_for(
        &self,
        route: &EntryIdentity,
    ) -> Result<Box<dyn RuntimeDriver>, RouteUnavailable> {
        let profile = Route::ALL
            .into_iter()
            .find(|profile| profile.engine().name() == route.engine)
            .ok_or_else(|| {
                RouteUnavailable::new(format!(
                    "this build manages no engine named {}, and the profiles it does manage are {}",
                    route.engine,
                    Route::ALL
                        .into_iter()
                        .map(Route::name)
                        .collect::<Vec<_>>()
                        .join(", ")
                ))
            })?;
        let pinned = Self::pinned_model(profile);
        if route.model != pinned {
            return Err(RouteUnavailable::new(format!(
                "the {} profile of this build runs {pinned}, and this run froze {}",
                profile.name(),
                route.model
            )));
        }
        crate::attempt::driver(profile, &self.registry)
            .map_err(|error| RouteUnavailable::new(error.to_string()))
    }
}

/// The private baseline the product establishes over every materialized copy: a repository of the
/// copy's own, with the copy committed to it.
///
/// It is the supervisor's own contract, reached through the supervisor's own admitted program, so
/// the copy a participant works over and the copy a managed attempt works over stand on the same
/// baseline and are read the same way afterwards.
#[derive(Clone, Copy, Debug, Default)]
pub struct PrivateGit;

impl PrivateWorkspaces for PrivateGit {
    fn establish(&self, workspace: &Path) -> Result<(), WorkspaceNotEstablished> {
        let program = admit_workspace_program()
            .map_err(|error| WorkspaceNotEstablished(error.to_string()))?;
        initialize_private_git(workspace, &program)
            .map_err(|error| WorkspaceNotEstablished(error.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::{ManagedRuntimes, PINNED_CLAUDE_MODEL};
    use ymp_application::ParticipantRuntimes;
    use ymp_domain::pool::EntryIdentity;
    use ymp_runtime_registry::RegistryAddress;

    /// The model a frozen route names is matched, and a route naming another is refused here by
    /// name rather than served by the profile's own model.
    ///
    /// It is checked at this level because this is the only level that knows both: the frozen entry
    /// and the model the managed profile is pinned to. The refusals reached through
    /// `admit_route` — an engine this build does not manage, an engine the operator holds back —
    /// stand in `ymp-application` and are measured against a run, and they answer a different
    /// question: whether this host will start the engine at all. Neither of them reads a model, so
    /// a route naming an admitted engine and a model the profile does not run would pass every one
    /// of them and reach the driver.
    ///
    /// The check that must fail: drop the comparison and let the profile serve whatever model it is
    /// asked for. `claude-sonnet-5` is then run by a profile pinned to `claude-opus-5`, and the run
    /// does work on a model it never froze.
    #[test]
    fn a_route_naming_another_model_is_refused_by_name() {
        let root = tempfile::tempdir().expect("temporary root");
        let host = ManagedRuntimes::under(RegistryAddress::Root(root.path().to_path_buf()));

        let refusal = host
            .driver_for(&EntryIdentity::new(
                "anthropic",
                "claude-code",
                "claude-sonnet-5",
            ))
            .err()
            .expect("a model this profile does not run was served");
        let stated = refusal.0;
        assert!(stated.contains("claude-sonnet-5"), "{stated}");
        assert!(stated.contains(PINNED_CLAUDE_MODEL), "{stated}");

        // An engine no profile of this build carries is refused with what it does carry, so the
        // operator reads which profiles exist rather than only that theirs does not.
        let unknown = host
            .driver_for(&EntryIdentity::new("anthropic", "gemini-cli", "gemini-3"))
            .err()
            .expect("an engine this build does not manage was served")
            .0;
        assert!(unknown.contains("gemini-cli"), "{unknown}");
        assert!(unknown.contains("claude-code"), "{unknown}");

        // The positive half: the pinned model is not what refuses the route this host does serve.
        let pinned = host
            .driver_for(&EntryIdentity::new(
                "anthropic",
                "claude-code",
                PINNED_CLAUDE_MODEL,
            ))
            .err()
            .map(|refusal| refusal.0);
        assert!(
            !pinned
                .as_deref()
                .is_some_and(|stated| stated.contains("and this run froze")),
            "the pinned model was refused as another model: {pinned:?}"
        );
    }
}
