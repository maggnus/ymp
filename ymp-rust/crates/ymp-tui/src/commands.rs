//! The command registry.
//!
//! Every slash command is declared once, with its arguments, its summary and the group it
//! belongs to. Completion, the command palette and the help page all read this table, so a
//! new command cannot appear in one surface and be missing from another.

/// Where a command belongs in the help page and the palette.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Group {
    Conversation,
    Navigate,
    Team,
    Knowledge,
    Appearance,
    System,
}

impl Group {
    pub fn title(self) -> &'static str {
        match self {
            Group::Conversation => "Conversation",
            Group::Navigate => "Navigate",
            Group::Team => "Team and providers",
            Group::Knowledge => "Knowledge and evidence",
            Group::Appearance => "Appearance",
            Group::System => "Session control",
        }
    }
    pub fn all() -> &'static [Group] {
        &[
            Group::Conversation,
            Group::Navigate,
            Group::Team,
            Group::Knowledge,
            Group::Appearance,
            Group::System,
        ]
    }
}

/// Whether typing the bare command name is already a complete instruction.
///
/// This is what decides what Enter does. A command with no arguments, or with only
/// optional ones, runs on a single Enter; only a command that cannot act without an
/// argument leaves a draft in the composer for the argument to be typed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Args {
    None,
    Optional,
    Required,
}

#[derive(Clone, Copy, Debug)]
pub struct Command {
    pub name: &'static str,
    pub usage: &'static str,
    pub summary: &'static str,
    pub group: Group,
    pub args: Args,
}

impl Command {
    /// Does the bare name execute on its own?
    pub fn complete_alone(&self) -> bool {
        self.args != Args::Required
    }
    /// Should completing the name leave room for an argument?
    pub fn wants_argument(&self) -> bool {
        self.args != Args::None
    }
}

pub const COMMANDS: &[Command] = &[
    Command {
        name: "/chat",
        usage: "/chat",
        summary: "Return to the conversation.",
        group: Group::Conversation,
        args: Args::None,
    },
    Command {
        name: "/new",
        usage: "/new",
        summary: "Start an unrelated task. The next prompt opens a new session.",
        group: Group::Conversation,
        args: Args::None,
    },
    Command {
        name: "/details",
        usage: "/details",
        summary: "Toggle full attributed agent messages instead of collapsed activity.",
        group: Group::Conversation,
        args: Args::None,
    },
    Command {
        name: "/help",
        usage: "/help",
        summary: "Command reference and keyboard map.",
        group: Group::Navigate,
        args: Args::None,
    },
    Command {
        name: "/tasks",
        usage: "/tasks",
        summary: "Task graph, assignments and outcomes for the current session.",
        group: Group::Navigate,
        args: Args::None,
    },
    Command {
        name: "/sessions",
        usage: "/sessions",
        summary: "Saved sessions for this project. Opening one only reads it.",
        group: Group::Navigate,
        args: Args::None,
    },
    Command {
        name: "/files",
        usage: "/files",
        summary: "Files in the working directory.",
        group: Group::Navigate,
        args: Args::None,
    },
    Command {
        name: "/diff",
        usage: "/diff",
        summary: "Files created, modified or deleted during the loaded session.",
        group: Group::Navigate,
        args: Args::None,
    },
    Command {
        name: "/providers",
        usage: "/providers",
        summary: "Configured local providers and their availability.",
        group: Group::Team,
        args: Args::None,
    },
    Command {
        name: "/agents",
        usage: "/agents",
        summary: "Agent profiles, with model and instruction editing.",
        group: Group::Team,
        args: Args::None,
    },
    Command {
        name: "/agent",
        usage: "/agent add ID PROVIDER [MODEL] | model ID MODEL | instructions ID TEXT | ID",
        summary: "Create or edit an agent profile, or inspect one.",
        group: Group::Team,
        args: Args::Required,
    },
    Command {
        name: "/team",
        usage: "/team [add ID | remove ID]",
        summary: "Team membership for new sessions.",
        group: Group::Team,
        args: Args::Optional,
    },
    Command {
        name: "/limits",
        usage: "/limits [turns N | parallel N | attempts N | timeout SECONDS]",
        summary: "Turn budget and parallelism for the next run.",
        group: Group::Team,
        args: Args::Optional,
    },
    Command {
        name: "/memory",
        usage: "/memory [QUERY] | /memory forget ID",
        summary: "Search verified project and global knowledge, or retire an entry.",
        group: Group::Knowledge,
        args: Args::Optional,
    },
    Command {
        name: "/reputation",
        usage: "/reputation",
        summary: "Evidence behind competence estimates.",
        group: Group::Knowledge,
        args: Args::None,
    },
    Command {
        name: "/theme",
        usage: "/theme [ID]",
        summary: "Choose a colour theme. The choice is remembered.",
        group: Group::Appearance,
        args: Args::Optional,
    },
    Command {
        name: "/sidebar",
        usage: "/sidebar",
        summary: "Show or hide the context sidebar.",
        group: Group::Appearance,
        args: Args::None,
    },
    Command {
        name: "/resume",
        usage: "/resume [SESSION_ID]",
        summary: "Continue an interrupted run. This starts agents.",
        group: Group::System,
        args: Args::Optional,
    },
    Command {
        name: "/pause",
        usage: "/pause",
        summary: "Stop active turns and keep the session resumable.",
        group: Group::System,
        args: Args::None,
    },
    Command {
        name: "/stop",
        usage: "/stop",
        summary: "Stop active turns immediately.",
        group: Group::System,
        args: Args::None,
    },
    Command {
        name: "/quit",
        usage: "/quit",
        summary: "Leave ymp.",
        group: Group::System,
        args: Args::None,
    },
];

/// Commands whose name starts with `prefix`, for inline completion.
pub fn matching(prefix: &str) -> Vec<&'static Command> {
    COMMANDS
        .iter()
        .filter(|c| c.name.starts_with(prefix))
        .collect()
}

/// Exact lookup by command name, with or without the leading slash.
pub fn find(name: &str) -> Option<&'static Command> {
    let name = name.strip_prefix('/').unwrap_or(name);
    COMMANDS.iter().find(|c| &c.name[1..] == name)
}

/// Palette search: a case-insensitive subsequence match over the name, then the summary.
/// An empty query lists everything so the palette is also a browsable index.
pub fn search(query: &str) -> Vec<&'static Command> {
    let query = query.trim().trim_start_matches('/').to_ascii_lowercase();
    if query.is_empty() {
        return COMMANDS.iter().collect();
    }
    let mut exact: Vec<&'static Command> = Vec::new();
    let mut loose: Vec<&'static Command> = Vec::new();
    for command in COMMANDS {
        let name = command.name[1..].to_ascii_lowercase();
        if name.starts_with(&query) {
            exact.push(command);
        } else if subsequence(&query, &name)
            || command.summary.to_ascii_lowercase().contains(&query)
        {
            loose.push(command);
        }
    }
    exact.extend(loose);
    exact
}

fn subsequence(needle: &str, haystack: &str) -> bool {
    let mut chars = haystack.chars();
    needle.chars().all(|c| chars.any(|h| h == c))
}

/// The keyboard map, shown on the help page and in the keys overlay.
pub const KEYS: &[(&str, &str)] = &[
    ("Enter", "Send the prompt, or activate the selected row"),
    ("Ctrl+J", "Insert a newline in the composer"),
    ("Ctrl+U", "Clear whichever field owns the keyboard"),
    (
        "Tab",
        "Complete a command, or move focus to the next region",
    ),
    ("Shift+Tab", "Move focus to the previous region"),
    ("Ctrl+P", "Open the command palette"),
    (
        "Esc",
        "Close the topmost surface, then return focus to the composer",
    ),
    (
        "Up / Down",
        "Move the selection, or recall composer history",
    ),
    (
        "PageUp / PageDown",
        "Scroll the transcript or the current list",
    ),
    ("Home / End", "Jump to the oldest or the latest message"),
    ("Ctrl+T", "Open the theme chooser"),
    ("Ctrl+B", "Show or hide the context sidebar"),
    ("Ctrl+L", "Toggle detailed agent messages"),
    ("Ctrl+C", "Stop an active run, or leave ymp when idle"),
    ("Ctrl+D", "Leave ymp when the composer is empty"),
];
