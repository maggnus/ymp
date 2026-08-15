//! Which runtime profile a run is routed to, and the managed attempt it starts.
//!
//! A run is done by one of the runtime profiles this host can start. Which one is not inferred
//! from what happens to be installed: the operator names it, or exactly one managed profile is
//! ready and that is the one route there is. A profile that was named and is not ready stops the
//! run where it is named; nothing is ever routed to a second profile in its place, because a run
//! done by a different agent is a different run and the operator authorized this one.
//!
//! The attempt itself belongs to `ymp-runtime-supervisor`: it captures the source, starts the
//! attempt in the journal, holds the private workspace, admits the programs the launch enters and
//! the utilities the run ends its processes with, writes the runtime evidence, and keeps the
//! kernel record of the process slice. This module builds the driver behind the controller's own
//! gate and hands the work over; it neither starts a process itself nor decides anything the
//! supervisor decides.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use anyhow::Context;
use ymp_application::Application;
use ymp_domain::ContractBinding;
use ymp_domain::contract::ContractDocument;
use ymp_runtime_api::{RuntimeDriver, RuntimeKind};
use ymp_runtime_claude::ClaudeRuntime;
use ymp_runtime_codex::CodexRuntime;
use ymp_runtime_registry::{Engine, RegistryAddress};
use ymp_runtime_supervisor::{
    ManagedCandidateRequest, ManagedContract, ManagedRunHandle, ManagedVerifier,
    admit_runtime_start, start_managed_candidate,
};

use crate::runtimes::Report;

/// The managed runtime profiles a run can be routed to.
///
/// The in-process fixture runtime is deliberately absent. It attests nothing about the programs a
/// launch enters, so a candidate a run of it assembled would carry no evidence of the program that
/// wrote it, and the controller refuses to start one.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Route {
    Codex,
    ClaudeCode,
}

impl Route {
    /// Every route, in the order the runtimes page lists them.
    pub const ALL: [Self; 2] = [Self::Codex, Self::ClaudeCode];

    /// The profile name, spelled as the runtimes page spells it and as a request line states it.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Codex => "codex",
            Self::ClaudeCode => "claude-code",
        }
    }

    pub const fn kind(self) -> RuntimeKind {
        match self {
            Self::Codex => RuntimeKind::Codex,
            Self::ClaudeCode => RuntimeKind::ClaudeCode,
        }
    }

    /// The registry entity this route starts. Every route is an engine, which is what makes the
    /// registry's enabled flag reachable from every path that would start one.
    pub const fn engine(self) -> Engine {
        match self {
            Self::Codex => Engine::Codex,
            Self::ClaudeCode => Engine::ClaudeCode,
        }
    }

    /// The route a stated name selects. Spelling is exact: a name that selects nothing is refused
    /// rather than resolved to the nearest profile.
    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|route| route.name() == value.trim())
    }
}

/// Where a run would be routed, given what the operator named and what this host offers.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Routing {
    /// The profile this run would use.
    Ready(Route),
    /// No run can be routed, in the words the operator is shown.
    Refused(String),
    /// The host has not been probed yet, so nothing is known about any profile.
    Probing,
}

impl Routing {
    pub fn route(&self) -> Option<Route> {
        match self {
            Self::Ready(route) => Some(*route),
            _ => None,
        }
    }
}

/// Resolve the route of a run before anything is spent.
///
/// A named profile is held to: it is used when it is ready, and it stops the run when it is not.
/// With no profile named, a single ready managed profile is the only route there is and is taken;
/// several ready profiles are a choice the operator has to make, because the product would
/// otherwise pick the agent that does the work.
pub fn resolve(named: Option<Route>, report: Option<&Report>) -> Routing {
    let Some(report) = report else {
        return Routing::Probing;
    };
    if let Some(route) = named {
        return match state_of(route, report) {
            Some((true, _)) => Routing::Ready(route),
            Some((false, detail)) => Routing::Refused(format!(
                "the {} profile is not ready — {detail}. It is the profile you named, so nothing \
                 else is used in its place: make it ready, or name another with `runtime <name>`.",
                route.name()
            )),
            None => Routing::Refused(format!(
                "this host reported nothing about the {} profile",
                route.name()
            )),
        };
    }
    let ready: Vec<Route> = Route::ALL
        .into_iter()
        .filter(|route| state_of(*route, report).is_some_and(|(ready, _)| ready))
        .collect();
    match ready.as_slice() {
        [route] => Routing::Ready(*route),
        [] => Routing::Refused(format!(
            "no runtime profile on this host can do the work — {}. Nothing is started.",
            Route::ALL
                .into_iter()
                .map(|route| match state_of(route, report) {
                    Some((_, detail)) => format!("{}: {detail}", route.name()),
                    None => format!("{}: not reported by this host", route.name()),
                })
                .collect::<Vec<_>>()
                .join(" · ")
        )),
        several => Routing::Refused(format!(
            "more than one runtime profile is ready ({}) — name the one this run uses with \
             `runtime <name>`, because which agent does the work is yours to decide",
            several
                .iter()
                .map(|route| route.name())
                .collect::<Vec<_>>()
                .join(", ")
        )),
    }
}

/// The routing as a surface states it: the profile that would do the work, and what is true about
/// the choice either way.
///
/// Every surface that shows the routing reads it from here, so the coverage map, the transcript
/// and the equivalent command cannot state different things about the same host.
pub fn routing_facts(named: Option<Route>, report: Option<&Report>) -> (Option<String>, String) {
    match resolve(named, report) {
        Routing::Ready(route) => (
            Some(route.name().to_owned()),
            format!("{} would do the work of this run", route.name()),
        ),
        Routing::Refused(reason) => (None, reason),
        Routing::Probing => (
            None,
            "the runtime profiles of this host have not been probed yet".to_owned(),
        ),
    }
}

/// Whether the probe found this profile ready, and what it reported about it.
fn state_of(route: Route, report: &Report) -> Option<(bool, String)> {
    report
        .profiles
        .iter()
        .find(|profile| profile.name == route.name())
        .map(|profile| (profile.ready(), profile.detail.clone()))
}

/// Start the managed attempt of this run on the named profile.
///
/// What the agent is given is read from the store: the contract object the journal bound the run
/// to, and nothing the session happens to hold. A later process therefore starts the attempt of a
/// run it did not authorize against exactly what was authorized, and a store whose contract object
/// no longer matches its binding starts nothing.
///
/// The driver is built here and nowhere else in this crate, and it passes the controller's own
/// gate before it is handed over: the runtime must attest the programs its launch enters, and the
/// utilities the run observes and ends its own processes with must be admitted.
pub fn start(
    application: Arc<Mutex<Application>>,
    route: Route,
    registry: &RegistryAddress,
) -> anyhow::Result<ManagedRunHandle> {
    let contract = {
        let application = application
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let (binding, document) = application
            .approved_contract()
            .context("read the contract this run was approved against")?;
        managed_contract(&binding, &document)
    };
    let driver = driver(route, registry)?;
    let bridge_executable =
        std::env::current_exe().context("resolve the running ymp executable")?;
    start_managed_candidate(
        application,
        driver,
        ManagedCandidateRequest {
            contract,
            bridge_executable,
        },
    )
}

/// The only place this crate builds a runtime driver, and therefore the only place a runtime can
/// be started from.
///
/// Two admissions stand in front of every driver, and they answer different questions. The
/// registry answers whether this host admits the engine at all, and refuses a disabled one with
/// the reason the operator recorded — before the driver is built, so a disabled engine starts no
/// process even where the routing that led here was stale. [`admit_runtime_start`] then answers
/// whether the driver attests what it launches.
fn driver(route: Route, registry: &RegistryAddress) -> anyhow::Result<Box<dyn RuntimeDriver>> {
    registry.registry().admit(route.engine())?;
    let driver: Box<dyn RuntimeDriver> = match route {
        Route::Codex => Box::new(CodexRuntime::default()),
        Route::ClaudeCode => Box::new(ClaudeRuntime::default()),
    };
    admit_runtime_start(driver.kind())?;
    Ok(driver)
}

/// The approved contract, as the controller reads one. Every value comes from the stored object
/// and its journal binding; nothing is added that the operator did not authorize.
fn managed_contract(binding: &ContractBinding, document: &ContractDocument) -> ManagedContract {
    ManagedContract {
        contract_id: binding.contract_id.clone(),
        contract_digest: binding.contract_digest.clone(),
        source: document.source.clone(),
        prompt: document.prompt.clone(),
        capture_exclusions: document.capture_exclusions.clone(),
        verifier: Some(ManagedVerifier {
            program: document.verifier.program.clone(),
            arguments: document.verifier.arguments.clone(),
            negative_control: document.verifier.negative_control.clone(),
            oracle_digest: document.verifier.oracle_digest.clone(),
            wall_time_ms: document.verifier.wall_time_ms,
            output_limit_bytes: document.verifier.output_limit_bytes,
        }),
    }
}

/// Where a run puts the candidate it hands to the verifier, under the store's own root.
pub fn verification_inputs(data_root: &std::path::Path) -> PathBuf {
    data_root.join("verification-inputs")
}

#[cfg(test)]
mod tests {
    use super::{Engine, RegistryAddress, Route, Routing, driver, resolve};
    use crate::runtimes::{EngineFacts, ProfileFacts, Report};
    use ymp_runtime_api::Readiness;
    use ymp_runtime_registry::ModelCatalog;
    use ymp_runtime_registry::Registry;

    fn profile(name: &str, readiness: Readiness) -> ProfileFacts {
        ProfileFacts {
            name: name.to_owned(),
            runtime: name.to_owned(),
            model_route: None,
            executable: format!("/nonexistent/{name}"),
            version: None,
            readiness,
            detail: format!("{name} probe detail"),
            registry: Route::parse(name).map(|route| EngineFacts {
                engine: route.engine(),
                enabled: true,
                disabled_reason: None,
                models: ModelCatalog::default(),
            }),
        }
    }

    fn report(codex: Readiness, claude: Readiness) -> Report {
        Report {
            profiles: vec![
                profile("fake", Readiness::Ready),
                profile("codex", codex),
                profile("claude-code", claude),
            ],
        }
    }

    #[test]
    fn one_ready_managed_profile_is_the_only_route_there_is() {
        assert_eq!(
            resolve(
                None,
                Some(&report(Readiness::Ready, Readiness::NotInstalled))
            ),
            Routing::Ready(Route::Codex)
        );
    }

    /// The fixture profile is ready in every report and is never a route: a run routed to it would
    /// carry no evidence of the program that wrote its candidate.
    #[test]
    fn the_fixture_profile_is_never_routed_to() {
        let Routing::Refused(reason) = resolve(
            None,
            Some(&report(Readiness::NotInstalled, Readiness::Unauthenticated)),
        ) else {
            panic!("a run was routed with no managed profile ready");
        };
        assert!(!reason.contains("fake"), "{reason}");
        assert!(
            reason.contains("codex") && reason.contains("claude-code"),
            "{reason}"
        );
    }

    #[test]
    fn a_named_profile_that_is_not_ready_stops_the_run_instead_of_being_replaced() {
        let Routing::Refused(reason) = resolve(
            Some(Route::ClaudeCode),
            Some(&report(Readiness::Ready, Readiness::Unauthenticated)),
        ) else {
            panic!("an unauthenticated profile was routed to");
        };
        assert!(reason.contains("claude-code"), "{reason}");
        assert!(
            reason.contains("nothing else is used in its place"),
            "{reason}"
        );
    }

    #[test]
    fn two_ready_profiles_are_a_choice_the_product_does_not_make() {
        let Routing::Refused(reason) =
            resolve(None, Some(&report(Readiness::Ready, Readiness::Ready)))
        else {
            panic!("the product chose the agent that does the work");
        };
        assert!(reason.contains("runtime <name>"), "{reason}");
        assert_eq!(
            resolve(
                Some(Route::ClaudeCode),
                Some(&report(Readiness::Ready, Readiness::Ready))
            ),
            Routing::Ready(Route::ClaudeCode)
        );
    }

    #[test]
    fn nothing_is_known_before_the_probe_returns() {
        assert_eq!(resolve(Some(Route::Codex), None), Routing::Probing);
    }

    /// A disabled engine is not offered: whatever its probe would have reported, the profile is
    /// not ready and the refusal repeats the reason the registry records.
    #[test]
    fn a_disabled_engine_is_never_offered_as_a_route() {
        let mut report = report(Readiness::Ready, Readiness::Ready);
        for profile in &mut report.profiles {
            if profile.name == Route::Codex.name()
                && let Some(registry) = profile.registry.as_mut()
            {
                registry.enabled = false;
                registry.disabled_reason = Some("usage limit until 2026-09-12".to_owned());
                profile.detail =
                    "disabled in the registry — usage limit until 2026-09-12".to_owned();
            }
        }
        // With one engine disabled the other is the only route there is, so nothing has to be
        // named for a run to be routed.
        assert_eq!(
            resolve(None, Some(&report)),
            Routing::Ready(Route::ClaudeCode)
        );

        let Routing::Refused(reason) = resolve(Some(Route::Codex), Some(&report)) else {
            panic!("a disabled engine was routed to");
        };
        assert!(reason.contains("usage limit until 2026-09-12"), "{reason}");
    }

    /// The load-bearing half: an attempt that reaches the driver for a disabled engine is refused
    /// there, with the registry's reason, before any process is built or started. The routing above
    /// can be stale; this gate cannot be bypassed by a caller that did not look.
    #[test]
    fn an_attempt_on_a_disabled_engine_is_refused_at_the_driver_with_the_registry_reason() {
        let root = tempfile::tempdir().expect("temporary root");
        let address = RegistryAddress::Root(root.path().to_path_buf());
        let registry = Registry::under(root.path());
        registry
            .set_enabled(Engine::ClaudeCode, false, Some("held back for this check"))
            .expect("disable the engine");
        let Err(error) = driver(Route::ClaudeCode, &address) else {
            panic!("a disabled engine built a driver");
        };
        let stated = error.to_string();
        assert!(stated.contains("held back for this check"), "{stated}");
        assert!(stated.contains("claude-code"), "{stated}");

        // Positive half: the same engine, enabled, passes the registry gate and is refused by
        // nothing the registry decides.
        registry
            .set_enabled(Engine::ClaudeCode, true, None)
            .expect("enable the engine");
        let refused_again = match driver(Route::ClaudeCode, &address) {
            Ok(_) => None,
            Err(error) => Some(error.to_string()),
        };
        assert!(
            !refused_again
                .as_deref()
                .is_some_and(|stated| stated.contains("is disabled in the registry")),
            "the enabled engine was still refused by the registry: {refused_again:?}"
        );
    }
}
