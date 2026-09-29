//! The composition behind the interface: one durable store, one workspace root
//! and one execution backend. It selects strategies and adapters; every decision
//! about a session stays with the kernel.
use crate::store::{journal, resolve, status, view};
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
    sync::Arc,
};
use ymp_runtime::{
    application::{RecoveryIntent, SessionView},
    backends::{
        claude::ClaudeStreamJson,
        scripted_team::{ScriptedTeam, TeamScript},
    },
    checks::retained::RetainedBytes,
    clock::{Clock, SystemClock},
    dispatcher::{Dispatcher, SessionPolicies, SessionStart},
    domain::{
        Denial, Digest, Id, Prob, Result,
        coordination::CommitmentTerms,
        identity::{
            Agent, DependencyKind, Discovery, DiscoverySource, ProfileSettings, Provider,
            RegistryFacts,
        },
        journal::{Capability, MethodKind},
        resources::{
            PriceBook, PriceWeightedParameters, PurposeBoundedParameters, Rates, UnknownUsage,
        },
        task::{Constraints, Criterion, CriterionKind, CriterionOrigin, Goal, Pins, Real, Task},
        verification::{AssessmentRules, CheckSpec},
        workspace::{CaptureLimits, WorkspacePath},
    },
    kernel::{
        intake::IntakeRequest,
        ports::{
            checks::NativeDiscovery,
            execution::{ClaudeParameters, ExecutionBackend},
            planning::ContributionParameters,
            progress::{DiagnosisParameters, EscalationParameters, MonitorParameters},
        },
        session::{SessionDefinition, VisibleCheck},
    },
    live_session::LiveSession,
    policies::*,
    readiness::StaticDependencyProbe,
    workspace::direct::Direct,
};
use ymp_storage::journal::SqliteJournal;
use ymp_tui::{Draft, Host, Member, SessionRow};

/// Largest file `/preserve` reads to state an expectation.
const PRESERVED_BYTES: u64 = 1_048_576;
/// Longest expected text quoted inside a criterion.
const QUOTED_BYTES: usize = 400;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BackendChoice {
    /// A recorded program of role responses. No model is involved.
    Scripted { program: PathBuf },
    /// The installed Claude Code executable with its own authentication.
    Claude {
        executable: PathBuf,
        model: String,
        effort: Option<String>,
    },
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Options {
    pub store: PathBuf,
    pub workspace: PathBuf,
    pub backend: BackendChoice,
}
enum Team {
    Scripted(TeamScript),
    Claude {
        backend: Arc<ClaudeStreamJson>,
        discovery: Box<Discovery>,
        executable: PathBuf,
        model: String,
        effort: Option<String>,
    },
}
pub struct Composition {
    journal: Arc<SqliteJournal>,
    clock: Arc<SystemClock>,
    store: PathBuf,
    workspace: PathBuf,
    team: Team,
}
fn refused(code: &str, message: impl Into<String>) -> Denial {
    Denial::new(code, message)
}
fn real(value: f64) -> Result<Real> {
    Real::new(value)
}
fn policies(native: bool) -> Result<SessionPolicies> {
    // Native values repeat the recorded light-model pilot; scripted calls cost 2.
    // Durations are wall-clock in both cases: a lease outlasts verification.
    let call = if native { 16000.0 } else { 10.0 };
    let lease = 480_000;
    Ok(SessionPolicies {
        readiness: Box::new(StaticDependencyProbe::new()?),
        intake: if native {
            Box::new(intake::NoQuestions::new()?)
        } else {
            Box::new(intake::VoiClarification::new(real(1.0)?)?)
        },
        method: Box::new(method::FixedMethod::new(MethodKind::SoloWithVerifier)?),
        planner: Box::new(planner::AsNeededDecomposition::new()?),
        contributions: Box::new(contribution::FixedWorkflow::new(ContributionParameters {
            expected: real(call)?,
            p90: real(call)?,
            p_success: Prob::new(0.5)?,
            delta_belief: real(0.1)?,
        })?),
        belief: Box::new(belief::LikelihoodRatioTable::standard()?),
        credit: Box::new(credit::ConfirmedOnly::new()?),
        monitor: Box::new(progress::EvidenceDelta::new(MonitorParameters {
            epsilon: real(0.001)?,
            stall_limit: 3,
        })?),
        diagnosis: Box::new(diagnosis::RuleBasedDiagnoser::new(DiagnosisParameters {
            p_min: Prob::new(0.1)?,
            mutation_threshold: Prob::new(0.8)?,
        })?),
        escalation: Box::new(escalation::DiagnosisFirstLadder::new(
            EscalationParameters {
                max_uses: 1,
                max_cost: real(call)?,
                timeout_ms: 60_000,
            },
        )?),
        award: Arc::new(award::FirstOffer::with_commitment_terms(CommitmentTerms {
            lease_duration: lease,
            renewal_duration: lease,
            renew_on: BTreeSet::new(),
            renewals: 0,
            release_delta: real(1.0)?,
        })?),
        cost: Arc::new(resources::PriceWeighted::new(PriceWeightedParameters {
            expected_input: if native { 10_000 } else { 10 },
            expected_output: if native { 1000 } else { 0 },
            p90_factor: real(if native { 2.0 } else { 1.0 })?,
        })?),
        resources: Box::new(resources::PurposeBounded::new(PurposeBoundedParameters {
            max_cost: real(if native { 30000.0 } else { 100.0 })?,
            timeout: 240_000,
            native_turns: if native { 1 } else { 2 },
            output_chars: if native { 8000 } else { 16000 },
            report_call_cost: real(if native { 15000.0 } else { 10.0 })?,
        })?),
        context: Box::new(context::CriteriaProjection::new(4)?),
        reviewer: Box::new(reviewer::AnyNonProducer::new()?),
        // The report explains recorded facts without another model call.
        narrative: Box::new(narrative::DeterministicReport::new()?),
        audit: Box::new(claim_audit::EvidenceClassRules::new()?),
    })
}
fn criterion(index: usize, expectation: &ymp_tui::Expectation) -> Result<Criterion> {
    let text = if expectation.preserve {
        format!("{} keeps its original bytes", expectation.path)
    } else {
        match std::str::from_utf8(&expectation.bytes) {
            Ok(text) if text.len() <= QUOTED_BYTES => {
                format!("{} contains exactly {text:?}", expectation.path)
            }
            _ => format!(
                "{} contains exactly the stated {} bytes",
                expectation.path,
                expectation.bytes.len()
            ),
        }
    };
    Ok(Criterion {
        id: Id::new(format!("criterion-{}", index + 1))?,
        text,
        kind: if expectation.preserve {
            CriterionKind::Preserve
        } else {
            CriterionKind::NewBehavior
        },
        weight: real(1.0)?,
        required: true,
        origin: CriterionOrigin::User,
        needs_class: BTreeSet::new(),
    })
}
impl Composition {
    /// Native discovery happens here, before the terminal changes mode. It reads
    /// metadata only: no prompt is sent and no credential is copied.
    pub fn open(options: Options) -> Result<Self> {
        let workspace = options
            .workspace
            .canonicalize()
            .ok()
            .filter(|path| path.is_dir())
            .ok_or_else(|| refused("workspace", "The workspace directory does not exist"))?;
        // Runtime records must never appear among the files agents read and write.
        // A refused place is left untouched, so it is checked before it is created.
        if resolve(&options.store)?.starts_with(&workspace) {
            return Err(refused(
                "store",
                "The store directory must be outside the workspace",
            ));
        }
        let journal = journal(&options.store, true)?;
        let store = options
            .store
            .canonicalize()
            .map_err(|_| refused("store", "Cannot resolve the store directory"))?;
        if store.starts_with(&workspace) {
            return Err(refused(
                "store",
                "The store directory must be outside the workspace",
            ));
        }
        let clock = Arc::new(SystemClock);
        let team = match options.backend {
            BackendChoice::Scripted { program } => {
                let bytes = std::fs::read(&program)
                    .map_err(|_| refused("program", "Cannot read the scripted program"))?;
                let script: TeamScript = serde_json::from_slice(&bytes).map_err(|error| {
                    refused(
                        "program",
                        format!("The scripted program is malformed: {error}"),
                    )
                })?;
                script.validate()?;
                Team::Scripted(script)
            }
            BackendChoice::Claude {
                executable,
                model,
                effort,
            } => {
                let backend = Arc::new(ClaudeStreamJson::new(
                    Id::new("claude")?,
                    ClaudeParameters {
                        // The native tool is installed as a link to its release.
                        executable: executable.canonicalize().map_err(|_| {
                            refused("executable", "The Claude executable does not exist")
                        })?,
                        source: DiscoverySource::Native,
                        connect_timeout_ms: 30000,
                        frame_bytes: 1_048_576,
                        round_trips: 8,
                    },
                    clock.now()?,
                )?);
                let discovery = backend.discover()?;
                let offered = discovery.offerings.iter().any(|offering| {
                    offering.model.as_deref() == Some(model.as_str())
                        && effort.as_ref().is_none_or(|effort| {
                            offering
                                .efforts
                                .as_ref()
                                .is_some_and(|all| all.contains(effort))
                        })
                });
                if !offered {
                    let models: Vec<_> = discovery
                        .offerings
                        .iter()
                        .filter_map(|offering| offering.model.clone())
                        .collect();
                    return Err(refused(
                        "model",
                        format!(
                            "Native discovery does not offer this model and effort together; it offers: {}",
                            models.join(", ")
                        ),
                    ));
                }
                Team::Claude {
                    executable: backend.configuration().executable.clone(),
                    backend,
                    discovery: Box::new(discovery),
                    model,
                    effort,
                }
            }
        };
        Ok(Self {
            journal,
            clock,
            store,
            workspace,
            team,
        })
    }
    pub fn store(&self) -> &Path {
        &self.store
    }
    fn native(&self) -> bool {
        matches!(self.team, Team::Claude { .. })
    }
    fn provider(&self) -> Result<Id<Provider>> {
        Id::new(if self.native() { "claude" } else { "scripted" })
    }
    fn agents(&self) -> [&'static str; 2] {
        if self.native() {
            ["claude-a", "claude-b"]
        } else {
            ["scripted-a", "scripted-b"]
        }
    }
    fn settings(&self) -> ProfileSettings {
        match &self.team {
            Team::Scripted(_) => ProfileSettings {
                model: Some("scripted".into()),
                effort: None,
            },
            Team::Claude { model, effort, .. } => ProfileSettings {
                model: Some(model.clone()),
                effort: effort.clone(),
            },
        }
    }
    fn backend(&self) -> Result<Arc<dyn ExecutionBackend>> {
        Ok(match &self.team {
            Team::Scripted(script) => Arc::new(ScriptedTeam::new(script.clone())?),
            Team::Claude { backend, .. } => backend.clone(),
        })
    }
    fn request(&self, session: Id, draft: &Draft, now: u64) -> Result<SessionStart> {
        let native = self.native();
        let criteria = draft
            .expectations
            .iter()
            .enumerate()
            .map(|(index, expectation)| criterion(index, expectation))
            .collect::<Result<Vec<_>>>()?;
        let checks = draft
            .expectations
            .iter()
            .zip(&criteria)
            .map(|(expectation, criterion)| {
                Ok(VisibleCheck {
                    criterion: criterion.reference()?,
                    spec: CheckSpec::ExactBytes {
                        path: WorkspacePath::new(expectation.path.as_str())?,
                        digest: Digest::of(&expectation.bytes),
                    },
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let settings = self.settings();
        let pins = match &self.team {
            Team::Scripted(_) => Pins::unrestricted(),
            Team::Claude { model, effort, .. } => Pins {
                team_size: Some(2),
                roster: Some(
                    self.agents()
                        .into_iter()
                        .map(Id::new)
                        .collect::<Result<_>>()?,
                ),
                models: Some(BTreeSet::from([model.clone()])),
                efforts: effort.clone().map(|effort| BTreeSet::from([effort])),
            },
        };
        let mut facts = RegistryFacts {
            agents: self
                .agents()
                .into_iter()
                .map(|name| {
                    Ok(Agent {
                        id: Id::new(name)?,
                        name: name.into(),
                        provider: self.provider()?,
                        defaults: settings.clone(),
                        instructions: "Use the host-provided role context and exact structured response contract. Be concise. Use only the mediated file tools. Preserve attribution; do not claim checks you did not observe.".into(),
                        enabled: true,
                    })
                })
                .collect::<Result<_>>()?,
            discoveries: vec![match &self.team {
                Team::Scripted(_) => ScriptedTeam::discovery(self.provider()?, now),
                Team::Claude { discovery, .. } => (**discovery).clone(),
            }],
            dependencies: vec![],
        };
        if let Team::Claude { executable, .. } = &self.team {
            facts.dependencies.push(StaticDependencyProbe::capture(
                self.provider()?,
                None,
                executable,
                DependencyKind::Executable,
                now,
            )?);
        }
        Ok(SessionStart {
            session,
            task: IntakeRequest {
                task: Task {
                    id: Id::new("task")?,
                    goal: Goal {
                        request: draft.goal.clone(),
                        assumptions: vec![],
                        clarifications: vec![],
                    },
                    contract: Id::new("contract")?,
                    constraints: Constraints {
                        budget: real(draft.budget)?,
                        verification_reserve: real(draft.budget * if native { 0.24 } else { 0.2 })?,
                        deadline: native.then_some(now + 480_000),
                        pins,
                        allowed: BTreeSet::from([Capability::ReadFiles, Capability::WriteFiles]),
                        parallel_limit: 1,
                        attempt_limit: 2,
                        max_members: 2,
                    },
                },
                criteria,
            },
            facts,
            definition: SessionDefinition {
                workspace: Id::new("workspace")?,
                // The window must outlast the recording of every offer on a real clock.
                offer_window_ms: if native { 5000 } else { 2500 },
                checks,
                rules: AssessmentRules {
                    mutation_threshold: Prob::new(0.8)?,
                },
            },
            pricebook: PriceBook {
                version: "relative-token-weights-not-currency-v1".into(),
                rates: vec![],
                fallback: Rates::fallback(),
            },
            unknown_usage: UnknownUsage::Stop,
        })
    }
}
impl Host for Composition {
    fn summary(&self) -> String {
        let backend = match &self.team {
            Team::Scripted(_) => "scripted team (no model)".to_string(),
            Team::Claude { model, effort, .. } => format!(
                "Claude {model} effort {}",
                effort.as_deref().unwrap_or("not set")
            ),
        };
        format!(
            "{backend} · workspace {} · store {}",
            self.workspace.display(),
            self.store.display()
        )
    }
    fn members(&self) -> Vec<Member> {
        let settings = self.settings();
        self.agents()
            .into_iter()
            .map(|agent| Member {
                agent: agent.into(),
                provider: if self.native() { "claude" } else { "scripted" }.into(),
                model: settings.model.clone(),
                effort: settings.effort.clone(),
            })
            .collect()
    }
    fn budget(&self) -> f64 {
        if self.native() { 250_000.0 } else { 100.0 }
    }
    fn now(&self) -> u64 {
        self.clock.now().unwrap_or_default()
    }
    fn sessions(&self) -> Result<Vec<SessionRow>> {
        self.journal
            .sessions()?
            .into_iter()
            .map(|(session, revision)| {
                let view = view(&self.journal, &session)?;
                Ok(SessionRow {
                    session: session.as_str().into(),
                    status: status(&view),
                    revision,
                    goal: view
                        .task()
                        .map(|task| task.goal.request.clone())
                        .unwrap_or_default(),
                })
            })
            .collect()
    }
    fn view(&self, session: &Id) -> Result<SessionView> {
        view(&self.journal, session)
    }
    fn file(&self, path: &str) -> Result<Vec<u8>> {
        let path = WorkspacePath::new(path)?;
        let named = self.workspace.join(path.as_str());
        let target = named
            .canonicalize()
            .map_err(|_| refused("file", "The workspace has no such file"))?;
        // The workspace refuses links as well; a preserved link could never pass.
        if target != named {
            return Err(refused("file", "The path passes through a symbolic link"));
        }
        let metadata = std::fs::metadata(&target)
            .map_err(|_| refused("file", "Cannot inspect the workspace file"))?;
        if !target.starts_with(&self.workspace) || !metadata.is_file() {
            return Err(refused("file", "The path is not a file of the workspace"));
        }
        if metadata.len() > PRESERVED_BYTES {
            return Err(refused("file", "The file is too large to preserve"));
        }
        std::fs::read(&target).map_err(|_| refused("file", "Cannot read the workspace file"))
    }
    fn start(&self, draft: &Draft) -> Result<(Id, LiveSession)> {
        draft.check()?;
        let now = self.clock.now()?;
        let session: Id = Id::new(format!("session-{now}"))?;
        if view(&self.journal, &session)?.opened() {
            return Err(refused(
                "session_exists",
                "A session with this identifier is already recorded; start again",
            ));
        }
        let request = self.request(session.clone(), draft, now)?;
        let (journal, backend, workspace) = (
            self.journal.clone(),
            self.backend()?,
            self.workspace.clone(),
        );
        let native = self.native();
        let clock: Arc<dyn Clock> = self.clock.clone();
        let live = LiveSession::spawn(clock.clone(), move || {
            let content = Arc::new(journal.content_store());
            Dispatcher::start(
                journal,
                content,
                request,
                policies(native)?,
                backend,
                Arc::new(Direct::open(&workspace, CaptureLimits::default())?),
                Arc::new(RetainedBytes),
                clock,
            )
        })?;
        Ok((session, live))
    }
    fn recover(&self, session: &Id, intent: RecoveryIntent) -> Result<LiveSession> {
        // The runtime records the owner's intent before it compares strategies,
        // so a composition that cannot drive this session refuses here first.
        let recorded = view(&self.journal, session)?;
        for selected in policies(self.native())?.selections() {
            if recorded.policies().get(&selected.policy.port) != Some(&selected)
                && selected.policy.port != "NarrativeComposer"
            {
                return Err(Denial::new(
                    "policy_selection",
                    "This composition does not use the strategies the session recorded",
                ));
            }
        }
        let (journal, backend, workspace) = (
            self.journal.clone(),
            self.backend()?,
            self.workspace.clone(),
        );
        let native = self.native();
        let session = session.clone();
        let clock: Arc<dyn Clock> = self.clock.clone();
        LiveSession::spawn(clock.clone(), move || {
            let content = Arc::new(journal.content_store());
            Dispatcher::recover(
                journal,
                content,
                session,
                intent,
                policies(native)?,
                backend,
                Arc::new(Direct::open(&workspace, CaptureLimits::default())?),
                Arc::new(RetainedBytes),
                clock,
            )
        })
    }
}
