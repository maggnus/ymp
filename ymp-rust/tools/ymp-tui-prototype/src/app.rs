use crate::forms::{Action, Confirmation, Editor, MenuItem};
use crate::{
    resources::{Kind, ResourceView, World},
    simulation::{Simulation, Stage},
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Rect;
use unicode_segmentation::UnicodeSegmentation;

#[derive(Clone, PartialEq, Eq)]
pub enum Layer {
    Commands,
    Detail(String),
    Actions,
    Editor,
    Confirm,
    Help,
    Services,
}
#[derive(Clone, Copy)]
pub enum Speaker {
    You,
    Summary,
}
pub struct Message {
    pub speaker: Speaker,
    pub text: String,
    pub author: Option<String>,
    pub tool: Option<String>,
}
pub struct Command {
    pub name: &'static str,
    pub description: &'static str,
}
pub const COMMANDS: [Command; 19] = [
    Command {
        name: "/chat",
        description: "Conversation and current task",
    },
    Command {
        name: "/tasks",
        description: "Tasks, specification and results",
    },
    Command {
        name: "/agents",
        description: "Agents, assignments and frozen settings",
    },
    Command {
        name: "/board",
        description: "Attributed agent messages",
    },
    Command {
        name: "/providers",
        description: "Model access and configuration",
    },
    Command {
        name: "/tools",
        description: "External commands and process output",
    },
    Command {
        name: "/checks",
        description: "Candidate-specific check evidence",
    },
    Command {
        name: "/files",
        description: "Candidate files and delivery",
    },
    Command {
        name: "/continue",
        description: "Continue task clarification",
    },
    Command {
        name: "/start",
        description: "Authorize development",
    },
    Command {
        name: "/stop",
        description: "Stop development and its tools",
    },
    Command {
        name: "/retry",
        description: "Start a new run with retained history",
    },
    Command {
        name: "/accept",
        description: "Accept the checked demo result",
    },
    Command {
        name: "/new",
        description: "New task",
    },
    Command {
        name: "/scenario",
        description: "Begin the Battleship scenario",
    },
    Command {
        name: "/limits",
        description: "Future run ceilings: /limits 6 3",
    },
    Command {
        name: "/actions",
        description: "Actions for the selected entity",
    },
    Command {
        name: "/add",
        description: "Create an entity in this view",
    },
    Command {
        name: "/help",
        description: "Keyboard help",
    },
];
pub enum Effect {
    Export { kind: String, id: String },
    RemoveExport(String),
}
struct Back {
    task: Option<String>,
    table: Option<ResourceView>,
    layers: Vec<Layer>,
    scroll: u16,
}
pub struct App {
    pub world: World,
    pub menu: Vec<MenuItem>,
    pub menu_selection: usize,
    pub menu_target: Option<(Kind, String)>,
    pub editor: Option<Editor>,
    pub confirmation: Option<Confirmation>,
    pub effects: Vec<Effect>,
    pub editor_hits: Vec<(Rect, usize)>,
    pub menu_hits: Vec<(Rect, usize)>,
    pub buttons: Vec<(Rect, Action)>,
    pub persistence_error: Option<String>,
    pub expanded_tools: std::collections::BTreeSet<String>,
    pub visible_tools: Vec<String>,
    pub tool_focus: Option<String>,
    pub simulation: Option<Simulation>,
    pub layers: Vec<Layer>,
    pub table: Option<ResourceView>,
    pub cached_tables: Vec<ResourceView>,
    history: Vec<Back>,
    pub messages: Vec<Message>,
    pub draft: String,
    pub drafts: std::collections::BTreeMap<String, (String, usize)>,
    draft_owner: String,
    pub cursor: usize,
    pub chat_filter: String,
    pub filtering_chat: bool,
    pub command_query: String,
    pub command_selection: usize,
    pub detail_scroll: u16,
    pub sidebar: bool,
    pub paused: bool,
    pub scroll: u16,
    pub tick: usize,
    pub quit: bool,
    pub toast: Option<(String, usize)>,
    pub width: u16,
    pub action_hits: Vec<(Rect, String)>,
    seen_revision: u64,
}
impl App {
    pub fn new() -> Self {
        let sim = Simulation::default();
        let world = sim.project();
        Self {
            world,
            menu: vec![],
            menu_selection: 0,
            menu_target: None,
            editor: None,
            confirmation: None,
            effects: vec![],
            editor_hits: vec![],
            menu_hits: vec![],
            buttons: vec![],
            persistence_error: None,
            expanded_tools: Default::default(),
            visible_tools: vec![],
            tool_focus: None,
            simulation: Some(sim),
            layers: vec![],
            table: None,
            cached_tables: vec![],
            history: vec![],
            messages: vec![],
            draft: String::new(),
            drafts: Default::default(),
            draft_owner: String::new(),
            cursor: 0,
            chat_filter: String::new(),
            filtering_chat: false,
            command_query: String::new(),
            command_selection: 0,
            detail_scroll: 0,
            sidebar: true,
            paused: false,
            scroll: 0,
            tick: 0,
            quit: false,
            toast: None,
            width: 144,
            action_hits: vec![],
            seen_revision: 0,
        }
    }
    pub fn demo(count: usize) -> Self {
        let mut app = Self::new();
        app.simulation = None;
        app.world = World::demo(count);
        app.messages.push(Message {
            speaker: Speaker::Summary,
            text: format!(
                "Explicit table stress fixture: {count} sample records. No agents were started."
            ),
            author: None,
            tool: None,
        });
        app
    }
    pub fn title(&self) -> String {
        if let Some(sim) = &self.simulation {
            sim.task().map_or("New task".into(), |t| {
                format!(
                    "{} · {}",
                    t.id,
                    t.goal.graphemes(true).take(24).collect::<String>()
                )
            })
        } else {
            "Table stress fixture".into()
        }
    }
    pub fn sync(&mut self) {
        let Some(sim) = &self.simulation else {
            return;
        };
        if sim.revision == self.seen_revision {
            return;
        }
        let context = sim.task().map_or(String::new(), |t| t.id.clone());
        if context != self.draft_owner {
            if self.draft_owner.is_empty() || sim.tasks.iter().any(|t| t.id == self.draft_owner) {
                self.drafts
                    .insert(self.draft_owner.clone(), (self.draft.clone(), self.cursor));
            }
            let (draft, cursor) = self.drafts.get(&context).cloned().unwrap_or_default();
            self.draft = draft;
            self.cursor = cursor;
            self.draft_owner = context;
            self.chat_filter.clear();
            self.tool_focus = None;
            self.cached_tables
                .retain(|t| matches!(t.data.kind, Kind::Tasks | Kind::Providers | Kind::Activity));
        }
        self.drafts
            .retain(|id, _| id.is_empty() || sim.tasks.iter().any(|t| &t.id == id));
        self.seen_revision = sim.revision;
        self.world = sim.project();
        self.messages = sim.task().map_or_else(Vec::new, |t| {
            t.entries
                .iter()
                .map(|e| Message {
                    speaker: if e.user {
                        Speaker::You
                    } else {
                        Speaker::Summary
                    },
                    text: e.text.clone(),
                    author: e.author.clone(),
                    tool: e.tool.clone(),
                })
                .collect()
        });
        if let Some(table) = &mut self.table {
            table.refresh(self.world.dataset(table.data.kind));
        }
        for table in &mut self.cached_tables {
            table.refresh(self.world.dataset(table.data.kind));
        }
        for back in &mut self.history {
            if let Some(table) = &mut back.table {
                table.refresh(self.world.dataset(table.data.kind));
            }
        }
        if self.layers.contains(&Layer::Actions) {
            self.refresh_menu();
        }
    }
    pub fn advance(&mut self, now: u64) {
        self.tick = self.tick.wrapping_add(1);
        if self
            .toast
            .as_ref()
            .is_some_and(|(_, until)| self.tick >= *until)
        {
            self.toast = None;
        }
        if let Some(sim) = &mut self.simulation {
            if self.paused {
                let elapsed = now.saturating_sub(sim.now);
                for t in sim.tasks.iter_mut().filter(|t| t.stage.busy()) {
                    t.next_step = t.next_step.saturating_add(elapsed);
                }
                for a in sim.agents.iter_mut().filter(|a| a.ended.is_none()) {
                    if let Some(due) = &mut a.due {
                        *due = due.saturating_add(elapsed);
                    }
                }
                if sim.now / 1000 != now / 1000 {
                    sim.revision += 1;
                }
                sim.now = now;
            } else {
                sim.advance_to(now);
            }
        }
        self.sync();
    }
    fn save_table(&mut self) {
        if let Some(table) = self.table.take() {
            self.cached_tables
                .retain(|v| v.data.kind != table.data.kind);
            self.cached_tables.push(table);
        }
    }
    pub fn open_table(&mut self, kind: Kind, query: &str) {
        self.save_table();
        self.table = Some(
            if let Some(i) = self.cached_tables.iter().position(|v| v.data.kind == kind) {
                self.cached_tables.remove(i)
            } else {
                ResourceView::new(self.world.dataset(kind), "")
            },
        );
        if !query.is_empty()
            && let Some(t) = &mut self.table
        {
            t.query = query.into();
            t.rebuild();
        }
        self.layers.clear();
        self.detail_scroll = 0;
    }
    pub(crate) fn chat(&mut self) {
        self.save_table();
        self.layers.clear();
        self.history.clear();
    }
    pub fn open_commands(&mut self) {
        if self.layers.last() != Some(&Layer::Commands) {
            self.layers.push(Layer::Commands);
        }
        self.command_query = "/".into();
        self.command_selection = 0;
    }
    pub fn command_available(&self, index: usize) -> bool {
        let stage = self
            .simulation
            .as_ref()
            .and_then(|s| s.task())
            .map(|t| t.stage);
        match COMMANDS[index].name {
            "/continue" => stage == Some(Stage::ProviderRequired),
            "/start" => stage == Some(Stage::Ready),
            "/stop" => self.simulation.as_ref().is_some_and(Simulation::busy),
            "/retry" => matches!(
                stage,
                Some(Stage::Stopped | Stage::Review | Stage::Delivered)
            ),
            "/accept" => stage == Some(Stage::Review),
            _ => true,
        }
    }
    pub fn commands(&self) -> Vec<usize> {
        let query = self
            .command_query
            .split_whitespace()
            .next()
            .unwrap_or("")
            .trim_start_matches('/')
            .to_lowercase();
        (0..COMMANDS.len())
            .filter(|&i| {
                self.command_available(i)
                    && (COMMANDS[i].name.contains(&query)
                        || COMMANDS[i].description.to_lowercase().contains(&query))
            })
            .collect()
    }
    pub fn notice(&mut self, text: &str) {
        self.toast = Some((text.into(), self.tick + 40));
    }
    pub fn execute_command(&mut self, command: &str) {
        let mut parts = command.trim().splitn(2, char::is_whitespace);
        let name = parts.next().unwrap_or("");
        let query = parts.next().unwrap_or("").trim();
        if name == "/actions" {
            self.layers.retain(|l| *l != Layer::Commands);
            self.open_actions();
            return;
        }
        if name == "/add" {
            self.layers.retain(|l| *l != Layer::Commands);
            self.add_in_current_view();
            return;
        }
        self.layers.clear();
        self.history.clear();
        if let Some(kind) = Kind::parse(name) {
            self.open_table(kind, query);
            return;
        }
        match name {
            "/chat" => self.chat(),
            "/actions" => self.open_actions(),
            "/add" => self.add_in_current_view(),
            "/help" => self.layers.push(Layer::Help),
            "/scenario" => {
                if self.simulation.as_ref().is_some_and(Simulation::busy) {
                    self.notice("Stop current development before opening another scenario.");
                    return;
                }
                if self.simulation.is_none() {
                    *self = Self::new();
                }
                if let Some(s) = &mut self.simulation {
                    let _ = s.new_task();
                    s.submit("Разработать игру «Морской бой» в браузере против компьютера.");
                }
                self.chat();
                self.sync();
            }
            "/new" => {
                if let Some(s) = &mut self.simulation
                    && let Err(e) = s.new_task()
                {
                    self.notice(e);
                    return;
                }
                self.chat();
                self.draft.clear();
                self.cursor = 0;
                self.chat_filter.clear();
                self.cached_tables.clear();
                self.sync();
            }
            "/limits" => {
                let values: Vec<_> = query
                    .split_whitespace()
                    .filter_map(|s| s.parse::<usize>().ok())
                    .collect();
                if query.split_whitespace().count() != 2
                    || values.len() != 2
                    || values[0] == 0
                    || values[0] > 10000
                    || values[1] == 0
                    || values[1] > values[0]
                {
                    self.notice("Usage: /limits <max agents 1..10000> <parallel 1..max agents>");
                } else if let Some(s) = &mut self.simulation {
                    if let Err(e) = s.set_limits(values[0], values[1]) {
                        self.notice(&e);
                        return;
                    }
                    self.notice(
                        "Limits updated for the next run. Existing run limits are unchanged.",
                    );
                    self.sync();
                }
            }
            "/continue" | "/start" | "/stop" | "/retry" | "/accept" => {
                let result = if let Some(s) = &mut self.simulation {
                    match name {
                        "/continue" => s.continue_conversation(),
                        "/start" => s.start(false),
                        "/retry" => s.start(true),
                        "/stop" => s.stop(),
                        _ => s.accept(),
                    }
                } else {
                    Err("This action belongs to the lifecycle simulation. /scenario")
                };
                if let Err(e) = result {
                    self.notice(e);
                } else {
                    self.chat();
                }
                self.sync();
            }
            _ => self.notice("Unknown command. /help"),
        }
    }
    pub fn click_action(&mut self, x: u16, y: u16) {
        if let Some(Layer::Editor) = self.layers.last() {
            if let Some((rect, index)) = self
                .editor_hits
                .iter()
                .find(|(r, _)| r.contains((x, y).into()))
                .copied()
                && let Some(e) = &mut self.editor
            {
                e.selected = index;
                if index == e.fields.len() {
                    self.save_editor();
                } else if index == e.fields.len() + 1 {
                    self.dismiss_top();
                } else if matches!(e.fields[index].value, crate::forms::Value::Choice { .. }) {
                    e.key(if x <= rect.x + 1 {
                        KeyCode::Left
                    } else {
                        KeyCode::Right
                    });
                }
            }
            return;
        }
        if let Some(Layer::Actions) = self.layers.last() {
            if let Some(index) = self
                .menu_hits
                .iter()
                .find(|(r, _)| r.contains((x, y).into()))
                .map(|(_, i)| *i)
            {
                self.menu_selection = index;
                self.choose_action();
            }
            return;
        }
        if let Some(Layer::Confirm) = self.layers.last() {
            if let Some(index) = self
                .editor_hits
                .iter()
                .find(|(r, _)| r.contains((x, y).into()))
                .map(|(_, i)| *i)
            {
                if index == 1 {
                    if let Some(c) = self.confirmation.clone() {
                        self.perform(c.action, true);
                    }
                } else {
                    self.dismiss_top();
                }
            }
            return;
        }
        if let Some(action) = self
            .buttons
            .iter()
            .find(|(r, _)| r.contains((x, y).into()))
            .map(|(_, a)| a.clone())
        {
            self.perform(action, false);
            return;
        }

        if !self.layers.is_empty() && self.layers.last() != Some(&Layer::Services) {
            return;
        }
        if let Some(command) = self
            .action_hits
            .iter()
            .find(|(r, _)| r.contains((x, y).into()))
            .map(|(_, c)| c.clone())
        {
            self.execute_command(&command);
        }
    }
    pub fn paste(&mut self, text: &str) {
        if self.layers.last() == Some(&Layer::Editor) {
            if let Some(e) = &mut self.editor {
                e.paste(text);
            }
            return;
        }

        let clean: String = text
            .chars()
            .filter_map(|c| match c {
                '\n' | '\r' | '\t' => Some(' '),
                c if c.is_control() => None,
                c => Some(c),
            })
            .collect();
        if self.layers.last() == Some(&Layer::Commands) {
            if self.command_query == "/" && clean.starts_with('/') {
                self.command_query = clean;
            } else {
                self.command_query.push_str(&clean);
            }
            self.command_selection = 0;
        } else if self.layers.is_empty() {
            if let Some(t) = &mut self.table {
                if t.editing {
                    t.query.push_str(&clean);
                    t.rebuild();
                }
            } else if self.filtering_chat {
                self.chat_filter.push_str(&clean);
                self.scroll = 0;
            } else {
                self.draft.insert_str(self.cursor, &clean);
                self.cursor += clean.len();
            }
        }
    }
    pub fn key(&mut self, key: KeyEvent) {
        if key.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key.code, KeyCode::Char('q' | 'c'))
        {
            self.layers.retain(|l| *l != Layer::Confirm);
            self.confirmation = None;
            self.perform(Action::Quit, false);
            return;
        }

        if self.layers.last() == Some(&Layer::Editor) {
            if key.modifiers.contains(KeyModifiers::CONTROL) {
                match key.code {
                    KeyCode::Char('s') => self.save_editor(),
                    KeyCode::Char('u') => {
                        if let Some(e) = &mut self.editor {
                            e.clear();
                        }
                    }
                    KeyCode::Char('q' | 'c') => self.perform(Action::Quit, false),
                    _ => {}
                }
            } else if let Some(e) = &mut self.editor {
                match e.key(key.code) {
                    crate::forms::EditorResult::Save => self.save_editor(),
                    crate::forms::EditorResult::Cancel => self.dismiss_top(),
                    _ => {}
                }
            }
            return;
        }
        if self.layers.last() == Some(&Layer::Confirm) {
            match key.code {
                KeyCode::Esc => self.dismiss_top(),
                KeyCode::Tab | KeyCode::Left | KeyCode::Right => {
                    if let Some(c) = &mut self.confirmation {
                        c.selected = !c.selected;
                    }
                }
                KeyCode::Enter => {
                    if let Some(c) = self.confirmation.clone() {
                        if c.selected {
                            self.perform(c.action, true);
                        } else {
                            self.dismiss_top();
                        }
                    }
                }
                _ => {}
            }
            return;
        }
        if self.layers.last() == Some(&Layer::Actions) {
            match key.code {
                KeyCode::Esc => self.dismiss_top(),
                KeyCode::Down | KeyCode::Tab if !self.menu.is_empty() => {
                    self.menu_selection = (self.menu_selection + 1) % self.menu.len()
                }
                KeyCode::Up if !self.menu.is_empty() => {
                    self.menu_selection =
                        (self.menu_selection + self.menu.len() - 1) % self.menu.len()
                }
                KeyCode::Enter => self.choose_action(),
                _ => {}
            }
            return;
        }

        if key.modifiers.contains(KeyModifiers::CONTROL) {
            match key.code {
                KeyCode::Char('c' | 'q') => self.perform(Action::Quit, false),
                KeyCode::Char('k') => self.open_commands(),
                KeyCode::Char('n') => self.execute_command("/new"),
                KeyCode::Char('p') => self.paused = !self.paused,
                KeyCode::Char('b') => {
                    if self.width < 110 {
                        self.layers.push(Layer::Services);
                    } else {
                        self.sidebar = !self.sidebar;
                    }
                }
                KeyCode::Char('f') if self.layers.is_empty() => {
                    if let Some(t) = &mut self.table {
                        t.editing = true;
                    } else {
                        self.filtering_chat = true;
                    }
                }
                KeyCode::Char('u') if self.layers.is_empty() => {
                    if let Some(t) = &mut self.table {
                        t.query.clear();
                        t.rebuild();
                    } else if self.filtering_chat || !self.chat_filter.is_empty() {
                        self.chat_filter.clear();
                        self.scroll = 0;
                    } else {
                        self.draft.clear();
                        self.cursor = 0;
                    }
                }
                _ => {}
            }
            return;
        }
        if key.code == KeyCode::Esc {
            if self.layers.is_empty() && self.tool_focus.take().is_some() {
                return;
            }

            if !self.layers.is_empty() {
                self.dismiss_top();
                return;
            }
            if let Some(t) = &mut self.table {
                if t.editing {
                    t.editing = false;
                } else if let Some(back) = self.history.pop() {
                    self.table = back.table;
                    self.layers = back.layers;
                    self.detail_scroll = back.scroll;
                    if let Some(s) = &mut self.simulation {
                        if let Some(id) = back.task {
                            s.select_task(&id);
                        } else {
                            s.active = None;
                            s.revision += 1;
                        }
                    }
                    self.sync();
                } else {
                    self.chat();
                }
            } else if self.filtering_chat {
                self.filtering_chat = false;
            } else {
                self.chat_filter.clear();
                self.scroll = 0;
            }
            return;
        }
        if let Some(layer) = self.layers.last().cloned() {
            if layer != Layer::Commands && key.code == KeyCode::Char('/') {
                self.open_commands();
                return;
            }
            if layer != Layer::Commands && key.code == KeyCode::Char('?') {
                if layer == Layer::Help {
                    self.layers.pop();
                } else {
                    self.layers.push(Layer::Help);
                }
                return;
            }
            match layer {
                Layer::Commands => self.palette_key(key.code),
                Layer::Detail(id) => self.detail_key(&id, key.code),
                _ => {}
            }
            return;
        }
        if key.code == KeyCode::Char('.') && self.table.is_some() {
            self.open_actions();
            return;
        }
        if self.table.is_none() && !self.filtering_chat {
            if key.code == KeyCode::Tab && !self.visible_tools.is_empty() {
                self.tool_focus = match &self.tool_focus {
                    None => self.visible_tools.first().cloned(),
                    Some(id) => self
                        .visible_tools
                        .iter()
                        .position(|x| x == id)
                        .and_then(|i| self.visible_tools.get(i + 1))
                        .cloned(),
                };
                return;
            }
            if let Some(id) = self.tool_focus.clone() {
                if matches!(key.code, KeyCode::Enter | KeyCode::Char(' ')) {
                    self.perform(Action::ToggleTool(id), false);
                    return;
                }
                if let KeyCode::Char(_) = key.code {
                    self.tool_focus = None;
                }
            }
        }
        if key.code == KeyCode::F(1) {
            self.layers.push(Layer::Help);
            return;
        }
        if self.table.is_some() {
            self.table_key(key);
        } else {
            self.input_key(key.code);
        }
    }
    fn detail_key(&mut self, id: &str, key: KeyCode) {
        if key == KeyCode::Char('.') {
            self.open_actions();
            return;
        }

        if key == KeyCode::Char(' ')
            && self
                .table
                .as_ref()
                .is_some_and(|t| t.data.kind == Kind::Providers)
        {
            if let Some(s) = &mut self.simulation {
                if let Some(index) = s.providers.iter().position(|p| p.id == id) {
                    s.toggle_provider(index);
                }
                self.sync();
            } else {
                let index = self
                    .table
                    .as_ref()
                    .and_then(|t| t.data.records.iter().position(|r| r.id == id))
                    .unwrap_or(0);
                self.world.toggle_provider(index);
                if let Some(t) = &mut self.table {
                    t.refresh(self.world.dataset(Kind::Providers));
                }
            }
            return;
        }
        if let KeyCode::Char(c) = key
            && let Some(table) = &self.table
            && let Some(record) = table.data.records.iter().find(|r| r.id == id)
            && let Some(link) = record.links.iter().find(|l| l.key == c).cloned()
        {
            self.follow(link.kind, link.query);
            return;
        }
        match key {
            KeyCode::Up => self.detail_scroll = self.detail_scroll.saturating_sub(1),
            KeyCode::Down => self.detail_scroll = self.detail_scroll.saturating_add(1),
            KeyCode::PageUp => self.detail_scroll = self.detail_scroll.saturating_sub(10),
            KeyCode::PageDown => self.detail_scroll = self.detail_scroll.saturating_add(10),
            KeyCode::Home => self.detail_scroll = 0,
            KeyCode::End => self.detail_scroll = u16::MAX,
            _ => {}
        }
    }
    fn table_key(&mut self, key: KeyEvent) {
        let t = self.table.as_mut().unwrap();
        if t.editing {
            match key.code {
                KeyCode::Char(c) if !c.is_control() => {
                    t.query.push(c);
                    t.rebuild();
                }
                KeyCode::Backspace => {
                    pop_grapheme(&mut t.query);
                    t.rebuild();
                }
                KeyCode::Enter => t.editing = false,
                _ => {}
            }
            return;
        }
        match key.code {
            KeyCode::Up => t.navigate(-1),
            KeyCode::Down => t.navigate(1),
            KeyCode::PageUp => t.page(false),
            KeyCode::PageDown => t.page(true),
            KeyCode::Home => t.navigate(-(t.selected as isize)),
            KeyCode::End => t.navigate(t.matches.len() as isize),
            KeyCode::Tab | KeyCode::Char(']') => t.cycle_filter(false),
            KeyCode::BackTab | KeyCode::Char('[') => t.cycle_filter(true),
            KeyCode::Char('f') => t.editing = true,
            KeyCode::Char('a') => self.open_actions(),
            KeyCode::Char('n') => self.add_in_current_view(),
            KeyCode::Char('e') => {
                let kind = t.data.kind;
                let id = t.current().map(|r| r.id.clone());
                if let Some(id) = id {
                    match kind {
                        Kind::Providers => self.perform(Action::EditProvider(id), false),
                        Kind::Tasks => self.perform(Action::EditTask(id), false),
                        _ => self.open_actions(),
                    }
                }
            }
            KeyCode::Char('s') => t.sort_next(),
            KeyCode::Char('S') => {
                t.descending = !t.descending;
                t.rebuild();
            }
            KeyCode::Char('/') => self.open_commands(),
            KeyCode::Char('?') => self.layers.push(Layer::Help),
            KeyCode::Enter => {
                if let Some(&i) = t.matches.get(t.selected) {
                    self.detail_scroll = 0;
                    self.layers
                        .push(Layer::Detail(t.data.records[i].id.clone()));
                }
            }
            _ => {}
        }
    }
    fn palette_key(&mut self, key: KeyCode) {
        let commands = self.commands();
        match key {
            KeyCode::Char(c) if !c.is_control() => {
                self.command_query.push(c);
                self.command_selection = 0;
            }
            KeyCode::Backspace => {
                pop_grapheme(&mut self.command_query);
                self.command_selection = 0;
            }
            KeyCode::Down | KeyCode::Tab if !commands.is_empty() => {
                self.command_selection = (self.command_selection + 1) % commands.len()
            }
            KeyCode::Up if !commands.is_empty() => {
                self.command_selection =
                    (self.command_selection + commands.len() - 1) % commands.len()
            }
            KeyCode::Enter => {
                let typed = self.command_query.clone();
                if COMMANDS
                    .iter()
                    .any(|c| typed.split_whitespace().next() == Some(c.name))
                {
                    self.execute_command(&typed);
                } else if let Some(&i) = commands.get(self.command_selection) {
                    self.execute_command(COMMANDS[i].name);
                } else {
                    self.notice("No matching command");
                }
            }
            _ => {}
        }
    }
    fn input_key(&mut self, key: KeyCode) {
        if self.filtering_chat {
            match key {
                KeyCode::Char(c) if !c.is_control() => self.chat_filter.push(c),
                KeyCode::Backspace => pop_grapheme(&mut self.chat_filter),
                KeyCode::Enter => self.filtering_chat = false,
                _ => {}
            }
            self.scroll = 0;
            return;
        }
        match key {
            KeyCode::Char('/') if self.draft.is_empty() => self.open_commands(),
            KeyCode::Char('?') if self.draft.is_empty() => self.layers.push(Layer::Help),
            KeyCode::Char(c) if !c.is_control() => {
                self.draft.insert(self.cursor, c);
                self.cursor += c.len_utf8();
            }
            KeyCode::PageUp => self.scroll = self.scroll.saturating_add(6),
            KeyCode::PageDown => self.scroll = self.scroll.saturating_sub(6),
            KeyCode::Left => self.cursor = self.previous_boundary(),
            KeyCode::Right => {
                if let Some(g) = self.draft[self.cursor..].graphemes(true).next() {
                    self.cursor += g.len();
                }
            }
            KeyCode::Home => self.cursor = 0,
            KeyCode::End => {
                self.cursor = self.draft.len();
                self.scroll = 0;
            }
            KeyCode::Backspace if self.cursor > 0 => {
                let previous = self.previous_boundary();
                self.draft.replace_range(previous..self.cursor, "");
                self.cursor = previous;
            }
            KeyCode::Delete if self.cursor < self.draft.len() => {
                if let Some(g) = self.draft[self.cursor..].graphemes(true).next() {
                    self.draft
                        .replace_range(self.cursor..self.cursor + g.len(), "");
                }
            }
            KeyCode::Enter if !self.draft.trim().is_empty() => {
                let text = std::mem::take(&mut self.draft);
                self.cursor = 0;
                self.scroll = 0;
                if COMMANDS
                    .iter()
                    .any(|c| text.split_whitespace().next() == Some(c.name))
                {
                    self.execute_command(&text);
                } else if let Some(s) = &mut self.simulation {
                    if let Err(error) = s.send_message(&text) {
                        self.draft = text;
                        self.cursor = self.draft.len();
                        self.notice(&error);
                    } else {
                        self.sync();
                    }
                } else {
                    self.messages.push(Message {
                        speaker: Speaker::You,
                        text,
                        author: None,
                        tool: None,
                    });
                }
            }
            _ => {}
        }
    }
    pub fn dismiss_top(&mut self) {
        match self.layers.pop() {
            Some(Layer::Editor) => self.editor = None,
            Some(Layer::Confirm) => self.confirmation = None,
            Some(Layer::Actions) => {
                self.menu.clear();
                self.menu_target = None;
            }
            _ => {}
        }
    }
    pub fn choose_action(&mut self) {
        self.refresh_menu();
        if let Some(item) = self.menu.get(self.menu_selection).cloned() {
            if let Some(reason) = item.reason {
                self.notice(&reason);
            } else {
                self.perform(item.action, false);
            }
        }
    }
    pub fn follow(&mut self, kind: Kind, query: String) {
        let previous_task = self
            .simulation
            .as_ref()
            .and_then(|s| s.task())
            .map(|t| t.id.clone());
        if self
            .table
            .as_ref()
            .is_some_and(|t| t.data.kind == Kind::Tasks)
        {
            let id = self.selected_record().map(|r| r.id.clone());
            if let Some(id) = id
                && let Some(s) = &mut self.simulation
            {
                s.select_task(&id);
                self.sync();
            }
        }
        self.layers
            .retain(|l| !matches!(l, Layer::Actions | Layer::Confirm));
        self.history.push(Back {
            task: previous_task,
            table: self.table.take(),
            layers: std::mem::take(&mut self.layers),
            scroll: self.detail_scroll,
        });
        self.open_table(kind, &query);
        if let Some(t) = &mut self.table {
            t.query = query;
            t.show_archived = true;
            t.rebuild();
        }
    }
    fn previous_boundary(&self) -> usize {
        self.draft[..self.cursor]
            .grapheme_indices(true)
            .next_back()
            .map_or(0, |(i, _)| i)
    }
    pub fn scenario_scene(name: &str) -> Result<Self, &'static str> {
        let mut app = Self::new();
        if name == "empty" {
            return Ok(app);
        }
        let s = app.simulation.as_mut().unwrap();
        s.submit("Разработать игру «Морской бой» в браузере против компьютера.");
        if name != "provider" {
            s.add_provider("OpenAI", crate::entities::Family::OpenAi, true)
                .map_err(|_| "Cannot configure fixture")?;
            s.continue_conversation()?;
        }
        if !matches!(name, "provider" | "clarify") {
            s.submit("Подтверждаю. Интерфейс игры на русском, без сети и регистрации.");
        }
        if !matches!(name, "provider" | "clarify" | "ready") {
            s.start(false)?;
            let steps = match name {
                "starting" => 0,
                "working" => 2,
                "failed" => 3,
                "checking" => 6,
                "review" | "done" => 7,
                _ => return Err("Unknown scene"),
            };
            for n in 0..steps {
                s.advance_to(3000 * (n + 1));
            }
            if name == "done" {
                s.accept()?;
            }
        }
        app.sync();
        Ok(app)
    }
}
fn pop_grapheme(text: &mut String) {
    if let Some((i, _)) = text.grapheme_indices(true).next_back() {
        text.truncate(i);
    }
}

impl App {
    pub fn ui_state(&self) -> crate::storage::UiState {
        fn bookmark(t: &ResourceView) -> crate::storage::Bookmark {
            crate::storage::Bookmark {
                kind: Some(t.data.kind),
                query: t.query.clone(),
                selected_id: t.current().map(|r| r.id.clone()),
                sort_column: t.sort_column,
                descending: t.descending,
                filter: t.data.filters[t.filter].into(),
                show_archived: t.show_archived,
            }
        }
        crate::storage::UiState {
            draft: self.draft.clone(),
            cursor: self.cursor,
            chat_filter: self.chat_filter.clone(),
            sidebar: self.sidebar,
            current: self.table.as_ref().map(bookmark),
            tables: self.cached_tables.iter().map(bookmark).collect(),
            detail_id: self.layers.iter().rev().find_map(|l| {
                if let Layer::Detail(id) = l {
                    Some(id.clone())
                } else {
                    None
                }
            }),
            expanded_tools: self.expanded_tools.iter().cloned().collect(),
            paused: self.paused,
            drafts: self.drafts.clone(),
        }
    }
    pub fn from_checkpoint(sim: Simulation, ui: crate::storage::UiState) -> Self {
        let mut app = Self::new();
        app.simulation = Some(sim);
        app.seen_revision = u64::MAX;
        app.sync();
        app.drafts = ui.drafts;
        app.draft_owner = app
            .simulation
            .as_ref()
            .and_then(|s| s.task())
            .map_or(String::new(), |t| t.id.clone());
        app.draft = ui.draft;
        app.cursor = ui.cursor;
        app.chat_filter = ui.chat_filter;
        app.sidebar = ui.sidebar;
        app.paused = ui.paused;
        app.expanded_tools = ui.expanded_tools.into_iter().collect();
        fn restore(app: &App, b: crate::storage::Bookmark) -> Option<ResourceView> {
            let kind = b.kind?;
            let mut table = ResourceView::new(app.world.dataset(kind), &b.query);
            table.show_archived = b.show_archived;
            table.sort_column = b.sort_column.filter(|i| *i < table.data.headers.len());
            table.descending = b.descending;
            table.filter = table
                .data
                .filters
                .iter()
                .position(|f| *f == b.filter)
                .unwrap_or(0);
            table.rebuild();
            if let Some(id) = b.selected_id
                && let Some(i) = table
                    .matches
                    .iter()
                    .position(|&i| table.data.records[i].id == id)
            {
                table.navigate(i as isize);
            }
            Some(table)
        }
        for b in ui.tables {
            if let Some(table) = restore(&app, b) {
                app.cached_tables.retain(|t| t.data.kind != table.data.kind);
                app.cached_tables.push(table);
            }
        }
        app.table = ui.current.and_then(|b| restore(&app, b));
        if let Some(id) = ui.detail_id
            && app
                .table
                .as_ref()
                .is_some_and(|t| t.data.records.iter().any(|r| r.id == id))
        {
            app.layers.push(Layer::Detail(id));
        }
        app
    }
}

#[cfg(test)]
mod management_ui_tests {
    use super::*;
    #[test]
    fn duplicate_form_is_rejected_and_cancel_does_not_mutate_configuration() {
        let mut app = App::new();
        app.open_table(Kind::Providers, "");
        app.perform(Action::AddProvider, false);
        app.editor.as_mut().unwrap().paste("Work");
        app.save_editor();
        assert_eq!(app.simulation.as_ref().unwrap().providers.len(), 1);
        assert!(app.world.agents.records.is_empty());
        app.perform(Action::AddProvider, false);
        app.editor.as_mut().unwrap().paste("work");
        app.save_editor();
        assert!(app.editor.as_ref().unwrap().error.is_some());
        assert_eq!(app.simulation.as_ref().unwrap().providers.len(), 1);
        app.dismiss_top();
        assert_eq!(app.simulation.as_ref().unwrap().providers.len(), 1);
    }
    #[test]
    fn deletion_confirmation_is_cancel_by_default() {
        let mut app = App::new();
        let id = app
            .simulation
            .as_mut()
            .unwrap()
            .add_provider("Work", crate::entities::Family::OpenAi, false)
            .unwrap();
        app.sync();
        app.perform(Action::RemoveProvider(id.clone()), false);
        assert!(!app.confirmation.as_ref().unwrap().selected);
        app.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert_eq!(app.simulation.as_ref().unwrap().providers.len(), 1);
        app.perform(Action::RemoveProvider(id), false);
        app.key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));
        app.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert!(app.simulation.as_ref().unwrap().providers.is_empty());
    }
    #[test]
    fn task_drafts_are_scoped_and_deleted_drafts_are_not_retained() {
        let mut app = App::new();
        let first = app
            .simulation
            .as_mut()
            .unwrap()
            .create_task("First", "")
            .unwrap();
        app.sync();
        app.paste("Первый черновик");
        let second = app
            .simulation
            .as_mut()
            .unwrap()
            .create_task("Second", "")
            .unwrap();
        app.sync();
        assert!(app.draft.is_empty());
        app.paste("Second draft");
        app.perform(Action::SelectTask(first.clone()), false);
        assert_eq!(app.draft, "Первый черновик");
        app.perform(Action::SelectTask(second.clone()), false);
        assert_eq!(app.draft, "Second draft");
        app.perform(Action::DeleteTask(second.clone()), true);
        assert!(!app.drafts.contains_key(&second));
        assert!(app.draft.is_empty());
        app.perform(Action::SelectTask(first), false);
        assert_eq!(app.draft, "Первый черновик");
    }
    #[test]
    fn evidence_menus_expose_export_but_not_destructive_edits() {
        let mut app = App::scenario_scene("review").unwrap();
        for kind in [Kind::Board, Kind::Checks] {
            app.open_table(kind, "");
            let items = app.action_items();
            assert!(
                items
                    .iter()
                    .any(|i| matches!(i.action, Action::Export { .. }))
            );
            assert!(!items.iter().any(|i| i.label.starts_with("Delete")
                || i.label.starts_with("Remove")
                || i.label == "Edit"));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn press(app: &mut App, code: KeyCode) {
        app.key(KeyEvent::new(code, KeyModifiers::NONE));
    }
    #[test]
    fn slash_navigation_and_relations_preserve_unicode_drafts_and_filters() {
        let mut app = App::scenario_scene("working").unwrap();
        app.paste("Черновик /tmp/世界?");
        let draft = app.draft.clone();
        app.open_commands();
        app.paste("/agents");
        press(&mut app, KeyCode::Enter);
        assert_eq!(app.table.as_ref().unwrap().data.kind, Kind::Agents);
        press(&mut app, KeyCode::Char('f'));
        app.paste("agent=A-0002");
        press(&mut app, KeyCode::Enter);
        assert_eq!(app.table.as_ref().unwrap().matches.len(), 1);
        press(&mut app, KeyCode::Enter);
        press(&mut app, KeyCode::Char('b'));
        assert_eq!(app.table.as_ref().unwrap().data.kind, Kind::Board);
        assert_eq!(app.table.as_ref().unwrap().query, "author=A-0002");
        press(&mut app, KeyCode::Esc);
        assert!(matches!(app.layers.last(), Some(Layer::Detail(_))));
        press(&mut app, KeyCode::Esc);
        app.execute_command("/chat");
        assert_eq!(app.draft, draft);
        app.execute_command("/agents");
        assert_eq!(app.table.as_ref().unwrap().query, "agent=A-0002");
        app.advance(9000);
        assert_eq!(app.table.as_ref().unwrap().current().unwrap().id, "A-0002");
    }
    #[test]
    fn provider_enable_and_sidebar_action_use_the_same_authorized_path() {
        let mut app = App::new();
        app.execute_command("/scenario");
        assert!(app.world.agents.records.is_empty());
        app.execute_command("/providers");
        app.perform(Action::AddProvider, false);
        app.editor.as_mut().unwrap().paste("OpenAI");
        app.save_editor();
        press(&mut app, KeyCode::Char(' '));
        assert!(app.world.agents.records.is_empty());
        app.execute_command("/continue");
        assert_eq!(app.world.agents.records.len(), 1);
        app.paste("Подтверждаю");
        press(&mut app, KeyCode::Enter);
        assert_eq!(
            app.simulation.as_ref().unwrap().task().unwrap().stage,
            Stage::Ready
        );
        app.action_hits
            .push((Rect::new(10, 10, 20, 1), "/start".into()));
        app.click_action(11, 10);
        assert_eq!(
            app.simulation.as_ref().unwrap().task().unwrap().stage,
            Stage::Developing
        );
        assert_eq!(app.world.agents.records.len(), 2);
    }
    #[test]
    fn input_preserves_paths_punctuation_and_graphemes() {
        let mut app = App::new();
        app.paste("/tmp/проект: 世界?");
        press(&mut app, KeyCode::Enter);
        assert_eq!(
            app.simulation.as_ref().unwrap().task().unwrap().goal,
            "/tmp/проект: 世界?"
        );
        app.paste("\u{1b}hello e\u{301}");
        press(&mut app, KeyCode::Backspace);
        assert_eq!(app.draft, "hello ");
    }
}
