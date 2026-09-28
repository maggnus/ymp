//! A real bounded local interpreter; all file operations use the host's capability.
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};
use ymp_domain::{
    Denial, Id, Result,
    assignment::{ErrorClass, Invocation, InvocationTerminal},
    journal::PolicySelection,
    resources::{Allowance, Coverage, Receipt, Usage},
    workspace::WorkspacePath,
};
use ymp_kernel::ports::execution::*;
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ScriptedStep {
    Emit(BackendObservation),
    Write { path: WorkspacePath, bytes: Vec<u8> },
    Read { path: WorkspacePath, limit: usize },
    Complete { usage: Usage, coverage: Coverage },
}
pub struct Scripted {
    selection: PolicySelection,
    steps: Vec<ScriptedStep>,
    runs: Mutex<BTreeMap<Id<Invocation>, Run>>,
}
struct Run {
    handle: Id,
    receipt: Id<Receipt>,
    allowance: Allowance,
    files: Option<Arc<dyn InvocationFiles>>,
    next: usize,
    output_chars: u64,
    events: Vec<BackendEvent>,
    usage: Usage,
    turns: u32,
    terminal: bool,
    coverage: Coverage,
    replies: BTreeMap<String, Option<String>>,
}
impl Scripted {
    pub fn new(steps: Vec<ScriptedStep>) -> Result<Self> {
        if steps.is_empty() || steps.len() > 4096 {
            return Err(Denial::new(
                "scripted_program",
                "Scripted needs a bounded nonempty program",
            ));
        }
        for step in &steps {
            match step {
                ScriptedStep::Write { bytes, .. } if bytes.len() > 65_536 => {
                    return Err(Denial::new(
                        "scripted_program",
                        "One write is bounded to 64 KiB",
                    ));
                }
                ScriptedStep::Read { limit, .. } if *limit == 0 || *limit > 65_536 => {
                    return Err(Denial::new(
                        "scripted_program",
                        "One read is bounded to 64 KiB",
                    ));
                }
                ScriptedStep::Emit(BackendObservation::Output(text)) if text.len() > 65_536 => {
                    return Err(Denial::new(
                        "scripted_program",
                        "One output is bounded to 64 KiB",
                    ));
                }
                ScriptedStep::Complete { usage, .. }
                | ScriptedStep::Emit(BackendObservation::Usage { usage, .. }) => {
                    usage.validate()?
                }
                _ => {}
            }
        }
        let parameters = serde_json::json!({ "steps": steps });
        if serde_json::to_vec(&parameters)
            .map_err(|_| Denial::new("scripted_program", "Cannot encode program"))?
            .len()
            > 1_048_576
        {
            return Err(Denial::new("scripted_program", "Program exceeds 1 MiB"));
        }
        Ok(Self {
            selection: PolicySelection::new("ExecutionBackend", "Scripted", "1", parameters)?,
            steps,
            runs: Mutex::new(BTreeMap::new()),
        })
    }
    fn lock(&self) -> Result<std::sync::MutexGuard<'_, BTreeMap<Id<Invocation>, Run>>> {
        self.runs
            .lock()
            .map_err(|_| Denial::new("scripted_unavailable", "Scripted state is poisoned"))
    }
    fn event(run: &mut Run, invocation: &Id<Invocation>, observation: BackendObservation) {
        if let BackendObservation::OperationRequest { correlation, .. } = &observation {
            run.replies.entry(correlation.clone()).or_insert(None);
        }
        if matches!(observation, BackendObservation::Terminal(_)) {
            run.terminal = true;
        }
        run.events.push(BackendEvent {
            invocation: invocation.clone(),
            sequence: run.events.len() as u64 + 1,
            observation,
        });
    }
}
impl ExecutionBackend for Scripted {
    fn selection(&self) -> &PolicySelection {
        &self.selection
    }
    fn start(&self, request: &ExecutionRequest<'_>) -> Result<BackendStart> {
        request.prompt.validate()?;
        request.allowance.validate()?;
        if request.grant.grant() != &request.assignment.grant
            || request.settings.model.as_ref() != Some(&request.assignment.profile.model)
            || request.settings.effort != request.assignment.profile.effort
        {
            return Err(Denial::new(
                "scripted_request",
                "Request does not match its assignment and grant",
            ));
        }
        let mut runs = self.lock()?;
        if runs.contains_key(&request.invocation) {
            return Err(Denial::new(
                "invocation_duplicate",
                "Scripted invocation has already started",
            ));
        }
        let handle = Id::new(format!(
            "scripted-{}",
            ymp_domain::Digest::of(request.invocation.as_str()).as_str()
        ))?;
        runs.insert(
            request.invocation.clone(),
            Run {
                handle: handle.clone(),
                receipt: request.receipt.clone(),
                allowance: request.allowance.clone(),
                files: request.files.clone(),
                next: 0,
                output_chars: 0,
                events: vec![],
                usage: Usage {
                    input: 0,
                    cache_read: 0,
                    cache_write: 0,
                    output: 0,
                    reasoning: None,
                },
                turns: 0,
                terminal: false,
                coverage: Coverage::Unknown,
                replies: BTreeMap::new(),
            },
        );
        Ok(BackendStart {
            handle: ExecutionHandle {
                invocation: request.invocation.clone(),
                handle,
            },
            sent: request.settings.clone(),
            reported: Default::default(),
            native_session: None,
        })
    }
    fn cancel(&self, handle: &ExecutionHandle) -> Result<()> {
        let mut runs = self.lock()?;
        let run = runs
            .get_mut(&handle.invocation)
            .ok_or_else(|| Denial::new("invocation_missing", "No Scripted invocation"))?;
        if run.handle != handle.handle {
            return Err(Denial::new(
                "invocation_identity",
                "Foreign Scripted handle",
            ));
        }
        if !run.terminal {
            Self::event(
                run,
                &handle.invocation,
                BackendObservation::Terminal(InvocationTerminal::Cancelled),
            );
        }
        Ok(())
    }
    fn events(
        &self,
        handle: &ExecutionHandle,
        after: u64,
        limit: usize,
    ) -> Result<Vec<BackendEvent>> {
        if limit == 0 || limit > 128 {
            return Err(Denial::new(
                "event_limit",
                "Request between 1 and 128 events",
            ));
        }
        let mut runs = self.lock()?;
        let run = runs
            .get_mut(&handle.invocation)
            .ok_or_else(|| Denial::new("invocation_missing", "No Scripted invocation"))?;
        if run.handle != handle.handle || after > run.events.len() as u64 {
            return Err(Denial::new(
                "invocation_identity",
                "Foreign handle or event cursor",
            ));
        }
        if after == run.events.len() as u64 && !run.terminal {
            let step = self.steps.get(run.next).cloned();
            run.next += 1;
            match step {
                Some(ScriptedStep::Emit(BackendObservation::Output(text))) => {
                    let next = run.output_chars.checked_add(text.chars().count() as u64);
                    if next.is_none_or(|next| next > run.allowance.output_chars) {
                        Self::event(
                            run,
                            &handle.invocation,
                            BackendObservation::Terminal(InvocationTerminal::Failed(
                                ErrorClass::Content,
                            )),
                        );
                    } else {
                        run.output_chars = next.unwrap();
                        Self::event(run, &handle.invocation, BackendObservation::Output(text));
                    }
                }
                Some(ScriptedStep::Emit(BackendObservation::Usage { usage, turns })) => {
                    if !usage.includes(&run.usage)
                        || turns < run.turns
                        || turns > run.allowance.native_turns
                    {
                        Self::event(
                            run,
                            &handle.invocation,
                            BackendObservation::Terminal(InvocationTerminal::Failed(
                                ErrorClass::Protocol,
                            )),
                        );
                    } else {
                        run.usage = usage.clone();
                        run.turns = turns;
                        Self::event(
                            run,
                            &handle.invocation,
                            BackendObservation::Usage { usage, turns },
                        );
                    }
                }
                Some(ScriptedStep::Emit(event)) => Self::event(run, &handle.invocation, event),
                Some(ScriptedStep::Write { path, bytes }) => {
                    let result = run
                        .files
                        .as_ref()
                        .ok_or_else(|| Denial::new("file_access", "No file capability"))
                        .and_then(|files| files.write(&path, &bytes));
                    if result.is_err() {
                        Self::event(
                            run,
                            &handle.invocation,
                            BackendObservation::ToolDenied(
                                ymp_domain::journal::Capability::WriteFiles,
                            ),
                        );
                    } else {
                        Self::event(
                            run,
                            &handle.invocation,
                            BackendObservation::Progress {
                                signal: ymp_domain::coordination::ProgressSignal::Heartbeat,
                                basis: None,
                            },
                        );
                    }
                }
                Some(ScriptedStep::Read { path, limit }) => {
                    let result = run
                        .files
                        .as_ref()
                        .ok_or_else(|| Denial::new("file_access", "No file capability"))
                        .and_then(|files| files.read(&path, limit));
                    match result {
                        Ok(bytes) => {
                            let text = String::from_utf8_lossy(&bytes).into_owned();
                            if run.output_chars + text.chars().count() as u64
                                > run.allowance.output_chars
                            {
                                Self::event(
                                    run,
                                    &handle.invocation,
                                    BackendObservation::Terminal(InvocationTerminal::Failed(
                                        ErrorClass::Content,
                                    )),
                                );
                            } else {
                                run.output_chars += text.chars().count() as u64;
                                Self::event(
                                    run,
                                    &handle.invocation,
                                    BackendObservation::Output(text),
                                );
                            }
                        }
                        Err(_) => Self::event(
                            run,
                            &handle.invocation,
                            BackendObservation::ToolDenied(
                                ymp_domain::journal::Capability::ReadFiles,
                            ),
                        ),
                    }
                }
                Some(ScriptedStep::Complete { usage, coverage }) => {
                    if !usage.includes(&run.usage) {
                        return Err(Denial::new(
                            "usage_baseline",
                            "Completion resets observed usage",
                        ));
                    }
                    run.usage = usage;
                    run.coverage = coverage;
                    Self::event(
                        run,
                        &handle.invocation,
                        BackendObservation::Terminal(InvocationTerminal::Completed),
                    );
                }
                None => Self::event(
                    run,
                    &handle.invocation,
                    BackendObservation::Terminal(InvocationTerminal::Failed(ErrorClass::Content)),
                ),
            }
        }
        Ok(run
            .events
            .iter()
            .skip(after as usize)
            .take(limit)
            .cloned()
            .collect())
    }
    fn receipt(&self, handle: &ExecutionHandle) -> Result<Receipt> {
        let runs = self.lock()?;
        let run = runs
            .get(&handle.invocation)
            .ok_or_else(|| Denial::new("invocation_missing", "No Scripted invocation"))?;
        if run.handle != handle.handle {
            return Err(Denial::new(
                "invocation_identity",
                "Foreign Scripted handle",
            ));
        }
        if !run.terminal {
            return Err(Denial::new(
                "receipt_pending",
                "Scripted invocation has not ended",
            ));
        }
        Ok(Receipt {
            id: run.receipt.clone(),
            invocation: handle.invocation.erased(),
            usage: run.usage.clone(),
            coverage: run.coverage,
            cost: None,
        })
    }
    fn reply(&self, handle: &ExecutionHandle, correlation: &str, result: &str) -> Result<()> {
        ymp_domain::require_text(result, 16_384)?;
        let mut runs = self.lock()?;
        let run = runs
            .get_mut(&handle.invocation)
            .ok_or_else(|| Denial::new("invocation_missing", "No Scripted invocation"))?;
        if run.handle != handle.handle {
            return Err(Denial::new(
                "invocation_identity",
                "Foreign Scripted handle",
            ));
        }
        let pending = run
            .replies
            .get_mut(correlation)
            .ok_or_else(|| Denial::new("operation_correlation", "No matching operation request"))?;
        if pending.as_ref().is_some_and(|prior| prior != result) {
            return Err(Denial::new(
                "operation_correlation",
                "Recorded operation reply changed",
            ));
        }
        *pending = Some(result.into());
        Ok(())
    }
}
