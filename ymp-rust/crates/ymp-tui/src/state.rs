//! View state: where the operator is, what is selected, what floats above.
//!
//! Data and view state stay apart. Everything with a value in it lives in
//! [`crate::projection::Projection`]; this module holds only the operator's position — which
//! surface is shown, which row is selected, whether the transcript follows the journal, and
//! what has been typed. Nothing here reads the application.

use std::collections::HashMap;

use crate::pages::Page;
use crate::projection::Projection;

/// Whether the transcript is pinned to the newest entry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Follow {
    /// Pinned to the bottom: new entries appear immediately.
    Live,
    /// The operator scrolled away. Drawing is paused, the run is not.
    Paused,
}

/// The input line.
#[derive(Clone, Debug, Default)]
pub struct Prompt {
    pub buffer: String,
    /// Shown instead of the buffer when input is suspended by a modal or the palette.
    pub suspended: Option<String>,
}

/// One entry of the command palette.
#[derive(Clone, Debug)]
pub struct PaletteItem {
    pub name: String,
    pub description: String,
    pub command: Command,
}

/// The `/` command palette.
#[derive(Clone, Debug)]
pub struct Palette {
    pub input: String,
    pub selected: usize,
    pub items: Vec<PaletteItem>,
}

impl Palette {
    pub fn matches(&self) -> Vec<&PaletteItem> {
        self.items
            .iter()
            .filter(|item| {
                item.name
                    .starts_with(self.input.trim_start_matches(COMMAND_PREFIX))
            })
            .collect()
    }
}

/// The character that opens the command line and prefixes every command the interface names.
///
/// An operator arriving from Claude Code types `/` for a command, so the interface offers its
/// commands under the same key rather than under a second convention of its own.
pub const COMMAND_PREFIX: char = '/';

/// What a palette entry does when chosen.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Command {
    OpenPage(PageKind),
    /// Review the coverage of the contract at this position in the projection.
    Authorize(usize),
    CancelRun,
    Quit,
}

/// The full-screen data pages. Each one is populated from a projection or stated as empty.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum PageKind {
    Runtimes,
    Candidates,
    Events,
    Budgets,
    /// Attempts of the current run. POC-1 has no participants, so the page names what the
    /// domain records: attempts started under the run.
    Attempts,
    Describe,
}

impl PageKind {
    /// Every page the interface supports, in palette order.
    pub const ALL: [Self; 6] = [
        Self::Runtimes,
        Self::Candidates,
        Self::Events,
        Self::Budgets,
        Self::Attempts,
        Self::Describe,
    ];

    pub fn command_name(self) -> &'static str {
        match self {
            Self::Runtimes => "runtimes",
            Self::Candidates => "candidates",
            Self::Events => "events",
            Self::Budgets => "budgets",
            Self::Attempts => "attempts",
            Self::Describe => "describe",
        }
    }
}

/// The typed confirmation for an irreversible command.
#[derive(Clone, Debug)]
pub struct Confirm {
    pub title: String,
    pub badge: String,
    pub consequences: Vec<String>,
    pub prompt_label: String,
    /// The exact text the operator must type. Always an identifier the state produced.
    pub required: String,
    pub typed: String,
    pub confirm_hint: String,
    pub cancel_hint: String,
    pub action: ConfirmAction,
}

/// What an exact confirmation executes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ConfirmAction {
    /// Cancel the named run through the application.
    CancelRun { run_id: String },
    /// Store the named contract and start the run it names, through the application.
    StartRun { contract_id: String, run_id: String },
}

impl Confirm {
    pub fn is_exact(&self) -> bool {
        self.typed == self.required
    }
}

/// One requirement row of the authorization coverage map.
#[derive(Clone, Debug)]
pub struct Requirement {
    pub state: RequirementState,
    pub name: String,
    pub checked_by: String,
    pub negative_control: String,
    pub detail: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RequirementState {
    Covered,
    Warning,
    Blocking,
}

/// The contract-authorization decision: a coverage map, not contract prose.
#[derive(Clone, Debug)]
pub struct Authorize {
    pub contract: String,
    pub badge: String,
    pub facts: Vec<(String, String)>,
    pub requirements: Vec<Requirement>,
    pub footer_note: String,
    pub blocking: usize,
    /// What authorizing would start, when this contract can start a run. `None` keeps the map
    /// reviewable and states why the action is unavailable instead of drawing it as enabled.
    pub action: Option<AuthorizeAction>,
    /// Why the action is unavailable, when it is.
    pub action_note: String,
}

/// The run an authorization would start.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthorizeAction {
    pub contract_id: String,
    pub run_id: String,
    /// The dimensions the run would start with, named and numbered by the projection.
    pub budget: Vec<(String, u32)>,
    pub source: String,
    pub verifier: String,
    pub negative_control: String,
}

/// What floats above the current surface.
#[derive(Clone, Debug)]
pub enum Modal {
    None,
    Palette(Palette),
    Authorize(Authorize),
    Confirm(Confirm),
    Keys,
}

/// Which full surface fills the frame.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Surface {
    Transcript,
    Page(PageKind),
}

/// The projection plus the operator's position in it.
#[derive(Clone, Debug)]
pub struct App {
    pub data: Projection,
    pub prompt: Prompt,
    pub follow: Follow,
    /// Offset from the bottom of the laid-out transcript, in lines.
    pub scroll: usize,
    pub surface: Surface,
    pub modal: Modal,
    pub should_quit: bool,
    /// Journal position shown while the transcript is scrolled back.
    pub viewing_around: Option<String>,
    /// Which candidate the describe surface is showing.
    pub describe_index: Option<usize>,
    /// Row selections, kept across projection rebuilds.
    selection: HashMap<PageKind, usize>,
}

impl App {
    pub fn new(data: Projection) -> Self {
        let mut app = Self {
            data: Projection::default(),
            prompt: Prompt::default(),
            follow: Follow::Live,
            scroll: 0,
            surface: Surface::Transcript,
            modal: Modal::None,
            should_quit: false,
            viewing_around: None,
            describe_index: None,
            selection: HashMap::new(),
        };
        app.adopt(data);
        app
    }

    /// Take a rebuilt projection and re-apply the operator's position within it.
    pub fn adopt(&mut self, mut data: Projection) {
        for (kind, page) in &mut data.pages {
            if let Some(stored) = self.selection.get(kind) {
                page.selected = (*stored).min(page.rows_len().saturating_sub(1));
            }
        }
        self.data = data;
        // A page that vanished takes the operator back to the transcript rather than to a
        // surface with nothing behind it.
        if let Surface::Page(kind) = self.surface
            && self.data.page(kind).is_none()
        {
            self.surface = Surface::Transcript;
        }
    }

    pub fn selection_of(&self, kind: PageKind) -> usize {
        self.data.page(kind).map_or(0, |page| page.selected)
    }

    pub fn select_next(&mut self, kind: PageKind) {
        if let Some(page) = self.data.page_mut(kind) {
            page.select_next();
            let selected = page.selected;
            self.selection.insert(kind, selected);
        }
    }

    pub fn select_prev(&mut self, kind: PageKind) {
        if let Some(page) = self.data.page_mut(kind) {
            page.select_prev();
            let selected = page.selected;
            self.selection.insert(kind, selected);
        }
    }

    /// Open the coverage map for the first contract the projection carries.
    pub fn open_authorize(&mut self) {
        self.open_authorize_at(0);
    }

    /// Open the coverage map for one contract of the projection.
    pub fn open_authorize_at(&mut self, index: usize) {
        let (Some(contract), Some(environment)) = (
            self.data.contracts.get(index),
            self.data.environment.as_ref(),
        ) else {
            return;
        };
        self.prompt.suspended = Some("decision open — input suspended".into());
        self.modal = Modal::Authorize(crate::decisions::authorize(
            contract,
            environment,
            self.data.runtimes.as_ref(),
            self.data.run.as_ref(),
        ));
    }

    /// Open the typed confirmation that ends the live run.
    pub fn open_cancel_confirm(&mut self) {
        let Some(run) = self.data.run.as_ref().filter(|run| run.is_live()) else {
            return;
        };
        self.prompt.suspended = Some("decision open — input suspended".into());
        self.modal = Modal::Confirm(crate::decisions::cancel_run(run));
    }

    /// The palette over the commands the current state actually offers. Quit is always there,
    /// so the operator can always leave.
    pub fn open_palette(&self) -> Palette {
        let mut items = self.data.commands.clone();
        if !items.iter().any(|item| item.command == Command::Quit) {
            items.push(PaletteItem {
                name: "quit".into(),
                description: "leave ymp — the run state stays journaled".into(),
                command: Command::Quit,
            });
        }
        items.sort_by(|a, b| a.name.cmp(&b.name));
        Palette {
            input: COMMAND_PREFIX.to_string(),
            selected: 0,
            items,
        }
    }

    pub fn page(&self, kind: PageKind) -> Option<&Page> {
        self.data.page(kind)
    }

    pub fn page_mut(&mut self, kind: PageKind) -> Option<&mut Page> {
        self.data.page_mut(kind)
    }

    /// Scrolling up leaves live mode: the transcript stops moving, the run does not.
    pub fn scroll_up(&mut self, lines: usize) {
        self.scroll = self.scroll.saturating_add(lines);
        self.follow = Follow::Paused;
    }

    pub fn scroll_down(&mut self, lines: usize) {
        self.scroll = self.scroll.saturating_sub(lines);
        if self.scroll == 0 {
            self.follow = Follow::Live;
        }
    }

    pub fn resume_live(&mut self) {
        self.scroll = 0;
        self.follow = Follow::Live;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scrolling_up_pauses_follow_and_returning_resumes_it() {
        let mut app = App::new(Projection::default());
        assert_eq!(app.follow, Follow::Live);
        app.scroll_up(12);
        assert_eq!(app.follow, Follow::Paused);
        app.scroll_down(12);
        assert_eq!(app.follow, Follow::Live);
    }

    #[test]
    fn the_palette_filters_by_prefix_and_always_offers_quit() {
        let mut app = App::new(Projection::default());
        app.data.commands = vec![
            PaletteItem {
                name: "candidates".into(),
                description: String::new(),
                command: Command::OpenPage(PageKind::Candidates),
            },
            PaletteItem {
                name: "cancel run".into(),
                description: String::new(),
                command: Command::CancelRun,
            },
        ];
        let mut palette = app.open_palette();
        assert!(palette.items.iter().any(|item| item.name == "quit"));
        assert_eq!(palette.input, "/");
        palette.input = "/c".into();
        let names: Vec<&str> = palette
            .matches()
            .iter()
            .map(|item| item.name.as_str())
            .collect();
        assert_eq!(names, vec!["cancel run", "candidates"]);
    }

    #[test]
    fn typed_confirmation_requires_an_exact_match() {
        let mut confirm = Confirm {
            title: "cancel run demo-run".into(),
            badge: "irreversible".into(),
            consequences: Vec::new(),
            prompt_label: "type the run id to confirm:".into(),
            required: "demo-run".into(),
            typed: "demo".into(),
            confirm_hint: "confirm — disabled until exact match".into(),
            cancel_hint: "keep running".into(),
            action: ConfirmAction::CancelRun {
                run_id: "demo-run".into(),
            },
        };
        assert!(!confirm.is_exact());
        confirm.typed = "demo-run".into();
        assert!(confirm.is_exact());
    }
}
