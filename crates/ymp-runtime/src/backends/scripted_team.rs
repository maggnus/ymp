//! Deterministic role responses for the fixed workflow. No model, network or
//! authentication is involved; every file operation uses the host's capability.
use super::scripted::{Scripted, ScriptedStep};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Mutex,
    time::{Duration, Instant},
};
use ymp_domain::{
    Denial, Digest, Id, Ref, Result,
    assignment::{Invocation, RoleKind},
    identity::{Discovery, DiscoverySource, ModelOffering, Provider, ProviderKind},
    journal::{Capability, PolicySelection},
    plan::{Plan, WorkItem, WorkState},
    resources::{Coverage, Receipt, Usage},
    verification::ReviewVerdict,
    workspace::WorkspacePath,
};
use ymp_kernel::{
    acceptance::CandidateVerdict,
    finalization::FinalVerdict,
    journal::ParameterSchemas,
    ports::{
        execution::*,
        planning::{IntakeOutput, PlanDefinition, PlanningPrompt, Question},
        reporting::{Assertion, DraftClaim, ReportDraft},
    },
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScriptedWrite {
    pub path: WorkspacePath,
    pub text: String,
}

/// The whole recorded program of a scripted team.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TeamScript {
    /// Files the Producer writes through its mediated grant.
    pub writes: Vec<ScriptedWrite>,
    /// Questions the intake call returns; the IntakePolicy decides which to ask.
    pub questions: Vec<Question>,
    /// Coverage of the Producer's receipt. Every other call reports complete usage.
    pub producer_coverage: Coverage,
    /// Wall-clock delay before a call reports its first observation.
    pub pace_ms: u64,
}
impl TeamScript {
    pub fn validate(&self) -> Result<()> {
        if self.writes.is_empty()
            || self.writes.len() > 64
            || self.writes.iter().any(|w| w.text.len() > 65_536)
            || self.questions.len() > 16
            || self.pace_ms > 60_000
            || self
                .writes
                .iter()
                .map(|w| &w.path)
                .collect::<BTreeSet<_>>()
                .len()
                != self.writes.len()
        {
            return Err(Denial::new(
                "backend_parameters",
                "ScriptedTeam requires bounded distinct writes, questions and pace",
            ));
        }
        Ok(())
    }
}
fn validate(selection: &PolicySelection) -> Result<()> {
    serde_json::from_value::<TeamScript>(selection.parameters.clone())
        .map_err(|_| Denial::new("backend_parameters", "ScriptedTeam program is malformed"))?
        .validate()
}
struct Run {
    program: Scripted,
    ready: Instant,
}
pub struct ScriptedTeam {
    selection: PolicySelection,
    script: TeamScript,
    runs: Mutex<BTreeMap<Id<Invocation>, Run>>,
}
fn context(text: &str) -> Denial {
    Denial::new("scripted_context", text)
}
fn basis(data: &serde_json::Value) -> Result<Vec<Id<ymp_domain::verification::Evidence>>> {
    serde_json::from_value::<Vec<Ref>>(data[7].clone())
        .map_err(|_| context("Recorded context names no evidence references"))?
        .into_iter()
        .map(|r| Id::new(r.id.as_str()))
        .collect()
}
fn encode<T: Serialize>(value: &T) -> Result<String> {
    serde_json::to_string(value).map_err(|_| context("Cannot encode the role response"))
}
fn field<T: serde::de::DeserializeOwned>(purpose: &serde_json::Value, name: &str) -> Result<T> {
    serde_json::from_value(purpose[name].clone())
        .map_err(|_| context("Recorded purpose lacks a required field"))
}
impl ScriptedTeam {
    pub fn new(script: TeamScript) -> Result<Self> {
        script.validate()?;
        Ok(Self {
            selection: PolicySelection::new(
                "ExecutionBackend",
                "ScriptedTeam",
                "1",
                serde_json::to_value(&script)
                    .map_err(|_| Denial::new("backend_parameters", "Cannot encode program"))?,
            )?,
            script,
            runs: Mutex::new(BTreeMap::new()),
        })
    }
    /// The composition registers the schema; the kernel knows no concrete adapter.
    pub fn register(schemas: &mut ParameterSchemas) -> Result<()> {
        schemas.register("ExecutionBackend", "ScriptedTeam", "1", validate)
    }
    /// Discovery of a fixture is a statement about the fixture, never about a provider.
    pub fn discovery(provider: Id<Provider>, observed_at: u64) -> Discovery {
        Discovery {
            provider: Provider {
                id: provider.clone(),
                kind: ProviderKind::Scripted,
                version: None,
                capabilities: Some(BTreeSet::from([
                    Capability::ReadFiles,
                    Capability::WriteFiles,
                ])),
            },
            offerings: vec![ModelOffering {
                provider,
                model: Some("scripted".into()),
                family: None,
                efforts: None,
                default_effort: None,
            }],
            default_model: Some("scripted".into()),
            adapter_available: true,
            source: DiscoverySource::ScriptedFixture,
            method: "ScriptedTeam/1 explicit program".into(),
            observed_at,
        }
    }
    fn lock(&self) -> Result<std::sync::MutexGuard<'_, BTreeMap<Id<Invocation>, Run>>> {
        self.runs
            .lock()
            .map_err(|_| Denial::new("scripted_unavailable", "ScriptedTeam state is poisoned"))
    }
    fn response(&self, r: &ExecutionRequest<'_>) -> Result<String> {
        let data: serde_json::Value = serde_json::from_str(&r.prompt.text)
            .map_err(|_| context("Recorded context is not structured"))?;
        let purpose = data
            .as_array()
            .and_then(|parts| parts.last())
            .ok_or_else(|| context("Recorded context has no purpose"))?;
        let operation = purpose["operation"].as_str().unwrap_or_default();
        if purpose["port"] == "IntakePolicy" {
            return encode(&IntakeOutput {
                criteria: vec![],
                questions: self.script.questions.clone(),
            });
        }
        match operation {
            "production" => Ok("Wrote the scripted files".into()),
            "candidate_verification" | "recovery_research" => {
                Ok("Read the retained candidate and its visible check".into())
            }
            "candidate_review" => encode(&CandidateVerdict {
                id: Id::new(format!("review-{}", Digest::of(r.invocation.as_str())))?,
                result: field(purpose, "result")?,
                criteria: field(purpose, "criteria")?,
                verdict: ReviewVerdict::Approve,
                findings: vec![],
                basis: basis(&data)?,
                rationale: "The trusted check confirmed the retained candidate".into(),
            }),
            "final_review" => {
                let aggregate: ymp_domain::report::FinalAggregate = field(purpose, "aggregate")?;
                encode(&FinalVerdict {
                    id: Id::new("final-review")?,
                    aggregate: aggregate.reference()?,
                    verdict: ReviewVerdict::Approve,
                    basis: basis(&data)?,
                    rationale: "The integrated retained bytes match the accepted result".into(),
                })
            }
            _ if r.assignment.role == RoleKind::Narrator => encode(&ReportDraft {
                claims: vec![DraftClaim {
                    text: "Uncertain: this statement establishes no broader claim.".into(),
                    assertion: Assertion::Uncertainty,
                }],
            }),
            _ => {
                let input: PlanningPrompt = field(purpose, "planning")?;
                let plan: Id<Plan> = Id::new("plan")?;
                let item: Id<WorkItem> = Id::new("item")?;
                encode(&PlanDefinition {
                    plan: Plan {
                        id: plan.clone(),
                        session: r.assignment.session.clone(),
                        version: 1,
                        items: BTreeSet::from([item.clone()]),
                        rationale: "One work item writes every scripted file".into(),
                        author: r.assignment.id.clone(),
                    },
                    items: vec![WorkItem {
                        id: item,
                        plan,
                        title: "Write the scripted files".into(),
                        targets: input.criteria.iter().map(|c| c.id.clone()).collect(),
                        deps: BTreeSet::new(),
                        needs: BTreeSet::from([Capability::ReadFiles, Capability::WriteFiles]),
                        writes: self.script.writes.iter().map(|w| w.path.clone()).collect(),
                        state: WorkState::Open,
                        attempts: vec![],
                        accepted: None,
                        parent: None,
                    }],
                })
            }
        }
    }
}
impl ExecutionBackend for ScriptedTeam {
    fn selection(&self) -> &PolicySelection {
        &self.selection
    }
    fn start(&self, request: &ExecutionRequest<'_>) -> Result<BackendStart> {
        let producer = request.assignment.role == RoleKind::Producer;
        let mut steps = vec![];
        if producer {
            steps.extend(self.script.writes.iter().map(|w| ScriptedStep::Write {
                path: w.path.clone(),
                bytes: w.text.clone().into_bytes(),
            }));
        }
        steps.extend([
            ScriptedStep::Emit(BackendObservation::Output(self.response(request)?)),
            ScriptedStep::Complete {
                usage: Usage {
                    input: 2,
                    cache_read: 0,
                    cache_write: 0,
                    output: 0,
                    reasoning: None,
                },
                coverage: if producer {
                    self.script.producer_coverage
                } else {
                    Coverage::Complete
                },
            },
        ]);
        let program = Scripted::new(steps)?;
        let started = program.start(request)?;
        self.lock()?.insert(
            request.invocation.clone(),
            Run {
                program,
                ready: Instant::now() + Duration::from_millis(self.script.pace_ms),
            },
        );
        Ok(started)
    }
    fn cancel(&self, handle: &ExecutionHandle) -> Result<()> {
        let mut runs = self.lock()?;
        let run = runs
            .get_mut(&handle.invocation)
            .ok_or_else(|| Denial::new("invocation_missing", "No ScriptedTeam invocation"))?;
        run.ready = Instant::now();
        run.program.cancel(handle)
    }
    fn events(
        &self,
        handle: &ExecutionHandle,
        after: u64,
        limit: usize,
    ) -> Result<Vec<BackendEvent>> {
        let runs = self.lock()?;
        let run = runs
            .get(&handle.invocation)
            .ok_or_else(|| Denial::new("invocation_missing", "No ScriptedTeam invocation"))?;
        if Instant::now() < run.ready {
            return Ok(vec![]);
        }
        run.program.events(handle, after, limit)
    }
    fn receipt(&self, handle: &ExecutionHandle) -> Result<Receipt> {
        self.lock()?
            .get(&handle.invocation)
            .ok_or_else(|| Denial::new("invocation_missing", "No ScriptedTeam invocation"))?
            .program
            .receipt(handle)
    }
    fn reply(&self, handle: &ExecutionHandle, correlation: &str, result: &str) -> Result<()> {
        self.lock()?
            .get(&handle.invocation)
            .ok_or_else(|| Denial::new("invocation_missing", "No ScriptedTeam invocation"))?
            .program
            .reply(handle, correlation, result)
    }
}
