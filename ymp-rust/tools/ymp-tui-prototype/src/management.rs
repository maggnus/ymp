//! Validated entity operations. UI menus and commands use these same mutation boundaries.
use crate::{entities::*, simulation::Simulation};

fn text(value: &str, label: &str, max: usize) -> Result<(), String> {
    if value.trim().is_empty() {
        return Err(format!("{label} is required."));
    }
    if value.chars().count() > max {
        return Err(format!("{label} exceeds {max} characters."));
    }
    if value
        .chars()
        .any(|c| c.is_control() && c != '\n' && c != '\t')
    {
        return Err(format!("{label} contains control characters."));
    }
    Ok(())
}
impl Simulation {
    pub(crate) fn record(
        &mut self,
        actor: impl Into<String>,
        action: impl Into<String>,
        entity: impl Into<String>,
        detail: impl Into<String>,
    ) {
        self.audit.push(Audit {
            sequence: self.audit.len() + 1,
            at: self.now,
            actor: actor.into(),
            action: action.into(),
            entity: entity.into(),
            detail: detail.into(),
        });
    }
    pub(crate) fn refresh_runs(&mut self) {
        for t in &self.tasks {
            if let Some(r) = self
                .runs
                .iter_mut()
                .find(|r| r.task == t.id && r.number == t.run)
            {
                r.state = t.stage;
                r.ended = t.run_ended;
            }
        }
    }
    pub fn add_provider(
        &mut self,
        name: &str,
        family: Family,
        enabled: bool,
    ) -> Result<String, String> {
        self.validate_provider_name(name, None)?;
        let id = format!("P-{:04}", self.next_provider);
        self.next_provider += 1;
        self.providers.push(Provider {
            id: id.clone(),
            name: name.trim().into(),
            family,
            enabled,
            checked: enabled.then_some(self.now),
            created: self.now,
        });
        self.record(
            "operator",
            "Connection added",
            id.clone(),
            format!("{} · {}", name.trim(), family.name()),
        );
        self.changed();
        Ok(id)
    }
    fn validate_provider_name(&self, name: &str, except: Option<&str>) -> Result<(), String> {
        text(name, "Connection name", 64)?;
        if name.chars().any(char::is_control) {
            return Err("Use a single-line connection name.".into());
        }
        if self.providers.iter().any(|p| {
            Some(p.id.as_str()) != except && p.name.to_lowercase() == name.trim().to_lowercase()
        }) {
            return Err("A connection with this name already exists.".into());
        }
        Ok(())
    }
    pub fn edit_provider(&mut self, id: &str, name: &str, family: Family) -> Result<(), String> {
        self.validate_provider_name(name, Some(id))?;
        let p = self
            .providers
            .iter_mut()
            .find(|p| p.id == id)
            .ok_or("Connection no longer exists.")?;
        let family_changed = p.family != family;
        p.name = name.trim().into();
        p.family = family;
        if family_changed {
            p.checked = None;
        }
        self.record(
            "operator",
            "Connection edited",
            id,
            format!(
                "{} · {}; existing launch profiles unchanged",
                name.trim(),
                family.name()
            ),
        );
        self.changed();
        Ok(())
    }
    pub fn set_provider_enabled(&mut self, id: &str, enabled: bool) -> Result<(), String> {
        let p = self
            .providers
            .iter_mut()
            .find(|p| p.id == id)
            .ok_or("Connection no longer exists.")?;
        if p.enabled == enabled {
            return Ok(());
        }
        p.enabled = enabled;
        if enabled {
            p.checked = Some(self.now);
        }
        self.record(
            "operator",
            if enabled {
                "Connection enabled"
            } else {
                "Connection disabled"
            },
            id,
            "Applies to future runs. Observation is simulated.",
        );
        self.changed();
        Ok(())
    }
    pub fn refresh_provider(&mut self, id: &str) -> Result<(), String> {
        let p = self
            .providers
            .iter_mut()
            .find(|p| p.id == id)
            .ok_or("Connection no longer exists.")?;
        if !p.enabled {
            return Err("Enable the connection before checking its profile.".into());
        }
        p.checked = Some(self.now);
        self.record(
            "operator",
            "Connection checked",
            id,
            "Simulated capability observation; no network request.",
        );
        self.changed();
        Ok(())
    }
    pub fn provider_removal_reason(&self, id: &str) -> Option<String> {
        if !self.providers.iter().any(|p| p.id == id) {
            return Some("Connection no longer exists.".into());
        }
        if let Some(a) = self
            .agents
            .iter()
            .find(|a| a.profile.connection == id && a.ended.is_none())
        {
            return Some(format!(
                "Agent {} still uses this connection. Stop its work first.",
                a.id
            ));
        }
        if let Some(t) = self
            .tasks
            .iter()
            .find(|t| t.stage.busy() && t.profiles.iter().any(|p| p.connection == id))
        {
            return Some(format!(
                "Run R-{:03} of {} still permits this connection. Stop the run first.",
                t.run, t.id
            ));
        }
        None
    }
    pub fn remove_provider(&mut self, id: &str) -> Result<(), String> {
        if let Some(reason) = self.provider_removal_reason(id) {
            return Err(reason);
        }
        let i = self
            .providers
            .iter()
            .position(|p| p.id == id)
            .ok_or("Connection no longer exists.")?;
        let p = self.providers.remove(i);
        self.record(
            "operator",
            "Connection removed",
            id,
            format!("{}; historical agent and run profiles retained", p.name),
        );
        self.changed();
        Ok(())
    }
    pub fn task_edit_reason(&self, id: &str) -> Option<String> {
        let Some(t) = self.tasks.iter().find(|t| t.id == id) else {
            return Some("Task no longer exists.".into());
        };
        if t.archived {
            return Some("Restore this task before editing it.".into());
        }
        if t.run > 0 {
            return Some(
                "This task has run history. Record feedback in Chat and start a new run.".into(),
            );
        }
        None
    }
    pub fn send_message(&mut self, message: &str) -> Result<(), String> {
        text(message, "Message", 16384)?;
        if self.task().is_some_and(|t| t.archived) {
            return Err("Restore the task before sending a message.".into());
        }
        if self.task().is_some_and(|t| t.stage == Stage::Cancelled) {
            return Err("Retry the cancelled task or create a new one.".into());
        }
        self.submit(message);
        Ok(())
    }
    pub fn create_task(&mut self, goal: &str, requirements: &str) -> Result<String, String> {
        text(goal, "Request", 8192)?;
        if requirements.len() > 16384 {
            return Err("Requirements exceed 16384 bytes.".into());
        }
        self.active = None;
        // A saved form is a local draft; it never silently starts a clarification agent.
        let id = format!("T-{:03}", self.next_task);
        self.next_task += 1;
        self.tasks.push(Task {
            id: id.clone(),
            goal: goal.trim().into(),
            requirements: requirements.into(),
            created: self.now,
            stage: Stage::ProviderRequired,
            archived: false,
            run: 0,
            run_started: None,
            run_ended: None,
            step: 0,
            next_step: 0,
            candidate: 0,
            entries: vec![Entry {
                author: None,
                text: goal.trim().into(),
                user: true,
                tool: None,
            }],
            profiles: vec![],
            max_agents: self.max_agents,
            max_parallel: self.max_parallel,
        });
        self.active = Some(self.tasks.len() - 1);
        self.record(
            "operator",
            "Task created",
            id.clone(),
            "Saved draft; development not authorized",
        );
        self.changed();
        Ok(id)
    }
    pub fn edit_task(&mut self, id: &str, goal: &str, requirements: &str) -> Result<(), String> {
        if let Some(reason) = self.task_edit_reason(id) {
            return Err(reason);
        }
        text(goal, "Request", 8192)?;
        if requirements.len() > 16384 {
            return Err("Requirements exceed 16384 bytes.".into());
        }
        for a in self
            .agents
            .iter_mut()
            .filter(|a| a.task == id && a.ended.is_none())
        {
            a.state = Status::Interrupted;
            a.ended = Some(self.now);
            a.output = "Task proposal was revised by the operator.".into();
        }
        let t = self.tasks.iter_mut().find(|t| t.id == id).unwrap();
        t.goal = goal.trim().into();
        t.requirements = requirements.into();
        t.stage = Stage::ProviderRequired;
        t.entries.push(Entry {
            author: None,
            text: format!("Task revised: {}\n{}", goal.trim(), requirements),
            user: true,
            tool: None,
        });
        self.record(
            "operator",
            "Task edited",
            id,
            "Previous clarification closed; a new conversation turn is required",
        );
        self.changed();
        Ok(())
    }
    pub fn task_delete_reason(&self, id: &str) -> Option<String> {
        let Some(t) = self.tasks.iter().find(|t| t.id == id) else {
            return Some("Task no longer exists.".into());
        };
        if t.run > 0
            || self.agents.iter().any(|a| a.task == id)
            || self.exports.iter().any(|e| e.task == id)
        {
            return Some("This task has execution history. Archive it instead.".into());
        }
        None
    }
    pub fn delete_task(&mut self, id: &str) -> Result<(), String> {
        if let Some(reason) = self.task_delete_reason(id) {
            return Err(reason);
        }
        let active_id = self.task().map(|t| t.id.clone());
        self.tasks.retain(|t| t.id != id);
        self.active = active_id.and_then(|id| self.tasks.iter().position(|t| t.id == id));
        self.record(
            "operator",
            "Draft deleted",
            id,
            "No execution history existed",
        );
        self.changed();
        Ok(())
    }
    pub fn task_archive_reason(&self, id: &str) -> Option<String> {
        let Some(t) = self.tasks.iter().find(|t| t.id == id) else {
            return Some("Task no longer exists.".into());
        };
        if t.stage.busy()
            || self
                .agents
                .iter()
                .any(|a| a.task == id && a.ended.is_none())
        {
            Some("Stop active work before archiving this task.".into())
        } else {
            None
        }
    }
    pub fn archive_task(&mut self, id: &str, archived: bool) -> Result<(), String> {
        if archived && let Some(reason) = self.task_archive_reason(id) {
            return Err(reason);
        }
        let t = self
            .tasks
            .iter_mut()
            .find(|t| t.id == id)
            .ok_or("Task no longer exists.")?;
        t.archived = archived;
        self.record(
            "operator",
            if archived {
                "Task archived"
            } else {
                "Task restored"
            },
            id,
            "History and evidence retained",
        );
        self.changed();
        Ok(())
    }
    pub fn cancel_task(&mut self, id: &str) -> Result<(), String> {
        let i = self
            .tasks
            .iter()
            .position(|t| t.id == id)
            .ok_or("Task no longer exists.")?;
        if self.tasks[i].archived
            || matches!(self.tasks[i].stage, Stage::Delivered | Stage::Cancelled)
        {
            return Err("This task is already closed. Restore or retry it instead.".into());
        }
        for a in self
            .agents
            .iter_mut()
            .filter(|a| a.task == id && a.ended.is_none())
        {
            a.state = Status::Interrupted;
            a.ended = Some(self.now);
            a.output = "Task cancelled by operator.".into();
        }
        for t in self
            .tools
            .iter_mut()
            .filter(|t| t.task == id && t.ended.is_none())
        {
            t.status = Status::Stopped;
            t.ended = Some(self.now);
            t.output.push_str("\nTask cancelled.");
        }
        self.tasks[i].stage = Stage::Cancelled;
        self.tasks[i].run_ended = Some(self.now);
        self.record(
            "operator",
            "Task cancelled",
            id,
            "All active assignments and owned processes stopped",
        );
        self.changed();
        Ok(())
    }
    pub fn agent_creation_reason(&self, task: &str) -> Option<String> {
        let Some(t) = self.tasks.iter().find(|t| t.id == task) else {
            return Some("Select a task first.".into());
        };
        if t.archived || t.stage != Stage::Developing {
            return Some(
                "An active development run is required. Start or retry the task first.".into(),
            );
        }
        let agents: Vec<_> = self
            .agents
            .iter()
            .filter(|a| a.task == task && a.run == t.run)
            .collect();
        if agents.len() >= t.max_agents {
            return Some(format!(
                "This run has used its {} agent starts. A new run is required.",
                t.max_agents
            ));
        }
        if agents.iter().filter(|a| a.ended.is_none()).count() >= t.max_parallel {
            return Some(format!(
                "All {} concurrent slots are occupied.",
                t.max_parallel
            ));
        }
        None
    }
    pub fn create_agent(
        &mut self,
        task: &str,
        connection: &str,
        assignment: &str,
    ) -> Result<String, String> {
        self.spawn_manual(task, connection, assignment, None)
    }
    fn spawn_manual(
        &mut self,
        task: &str,
        connection: &str,
        assignment: &str,
        replaces: Option<String>,
    ) -> Result<String, String> {
        if let Some(reason) = self.agent_creation_reason(task) {
            return Err(reason);
        }
        text(assignment, "Assignment", 8192)?;
        let t = self.tasks.iter().find(|t| t.id == task).unwrap();
        let run = t.run;
        let profile = t
            .profiles
            .iter()
            .find(|p| p.connection == connection)
            .cloned()
            .ok_or("This connection is outside the frozen run profile.")?;
        let source = replaces
            .as_ref()
            .map_or("Operator: Add agent".into(), |id| {
                format!("Operator: replacement for {id}")
            });
        let id = self.add_agent(task.into(), run, source, assignment.trim().into(), profile);
        let a = self.agents.last_mut().unwrap();
        a.role = Role::Manual;
        a.due = Some(self.now + 8000);
        a.replaces = replaces;
        self.changed();
        Ok(id)
    }
    pub fn stop_agent(&mut self, id: &str) -> Result<(), String> {
        let i = self
            .agents
            .iter()
            .position(|a| a.id == id)
            .ok_or("Agent no longer exists.")?;
        if self.agents[i].ended.is_some() {
            return Err("This agent has already ended.".into());
        }
        let task = self.agents[i].task.clone();
        let role = self.agents[i].role;
        if role == Role::Origin {
            let old = self.active;
            self.active = self.tasks.iter().position(|t| t.id == task);
            let result = self.stop().map_err(str::to_string);
            self.active = old;
            result?;
        } else {
            let a = &mut self.agents[i];
            a.state = Status::Interrupted;
            a.ended = Some(self.now);
            a.output = "Stopped by operator; partial records retained.".into();
            for t in self
                .tools
                .iter_mut()
                .filter(|t| t.agent == id && t.ended.is_none())
            {
                t.status = Status::Stopped;
                t.ended = Some(self.now);
                t.output.push_str("\nStopped with its agent.");
            }
            if role == Role::Clarifier
                && let Some(t) = self.tasks.iter_mut().find(|t| t.id == task)
            {
                t.stage = Stage::ProviderRequired;
            }
        }
        self.record(
            "operator",
            "Agent stopped",
            id,
            if role == Role::Origin {
                "Origin and its development run stopped"
            } else {
                "Individual assignment stopped"
            },
        );
        self.changed();
        Ok(())
    }
    pub fn agent_retry_reason(&self, id: &str) -> Option<String> {
        let Some(a) = self.agents.iter().find(|a| a.id == id) else {
            return Some("Agent no longer exists.".into());
        };
        if a.ended.is_none() {
            return Some("Stop the agent before replacing its attempt.".into());
        }
        let Some(t) = self.tasks.iter().find(|t| t.id == a.task) else {
            return Some("Task no longer exists.".into());
        };
        if t.archived || a.archived {
            return Some("Restore the task and agent first.".into());
        }
        if a.role == Role::Origin {
            if !matches!(
                t.stage,
                Stage::Stopped | Stage::Review | Stage::Delivered | Stage::Cancelled
            ) {
                return Some("The previous run is still active.".into());
            }
            if self.profiles().is_empty() {
                return Some("Enable an observed connection first.".into());
            }
            return None;
        }
        if a.role == Role::Clarifier {
            return if t.stage == Stage::ProviderRequired && !self.profiles().is_empty() {
                None
            } else {
                Some("Clarification is not waiting for a new agent.".into())
            };
        }
        if a.run != t.run {
            return Some(
                "This agent belongs to an older run. Add a new assignment to the current run."
                    .into(),
            );
        }
        self.agent_creation_reason(&a.task)
    }
    pub fn retry_agent(&mut self, id: &str) -> Result<String, String> {
        if let Some(reason) = self.agent_retry_reason(id) {
            return Err(reason);
        }
        let a = self.agents.iter().find(|a| a.id == id).unwrap().clone();
        if a.role == Role::Origin || a.role == Role::Clarifier {
            let old = self.active;
            self.active = self.tasks.iter().position(|t| t.id == a.task);
            let result = if a.role == Role::Origin {
                self.start(true)
            } else {
                self.continue_conversation()
            };
            self.active = old;
            result.map_err(str::to_string)?;
            let new = self.agents.last_mut().unwrap();
            new.replaces = Some(id.into());
            let new_id = new.id.clone();
            self.record("operator", "Agent replaced", new_id.clone(), id);
            self.changed();
            Ok(new_id)
        } else {
            self.spawn_manual(
                &a.task,
                &a.profile.connection,
                &a.assignment,
                Some(id.into()),
            )
        }
    }
    pub fn archive_agent(&mut self, id: &str, archived: bool) -> Result<(), String> {
        let a = self
            .agents
            .iter_mut()
            .find(|a| a.id == id)
            .ok_or("Agent no longer exists.")?;
        if archived && a.ended.is_none() {
            return Err("Stop the agent before archiving it.".into());
        }
        a.archived = archived;
        self.record(
            "operator",
            if archived {
                "Agent archived"
            } else {
                "Agent restored"
            },
            id,
            "Assignment, frozen profile, messages and output retained",
        );
        self.changed();
        Ok(())
    }
    pub fn stop_tool(&mut self, id: &str) -> Result<(), String> {
        let t = self
            .tools
            .iter_mut()
            .find(|t| t.id == id)
            .ok_or("Tool record no longer exists.")?;
        if t.ended.is_some() {
            return Err("This process has already ended.".into());
        }
        t.status = Status::Stopped;
        t.ended = Some(self.now);
        t.output
            .push_str("\nProcess stopped by operator. Its owning agent may continue.");
        self.record(
            "operator",
            "Tool stopped",
            id,
            "Process ended; output retained",
        );
        self.changed();
        Ok(())
    }
    pub(crate) fn advance_manual_agents(&mut self) {
        let ids: Vec<_> = self
            .agents
            .iter()
            .filter(|a| a.role == Role::Manual && a.ended.is_none())
            .map(|a| a.id.clone())
            .collect();
        for id in ids {
            let i = self.agents.iter().position(|a| a.id == id).unwrap();
            let a = self.agents[i].clone();
            if self.now >= a.created + 1000 && a.state == Status::Starting {
                self.agents[i].state = Status::Running;
                self.tool(
                    &a.task,
                    a.run,
                    &id,
                    "inspect assignment",
                    "Running",
                    "Simulated inspection of the operator's assigned scope.",
                );
                self.changed();
            }
            if a.due.is_some_and(|at| self.now >= at) {
                self.agents[i].state = Status::Completed;
                self.agents[i].ended = Some(self.now);
                self.agents[i].output = format!("Simulated assignment return: {}", a.assignment);
                for t in self
                    .tools
                    .iter_mut()
                    .filter(|t| t.agent == id && t.ended.is_none())
                {
                    t.status = Status::Completed;
                    t.ended = Some(self.now);
                    t.output = "Simulated assignment inspection completed; exit 0.".into();
                }
                self.post(&a.task,&id,&format!("Assignment returned: {}. This is a simulated observation, not independent verification.",a.assignment));
                self.record(
                    id.clone(),
                    "Assignment returned",
                    id,
                    "Simulated manual assignment completed",
                );
                self.changed();
            }
        }
    }
    pub(crate) fn capture_candidate(&mut self, task: &str, run: usize, number: usize) {
        let id = format!("{task}/R-{run:03}/C-{number:03}");
        if self.candidates.iter().any(|c| c.id == id) {
            return;
        }
        self.candidates.push(Candidate {
            id: id.clone(),
            task: task.into(),
            run,
            number,
            created: self.now,
            files: vec![
                "src/game.ts".into(),
                "src/rules.test.ts".into(),
                "README.md".into(),
            ],
        });
        self.record(
            format!("{task}/R-{run:03}"),
            "Candidate recorded",
            id,
            "Immutable simulated manifest",
        );
    }
    pub fn recover(&mut self, downtime: u64) {
        self.now = self.now.saturating_add(downtime);
        let active: Vec<_> = self
            .tasks
            .iter()
            .filter(|t| t.stage.busy())
            .map(|t| t.id.clone())
            .collect();
        for id in active {
            for a in self
                .agents
                .iter_mut()
                .filter(|a| a.task == id && a.ended.is_none())
            {
                a.state = Status::Interrupted;
                a.ended = Some(self.now);
                a.output="Recovered as stopped: no process is assumed alive after reopening the simulator.".into();
            }
            for t in self
                .tools
                .iter_mut()
                .filter(|t| t.task == id && t.ended.is_none())
            {
                t.status = Status::Stopped;
                t.ended = Some(self.now);
                t.output.push_str("\nClosed during recovery.");
            }
            if let Some(t) = self.tasks.iter_mut().find(|t| t.id == id) {
                t.stage = Stage::Stopped;
                t.run_ended = Some(self.now);
            }
            self.record(
                "recovery",
                "Run recovered as stopped",
                id,
                "An explicit retry is required",
            );
        }
        self.changed();
    }
    pub fn shutdown(&mut self) {
        let active: Vec<_> = self
            .tasks
            .iter()
            .filter(|t| t.stage.busy())
            .map(|t| t.id.clone())
            .collect();
        let old = self.active;
        for id in active {
            self.active = self.tasks.iter().position(|t| t.id == id);
            let _ = self.stop();
            self.record(
                "operator",
                "Run stopped on exit",
                id,
                "Checkpoint retains all history",
            );
        }
        self.active = old;
        self.changed();
    }
}

impl Simulation {
    pub fn set_limits(&mut self, max: usize, parallel: usize) -> Result<(), String> {
        if max == 0 || max > 10000 || parallel == 0 || parallel > max {
            return Err("Use 1..10000 agent starts and 1..max concurrent attempts.".into());
        }
        self.max_agents = max;
        self.max_parallel = parallel;
        self.record(
            "operator",
            "Future limits edited",
            "configuration",
            format!("{max} starts / {parallel} concurrent"),
        );
        self.changed();
        Ok(())
    }
    pub fn report(&self, kind: &str, id: &str) -> Result<(String, serde_json::Value), String> {
        macro_rules! report {
            ($list:expr,$task:ident) => {{
                let item = $list
                    .iter()
                    .find(|x| x.id == id)
                    .ok_or("Record no longer exists.")?;
                Ok((
                    item.$task.clone(),
                    serde_json::to_value(item).map_err(|e| e.to_string())?,
                ))
            }};
        }
        match kind {
            "task" => {
                let t = self
                    .tasks
                    .iter()
                    .find(|t| t.id == id)
                    .ok_or("Task no longer exists.")?;
                Ok((
                    t.id.clone(),
                    serde_json::json!({"task":t,"runs":self.runs.iter().filter(|r|r.task==id).collect::<Vec<_>>(),"agents":self.agents.iter().filter(|a|a.task==id).collect::<Vec<_>>(),"messages":self.posts.iter().filter(|m|m.task==id).collect::<Vec<_>>(),"checks":self.checks.iter().filter(|c|c.task==id).collect::<Vec<_>>() }),
                ))
            }
            "agent" => report!(self.agents, task),
            "message" => report!(self.posts, task),
            "tool" => report!(self.tools, task),
            "check" => report!(self.checks, task),
            "candidate" => report!(self.candidates, task),
            _ => Err("This record cannot be exported.".into()),
        }
    }
    pub fn validate(&self) -> anyhow::Result<()> {
        use anyhow::{bail, ensure};
        use std::collections::BTreeSet;
        ensure!(
            self.active.is_none_or(|i| i < self.tasks.len()),
            "Selected task is missing"
        );
        ensure!(
            self.max_agents > 0
                && self.max_agents <= 10000
                && self.max_parallel > 0
                && self.max_parallel <= self.max_agents,
            "Invalid capacity"
        );
        macro_rules! unique {
            ($items:expr) => {{
                let mut ids = BTreeSet::new();
                for x in $items {
                    ensure!(
                        !x.id.is_empty() && ids.insert(x.id.as_str()),
                        "Duplicate or empty identifier"
                    );
                }
            }};
        }
        unique!(&self.providers);
        unique!(&self.tasks);
        unique!(&self.agents);
        unique!(&self.posts);
        unique!(&self.tools);
        unique!(&self.checks);
        unique!(&self.candidates);
        unique!(&self.exports);
        let mut names = BTreeSet::new();
        for p in &self.providers {
            ensure!(
                !p.name.trim().is_empty()
                    && p.name.chars().count() <= 64
                    && names.insert(p.name.to_lowercase()),
                "Invalid or duplicate connection name"
            );
            ensure!(
                p.created <= self.now && p.checked.is_none_or(|at| at <= self.now),
                "Invalid provider timestamp"
            );
            ensure!(
                p.id.strip_prefix("P-")
                    .and_then(|id| id.parse::<usize>().ok())
                    .is_some_and(|n| n < self.next_provider),
                "Connection counter would reuse an identifier"
            );
        }
        for t in &self.tasks {
            ensure!(
                !t.goal.trim().is_empty() && t.created <= self.now,
                "Invalid task"
            );
            ensure!(
                t.id.strip_prefix("T-")
                    .and_then(|id| id.parse::<usize>().ok())
                    .is_some_and(|n| n < self.next_task),
                "Task counter would reuse an identifier"
            );
            ensure!(
                t.max_agents > 0 && t.max_parallel > 0 && t.max_parallel <= t.max_agents,
                "Invalid frozen capacity"
            );
        }
        for a in &self.agents {
            ensure!(
                self.tasks.iter().any(|t| t.id == a.task),
                "Agent refers to a missing task"
            );
            ensure!(
                !a.assignment.trim().is_empty() && !a.source.trim().is_empty(),
                "Agent has no assignment or creation source"
            );
            ensure!(
                a.created <= self.now && a.ended.is_none_or(|at| at >= a.created && at <= self.now),
                "Invalid agent timestamp"
            );
            ensure!(
                !a.archived || a.ended.is_some(),
                "An active agent cannot be archived"
            );
            let family = Family::ALL
                .into_iter()
                .find(|f| f.key() == a.profile.provider)
                .ok_or_else(|| anyhow::anyhow!("Unknown frozen provider family"))?;
            ensure!(
                family.engine() == a.profile.engine
                    && family.model() == a.profile.model
                    && a.profile.effort == "low"
                    && !a.profile.connection.is_empty(),
                "Invalid frozen launch profile"
            );
            ensure!(
                a.run == 0
                    || self
                        .runs
                        .iter()
                        .any(|r| r.task == a.task && r.number == a.run),
                "Agent refers to a missing run"
            );
            ensure!(
                a.replaces.as_ref().is_none_or(|id| self
                    .agents
                    .iter()
                    .any(|old| &old.id == id && old.task == a.task && old.id != a.id)),
                "Invalid agent replacement"
            );
        }
        let mut runs = BTreeSet::new();
        for r in &self.runs {
            ensure!(
                runs.insert((&r.task, r.number)) && self.tasks.iter().any(|t| t.id == r.task),
                "Invalid or duplicate run"
            );
            ensure!(
                self.agents.iter().any(|a| a.id == r.origin
                    && a.task == r.task
                    && a.run == r.number
                    && a.role == Role::Origin),
                "Run origin is missing"
            );
            ensure!(
                r.max_agents > 0 && r.max_parallel > 0 && r.max_parallel <= r.max_agents,
                "Invalid run capacity"
            );
            ensure!(
                self.agents
                    .iter()
                    .filter(|a| a.task == r.task && a.run == r.number)
                    .count()
                    <= r.max_agents,
                "Agent-start ceiling exceeded"
            );
        }
        for p in &self.posts {
            ensure!(
                self.agents
                    .iter()
                    .any(|a| a.id == p.author && a.task == p.task)
                    && p.at <= self.now,
                "Message origin is missing"
            );
        }
        for t in &self.tools {
            ensure!(
                self.agents
                    .iter()
                    .any(|a| a.id == t.agent && a.task == t.task && a.run == t.run)
                    && t.started <= self.now,
                "Tool ownership is invalid"
            );
        }
        for c in &self.candidates {
            ensure!(
                self.runs
                    .iter()
                    .any(|r| r.task == c.task && r.number == c.run),
                "Candidate origin is missing"
            );
        }
        for c in &self.checks {
            ensure!(
                self.candidates
                    .iter()
                    .any(|v| v.task == c.task && v.run == c.run && v.number == c.candidate),
                "Check refers to a missing candidate"
            );
        }
        for (i, a) in self.audit.iter().enumerate() {
            ensure!(
                a.sequence == i + 1 && a.at <= self.now && !a.actor.is_empty(),
                "Audit sequence is invalid"
            );
        }
        for e in &self.exports {
            ensure!(
                self.tasks.iter().any(|t| t.id == e.task) && e.digest.len() == 64,
                "Export provenance is invalid"
            );
        }
        fn strings(value: &serde_json::Value) -> bool {
            match value {
                serde_json::Value::String(s) => {
                    !s.chars().any(|c| c.is_control() && c != '\n' && c != '\t')
                }
                serde_json::Value::Array(v) => v.iter().all(strings),
                serde_json::Value::Object(o) => o.values().all(strings),
                _ => true,
            }
        }
        if !strings(&serde_json::to_value(self)?) {
            bail!("Control characters are not allowed in checkpoint text");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    pub fn ready() -> Simulation {
        let mut s = Simulation::default();
        s.create_task("Морской бой", "").unwrap();
        s.add_provider("Work", Family::OpenAi, true).unwrap();
        s.continue_conversation().unwrap();
        s.send_message("Подтверждаю правила").unwrap();
        s
    }
    #[test]
    fn connection_crud_preserves_identity_and_frozen_execution() {
        let mut s = Simulation::default();
        assert!(s.providers.is_empty());
        assert!(s.add_provider("  ", Family::OpenAi, false).is_err());
        assert!(s.providers.is_empty());
        let id = s.add_provider("Work", Family::OpenAi, false).unwrap();
        assert!(s.agents.is_empty());
        assert!(s.add_provider("work", Family::Anthropic, true).is_err());
        s.set_provider_enabled(&id, true).unwrap();
        assert!(s.agents.is_empty());
        s.create_task("Game", "").unwrap();
        s.continue_conversation().unwrap();
        s.send_message("Yes").unwrap();
        s.start(false).unwrap();
        let frozen = s.agents.last().unwrap().profile.clone();
        s.edit_provider(&id, "Renamed", Family::Anthropic).unwrap();
        assert_eq!(s.agents.last().unwrap().profile, frozen);
        assert!(s.profiles().is_empty());
        assert!(s.remove_provider(&id).is_err());
        s.refresh_provider(&id).unwrap();
        s.stop().unwrap();
        s.remove_provider(&id).unwrap();
        assert!(s.providers.is_empty());
        assert_eq!(s.agents.last().unwrap().profile, frozen);
        s.validate().unwrap();
        let next = s.add_provider("Work", Family::OpenAi, true).unwrap();
        assert_ne!(next, id);
        s.validate().unwrap();
    }
    #[test]
    fn manual_agents_require_scope_and_keep_stop_replacement_history() {
        let mut s = ready();
        let task = s.task().unwrap().id.clone();
        let p = s.providers[0].id.clone();
        assert!(s.create_agent(&task, &p, "Review placement").is_err());
        s.set_limits(3, 2).unwrap();
        s.start(false).unwrap();
        assert!(s.create_agent(&task, &p, "").is_err());
        assert!(s.create_agent(&task, "P-missing", "Review").is_err());
        let a = s.create_agent(&task, &p, "Review placement").unwrap();
        assert!(
            s.agents
                .iter()
                .find(|x| x.id == a)
                .unwrap()
                .source
                .contains("Operator")
        );
        assert!(s.archive_agent(&a, true).is_err());
        assert!(s.create_agent(&task, &p, "Extra").is_err());
        s.advance_to(1000);
        s.stop_agent(&a).unwrap();
        assert!(
            s.tools
                .iter()
                .filter(|t| t.agent == a)
                .all(|t| t.ended.is_some())
        );
        let replacement = s.retry_agent(&a).unwrap();
        assert_ne!(replacement, a);
        assert_eq!(
            s.agents.last().unwrap().replaces.as_deref(),
            Some(a.as_str())
        );
        s.archive_agent(&a, true).unwrap();
        assert!(
            s.create_agent(&task, &p, "Budget cannot be reclaimed by archive")
                .is_err()
        );
        s.archive_agent(&a, false).unwrap();
        s.stop().unwrap();
        s.validate().unwrap();
    }
    #[test]
    fn draft_deletion_and_task_archival_do_not_erase_execution_evidence() {
        let mut s = Simulation::default();
        let first = s.create_task("Draft", "").unwrap();
        s.edit_task(&first, "Changed", "Notes").unwrap();
        assert!(s.agents.is_empty());
        s.delete_task(&first).unwrap();
        let next = s.create_task("Game", "").unwrap();
        assert_ne!(first, next);
        s.add_provider("Work", Family::OpenAi, true).unwrap();
        s.continue_conversation().unwrap();
        assert!(s.delete_task(&next).is_err());
        assert!(s.archive_task(&next, true).is_err());
        s.send_message("Yes").unwrap();
        s.start(false).unwrap();
        assert!(s.edit_task(&next, "Rewrite history", "").is_err());
        s.cancel_task(&next).unwrap();
        let count = s.agents.len();
        s.archive_task(&next, true).unwrap();
        assert_eq!(s.agents.len(), count);
        assert!(s.send_message("Cannot edit archived work").is_err());
        s.archive_task(&next, false).unwrap();
        assert_eq!(s.task().unwrap().stage, Stage::Cancelled);
        s.validate().unwrap();
    }
    #[test]
    fn archive_is_visibility_not_reclaimed_capacity() {
        let mut s = ready();
        s.start(false).unwrap();
        for n in 1..=7 {
            s.advance_to(n * 3000);
        }
        let task = s.task().unwrap().id.clone();
        let id = s.agents.last().unwrap().id.clone();
        s.archive_agent(&id, true).unwrap();
        let world = s.project();
        let mut table = crate::resources::ResourceView::new(world.agents, "");
        assert!(
            !table
                .matches
                .iter()
                .any(|&i| table.data.records[i].id == id)
        );
        table.show_archived = true;
        table.rebuild();
        assert!(
            table
                .matches
                .iter()
                .any(|&i| table.data.records[i].id == id)
        );
        let checks = s.checks.len();
        let posts = s.posts.len();
        s.archive_task(&task, true).unwrap();
        s.archive_task(&task, false).unwrap();
        assert_eq!(s.checks.len(), checks);
        assert_eq!(s.posts.len(), posts);
        s.validate().unwrap();
    }
}
