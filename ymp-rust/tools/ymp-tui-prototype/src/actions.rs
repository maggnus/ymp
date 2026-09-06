//! Contextual actions and editors. There is one mutation path per operation, shared by mouse and keys.
use crate::{
    app::{App, Effect, Layer},
    entities::{Family, Role, Stage},
    forms::{Action, Confirmation, Editor, FormKind, MenuItem},
    resources::{Kind, Record},
};
impl App {
    pub fn selected_record(&self) -> Option<&Record> {
        let table = self.table.as_ref()?;
        if self.layers.contains(&Layer::Actions)
            && let Some((kind, id)) = &self.menu_target
            && table.data.kind == *kind
        {
            return table.data.records.iter().find(|r| &r.id == id);
        }
        if let Some(Layer::Detail(id)) = self
            .layers
            .iter()
            .rev()
            .find(|l| matches!(l, Layer::Detail(_)))
        {
            table.data.records.iter().find(|r| &r.id == id)
        } else {
            table.current()
        }
    }
    pub fn action_items(&self) -> Vec<MenuItem> {
        let Some(s) = &self.simulation else {
            return vec![];
        };
        let kind = self.table.as_ref().map(|t| t.data.kind);
        let selected = self.selected_record();
        let mut items = Vec::new();
        match kind {
            Some(Kind::Providers) => {
                items.push(MenuItem::new("Add provider", Action::AddProvider, None));
                if let Some(p) = selected.and_then(|r| s.providers.iter().find(|p| p.id == r.id)) {
                    items.push(MenuItem::new(
                        "Edit",
                        Action::EditProvider(p.id.clone()),
                        None,
                    ));
                    items.push(MenuItem::new(
                        if p.enabled { "Disable" } else { "Enable" },
                        Action::EnableProvider(p.id.clone(), !p.enabled),
                        None,
                    ));
                    items.push(MenuItem::new(
                        "Refresh profile",
                        Action::RefreshProvider(p.id.clone()),
                        (!p.enabled).then(|| "Enable the connection first.".into()),
                    ));
                    items.push(MenuItem::new(
                        "Remove connection",
                        Action::RemoveProvider(p.id.clone()),
                        s.provider_removal_reason(&p.id),
                    ));
                }
                items.push(MenuItem::new("Run limits", Action::Limits, None));
            }
            Some(Kind::Tasks) | None => {
                items.push(MenuItem::new("New task", Action::AddTask, None));
                let task = if kind.is_none() {
                    s.task()
                } else {
                    selected.and_then(|r| s.tasks.iter().find(|t| t.id == r.id))
                };
                if let Some(t) = task {
                    items.push(MenuItem::new(
                        "Conversation",
                        Action::SelectTask(t.id.clone()),
                        None,
                    ));
                    items.push(MenuItem::new(
                        "Edit proposal",
                        Action::EditTask(t.id.clone()),
                        s.task_edit_reason(&t.id),
                    ));
                    items.push(MenuItem::new(
                        "Start development",
                        Action::StartTask(t.id.clone()),
                        (t.archived || t.stage != Stage::Ready)
                            .then(|| "Review the task proposal first.".into()),
                    ));
                    items.push(MenuItem::new(
                        "Stop development",
                        Action::StopTask(t.id.clone()),
                        (!t.stage.busy()).then(|| "No development run is active.".into()),
                    ));
                    items.push(MenuItem::new(
                        "Retry development",
                        Action::RetryTask(t.id.clone()),
                        (t.archived
                            || !matches!(
                                t.stage,
                                Stage::Stopped
                                    | Stage::Review
                                    | Stage::Delivered
                                    | Stage::Cancelled
                            ))
                        .then(|| "Finish or stop the current run first.".into()),
                    ));
                    items.push(MenuItem::new(
                        "Cancel task",
                        Action::CancelTask(t.id.clone()),
                        (t.archived || matches!(t.stage, Stage::Cancelled | Stage::Delivered))
                            .then(|| "This task is already closed.".into()),
                    ));
                    items.push(MenuItem::new(
                        if t.archived {
                            "Restore task"
                        } else {
                            "Archive task"
                        },
                        Action::ArchiveTask(t.id.clone(), !t.archived),
                        if t.archived {
                            None
                        } else {
                            s.task_archive_reason(&t.id)
                        },
                    ));
                    items.push(MenuItem::new(
                        "Delete unused draft",
                        Action::DeleteTask(t.id.clone()),
                        s.task_delete_reason(&t.id),
                    ));
                    items.push(MenuItem::new(
                        "Export task record",
                        Action::Export {
                            kind: "task".into(),
                            id: t.id.clone(),
                        },
                        None,
                    ));
                }
            }
            Some(Kind::Agents) => {
                items.push(MenuItem::new(
                    "Add agent",
                    Action::AddAgent,
                    s.task().map_or(Some("Select a task first.".into()), |t| {
                        s.agent_creation_reason(&t.id)
                    }),
                ));
                if let Some(a) = selected.and_then(|r| s.agents.iter().find(|a| a.id == r.id)) {
                    items.push(MenuItem::new(
                        "Stop agent",
                        Action::StopAgent(a.id.clone()),
                        a.ended
                            .is_some()
                            .then(|| "This agent has already ended.".into()),
                    ));
                    items.push(MenuItem::new(
                        "Replacement attempt",
                        Action::RetryAgent(a.id.clone()),
                        s.agent_retry_reason(&a.id),
                    ));
                    items.push(MenuItem::new(
                        if a.archived {
                            "Restore agent"
                        } else {
                            "Archive agent"
                        },
                        Action::ArchiveAgent(a.id.clone(), !a.archived),
                        (a.ended.is_none() && !a.archived)
                            .then(|| "Stop this agent before archiving it.".into()),
                    ));
                    items.push(MenuItem::new(
                        "Export agent record",
                        Action::Export {
                            kind: "agent".into(),
                            id: a.id.clone(),
                        },
                        None,
                    ));
                }
            }
            Some(Kind::Tools) => {
                if let Some(t) = selected.and_then(|r| s.tools.iter().find(|t| t.id == r.id)) {
                    items.push(MenuItem::new(
                        "Stop process",
                        Action::StopTool(t.id.clone()),
                        t.ended
                            .is_some()
                            .then(|| "This process has already ended.".into()),
                    ));
                    items.push(MenuItem::new(
                        "Export tool output",
                        Action::Export {
                            kind: "tool".into(),
                            id: t.id.clone(),
                        },
                        None,
                    ));
                }
            }
            Some(Kind::Board) => {
                if let Some(r) = selected {
                    items.push(MenuItem::new(
                        "Export message",
                        Action::Export {
                            kind: "message".into(),
                            id: r.id.clone(),
                        },
                        None,
                    ));
                }
            }
            Some(Kind::Checks) => {
                if let Some(r) = selected {
                    items.push(MenuItem::new(
                        "Export check evidence",
                        Action::Export {
                            kind: "check".into(),
                            id: r.id.clone(),
                        },
                        None,
                    ));
                }
            }
            Some(Kind::Files) => {
                if let Some(r) = selected {
                    if let Some(e) = s.exports.iter().find(|e| e.id == r.id) {
                        items.push(MenuItem::new(
                            "Export another copy",
                            Action::Export {
                                kind: e.kind.clone(),
                                id: e.entity.clone(),
                            },
                            None,
                        ));
                        items.push(MenuItem::new(
                            "Remove exported report",
                            Action::RemoveExport(e.id.clone()),
                            e.removed
                                .then(|| "This export has already been removed.".into()),
                        ));
                    } else if let Some(c) = s
                        .candidates
                        .iter()
                        .find(|c| r.id.starts_with(&format!("{}/", c.id)))
                    {
                        items.push(MenuItem::new(
                            "Export candidate manifest",
                            Action::Export {
                                kind: "candidate".into(),
                                id: c.id.clone(),
                            },
                            None,
                        ));
                    }
                }
            }
            _ => {}
        }
        if let Some(r) = selected {
            for link in &r.links {
                items.push(MenuItem::new(
                    link.label,
                    Action::Follow {
                        kind: link.kind,
                        query: link.query.clone(),
                    },
                    None,
                ));
            }
        }
        if let Some(t) = &self.table
            && matches!(t.data.kind, Kind::Tasks | Kind::Agents | Kind::Files)
        {
            items.push(MenuItem::new(
                if t.show_archived {
                    "Hide archived"
                } else {
                    "Include archived"
                },
                Action::ShowArchived,
                None,
            ));
        }
        items
    }
    pub fn open_actions(&mut self) {
        self.menu_target = self
            .table
            .as_ref()
            .and_then(|t| self.selected_record().map(|r| (t.data.kind, r.id.clone())));
        self.menu = self.action_items();
        self.menu_selection = 0;
        if self.layers.last() != Some(&Layer::Actions) {
            self.layers.push(Layer::Actions);
        }
    }
    pub fn add_in_current_view(&mut self) {
        let action = match self.table.as_ref().map(|t| t.data.kind) {
            Some(Kind::Providers) => Action::AddProvider,
            Some(Kind::Agents) => Action::AddAgent,
            _ => Action::AddTask,
        };
        self.perform(action, false);
    }
    pub fn refresh_menu(&mut self) {
        let fresh = self.action_items();
        for item in &mut self.menu {
            item.reason = fresh
                .iter()
                .find(|f| f.action == item.action)
                .map_or(Some("This action no longer applies.".into()), |f| {
                    f.reason.clone()
                });
        }
    }
    pub fn confirm_action(&mut self, action: Action) -> bool {
        let Some(s) = &self.simulation else {
            return false;
        };
        let description=match &action{
            Action::RemoveProvider(id)=>s.providers.iter().find(|p|&p.id==id).map(|p|("Remove connection",format!("{} ({id})\nConfiguration is removed. Historical launch profiles remain.",p.name),"Remove")),
            Action::DeleteTask(id)=>s.tasks.iter().find(|t|&t.id==id).map(|t|("Delete draft",format!("{id}\n{}\nThis unused request and its draft messages will be removed.",t.goal),"Delete")),
            Action::StopAgent(id)=>s.agents.iter().find(|a|&a.id==id).map(|a|("Stop agent",format!("{id} · {} / R-{:03}\n{}\n\nAssignment: {}",a.task,a.run,if a.role==Role::Origin{"The origin, its run and other active assignments stop. History remains."}else{"This agent and its active tools stop. History remains."},a.assignment.chars().take(180).collect::<String>()),"Stop")),
            Action::StopTask(id)=>Some(("Stop development",format!("{id}\nAll active assignments and owned processes in this run stop. History remains."),"Stop")),
            Action::CancelTask(id)=>Some(("Cancel task",format!("{id}\nActive work stops and the task is marked cancelled. Records remain."),"Cancel task")),
            Action::StopTool(id)=>s.tools.iter().find(|t|&t.id==id).map(|t|("Stop process",format!("{id}\n{}\nOutput remains; the owning agent may continue.",t.command),"Stop")),
            Action::RemoveExport(id)=>s.exports.iter().find(|e|&e.id==id).map(|e|("Remove exported report",format!("{}\nOnly this owned report is removed. Source records and project files remain.",e.relative_path),"Remove")),
            Action::Quit if s.tasks.iter().any(|t|t.stage.busy())||self.editor.is_some()=>Some(("Exit ymp","Active simulations will stop and saved records remain. Unsaved form edits are discarded.".into(),"Exit")),_=>None,
        };
        if let Some((title, detail, accept)) = description {
            self.confirmation = Some(Confirmation {
                title: title.into(),
                detail,
                accept: accept.into(),
                action,
                selected: false,
            });
            self.layers.push(Layer::Confirm);
            true
        } else {
            false
        }
    }
    pub fn perform(&mut self, action: Action, confirmed: bool) {
        if action == Action::BeginScenario {
            self.execute_command("/scenario");
            return;
        }
        if action == Action::Answer {
            let answer = if self.draft.trim().is_empty() {
                "Use the proposed rules.".into()
            } else {
                self.draft.clone()
            };
            if let Some(s) = &mut self.simulation
                && s.task().is_some_and(|t| t.stage == Stage::Clarifying)
            {
                match s.send_message(&answer) {
                    Ok(()) => {
                        self.draft.clear();
                        self.cursor = 0;
                        self.sync();
                    }
                    Err(e) => self.notice(&e),
                }
            }
            return;
        }
        if action == Action::EditAnswer {
            if let Some(t) = self
                .simulation
                .as_ref()
                .and_then(|s| s.task())
                .filter(|t| t.stage == Stage::Clarifying)
            {
                self.editor = Some(Editor::answer(t.id.clone(), &self.draft));
                self.layers.push(Layer::Editor);
            }
            return;
        }
        if action == Action::Menu {
            self.open_actions();
            return;
        }
        if action == Action::ClearFilter {
            if let Some(t) = &mut self.table {
                t.query.clear();
                t.rebuild();
            } else {
                self.chat_filter.clear();
            }
            return;
        }
        if let Action::ToggleTool(id) = action {
            if !self.expanded_tools.remove(&id) {
                self.expanded_tools.insert(id);
            }
            return;
        }
        if !confirmed && self.confirm_action(action.clone()) {
            return;
        }
        if let Action::Follow { kind, query } = action {
            self.follow(kind, query);
            return;
        }
        if action == Action::ShowArchived {
            if let Some(t) = &mut self.table {
                t.show_archived = !t.show_archived;
                if !t.show_archived && t.data.filters[t.filter] == "Archived" {
                    t.filter = 0;
                }
                t.rebuild();
            }
            self.layers.clear();
            return;
        }
        let Some(s) = &mut self.simulation else {
            self.notice("The table stress fixture is read-only.");
            return;
        };
        let mut form = None;
        let mut focus = None;
        let result: Result<(), String> = match action {
            Action::AddProvider => {
                form = Some(Editor::provider(None, "", Family::OpenAi));
                Ok(())
            }
            Action::EditProvider(id) => {
                if let Some(p) = s.providers.iter().find(|p| p.id == id) {
                    form = Some(Editor::provider(Some(id), &p.name, p.family));
                    Ok(())
                } else {
                    Err("Connection no longer exists.".into())
                }
            }
            Action::EnableProvider(id, value) => s.set_provider_enabled(&id, value),
            Action::RefreshProvider(id) => s.refresh_provider(&id),
            Action::RemoveProvider(id) => s.remove_provider(&id),
            Action::AddTask => {
                form = Some(Editor::task(None, "", ""));
                Ok(())
            }
            Action::EditTask(id) => {
                if let Some(reason) = s.task_edit_reason(&id) {
                    Err(reason)
                } else {
                    let t = s.tasks.iter().find(|t| t.id == id).unwrap();
                    form = Some(Editor::task(Some(id), &t.goal, &t.requirements));
                    Ok(())
                }
            }
            Action::SelectTask(id) => {
                s.select_task(&id);
                focus = Some((Kind::Tasks, String::new()));
                Ok(())
            }
            Action::StartTask(id) => {
                s.select_task(&id);
                s.start(false).map_err(str::to_string)
            }
            Action::RetryTask(id) => {
                s.select_task(&id);
                s.start(true).map_err(str::to_string)
            }
            Action::StopTask(id) => {
                s.select_task(&id);
                s.stop().map_err(str::to_string)
            }
            Action::CancelTask(id) => s.cancel_task(&id),
            Action::DeleteTask(id) => s.delete_task(&id),
            Action::ArchiveTask(id, value) => s.archive_task(&id, value),
            Action::AddAgent => {
                if let Some(t) = s.task() {
                    if let Some(reason) = s.agent_creation_reason(&t.id) {
                        Err(reason)
                    } else {
                        form = Some(Editor::agent(
                            t.id.clone(),
                            format!("{} · R-{:03} · frozen model access", t.id, t.run),
                            t.profiles
                                .iter()
                                .map(|p| {
                                    (
                                        p.connection.clone(),
                                        format!("{} · {}", p.connection_name, p.model),
                                    )
                                })
                                .collect(),
                        ));
                        Ok(())
                    }
                } else {
                    Err("Select a task first.".into())
                }
            }
            Action::StopAgent(id) => s.stop_agent(&id),
            Action::RetryAgent(id) => s.retry_agent(&id).map(|new| {
                focus = Some((Kind::Agents, new));
            }),
            Action::ArchiveAgent(id, value) => s.archive_agent(&id, value),
            Action::StopTool(id) => s.stop_tool(&id),
            Action::Export { kind, id } => {
                self.effects.push(Effect::Export { kind, id });
                Ok(())
            }
            Action::RemoveExport(id) => {
                self.effects.push(Effect::RemoveExport(id));
                Ok(())
            }
            Action::Limits => {
                form = Some(Editor::limits(s.max_agents, s.max_parallel));
                Ok(())
            }
            Action::Quit => {
                s.shutdown();
                self.quit = true;
                Ok(())
            }
            _ => Ok(()),
        };
        match result {
            Err(error) => {
                self.notice(&error);
                self.layers
                    .retain(|l| !matches!(l, Layer::Actions | Layer::Confirm));
            }
            Ok(()) => {
                self.layers
                    .retain(|l| !matches!(l, Layer::Actions | Layer::Confirm));
                self.confirmation = None;
                if let Some(editor) = form {
                    self.editor = Some(editor);
                    self.layers.push(Layer::Editor);
                    return;
                }
                self.sync();
                if let Some((kind, id)) = focus {
                    if id.is_empty() {
                        self.chat();
                    } else {
                        self.open_table(kind, &format!("id={id}"));
                        self.layers.push(Layer::Detail(id));
                    }
                } else {
                    self.layers.clear();
                }
            }
        }
    }
    pub fn save_editor(&mut self) {
        let Some(editor) = self.editor.clone() else {
            return;
        };
        let Some(s) = &mut self.simulation else {
            return;
        };
        let mut focus = None;
        let result: Result<(), String> = match &editor.kind {
            FormKind::Provider { id } => {
                let family = if editor.get(1) == "anthropic" {
                    Family::Anthropic
                } else {
                    Family::OpenAi
                };
                if let Some(id) = id {
                    s.edit_provider(id, editor.get(0), family)
                } else {
                    s.add_provider(editor.get(0), family, editor.get(2) == "yes")
                        .map(|id| focus = Some((Kind::Providers, id)))
                }
            }
            FormKind::Task { id } => {
                if let Some(id) = id {
                    s.edit_task(id, editor.get(0), editor.get(1))
                } else {
                    s.create_task(editor.get(0), editor.get(1))
                        .map(|_| focus = Some((Kind::Tasks, String::new())))
                }
            }
            FormKind::Agent { task } => s
                .create_agent(task, editor.get(1), editor.get(0))
                .map(|id| focus = Some((Kind::Agents, id))),
            FormKind::Answer { task } => {
                if s.task()
                    .is_some_and(|t| &t.id == task && t.stage == Stage::Clarifying)
                {
                    s.send_message(editor.get(0))
                } else {
                    Err("This task is no longer waiting for an answer.".into())
                }
            }
            FormKind::Limits => match (editor.get(0).parse(), editor.get(1).parse()) {
                (Ok(max), Ok(parallel)) => s.set_limits(max, parallel),
                _ => Err("Limits must be positive integers.".into()),
            },
        };
        if let Err(error) = result {
            if let Some(e) = &mut self.editor {
                e.error = Some(error);
            }
            return;
        }
        if matches!(editor.kind, FormKind::Answer { .. }) {
            self.draft.clear();
            self.cursor = 0;
        }
        self.editor = None;
        self.layers.clear();
        self.sync();
        if let Some((kind, id)) = focus {
            if id.is_empty() {
                self.chat();
            } else {
                self.open_table(kind, &format!("id={id}"));
                self.layers.push(Layer::Detail(id));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn an_open_menu_keeps_its_entity_when_a_live_filter_changes() {
        let mut app = App::scenario_scene("working").unwrap();
        app.open_table(Kind::Agents, "state=Running");
        app.table.as_mut().unwrap().navigate(1);
        let id = app.table.as_ref().unwrap().current().unwrap().id.clone();
        app.open_actions();
        app.advance(9000);
        assert_eq!(app.selected_record().unwrap().id, id);
        let index = app
            .menu
            .iter()
            .position(|m| m.action == Action::ArchiveAgent(id.clone(), true))
            .unwrap();
        assert!(app.menu[index].reason.is_none());
        app.menu_selection = index;
        app.choose_action();
        assert!(
            app.simulation
                .as_ref()
                .unwrap()
                .agents
                .iter()
                .find(|a| a.id == id)
                .unwrap()
                .archived
        );
    }
}
