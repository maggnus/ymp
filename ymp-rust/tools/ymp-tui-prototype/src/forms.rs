//! Editors and contextual operations; field values are drafts until the model accepts Save.
use crate::entities::Family;
use crossterm::event::KeyCode;
use unicode_segmentation::UnicodeSegmentation;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    Menu,
    BeginScenario,
    Answer,
    EditAnswer,
    ClearFilter,
    ToggleTool(String),
    AddProvider,
    EditProvider(String),
    EnableProvider(String, bool),
    RefreshProvider(String),
    RemoveProvider(String),
    AddTask,
    EditTask(String),
    SelectTask(String),
    StartTask(String),
    StopTask(String),
    RetryTask(String),
    CancelTask(String),
    DeleteTask(String),
    ArchiveTask(String, bool),
    AddAgent,
    StopAgent(String),
    RetryAgent(String),
    ArchiveAgent(String, bool),
    StopTool(String),
    Export {
        kind: String,
        id: String,
    },
    RemoveExport(String),
    Follow {
        kind: crate::resources::Kind,
        query: String,
    },
    ShowArchived,
    Limits,
    Quit,
}
#[derive(Clone)]
pub struct MenuItem {
    pub label: String,
    pub action: Action,
    pub reason: Option<String>,
}
impl MenuItem {
    pub fn new(label: impl Into<String>, action: Action, reason: Option<String>) -> Self {
        Self {
            label: label.into(),
            action,
            reason,
        }
    }
}
#[derive(Clone)]
pub struct Confirmation {
    pub title: String,
    pub detail: String,
    pub accept: String,
    pub action: Action,
    pub selected: bool,
}
#[derive(Clone)]
pub enum FormKind {
    Provider { id: Option<String> },
    Task { id: Option<String> },
    Agent { task: String },
    Limits,
    Answer { task: String },
}
#[derive(Clone)]
pub enum Value {
    Text {
        value: String,
        cursor: usize,
    },
    Choice {
        options: Vec<(String, String)>,
        selected: usize,
    },
}
#[derive(Clone)]
pub struct Field {
    pub label: String,
    pub value: Value,
}
impl Field {
    pub fn text(label: &str, value: &str) -> Self {
        Self {
            label: label.into(),
            value: Value::Text {
                value: value.into(),
                cursor: value.len(),
            },
        }
    }
    pub fn choice(label: &str, options: Vec<(String, String)>, selected: usize) -> Self {
        Self {
            label: label.into(),
            value: Value::Choice { options, selected },
        }
    }
    pub fn get(&self) -> &str {
        match &self.value {
            Value::Text { value, .. } => value,
            Value::Choice { options, selected } => options.get(*selected).map_or("", |(id, _)| id),
        }
    }
    pub fn display(&self) -> &str {
        match &self.value {
            Value::Text { value, .. } => value,
            Value::Choice { options, selected } => {
                options.get(*selected).map_or("—", |(_, label)| label)
            }
        }
    }
}
#[derive(Clone)]
pub struct Editor {
    pub title: String,
    pub context: String,
    pub kind: FormKind,
    pub fields: Vec<Field>,
    pub selected: usize,
    pub error: Option<String>,
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum EditorResult {
    None,
    Save,
    Cancel,
}
impl Editor {
    pub fn provider(id: Option<String>, name: &str, family: Family) -> Self {
        let mut fields = vec![
            Field::text("Name", name),
            Field::choice(
                "Type",
                Family::ALL
                    .iter()
                    .map(|f| (f.key().into(), f.name().into()))
                    .collect(),
                usize::from(family == Family::Anthropic),
            ),
        ];
        if id.is_none() {
            fields.push(Field::choice(
                "Enabled",
                vec![("no".into(), "No".into()), ("yes".into(), "Yes".into())],
                0,
            ));
        }
        Self {
            title: if id.is_none() {
                "Add provider"
            } else {
                "Edit provider"
            }
            .into(),
            context: "Model and runtime follow the supported profile.".into(),
            kind: FormKind::Provider { id },
            fields,
            selected: 0,
            error: None,
        }
    }
    pub fn task(id: Option<String>, request: &str, requirements: &str) -> Self {
        Self {
            title: if id.is_none() {
                "New task"
            } else {
                "Edit task"
            }
            .into(),
            context: "Saving a draft does not start agents.".into(),
            kind: FormKind::Task { id },
            fields: vec![
                Field::text("Request", request),
                Field::text("Requirements", requirements),
            ],
            selected: 0,
            error: None,
        }
    }
    pub fn limits(agents: usize, parallel: usize) -> Self {
        Self {
            title: "Run limits".into(),
            context: "Ceilings for future runs; existing run limits stay frozen.".into(),
            kind: FormKind::Limits,
            fields: vec![
                Field::text("Maximum agents", &agents.to_string()),
                Field::text("Concurrent attempts", &parallel.to_string()),
            ],
            selected: 0,
            error: None,
        }
    }
    pub fn agent(task: String, context: String, connections: Vec<(String, String)>) -> Self {
        Self {
            title: "Add agent".into(),
            context,
            kind: FormKind::Agent { task },
            fields: vec![
                Field::text("Assignment", ""),
                Field::choice("Connection", connections, 0),
            ],
            selected: 0,
            error: None,
        }
    }
    pub fn answer(task: String, draft: &str) -> Self {
        Self {
            title: "Task clarification".into(),
            context: "Requirements for the proposed task".into(),
            kind: FormKind::Answer { task },
            fields: vec![Field::text("Requirements", draft)],
            selected: 0,
            error: None,
        }
    }
    pub fn get(&self, index: usize) -> &str {
        self.fields.get(index).map_or("", Field::get)
    }
    pub fn paste(&mut self, text: &str) {
        if let Some(Field {
            value: Value::Text { value, cursor },
            ..
        }) = self.fields.get_mut(self.selected)
        {
            let clean: String = text
                .chars()
                .filter_map(|c| match c {
                    '\n' | '\r' | '\t' => Some(' '),
                    c if c.is_control() => None,
                    c => Some(c),
                })
                .collect();
            value.insert_str(*cursor, &clean);
            *cursor += clean.len();
            self.error = None;
        }
    }
    pub fn clear(&mut self) {
        if let Some(Field {
            value: Value::Text { value, cursor },
            ..
        }) = self.fields.get_mut(self.selected)
        {
            value.clear();
            *cursor = 0;
        }
    }
    pub fn key(&mut self, key: KeyCode) -> EditorResult {
        let positions = self.fields.len() + 2;
        match key {
            KeyCode::Esc => return EditorResult::Cancel,
            KeyCode::Tab | KeyCode::Down => {
                self.selected = (self.selected + 1) % positions;
                return EditorResult::None;
            }
            KeyCode::BackTab | KeyCode::Up => {
                self.selected = (self.selected + positions - 1) % positions;
                return EditorResult::None;
            }
            KeyCode::Enter => {
                if self.selected == self.fields.len() {
                    return EditorResult::Save;
                }
                if self.selected == self.fields.len() + 1 {
                    return EditorResult::Cancel;
                }
                self.selected += 1;
                return EditorResult::None;
            }
            _ => {}
        }
        if let Some(field) = self.fields.get_mut(self.selected) {
            match &mut field.value {
                Value::Choice { options, selected } => {
                    if !options.is_empty() {
                        match key {
                            KeyCode::Right | KeyCode::Char(' ') => {
                                *selected = (*selected + 1) % options.len()
                            }
                            KeyCode::Left => {
                                *selected = (*selected + options.len() - 1) % options.len()
                            }
                            _ => {}
                        }
                    }
                }
                Value::Text { value, cursor } => match key {
                    KeyCode::Char(c) if !c.is_control() => {
                        value.insert(*cursor, c);
                        *cursor += c.len_utf8();
                    }
                    KeyCode::Left => {
                        *cursor = value[..*cursor]
                            .grapheme_indices(true)
                            .next_back()
                            .map_or(0, |(i, _)| i)
                    }
                    KeyCode::Right => {
                        if let Some(g) = value[*cursor..].graphemes(true).next() {
                            *cursor += g.len();
                        }
                    }
                    KeyCode::Home => *cursor = 0,
                    KeyCode::End => *cursor = value.len(),
                    KeyCode::Backspace if *cursor > 0 => {
                        let previous = value[..*cursor]
                            .grapheme_indices(true)
                            .next_back()
                            .map_or(0, |(i, _)| i);
                        value.replace_range(previous..*cursor, "");
                        *cursor = previous;
                    }
                    KeyCode::Delete if *cursor < value.len() => {
                        if let Some(g) = value[*cursor..].graphemes(true).next() {
                            value.replace_range(*cursor..*cursor + g.len(), "");
                        }
                    }
                    _ => {}
                },
            }
        }
        EditorResult::None
    }
}
