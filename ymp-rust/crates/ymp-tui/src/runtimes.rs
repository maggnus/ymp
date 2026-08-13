//! Runtime readiness, projected from real probes.
//!
//! The drivers the workspace actually ships are the only profiles listed: the fake in-process
//! runtime, Codex and Claude Code. Profiles the design artifact draws but no driver implements
//! are absent rather than shown as blocked, because a blocked row would claim the product knows
//! something about a runtime it cannot start.
//!
//! Probing runs the runtime executable, so it happens once on a worker thread. Until the report
//! arrives the page carries the `probing` state marker instead of an empty table.

use ymp_runtime_api::{Readiness, RuntimeDriver, RuntimeKind};
use ymp_runtime_claude::ClaudeRuntime;
use ymp_runtime_codex::CodexRuntime;
use ymp_runtime_fake::FakeRuntime;

use crate::pages::{Body, Cell, Column, Page, Row};
use crate::style;
use crate::theme;

/// One runtime profile as the probe found it.
#[derive(Clone, Debug)]
pub struct ProfileFacts {
    pub name: String,
    pub runtime: String,
    /// The model route pinned by the driver profile, when the driver routes to a model.
    pub model_route: Option<String>,
    pub executable: String,
    pub version: Option<String>,
    pub readiness: Readiness,
    pub detail: String,
}

impl ProfileFacts {
    pub fn ready(&self) -> bool {
        self.readiness == Readiness::Ready
    }

    fn readiness_text(&self) -> &'static str {
        match self.readiness {
            Readiness::Ready => "ready",
            Readiness::NotInstalled => "not installed",
            Readiness::Unauthenticated => "unauthenticated",
            Readiness::Incompatible => "incompatible",
            Readiness::Unavailable => "unavailable",
        }
    }
}

/// The completed probe of every shipped driver.
#[derive(Clone, Debug)]
pub struct Report {
    pub profiles: Vec<ProfileFacts>,
}

impl Report {
    pub fn ready_count(&self) -> usize {
        self.profiles.iter().filter(|p| p.ready()).count()
    }
}

/// Probe every shipped driver. Runs subprocesses; call it off the drawing thread.
pub fn probe_all() -> Report {
    let fake = FakeRuntime::default();
    let codex = CodexRuntime::default();
    let claude = ClaudeRuntime::default();

    let codex_route = codex.profile().model.clone();
    let claude_route = claude.profile().model.clone();

    let profiles = vec![
        facts("fake", &fake, None),
        facts("codex", &codex, Some(codex_route)),
        facts("claude-code", &claude, Some(claude_route)),
    ];
    Report { profiles }
}

fn facts(name: &str, driver: &dyn RuntimeDriver, model_route: Option<String>) -> ProfileFacts {
    match driver.probe() {
        Ok(report) => ProfileFacts {
            name: name.to_owned(),
            runtime: runtime_label(report.kind).to_owned(),
            model_route,
            executable: report.executable,
            version: report.version,
            readiness: report.readiness,
            detail: report.detail,
        },
        Err(error) => ProfileFacts {
            name: name.to_owned(),
            runtime: runtime_label(driver.kind()).to_owned(),
            model_route,
            executable: driver.executable().display().to_string(),
            version: None,
            readiness: Readiness::Unavailable,
            detail: error.to_string(),
        },
    }
}

fn runtime_label(kind: RuntimeKind) -> &'static str {
    match kind {
        RuntimeKind::Fake => "in-process",
        RuntimeKind::Codex => "codex",
        RuntimeKind::ClaudeCode => "claude-code",
    }
}

/// The `/runtimes` page. Without a report the page states that the probe is still running.
pub fn page(report: Option<&Report>, status: String) -> Page {
    let columns = vec![
        Column {
            title: "PROFILE",
            width: 16,
        },
        Column {
            title: "RUNTIME",
            width: 14,
        },
        Column {
            title: "MODEL ROUTE",
            width: 26,
        },
        Column {
            title: "VERSION",
            width: 12,
        },
        Column {
            title: "STATE",
            width: 0,
        },
    ];

    let Some(report) = report else {
        return Page {
            breadcrumb: vec!["transcript".into(), "runtimes(all) · probing…".into()],
            summary: Vec::new(),
            body: Body::Table {
                columns,
                rows: Vec::new(),
            },
            notes: vec![
                "probing each shipped driver — a profile appears as soon as its probe returns"
                    .into(),
            ],
            footer: style::spans(&status, theme::muted()),
            keys: vec![("Esc", "back")],
            selected: 0,
        };
    };

    let rows = report
        .profiles
        .iter()
        .map(|profile| Row {
            cells: vec![
                Cell::new(profile.name.clone(), theme::bold()),
                Cell::new(profile.runtime.clone(), theme::dim()),
                Cell::new(
                    profile
                        .model_route
                        .clone()
                        .unwrap_or_else(|| "—".to_owned()),
                    theme::muted(),
                ),
                Cell::new(
                    profile.version.clone().unwrap_or_else(|| "—".to_owned()),
                    theme::muted(),
                ),
                Cell::new(
                    profile.readiness_text(),
                    if profile.ready() {
                        theme::green()
                    } else {
                        theme::red()
                    },
                ),
            ],
            fix: if profile.ready() {
                None
            } else {
                Some(style::spans(
                    &format!("↳ {} · {}", profile.detail, profile.executable),
                    theme::muted(),
                ))
            },
            dim: false,
        })
        .collect::<Vec<_>>();

    let ready = report.ready_count();
    let unusable = report.profiles.len() - ready;
    Page {
        breadcrumb: vec![
            "transcript".into(),
            format!(
                "runtimes(all)[{}] · {ready} ready · {unusable} unusable",
                report.profiles.len()
            ),
        ],
        summary: Vec::new(),
        body: Body::Table { columns, rows },
        notes: vec![
            "readiness is the driver's own probe of this host; the fix line names what the \
             probe reported"
                .into(),
            "only shipped drivers are listed — a profile with no driver is unavailable, not \
             blocked"
                .into(),
        ],
        footer: style::spans(&status, theme::muted()),
        keys: vec![("Esc", "back")],
        selected: 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn report() -> Report {
        Report {
            profiles: vec![
                ProfileFacts {
                    name: "fake".into(),
                    runtime: "in-process".into(),
                    model_route: None,
                    executable: "ymp-internal-fake".into(),
                    version: Some("0.1.0".into()),
                    readiness: Readiness::Ready,
                    detail: "deterministic in-process runtime".into(),
                },
                ProfileFacts {
                    name: "codex".into(),
                    runtime: "codex".into(),
                    model_route: Some("openai/o4".into()),
                    executable: "/usr/bin/codex".into(),
                    version: None,
                    readiness: Readiness::NotInstalled,
                    detail: "executable not found".into(),
                },
            ],
        }
    }

    #[test]
    fn an_unusable_profile_carries_the_probe_detail_as_a_fix_line() {
        let page = page(Some(&report()), "idle".into());
        let Body::Table { rows, .. } = &page.body else {
            panic!("expected a table");
        };
        assert!(rows[0].fix.is_none());
        let fix = rows[1].fix.as_ref().expect("fix line");
        let text: String = fix.iter().map(|span| span.content.as_ref()).collect();
        assert!(text.contains("executable not found"), "{text}");
    }

    #[test]
    fn a_missing_report_states_the_probe_is_running_instead_of_an_empty_table() {
        let page = page(None, "idle".into());
        assert!(
            page.breadcrumb[1].contains("probing"),
            "{:?}",
            page.breadcrumb
        );
    }
}
