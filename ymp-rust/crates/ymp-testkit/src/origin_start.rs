//! A run that can actually ignite, and a host that serves its route without starting a process.
//!
//! Starting the origin participant needs three things a check cannot get from the host it runs on:
//! a product root whose pool resolves to something, a run created against that pool, and a runtime
//! that serves the route the run ignited on. The first two are records this fixture writes
//! ([`ready_root`](crate::ready_root) is the same state one measurement of one account leaves
//! behind); the third is the in-process fixture runtime, handed out by a factory that remembers
//! which route it was asked for.
//!
//! **What the factory remembers is the measurement.** A check that only read the journal could not
//! tell a start that consulted the frozen record from one that consulted the live pool while both
//! happened to agree. The route the factory was asked for is what the start actually intended to
//! run, so a build that resolved the route anywhere but the frozen record is visible here even
//! before the runtime produces an event.
//!
//! Nothing here reaches a network, starts a process or reads the operator's own root.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use ymp_application::{
    AcceptanceCondition, Application, ParticipantRuntimes, PreparedContract, RouteUnavailable,
    RunRequest, freeze_under, prepare_contract,
};
use ymp_domain::pool::EntryIdentity;
use ymp_runtime_api::{RuntimeDriver, Usage};
use ymp_runtime_fake::{FakeRuntime, ScriptStep};

/// A host that serves every route with the in-process fixture runtime, and remembers the routes it
/// was asked for in the order it was asked.
pub struct FakeRuntimes {
    script: Vec<ScriptStep>,
    requested: Mutex<Vec<EntryIdentity>>,
    /// The routes this host refuses to serve, as a host with one engine installed would.
    unserved: Vec<EntryIdentity>,
}

/// The life one fixture participant lives: it starts, it stops for an instruction, and it finishes
/// once it has been given one. It is the shortest script that exercises a yield and a resumption.
pub fn yielding_script() -> Vec<ScriptStep> {
    vec![
        ScriptStep::Output("the participant started".to_owned()),
        ScriptStep::Yield("cursor-1".to_owned()),
        ScriptStep::Output("the participant resumed".to_owned()),
        ScriptStep::Complete(Usage::default()),
    ]
}

impl Default for FakeRuntimes {
    fn default() -> Self {
        Self::with_script(yielding_script())
    }
}

impl FakeRuntimes {
    /// A host whose runtime plays the given script for every route it serves.
    pub fn with_script(script: Vec<ScriptStep>) -> Self {
        Self {
            script,
            requested: Mutex::new(Vec::new()),
            unserved: Vec::new(),
        }
    }

    /// The same host, serving no runtime for one route. It stands for a host whose engine is
    /// installed but which cannot reach that particular model.
    pub fn serving_nothing_for(mut self, route: EntryIdentity) -> Self {
        self.unserved.push(route);
        self
    }

    /// Every route this host was asked for, in order.
    pub fn requested(&self) -> Vec<EntryIdentity> {
        self.requested
            .lock()
            .expect("the fixture host lock")
            .clone()
    }

    /// The one route this host was asked for, where exactly one was asked for.
    pub fn only_route(&self) -> EntryIdentity {
        let requested = self.requested();
        assert_eq!(
            requested.len(),
            1,
            "the host was asked for {} routes: {requested:?}",
            requested.len()
        );
        requested.into_iter().next().expect("one route")
    }
}

impl ParticipantRuntimes for FakeRuntimes {
    fn driver_for(
        &self,
        route: &EntryIdentity,
    ) -> Result<Box<dyn RuntimeDriver>, RouteUnavailable> {
        self.requested
            .lock()
            .expect("the fixture host lock")
            .push(route.clone());
        if self.unserved.contains(route) {
            return Err(RouteUnavailable::new(format!(
                "this host serves no runtime for {route}"
            )));
        }
        Ok(Box::new(FakeRuntime::with_script(self.script.clone())))
    }
}

/// A product root, the project beside it, and the acceptance condition every run in it is judged
/// against.
pub struct OriginHost {
    _directory: tempfile::TempDir,
    pub root: PathBuf,
    pub project: PathBuf,
    pub source: PathBuf,
    program: PathBuf,
    negative_control: PathBuf,
}

impl OriginHost {
    /// A host with nothing enabled on it yet. What it becomes is written by
    /// [`crate::ready_root::measured`] or by an equivalent measurement of this check's own.
    pub fn empty() -> Self {
        let directory = tempfile::TempDir::new().expect("temporary directory");
        let base = directory
            .path()
            .canonicalize()
            .expect("resolve the fixture root");
        let root = base.join("root");
        let project = base.join("project");
        let source = project.join("source");
        let negative_control = project.join("negative-control");
        let program = project.join("verify.sh");
        std::fs::create_dir_all(&root).expect("product root");
        std::fs::create_dir_all(&source).expect("source");
        std::fs::create_dir_all(&negative_control).expect("negative control");
        std::fs::write(source.join("README.md"), b"fixture source\n").expect("source content");
        std::fs::write(&program, b"#!/bin/sh\nexit 1\n").expect("verifier program");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut permissions = std::fs::metadata(&program)
                .expect("verifier metadata")
                .permissions();
            permissions.set_mode(0o700);
            std::fs::set_permissions(&program, permissions).expect("make the verifier executable");
        }
        Self {
            _directory: directory,
            root,
            project,
            source,
            program,
            negative_control,
        }
    }

    /// A host one measured, enabled account has been recorded on, which is the state P2 and P3
    /// leave behind.
    pub fn measured() -> Self {
        let host = Self::empty();
        crate::ready_root::measured(&host.root);
        host
    }

    /// One store per run, since a store holds exactly one run.
    pub fn store(&self, name: &str) -> PathBuf {
        self.project.join("stores").join(name)
    }

    /// The contract every run of this host is judged against.
    pub fn contract(&self) -> PreparedContract {
        prepare_contract(&RunRequest {
            prompt: "produce the candidate the acceptance condition names".to_owned(),
            source: self.source.clone(),
            acceptance: Some(AcceptanceCondition::new(
                &self.program,
                &self.negative_control,
            )),
            capture_exclusions: Vec::new(),
            contract_id: None,
            budget: None,
        })
        .expect("the fixture contract is prepared")
    }

    /// Create one run against the pool as it stands now, which is what freezes that pool into it.
    pub fn start_run(&self, store: &Path) -> Application {
        let contract = self.contract();
        let frozen = freeze_under(&self.root, None).expect("this root freezes its pool");
        let (application, _) = Application::create_with_contract(store, &contract, &frozen)
            .expect("the run is created");
        application
    }
}
