use serde::{Deserialize, Serialize};
use std::{ops::Range, sync::Arc};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Agents,
    Tasks,
    Board,
    Providers,
    Activity,
    Sessions,
    Tools,
    Checks,
    Files,
}

impl Kind {
    pub fn name(self) -> &'static str {
        match self {
            Self::Agents => "agents",
            Self::Tasks => "tasks",
            Self::Board => "board",
            Self::Providers => "providers",
            Self::Activity => "activity",
            Self::Sessions => "sessions",
            Self::Tools => "tools",
            Self::Checks => "checks",
            Self::Files => "files",
        }
    }
    pub fn parse(name: &str) -> Option<Self> {
        match name.strip_prefix('/').unwrap_or(name) {
            "agents" => Some(Self::Agents),
            "tasks" => Some(Self::Tasks),
            "board" | "knowledge" => Some(Self::Board),
            "providers" => Some(Self::Providers),
            "activity" => Some(Self::Activity),
            "sessions" => Some(Self::Sessions),
            "tools" => Some(Self::Tools),
            "checks" => Some(Self::Checks),
            "files" => Some(Self::Files),
            _ => None,
        }
    }
}

pub const SESSIONS: [(&str, &str); 3] = [
    (
        "Reading list",
        "Build a calm reading list that works offline. I want search, tags, and keyboard shortcuts.",
    ),
    (
        "Quick notes",
        "Make it effortless to capture a thought and find it again later.",
    ),
    (
        "Search experience",
        "Design a search experience that keeps me oriented as I explore results.",
    ),
];

#[derive(Clone)]
pub struct Link {
    pub key: char,
    pub label: &'static str,
    pub kind: Kind,
    pub query: String,
}
#[derive(Clone)]
pub struct Record {
    pub id: String,
    pub state: &'static str,
    pub cells: Vec<String>,
    pub detail: String,
    search: String,
    pub links: Vec<Link>,
    pub archived: bool,
}

impl Record {
    pub fn new(id: String, state: &'static str, cells: Vec<String>, detail: String) -> Self {
        let search = cells.join(" ").to_lowercase();
        Self {
            id,
            state,
            cells,
            detail,
            search,
            links: Vec::new(),
            archived: false,
        }
    }
    pub fn archived(mut self, value: bool) -> Self {
        self.archived = value;
        self
    }
    pub fn link(mut self, key: char, label: &'static str, kind: Kind, query: String) -> Self {
        self.links.push(Link {
            key,
            label,
            kind,
            query,
        });
        self
    }
}

pub struct Dataset {
    pub kind: Kind,
    pub headers: Vec<&'static str>,
    pub widths: Vec<u16>,
    pub records: Vec<Record>,
    pub filters: Vec<&'static str>,
}

impl Dataset {
    pub fn new(
        kind: Kind,
        headers: Vec<&'static str>,
        widths: Vec<u16>,
        records: Vec<Record>,
    ) -> Arc<Self> {
        let mut filters = vec!["All"];
        for record in &records {
            if !matches!(
                kind,
                Kind::Board | Kind::Sessions | Kind::Providers | Kind::Files
            ) && !filters.contains(&record.state)
            {
                filters.push(record.state);
            }
        }
        Arc::new(Self {
            kind,
            headers,
            widths,
            records,
            filters,
        })
    }
}

#[derive(Default)]
pub struct Counts {
    pub running: usize,
    pub yielded: usize,
    pub failed: usize,
    pub completed: usize,
}

/// Immutable sample data. No runtime, process or model is started by these records.
pub struct World {
    pub agents: Arc<Dataset>,
    pub tasks: Arc<Dataset>,
    pub knowledge: Arc<Dataset>,
    pub activity: Arc<Dataset>,
    pub sessions: Arc<Dataset>,
    pub providers: Arc<Dataset>,
    pub tools: Arc<Dataset>,
    pub checks: Arc<Dataset>,
    pub files: Arc<Dataset>,
    pub counts: Counts,
    pub models: Vec<(String, usize)>,
}

impl World {
    pub fn demo(agent_count: usize) -> Self {
        // Actual model/profile names in ymp-runtime-codex and ymp-runtime-claude.
        const MODELS: [&str; 2] = ["gpt-5.6-terra", "claude-opus-5"];
        let mut counts = Counts::default();
        let mut model_counts = [0; 2];
        let mut agents = Vec::with_capacity(agent_count);
        let mut tasks = Vec::with_capacity(agent_count);
        for index in 0..agent_count {
            // Status combines ParticipantState and its optional ParticipantOutcome.
            let status = match index % 64 {
                0..48 => {
                    counts.running += 1;
                    "Running"
                }
                48..58 => {
                    counts.yielded += 1;
                    "Yielded"
                }
                58 => {
                    counts.failed += 1;
                    "Failed"
                }
                _ => {
                    counts.completed += 1;
                    "Completed"
                }
            };
            let route = index % MODELS.len();
            model_counts[route] += 1;
            let id = format!("A-{:04}", index + 1);
            let attempt = format!("X-{:04}", index + 1);
            let contract = format!("C-{:04}", index + 1);
            let detail = format!(
                "Provider: {}\nRuntime: {}{}",
                if route == 0 { "OpenAI" } else { "Anthropic" },
                if route == 0 { "Codex" } else { "Claude Code" },
                if status == "Failed" {
                    "\nReason: runtime failure"
                } else {
                    ""
                }
            );
            agents.push(Record::new(
                id.clone(),
                status,
                vec![
                    id.clone(),
                    status.into(),
                    MODELS[route].into(),
                    "low".into(),
                    "—".into(),
                    attempt,
                ],
                detail,
            ));
            let contract_state = if matches!(status, "Running" | "Yielded") {
                "Active"
            } else {
                "Returned"
            };
            tasks.push(Record::new(
                contract.clone(),
                contract_state,
                vec![contract, contract_state.into(), id, "—".into()],
                String::new(),
            ));
        }
        let agents = Dataset::new(
            Kind::Agents,
            vec!["AGENT", "STATUS", "MODEL", "EFFORT", "AGE", "ATTEMPT"],
            vec![11, 12, 20, 8, 7],
            agents,
        );
        let tasks = Dataset::new(
            Kind::Tasks,
            vec!["CONTRACT", "STATE", "PARTICIPANT", "CANDIDATE"],
            vec![14, 14, 16],
            tasks,
        );
        let knowledge = Dataset::new(Kind::Board, vec!["MESSAGE", "AUTHOR", "CONTENT"], vec![12, 12], vec![
            Record::new("M-001".into(), "Message", vec!["M-001".into(), "A-0001".into(), "Capture first. Organize later.".into()], "Let a person save an item before asking for metadata. Tags and other details can follow afterwards.".into()),
            Record::new("M-002".into(), "Message", vec!["M-002".into(), "A-0002".into(), "Give focus back.".into()], "Opening details or dismissing a menu preserves unfinished text and cursor position.".into()),
            Record::new("M-003".into(), "Message", vec!["M-003".into(), "A-0003".into(), "One action, two ways in.".into()], "Shortcuts and visible actions lead to the same operation.".into()),
        ]);
        let events = (agent_count * 8).clamp(256, 16_384);
        let activity = Dataset::new(
            Kind::Activity,
            vec!["SEQUENCE", "EVENT", "PARTICIPANT", "ATTEMPT"],
            vec![11, 25, 15],
            (0..events)
                .rev()
                .map(|i| {
                    let participant = i % agent_count.max(1);
                    let kind = [
                        "participant_started",
                        "participant_yielded",
                        "participant_resumed",
                        "participant_finished",
                    ][i % 4];
                    Record::new(
                        format!("E-{:05}", i + 1),
                        kind,
                        vec![
                            (i + 1).to_string(),
                            kind.into(),
                            format!("A-{:04}", participant + 1),
                            format!("X-{:04}", participant + 1),
                        ],
                        String::new(),
                    )
                })
                .collect(),
        );
        let sessions = Dataset::new(
            Kind::Sessions,
            vec!["SESSION", "TITLE", "MESSAGE"],
            vec![12, 23],
            SESSIONS
                .iter()
                .enumerate()
                .map(|(i, (title, message))| {
                    let id = format!("S-{:03}", i + 1);
                    Record::new(
                        id.clone(),
                        "Conversation",
                        vec![id, (*title).into(), (*message).into()],
                        String::new(),
                    )
                })
                .collect(),
        );
        let providers = Dataset::new(
            Kind::Providers,
            vec!["PROVIDER", "ENABLED", "STATE", "MODELS", "CHECKED"],
            vec![14, 10, 20, 10],
            [("openai", "Codex", MODELS[0]), ("anthropic", "Claude Code", MODELS[1])]
                .into_iter().map(|(id, engine, model)| Record::new(
                    id.into(), "Not checked", vec![id.into(), "No".into(), "Not checked".into(), "—".into(), "—".into()],
                    format!("Runtime: {engine}\nKnown profile: {model}\nEffort: low\nAuthentication: not checked\n\nDemo configuration applies to this session only.\nEnabling does not start an agent or check connectivity.")
                )).collect(),
        );
        Self {
            agents,
            tasks,
            knowledge,
            activity,
            sessions,
            providers,
            tools: Dataset::new(Kind::Tools, vec!["TOOL"], vec![], vec![]),
            checks: Dataset::new(Kind::Checks, vec!["CHECK"], vec![], vec![]),
            files: Dataset::new(Kind::Files, vec!["FILE"], vec![], vec![]),
            counts,
            models: MODELS
                .into_iter()
                .map(str::to_owned)
                .zip(model_counts)
                .collect(),
        }
    }

    pub fn toggle_provider(&mut self, index: usize) {
        let mut records = self.providers.records.clone();
        if let Some(row) = records.get_mut(index) {
            row.cells[1] = if row.cells[1] == "Yes" { "No" } else { "Yes" }.into();
            row.search = row.cells.join(" ").to_lowercase();
        }
        self.providers = Dataset::new(
            Kind::Providers,
            self.providers.headers.clone(),
            self.providers.widths.clone(),
            records,
        );
    }

    pub fn dataset(&self, kind: Kind) -> Arc<Dataset> {
        Arc::clone(match kind {
            Kind::Agents => &self.agents,
            Kind::Tasks => &self.tasks,
            Kind::Board => &self.knowledge,
            Kind::Activity => &self.activity,
            Kind::Sessions => &self.sessions,
            Kind::Providers => &self.providers,
            Kind::Tools => &self.tools,
            Kind::Checks => &self.checks,
            Kind::Files => &self.files,
        })
    }
}

/// Matching and ordering are cached on input changes. Rendering visits one visible page only.
pub struct ResourceView {
    pub data: Arc<Dataset>,
    pub query: String,
    pub editing: bool,
    pub filter: usize,
    pub sort_column: Option<usize>,
    pub descending: bool,
    pub matches: Vec<usize>,
    pub selected: usize,
    pub offset: usize,
    pub page_rows: usize,
    pub show_archived: bool,
}

impl ResourceView {
    pub fn new(data: Arc<Dataset>, query: &str) -> Self {
        let mut view = Self {
            data,
            query: query.into(),
            editing: false,
            filter: 0,
            sort_column: None,
            descending: false,
            matches: Vec::new(),
            selected: 0,
            offset: 0,
            page_rows: 20,
            show_archived: false,
        };
        view.rebuild();
        view
    }

    pub fn rebuild(&mut self) {
        let selected_id = self.current().map(|r| r.id.clone());
        let query = self.query.to_lowercase();
        let filter = self.data.filters[self.filter];
        self.matches = self
            .data
            .records
            .iter()
            .enumerate()
            .filter(|(_, r)| {
                if r.archived && !self.show_archived {
                    return false;
                }
                let matches = if let Some((field, value)) = query.split_once('=') {
                    if field.trim().eq_ignore_ascii_case("id") {
                        r.id.eq_ignore_ascii_case(value.trim())
                    } else {
                        self.data
                            .headers
                            .iter()
                            .position(|h| h.eq_ignore_ascii_case(field.trim()))
                            .is_some_and(|column| {
                                r.cells[column].eq_ignore_ascii_case(value.trim())
                            })
                    }
                } else {
                    r.search.contains(&query)
                };
                (filter == "All" || r.state == filter) && matches
            })
            .map(|(i, _)| i)
            .collect();
        if let Some(column) = self.sort_column {
            self.matches.sort_by(|&a, &b| {
                let order = self.data.records[a].cells[column]
                    .cmp(&self.data.records[b].cells[column])
                    .then(self.data.records[a].id.cmp(&self.data.records[b].id));
                if self.descending {
                    order.reverse()
                } else {
                    order
                }
            });
        }
        self.selected = selected_id
            .and_then(|id| {
                self.matches
                    .iter()
                    .position(|&i| self.data.records[i].id == id)
            })
            .unwrap_or(0);
        self.ensure_visible();
    }

    pub fn refresh(&mut self, data: Arc<Dataset>) {
        let id = self.current().map(|r| r.id.clone());
        let previous_position = self.selected;
        let previous_offset = self.offset;
        let old_filter = self.data.filters[self.filter];
        self.data = data;
        self.matches.clear();
        self.selected = 0;
        self.filter = self
            .data
            .filters
            .iter()
            .position(|s| *s == old_filter)
            .unwrap_or(0);
        self.rebuild();
        self.selected = id
            .and_then(|id| {
                self.matches
                    .iter()
                    .position(|&i| self.data.records[i].id == id)
            })
            .unwrap_or_else(|| previous_position.min(self.matches.len().saturating_sub(1)));
        self.offset = previous_offset;
        self.ensure_visible();
    }
    pub fn current(&self) -> Option<&Record> {
        self.matches
            .get(self.selected)
            .and_then(|&i| self.data.records.get(i))
    }
    pub fn navigate(&mut self, delta: isize) {
        self.selected = self
            .selected
            .saturating_add_signed(delta)
            .min(self.matches.len().saturating_sub(1));
        self.ensure_visible();
    }
    pub fn page(&mut self, forward: bool) {
        let delta = self.page_rows as isize * if forward { 1 } else { -1 };
        self.offset = self.offset.saturating_add_signed(delta);
        self.navigate(delta);
    }
    pub fn resize(&mut self, rows: usize) {
        self.page_rows = rows.max(1);
        self.ensure_visible();
    }
    pub fn visible(&self) -> Range<usize> {
        self.offset..(self.offset + self.page_rows).min(self.matches.len())
    }
    pub fn cycle_filter(&mut self, reverse: bool) {
        self.filter = (self.filter
            + if reverse {
                self.data.filters.len() - 1
            } else {
                1
            })
            % self.data.filters.len();
        if self.data.filters[self.filter] == "Archived" {
            self.show_archived = true;
        }
        self.rebuild();
    }
    pub fn sort_next(&mut self) {
        self.sort_column = Some(
            self.sort_column
                .map_or(0, |i| (i + 1) % self.data.headers.len()),
        );
        self.rebuild();
    }
    fn ensure_visible(&mut self) {
        if self.selected < self.offset {
            self.offset = self.selected;
        }
        if self.selected >= self.offset + self.page_rows {
            self.offset = self.selected + 1 - self.page_rows;
        }
        self.offset = self
            .offset
            .min(self.matches.len().saturating_sub(self.page_rows));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hundreds_are_counted_but_only_the_visible_page_is_rendered() {
        let world = World::demo(512);
        assert_eq!(
            (
                world.counts.running,
                world.counts.yielded,
                world.counts.failed,
                world.counts.completed
            ),
            (384, 80, 8, 40)
        );
        let mut view = ResourceView::new(world.agents, "");
        view.resize(13);
        view.navigate(511);
        assert_eq!(view.current().unwrap().id, "A-0512");
        assert_eq!(view.visible(), 499..512);
        view.query = "A-0512".into();
        view.rebuild();
        assert_eq!(view.matches.len(), 1);
        view.query = "missing-agent".into();
        view.rebuild();
        assert!(view.current().is_none());
        assert!(view.visible().is_empty());
    }
    #[test]
    fn state_filter_preserves_ids_and_has_exact_membership() {
        let world = World::demo(512);
        let mut view = ResourceView::new(world.agents, "");
        view.filter = view
            .data
            .filters
            .iter()
            .position(|&s| s == "Failed")
            .unwrap();
        view.rebuild();
        assert_eq!(view.matches.len(), 8);
        assert!(
            view.matches
                .iter()
                .all(|&i| view.data.records[i].state == "Failed")
        );
        let id = view.current().unwrap().id.clone();
        view.descending = true;
        view.sort_next();
        assert_eq!(view.current().unwrap().id, id);
    }
}

#[cfg(test)]
mod refresh_tests {
    use super::*;
    #[test]
    fn deleting_a_selected_row_cannot_leave_stale_indices() {
        let rows = |ids: &[&str]| {
            Dataset::new(
                Kind::Providers,
                vec!["NAME"],
                vec![],
                ids.iter()
                    .map(|id| {
                        Record::new((*id).into(), "Disabled", vec![(*id).into()], String::new())
                    })
                    .collect(),
            )
        };
        let mut view = ResourceView::new(rows(&["one", "two"]), "id=two");
        assert_eq!(view.current().unwrap().id, "two");
        view.refresh(rows(&["one"]));
        assert!(view.current().is_none());
        assert!(view.matches.is_empty());
        view.query.clear();
        view.rebuild();
        assert_eq!(view.current().unwrap().id, "one");
        view.refresh(rows(&[]));
        assert!(view.current().is_none());
    }
}
