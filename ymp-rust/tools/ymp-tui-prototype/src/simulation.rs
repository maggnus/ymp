//! Deterministic, in-memory product simulation. No processes, models, network or source writes.

pub use crate::entities::*;
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Simulation {
    pub now: u64,
    pub revision: u64,
    pub providers: Vec<Provider>,
    pub generation: u64,
    pub next_task: usize,
    pub next_provider: usize,
    pub audit: Vec<Audit>,
    pub runs: Vec<Run>,
    pub candidates: Vec<Candidate>,
    pub exports: Vec<Export>,
    pub tasks: Vec<Task>,
    pub active: Option<usize>,
    pub agents: Vec<Agent>,
    pub posts: Vec<Post>,
    pub tools: Vec<ToolCall>,
    pub checks: Vec<Check>,
    pub max_agents: usize,
    pub max_parallel: usize,
}
impl Default for Simulation {
    fn default() -> Self {
        Self {
            now: 0,
            revision: 0,
            providers: Vec::new(),
            generation: 0,
            next_task: 1,
            next_provider: 1,
            audit: vec![],
            runs: vec![],
            candidates: vec![],
            exports: vec![],
            tasks: vec![],
            active: None,
            agents: vec![],
            posts: vec![],
            tools: vec![],
            checks: vec![],
            max_agents: 6,
            max_parallel: 3,
        }
    }
}
pub fn age(now: u64, at: u64) -> String {
    let Some(ms) = now.checked_sub(at) else {
        return "—".into();
    };
    let s = ms / 1000;
    if s < 60 {
        format!("{s}s")
    } else if s < 3600 {
        format!("{}m {}s", s / 60, s % 60)
    } else {
        format!("{}h {}m", s / 3600, s % 3600 / 60)
    }
}
impl Simulation {
    pub fn task(&self) -> Option<&Task> {
        self.active.map(|i| &self.tasks[i])
    }
    pub fn busy(&self) -> bool {
        self.task()
            .is_some_and(|t| matches!(t.stage, Stage::Developing | Stage::Verifying))
    }
    pub(crate) fn changed(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.refresh_runs();
        self.revision = self.revision.wrapping_add(1);
    }
    pub(crate) fn say(&mut self, author: Option<String>, text: impl Into<String>, user: bool) {
        if let Some(i) = self.active {
            self.tasks[i].entries.push(Entry {
                author,
                text: text.into(),
                user,
                tool: None,
            });
        }
    }
    pub fn profiles(&self) -> Vec<Profile> {
        self.providers
            .iter()
            .filter(|p| p.enabled && p.checked.is_some())
            .map(Provider::profile)
            .collect()
    }
    pub fn toggle_provider(&mut self, index: usize) {
        if let Some(p) = self.providers.get(index) {
            let id = p.id.clone();
            let enabled = !p.enabled;
            let _ = self.set_provider_enabled(&id, enabled);
        }
    }
    pub fn new_task(&mut self) -> Result<(), &'static str> {
        self.active = None;
        self.changed();
        Ok(())
    }
    pub fn select_task(&mut self, id: &str) {
        if let Some(i) = self.tasks.iter().position(|t| t.id == id) {
            self.active = Some(i);
            self.changed();
        }
    }
    pub fn submit(&mut self, text: &str) {
        if self.active.is_none() {
            let id = format!("T-{:03}", self.next_task);
            self.next_task += 1;
            self.tasks.push(Task {
                id,
                goal: text.into(),
                requirements: String::new(),
                created: self.now,
                stage: Stage::ProviderRequired,
                archived: false,
                run: 0,
                run_started: None,
                run_ended: None,
                step: 0,
                next_step: 0,
                candidate: 0,
                entries: vec![],
                profiles: vec![],
                max_agents: self.max_agents,
                max_parallel: self.max_parallel,
            });
            self.active = Some(self.tasks.len() - 1);
            self.record(
                "operator",
                "Task created",
                self.tasks.last().unwrap().id.clone(),
                "Operator message",
            );
            self.say(None, text, true);
            if self.profiles().is_empty() {
                self.say(
                    None,
                    "Task saved. Enable a provider to continue the conversation. /providers",
                    false,
                );
            } else {
                let _ = self.continue_conversation();
            }
        } else {
            self.say(None, text, true);
            let i = self.active.unwrap();
            match self.tasks[i].stage {
                Stage::Clarifying => {
                    self.tasks[i].requirements=text.into(); self.tasks[i].stage=Stage::Ready;
                    let task_id=self.tasks[i].id.clone();
                    let author=self.agents.iter_mut().rev().find(|a|a.task==task_id && a.run==0 && a.ended.is_none()).map(|a| {
                        a.state=Status::Completed;a.ended=Some(self.now);a.output="Task specification proposed; no development authorized.".into();a.id.clone()
                    });
                    self.say(author,"Task proposal recorded: browser game against a computer, 10×10 board, manual/random placement, no network play. Your requirements are included verbatim in Tasks. Review the task, then /start authorizes development.",false);
                }
                Stage::Ready => {
                    self.tasks[i].requirements.push('\n');self.tasks[i].requirements.push_str(text);
                    self.say(None,"Task proposal updated. /start uses this revision.",false);
                }
                Stage::ProviderRequired => self.say(None,"Message saved. /providers configures model access; /continue resumes clarification.",false),
                _ => {
                    self.tasks[i].requirements.push_str("\nFeedback for next run: ");
                    self.tasks[i].requirements.push_str(text);
                    self.say(None,"Feedback recorded. The running assignment is unchanged. /stop stops active work; /retry starts a new attempt using the recorded feedback.",false);
                }
            }
        }
        self.changed();
    }
    pub fn continue_conversation(&mut self) -> Result<(), &'static str> {
        let Some(i) = self.active else {
            return Err("Describe a task first.");
        };
        if self.tasks[i].archived {
            return Err("Restore the task before continuing.");
        }
        if self.tasks[i].stage != Stage::ProviderRequired {
            return Err("The conversation is already available.");
        }
        let Some(profile) = self.profiles().first().cloned() else {
            return Err("Enable a provider first. /providers");
        };
        let task = self.tasks[i].id.clone();
        let id = self.add_agent(
            task,
            0,
            "Operator message".into(),
            "Clarify the task; read-only conversation".into(),
            profile,
        );
        self.agents.last_mut().unwrap().state = Status::Waiting;
        self.tasks[i].stage = Stage::Clarifying;
        self.say(Some(id),"For this Battleship simulation: a browser game against a computer, a 10×10 board, manual and random placement, and a new-game action. Ships cannot touch, including diagonally; a hit retains the turn. What would you change? Reply with your requirements, or confirm these choices.",false);
        self.changed();
        Ok(())
    }
    pub(crate) fn add_agent(
        &mut self,
        task: String,
        run: usize,
        source: String,
        assignment: String,
        profile: Profile,
    ) -> String {
        let id = format!("A-{:04}", self.agents.len() + 1);
        self.agents.push(Agent {
            id: id.clone(),
            task,
            run,
            source,
            assignment,
            profile,
            created: self.now,
            ended: None,
            state: Status::Starting,
            output: String::new(),
            archived: false,
            role: if run == 0 {
                Role::Clarifier
            } else {
                Role::Origin
            },
            due: None,
            replaces: None,
        });
        let a = self.agents.last().unwrap();
        self.record(
            a.source.clone(),
            "Agent created",
            id.clone(),
            a.assignment.clone(),
        );
        id
    }
    pub fn start(&mut self, retry: bool) -> Result<(), &'static str> {
        let Some(i) = self.active else {
            return Err("Describe a task first.");
        };
        if self.tasks[i].archived {
            return Err("Restore the task before starting work.");
        }
        let stage = self.tasks[i].stage;
        let allowed = if retry {
            matches!(
                stage,
                Stage::Stopped | Stage::Review | Stage::Delivered | Stage::Cancelled
            )
        } else {
            stage == Stage::Ready
        };
        if !allowed {
            return Err("This task is not ready for a new development run.");
        }
        if self.max_agents == 0 || self.max_parallel == 0 {
            return Err("Run capacity must permit at least one agent and one active attempt.");
        }
        let profiles = self.profiles();
        let Some(origin) = profiles.first().cloned() else {
            return Err("No enabled provider. /providers");
        };
        self.tasks[i].run += 1;
        self.tasks[i].run_started = Some(self.now);
        self.tasks[i].run_ended = None;
        self.tasks[i].step = 0;
        self.tasks[i].next_step = self.now + 2500;
        self.tasks[i].candidate = 1;
        self.tasks[i].profiles = profiles;
        self.tasks[i].max_agents = self.max_agents;
        self.tasks[i].max_parallel = self.max_parallel;
        self.tasks[i].stage = Stage::Developing;
        let id = self.add_agent(
            self.tasks[i].id.clone(),
            self.tasks[i].run,
            "Operator: Start development".into(),
            self.specification(),
            origin,
        );
        self.runs.push(Run {
            task: self.tasks[i].id.clone(),
            number: self.tasks[i].run,
            origin: id.clone(),
            started: self.now,
            ended: None,
            state: Stage::Developing,
            specification: self.specification(),
            profiles: self.tasks[i].profiles.clone(),
            max_agents: self.max_agents,
            max_parallel: self.max_parallel,
        });
        self.record(
            "operator",
            "Development authorized",
            self.tasks[i].id.clone(),
            format!("R-{:03}; origin {id}", self.tasks[i].run),
        );
        self.say(None,format!("Development R-{:03} authorized. {id} is starting with the approved task and frozen model settings. All activity in this session is simulated.",self.tasks[i].run),false);
        self.changed();
        Ok(())
    }
    pub fn stop(&mut self) -> Result<(), &'static str> {
        if !self.busy() {
            return Err("No active development run.");
        }
        let i = self.active.unwrap();
        let task = self.tasks[i].id.clone();
        let run = self.tasks[i].run;
        for agent in self
            .agents
            .iter_mut()
            .filter(|a| a.task == task && a.run == run && a.ended.is_none())
        {
            agent.state = Status::Interrupted;
            agent.ended = Some(self.now);
            agent.output = "Stopped by operator; partial work retained.".into();
        }
        for tool in self
            .tools
            .iter_mut()
            .filter(|t| t.task == task && t.run == run && t.ended.is_none())
        {
            tool.status = Status::Stopped;
            tool.ended = Some(self.now);
            tool.output.push_str("\nStopped by operator.");
        }
        self.tasks[i].stage = Stage::Stopped;
        self.tasks[i].run_ended = Some(self.now);
        self.say(None,"Development stopped. Agent records, tool output and partial candidates are retained. /retry creates a new run.",false);
        self.changed();
        Ok(())
    }
    pub fn accept(&mut self) -> Result<(), &'static str> {
        let Some(i) = self.active else {
            return Err("No candidate to accept.");
        };
        if self.tasks[i].stage != Stage::Review {
            return Err("A checked candidate is required before acceptance.");
        }
        self.tasks[i].stage = Stage::Delivered;
        self.say(None,"Demo result accepted. Files, changes and check evidence remain available in /files and /checks. No game files were copied into your project.",false);
        self.changed();
        Ok(())
    }
    pub fn advance_to(&mut self, now: u64) {
        if now < self.now {
            return;
        }
        if self.now / 1000 != now / 1000 {
            self.revision = self.revision.wrapping_add(1);
        }
        self.now = now;
        self.advance_manual_agents();
        // Advance each run even if the operator inspects a different historical task.
        let selected = self.active;
        for i in 0..self.tasks.len() {
            if matches!(self.tasks[i].stage, Stage::Developing | Stage::Verifying)
                && now >= self.tasks[i].next_step
            {
                self.active = Some(i);
                self.step();
                self.tasks[i].next_step = now + 3000;
                self.changed();
            }
        }
        self.active = selected;
    }
    pub(crate) fn post(&mut self, task: &str, author: &str, text: &str) {
        self.posts.push(Post {
            id: format!("M-{:03}", self.posts.len() + 1),
            task: task.into(),
            author: author.into(),
            text: text.into(),
            at: self.now,
        });
    }
    pub(crate) fn tool(
        &mut self,
        task: &str,
        run: usize,
        agent: &str,
        command: &str,
        status: &'static str,
        output: &str,
    ) {
        self.tools.push(ToolCall {
            id: format!("X-{:03}", self.tools.len() + 1),
            task: task.into(),
            run,
            agent: agent.into(),
            command: command.into(),
            started: self.now,
            ended: if status == "Running" {
                None
            } else {
                Some(self.now)
            },
            status: Status::from_label(status),
            output: output.into(),
        });
        let id = self.tools.last().unwrap().id.clone();
        if let Some(task) = self.tasks.iter_mut().find(|t| t.id == task) {
            task.entries.push(Entry {
                author: Some(agent.into()),
                text: command.into(),
                user: false,
                tool: Some(id.clone()),
            });
        }
        self.record(agent, "Tool invoked", id, command);
    }
    fn check(
        &mut self,
        task: &str,
        run: usize,
        candidate: usize,
        name: &'static str,
        passed: bool,
        detail: &'static str,
    ) {
        self.checks.push(Check {
            id: format!("V-{:03}", self.checks.len() + 1),
            task: task.into(),
            run,
            candidate,
            name: name.into(),
            passed,
            detail: detail.into(),
        });
    }
    fn step(&mut self) {
        let i = self.active.unwrap();
        let task = self.tasks[i].id.clone();
        let run = self.tasks[i].run;
        let Some(origin_index) = self
            .agents
            .iter()
            .position(|a| a.task == task && a.run == run)
        else {
            return;
        };
        if self.agents[origin_index].state == Status::Interrupted
            || self.agents[origin_index].state == Status::Failed
        {
            return;
        }
        let origin = self.agents[origin_index].id.clone();
        match self.tasks[i].step {
            0 => {
                self.agents[origin_index].state = Status::Running;
                self.capture_candidate(&task, run, 1);
                self.tool(
                    &task,
                    run,
                    &origin,
                    "write_file src/game.ts",
                    "Completed",
                    "Simulated: game rules and board interactions written to candidate C-001.",
                );
                self.post(&task,&origin,"Separate ship-placement rules from board interaction. Request a focused check of overlap and diagonal touching.");
                self.say(Some(origin.clone()),"The first candidate is prepared. I am checking placement rules before returning it.",false);
            }
            1 => {
                let count = self
                    .agents
                    .iter()
                    .filter(|a| a.task == task && a.run == run)
                    .count();
                let active = self
                    .agents
                    .iter()
                    .filter(|a| a.task == task && a.run == run && a.ended.is_none())
                    .count();
                if count < self.tasks[i].max_agents && active < self.tasks[i].max_parallel {
                    let profile = self.tasks[i].profiles.last().unwrap().clone();
                    let child=self.add_agent(task.clone(),run,format!("{origin}: placement-check request"),"Check placement rules: bounds, overlap and diagonal touching. Return evidence on Board.".into(),profile);
                    self.agents.last_mut().unwrap().state = Status::Running;
                    self.agents.last_mut().unwrap().role = Role::Recruit;
                    self.tool(
                        &task,
                        run,
                        &child,
                        "npm test -- --run placement",
                        "Running",
                        "Simulated test invocation; awaiting output.",
                    );
                    self.say(None,format!("{child} joined after an admitted request from {origin}. Assignment: placement-rule checks."),false);
                } else {
                    self.post(&task,&origin,"Recruitment was refused by the frozen capacity limit. I will perform the placement check in this attempt.");
                    self.tool(
                        &task,
                        run,
                        &origin,
                        "npm test -- --run placement",
                        "Running",
                        "Simulated test invocation; no additional participant admitted.",
                    );
                }
            }
            2 => {
                let author = self
                    .agents
                    .iter()
                    .rev()
                    .find(|a| {
                        a.task == task
                            && a.run == run
                            && a.role == Role::Recruit
                            && a.ended.is_none()
                    })
                    .unwrap_or(&self.agents[origin_index])
                    .id
                    .clone();
                for t in self
                    .tools
                    .iter_mut()
                    .filter(|t| t.task == task && t.run == run && t.ended.is_none())
                {
                    t.status = Status::Failed;
                    t.ended = Some(self.now);
                    t.output="Simulated exit 1: diagonal-touching case failed; expected rejected placement.".into();
                }
                self.check(&task,run,1,"Placement rules",false,"Simulated agent test: diagonally touching ships were accepted. This evidence belongs to C-001.");
                self.post(&task,&author,&format!("C-001 allows diagonal touching. The failing placement case is attached to {}; this needs a correction.",self.checks.last().unwrap().id));
                for a in self.agents.iter_mut().filter(|a| {
                    a.task == task && a.run == run && a.role == Role::Recruit && a.ended.is_none()
                }) {
                    a.state = Status::Completed;
                    a.ended = Some(self.now);
                    a.output = "Returned the failing placement example on Board.".into();
                }
                self.say(None,"The first candidate failed a placement check. The failure is retained; a correction is in progress.",false);
            }
            3 => {
                self.tasks[i].candidate = 2;
                self.capture_candidate(&task, run, 2);
                self.tool(
                    &task,
                    run,
                    &origin,
                    "apply_patch src/game.ts",
                    "Completed",
                    "Simulated: reject diagonal neighbours; candidate C-002 supersedes C-001.",
                );
                self.post(&task,&origin,"C-002 corrects diagonal placement. Earlier C-001 check results do not establish validity of this revision.");
            }
            4 => {
                self.tool(&task,run,&origin,"npm run dev -- --host 127.0.0.1","Running","Simulated server readiness: http://127.0.0.1:5173 (no real server was started).");
                self.tool(&task,run,&origin,"browser: placement → shots → victory","Completed","Simulated browser observations: placement, repeated-shot protection and terminal state. No browser was opened.");
                self.say(Some(origin.clone()),"Checking the revised candidate through external server and browser tools. Tool output and process ownership are in /tools.",false);
            }
            5 => {
                if self.agents.iter().any(|a| {
                    a.task == task && a.run == run && a.role == Role::Manual && a.ended.is_none()
                }) {
                    return;
                }
                for t in self
                    .tools
                    .iter_mut()
                    .filter(|t| t.task == task && t.run == run && t.ended.is_none())
                {
                    t.status = Status::Completed;
                    t.ended = Some(self.now);
                    t.output
                        .push_str("\nSimulated server stopped after the check; exit 0.");
                }
                self.agents[origin_index].state = Status::Completed;
                self.agents[origin_index].ended = Some(self.now);
                self.agents[origin_index].output="Returned C-002. Completion does not establish verification or operator acceptance.".into();
                self.tasks[i].stage = Stage::Verifying;
                self.say(None,"The development agent returned C-002 and finished. Separate candidate checks are pending; the result is not yet accepted.",false);
            }
            6 => {
                self.check(&task,run,2,"Placement rules",true,"Simulated verification: board bounds, overlap and diagonal touching pass on C-002.");
                self.check(&task,run,2,"Turn and victory rules",true,"Simulated verification: repeated shots, hit/miss turns and victory pass on C-002.");
                self.check(&task,run,2,"Browser interaction",true,"Simulated browser check on C-002: placement, shots, terminal state. Broader usability remains unverified.");
                self.tasks[i].stage = Stage::Review;
                self.tasks[i].run_ended = Some(self.now);
                self.say(None,"C-002 is ready for review: 3 simulated checks passed. Inspect /files and /checks, then /accept or record feedback and /retry. The previous failure remains in the history.",false);
            }
            _ => {}
        }
        self.tasks[i].step += 1;
    }
    pub fn specification(&self) -> String {
        self.task().map_or_else(String::new,|t|format!("Goal: {}\nRequirements: {}\n\nScenario: browser Battleship against a computer; 10×10 board; manual/random placement; no touching ships; repeated shots rejected; hit retains turn; last ship sunk wins.\nDeliverables: source, rule tests, usage document.\nExternal tools are allowed; no embedded playable screen.\nSimulation only; no model execution or project writes.",t.goal,t.requirements))
    }
    pub fn next_action(&self) -> (&'static str, &'static str) {
        if self.task().is_some_and(|t| t.archived) {
            return ("Restore task", "/actions");
        }
        match self.task().map(|t| t.stage) {
            None => ("Describe a task", ""),
            Some(Stage::ProviderRequired) if self.profiles().is_empty() => {
                ("Configure provider", "/providers")
            }
            Some(Stage::ProviderRequired) => ("Continue conversation", "/continue"),
            Some(Stage::Clarifying) => ("Answer in chat", ""),
            Some(Stage::Ready) => ("Start development", "/start"),
            Some(Stage::Developing) => ("Inspect agents", "/agents"),
            Some(Stage::Verifying) => ("Review checks", "/checks"),
            Some(Stage::Review) => ("Review result", "/files"),
            Some(Stage::Delivered) => ("New task", "/new"),
            Some(Stage::Stopped) => ("Retry development", "/retry"),
            Some(Stage::Cancelled) => ("New task", "/new"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn prepared() -> Simulation {
        let mut s = Simulation::default();
        s.submit("Морской бой");
        s.add_provider("OpenAI", Family::OpenAi, true).unwrap();
        s.continue_conversation().unwrap();
        s.submit("Браузер, компьютер, интерфейс на русском.");
        s
    }
    #[test]
    fn agents_require_a_request_provider_and_development_authorization() {
        let mut s = Simulation::default();
        s.advance_to(50000);
        assert!(s.agents.is_empty());
        assert!(s.project().tasks.records.is_empty());
        assert!(s.continue_conversation().is_err());
        s.submit("Морской бой");
        assert!(s.agents.is_empty());
        assert!(s.continue_conversation().is_err());
        s.add_provider("OpenAI", Family::OpenAi, true).unwrap();
        assert!(s.agents.is_empty());
        s.continue_conversation().unwrap();
        assert_eq!(s.agents.len(), 1);
        assert_eq!(s.agents[0].run, 0);
        assert_eq!(s.agents[0].state, "Waiting");
        assert!(s.start(false).is_err());
        s.submit("Подтверждаю");
        assert_eq!(s.task().unwrap().stage, Stage::Ready);
        assert_eq!(s.agents[0].state, "Completed");
        assert!(s.accept().is_err());
        s.start(false).unwrap();
        assert_eq!(s.agents.len(), 2);
        assert!(s.start(false).is_err());
        assert_eq!(s.agents.len(), 2);
    }
    #[test]
    fn failure_correction_frozen_settings_and_delivery_are_separate_facts() {
        let mut s = prepared();
        s.add_provider("Anthropic", Family::Anthropic, true)
            .unwrap();
        s.start(false).unwrap();
        s.toggle_provider(0);
        s.toggle_provider(1);
        s.advance_to(3000);
        s.advance_to(6000);
        assert_eq!(s.agents.len(), 3);
        assert_eq!(s.agents[2].profile.provider, "anthropic");
        assert!(s.agents[2].source.contains(&s.agents[1].id));
        s.advance_to(9000);
        assert!(!s.checks[0].passed);
        assert_eq!(s.checks[0].candidate, 1);
        assert!(s.accept().is_err());
        s.advance_to(12000);
        assert_eq!(s.task().unwrap().candidate, 2);
        assert_eq!(s.checks.len(), 1);
        s.advance_to(15000);
        assert_eq!(s.tools.iter().filter(|t| t.ended.is_none()).count(), 1);
        s.advance_to(18000);
        assert_eq!(s.agents[1].state, "Completed");
        assert_eq!(s.task().unwrap().stage, Stage::Verifying);
        assert!(s.accept().is_err());
        s.advance_to(21000);
        assert_eq!(s.task().unwrap().stage, Stage::Review);
        assert_eq!(
            s.checks
                .iter()
                .filter(|c| c.candidate == 2 && c.passed)
                .count(),
            3
        );
        assert!(s.tools.iter().all(|t| t.ended.is_some()));
        s.accept().unwrap();
        assert_eq!(s.task().unwrap().stage, Stage::Delivered);
        assert!(s.start(true).is_err());
        s.toggle_provider(0);
        s.start(true).unwrap();
        assert_eq!(s.task().unwrap().run, 2);
        assert_eq!(s.checks[0].run, 1);
        assert_eq!(s.agents[1].profile.provider, "openai");
    }
    #[test]
    fn stop_ends_owned_tools_and_capacity_never_manufactures_a_team() {
        let mut s = prepared();
        s.max_agents = 1;
        s.max_parallel = 1;
        s.start(false).unwrap();
        for n in 1..=5 {
            s.advance_to(n * 3000);
        }
        assert_eq!(s.agents.iter().filter(|a| a.run == 1).count(), 1);
        assert!(s.posts.iter().any(|p| p.text.contains("refused")));
        s.stop().unwrap();
        assert_eq!(s.task().unwrap().stage, Stage::Stopped);
        assert!(s.tools.iter().all(|t| t.ended.is_some()));
        let count = s.tools.len();
        s.advance_to(90000);
        assert_eq!(s.tools.len(), count);
        assert!(s.accept().is_err());
        s.submit("Исправить повторный выстрел");
        s.start(true).unwrap();
        assert_eq!(s.task().unwrap().run, 2);
        assert!(
            s.agents
                .last()
                .unwrap()
                .assignment
                .contains("Исправить повторный выстрел")
        );
    }
    #[test]
    fn timestamps_are_elapsed_time_not_sequence_numbers() {
        assert_eq!(age(61000, 1000), "1m 0s");
        assert_eq!(age(1000, 61000), "—");
        let mut s = prepared();
        s.add_provider("Anthropic", Family::Anthropic, false)
            .unwrap();
        s.advance_to(5000);
        assert_eq!(s.project().agents.records[0].cells[4], "5s");
        assert!(s.project().providers.records[1].cells.last().unwrap() == "—");
    }
}
