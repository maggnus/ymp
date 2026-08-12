#![forbid(unsafe_code)]

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use ymp_runtime_api::{
    CancellationToken, InvocationRequest, ProbeReport, Readiness, RuntimeDriver, RuntimeError,
    RuntimeEvent, RuntimeEventKind, RuntimeKind, RuntimeSession, Usage,
};

#[derive(Clone, Debug)]
pub enum ScriptStep {
    Output(String),
    Yield(String),
    DuplicateLast,
    Malformed(String),
    Complete(Usage),
}

#[derive(Clone, Debug)]
pub struct FakeRuntime {
    executable: PathBuf,
    script: Vec<ScriptStep>,
}

impl Default for FakeRuntime {
    fn default() -> Self {
        Self {
            executable: PathBuf::from("ymp-internal-fake"),
            script: vec![
                ScriptStep::Output("fake runtime started".to_owned()),
                ScriptStep::Yield("cursor-1".to_owned()),
                ScriptStep::Output("fake runtime resumed".to_owned()),
                ScriptStep::Complete(Usage {
                    input_tokens: 10,
                    output_tokens: 5,
                    ..Usage::default()
                }),
            ],
        }
    }
}

impl FakeRuntime {
    pub fn with_script(script: Vec<ScriptStep>) -> Self {
        Self {
            script,
            ..Self::default()
        }
    }
}

impl RuntimeDriver for FakeRuntime {
    fn kind(&self) -> RuntimeKind {
        RuntimeKind::Fake
    }

    fn executable(&self) -> &Path {
        &self.executable
    }

    fn probe(&self) -> Result<ProbeReport, RuntimeError> {
        Ok(ProbeReport {
            kind: RuntimeKind::Fake,
            executable: self.executable.display().to_string(),
            version: Some(env!("CARGO_PKG_VERSION").to_owned()),
            readiness: Readiness::Ready,
            detail: "deterministic in-process runtime".to_owned(),
        })
    }

    fn start(&self, request: InvocationRequest) -> Result<Box<dyn RuntimeSession>, RuntimeError> {
        Ok(Box::new(FakeSession {
            invocation_id: request.invocation_id,
            steps: self.script.clone().into(),
            sequence: 0,
            last_event: None,
            started: false,
            yielded: false,
            cancellation: request.cancellation,
            interrupted: false,
            interruption_emitted: false,
        }))
    }
}

struct FakeSession {
    invocation_id: String,
    steps: VecDeque<ScriptStep>,
    sequence: u64,
    last_event: Option<RuntimeEvent>,
    started: bool,
    yielded: bool,
    cancellation: CancellationToken,
    interrupted: bool,
    interruption_emitted: bool,
}

impl FakeSession {
    fn emit(&mut self, event: RuntimeEventKind) -> RuntimeEvent {
        self.sequence += 1;
        let envelope = RuntimeEvent {
            sequence: self.sequence,
            event_id: format!("{}.event-{}", self.invocation_id, self.sequence),
            invocation_id: self.invocation_id.clone(),
            event,
        };
        self.last_event = Some(envelope.clone());
        envelope
    }
}

impl RuntimeSession for FakeSession {
    fn next_event(&mut self) -> Result<Option<RuntimeEvent>, RuntimeError> {
        if self.cancellation.is_cancelled() {
            self.interrupted = true;
            self.steps.clear();
        }
        if self.interrupted {
            if self.interruption_emitted {
                return Ok(None);
            }
            self.interruption_emitted = true;
            return Ok(Some(self.emit(RuntimeEventKind::Interrupted)));
        }
        if !self.started {
            self.started = true;
            return Ok(Some(self.emit(RuntimeEventKind::Started {
                opaque_session_id: format!("{}.opaque", self.invocation_id),
            })));
        }
        if self.yielded {
            return Ok(None);
        }
        let Some(step) = self.steps.pop_front() else {
            return Ok(None);
        };
        match step {
            ScriptStep::Output(text) => Ok(Some(self.emit(RuntimeEventKind::Output { text }))),
            ScriptStep::Yield(cursor) => {
                self.yielded = true;
                Ok(Some(self.emit(RuntimeEventKind::Yielded { cursor })))
            }
            ScriptStep::DuplicateLast => Ok(self.last_event.clone()),
            ScriptStep::Malformed(detail) => Err(RuntimeError::MalformedEvent(detail)),
            ScriptStep::Complete(usage) => {
                Ok(Some(self.emit(RuntimeEventKind::Completed { usage })))
            }
        }
    }

    fn resume(&mut self, _input: String) -> Result<(), RuntimeError> {
        if !self.yielded {
            return Err(RuntimeError::NotYielded);
        }
        self.yielded = false;
        Ok(())
    }

    fn interrupt(&mut self) -> Result<(), RuntimeError> {
        self.cancellation.cancel();
        self.interrupted = true;
        self.steps.clear();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{FakeRuntime, ScriptStep};
    use ymp_runtime_api::{
        InvocationRequest, RuntimeDriver, RuntimeError, RuntimeEventKind, Usage,
    };

    fn request() -> InvocationRequest {
        InvocationRequest {
            invocation_id: "invocation-1".to_owned(),
            attempt_id: "attempt-1".to_owned(),
            workspace: std::env::current_dir().expect("current directory"),
            mcp: None,
            prompt: "fixture".to_owned(),
            cancellation: Default::default(),
        }
    }

    #[test]
    fn session_yields_resumes_and_completes() {
        let runtime = FakeRuntime::default();
        let mut session = runtime.start(request()).expect("start session");
        assert!(matches!(
            session.next_event().expect("started").expect("event").event,
            RuntimeEventKind::Started { .. }
        ));
        assert!(matches!(
            session.next_event().expect("output").expect("event").event,
            RuntimeEventKind::Output { .. }
        ));
        assert!(matches!(
            session.next_event().expect("yield").expect("event").event,
            RuntimeEventKind::Yielded { .. }
        ));
        assert!(session.next_event().expect("wait").is_none());
        session.resume("wake".to_owned()).expect("resume");
        assert!(matches!(
            session.next_event().expect("output").expect("event").event,
            RuntimeEventKind::Output { .. }
        ));
        assert!(matches!(
            session
                .next_event()
                .expect("complete")
                .expect("event")
                .event,
            RuntimeEventKind::Completed { .. }
        ));
    }

    #[test]
    fn duplicate_and_malformed_events_are_reproducible() {
        let runtime = FakeRuntime::with_script(vec![
            ScriptStep::Output("one".to_owned()),
            ScriptStep::DuplicateLast,
            ScriptStep::Malformed("broken fixture".to_owned()),
        ]);
        let mut session = runtime.start(request()).expect("start session");
        session.next_event().expect("started");
        let first = session.next_event().expect("first").expect("event");
        let duplicate = session.next_event().expect("duplicate").expect("event");
        assert_eq!(first, duplicate);
        assert!(matches!(
            session.next_event(),
            Err(RuntimeError::MalformedEvent(_))
        ));
    }

    #[test]
    fn interrupt_is_terminal_for_the_session() {
        let runtime = FakeRuntime::with_script(vec![ScriptStep::Complete(Usage::default())]);
        let mut session = runtime.start(request()).expect("start session");
        session.next_event().expect("started");
        session.interrupt().expect("interrupt");
        assert!(matches!(
            session
                .next_event()
                .expect("interrupted")
                .expect("event")
                .event,
            RuntimeEventKind::Interrupted
        ));
        assert!(session.next_event().expect("terminal").is_none());
    }
}
