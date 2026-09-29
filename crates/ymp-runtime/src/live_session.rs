//! One thread owns the Dispatcher. Other threads send owner commands and read the
//! last published projection; they never wait for provider work. The published
//! projection is a journal replay and a command is the same trusted Application
//! operation a direct caller would use.
use crate::{
    application::{Clarification, IntakeNote, IntakeRefinement, SessionView},
    clock::Clock,
    dispatcher::{Dispatcher, Tick},
};
use std::{
    sync::{
        Arc, Mutex,
        mpsc::{Receiver, RecvTimeoutError, Sender, TryRecvError, channel},
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};
use ymp_domain::{Denial, Result};
use ymp_kernel::journal::{ContentStore, Journal};

/// What the owning thread last did. It is a local fact, never a recorded status.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Pulse {
    Opening,
    Working,
    NeedsUser,
    Observed,
    Delivered,
    Failed(Denial),
    Closed,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OwnerCommand {
    Interrupt,
    Answer {
        question: String,
        answer: String,
        /// The revision the user was looking at when answering.
        expected_revision: u64,
    },
}
/// Name of the thread that owns a Dispatcher.
pub const THREAD: &str = "ymp-session";
/// Longest time a recorded change stays unpublished while the session waits.
const PUBLISH: Duration = Duration::from_millis(250);
/// How many refused report steps of a stopped session are taken again. The
/// execution host records the end of a stopped call from its own threads, so a
/// report step prepared meanwhile can be refused because the journal moved.
const MOVED: u32 = 50;

/// A refused command leaves `after` equal to `before`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandOutcome {
    pub command: OwnerCommand,
    pub before: u64,
    pub after: u64,
    pub denial: Option<Denial>,
}
#[derive(Clone)]
pub struct Published {
    pub view: Option<Arc<SessionView>>,
    pub pulse: Pulse,
}
struct Shared {
    published: Published,
    outcomes: Vec<CommandOutcome>,
}
enum Message {
    Command(OwnerCommand),
    Close,
}
pub struct LiveSession {
    messages: Sender<Message>,
    shared: Arc<Mutex<Shared>>,
    thread: Option<JoinHandle<()>>,
}
fn poisoned() -> Denial {
    Denial::new("live_session", "The session thread ended abnormally")
}
impl LiveSession {
    /// `build` runs on the new thread because a Dispatcher and its strategies
    /// stay on the thread that created them.
    pub fn spawn<J, C, B>(clock: Arc<dyn Clock>, build: B) -> Result<Self>
    where
        J: Journal + 'static,
        C: ContentStore + 'static,
        B: FnOnce() -> Result<Dispatcher<J, C>> + Send + 'static,
    {
        let (messages, inbox) = channel();
        let shared = Arc::new(Mutex::new(Shared {
            published: Published {
                view: None,
                pulse: Pulse::Opening,
            },
            outcomes: vec![],
        }));
        let owned = shared.clone();
        let thread = std::thread::Builder::new()
            .name(THREAD.into())
            .spawn(move || {
                let pulse = match build() {
                    Ok(dispatcher) => Worker {
                        dispatcher,
                        clock,
                        inbox,
                        shared: owned.clone(),
                        revision: None,
                        read: None,
                    }
                    .run(),
                    Err(denial) => Pulse::Failed(denial),
                };
                if let Ok(mut shared) = owned.lock() {
                    shared.published.pulse = pulse;
                }
            })
            .map_err(|_| Denial::new("live_session", "Cannot start the session thread"))?;
        Ok(Self {
            messages,
            shared,
            thread: Some(thread),
        })
    }
    pub fn published(&self) -> Result<Published> {
        Ok(self
            .shared
            .lock()
            .map_err(|_| poisoned())?
            .published
            .clone())
    }
    /// Outcomes of commands handled since the previous call.
    pub fn outcomes(&self) -> Result<Vec<CommandOutcome>> {
        Ok(std::mem::take(
            &mut self.shared.lock().map_err(|_| poisoned())?.outcomes,
        ))
    }
    pub fn send(&self, command: OwnerCommand) -> Result<()> {
        self.messages
            .send(Message::Command(command))
            .map_err(|_| Denial::new("live_session", "The session thread has ended"))
    }
    /// The owning thread has returned: no command can reach the session any more.
    pub fn ended(&self) -> bool {
        self.thread.as_ref().is_none_or(JoinHandle::is_finished)
    }
    /// Stop driving the session. Nothing is recorded: a later recovery decides.
    pub fn close(mut self) {
        self.end();
    }
    fn end(&mut self) {
        let _ = self.messages.send(Message::Close);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
impl Drop for LiveSession {
    fn drop(&mut self) {
        self.end();
    }
}
struct Worker<J: Journal, C: ContentStore> {
    dispatcher: Dispatcher<J, C>,
    clock: Arc<dyn Clock>,
    inbox: Receiver<Message>,
    shared: Arc<Mutex<Shared>>,
    revision: Option<u64>,
    /// When the projection was last read for publication.
    read: Option<Instant>,
}
impl<J: Journal + 'static, C: ContentStore + 'static> Worker<J, C> {
    fn run(mut self) -> Pulse {
        let mut pulse = Pulse::Working;
        let mut settled = true;
        let mut moved = 0;
        loop {
            self.publish(&pulse, settled);
            // An idle session waits for the owner; a working one only looks.
            let message = if matches!(pulse, Pulse::Working) {
                match self.inbox.try_recv() {
                    Ok(message) => Some(message),
                    Err(TryRecvError::Empty) => None,
                    Err(TryRecvError::Disconnected) => return Pulse::Closed,
                }
            } else {
                match self.inbox.recv_timeout(Duration::from_millis(100)) {
                    Ok(message) => Some(message),
                    Err(RecvTimeoutError::Timeout) => None,
                    Err(RecvTimeoutError::Disconnected) => return Pulse::Closed,
                }
            };
            match message {
                Some(Message::Close) => return Pulse::Closed,
                Some(Message::Command(command)) => {
                    // A refused command changed nothing, so the pulse stays.
                    if self.command(command) {
                        pulse = Pulse::Working;
                    }
                    settled = true;
                    continue;
                }
                None if matches!(pulse, Pulse::Failed(_) | Pulse::Delivered | Pulse::Observed) => {
                    continue;
                }
                None => {}
            }
            let tick = self.dispatcher.tick();
            // Waiting repeats many times a second; reading the projection then
            // is a journal replay, so it is published at a bounded rate.
            settled = !matches!(tick, Ok(Tick::Waiting { .. }));
            pulse = match tick {
                Err(denial) if moved < MOVED && self.repeats(&denial) => {
                    moved += 1;
                    std::thread::sleep(Duration::from_millis(20));
                    Pulse::Working
                }
                Ok(Tick::Advanced) => Pulse::Working,
                Ok(Tick::Waiting { until }) => {
                    let delay = until
                        .saturating_sub(self.clock.now().unwrap_or(until))
                        .clamp(1, 20);
                    std::thread::sleep(Duration::from_millis(delay));
                    Pulse::Working
                }
                Ok(Tick::NeedsUser) => Pulse::NeedsUser,
                Ok(Tick::Observed) => Pulse::Observed,
                Ok(Tick::Delivered(_)) => Pulse::Delivered,
                Err(denial) => Pulse::Failed(denial),
            };
        }
    }
    /// Only a session the owner stopped takes a refused step again. Its steps
    /// compose the report: each reads the record first, continues from what is
    /// recorded and starts no call. Any other refused step may have recorded a
    /// part of itself or hold a prepared call, so it fails as it is.
    fn repeats(&self, denial: &Denial) -> bool {
        self.dispatcher.owner().stopped()
            && matches!(denial.code.as_str(), "stale_revision" | "report_phase")
    }
    /// Whether the command was accepted.
    fn command(&mut self, command: OwnerCommand) -> bool {
        let before = self.dispatcher.view().map(|view| view.revision());
        let result = before.clone().and_then(|_| self.apply(&command));
        let after = self.dispatcher.view().map(|view| view.revision());
        let before = before.unwrap_or_default();
        let accepted = result.is_ok();
        if let Ok(mut shared) = self.shared.lock() {
            shared.outcomes.push(CommandOutcome {
                command,
                before,
                after: after.unwrap_or(before),
                denial: result.err(),
            });
        }
        accepted
    }
    fn apply(&mut self, command: &OwnerCommand) -> Result<()> {
        let view = self.dispatcher.view()?;
        let at = self.clock.now()?.max(view.latest_at());
        let owner = self.dispatcher.owner();
        match command {
            OwnerCommand::Interrupt => self
                .dispatcher
                .application()
                .interrupt(&owner, at)
                .map(|_| ()),
            OwnerCommand::Answer {
                question,
                answer,
                expected_revision,
            } => {
                let task = view
                    .task()
                    .ok_or_else(|| Denial::new("task_missing", "No task is open"))?;
                self.dispatcher
                    .application()
                    .refine(
                        &owner,
                        IntakeRefinement {
                            expected_revision: *expected_revision,
                            at,
                            constraints: task.constraints.clone(),
                            criteria: view.criteria().to_vec(),
                            reason: "The owner answered an intake question".into(),
                            note: IntakeNote::Clarification(Clarification {
                                question: question.clone(),
                                answer: answer.clone(),
                                at,
                            }),
                        },
                    )
                    .map(|_| ())
            }
        }
    }
    fn publish(&mut self, pulse: &Pulse, settled: bool) {
        let due = settled || self.read.is_none_or(|read| read.elapsed() >= PUBLISH);
        let view = match due.then(|| self.dispatcher.view()) {
            Some(Ok(view)) if Some(view.revision()) != self.revision => {
                self.revision = Some(view.revision());
                Some(Arc::new(view))
            }
            _ => None,
        };
        if due {
            self.read = Some(Instant::now());
        }
        if let Ok(mut shared) = self.shared.lock() {
            if view.is_some() {
                shared.published.view = view;
            }
            shared.published.pulse = pulse.clone();
        }
    }
}
