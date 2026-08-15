//! Runtime engines as the registry holds them, and readiness as their probes report it.
//!
//! Two different things decide whether a run can be routed to an engine, and the page keeps them
//! apart. The **registry** holds the operator's decision — an engine is enabled or it is disabled
//! with a stated reason — together with what was measured about it and the models it can serve.
//! The **probe** answers what this host can start right now. An engine the operator disabled is
//! not probed at all: nothing about it is started, so it can be neither offered nor routed to,
//! and the row states the registry's reason rather than a readiness nobody measured.
//!
//! The drivers the workspace actually ships are the only profiles listed: the fake in-process
//! runtime, Codex and Claude Code. The fixture is not an engine — it has no executable, no
//! credential and no models — so it carries no registry row and is never routed to.
//!
//! Probing runs the engine executable, so it happens once on a worker thread. Measuring the model
//! catalog starts that executable once per candidate, so it happens only where the catalog is the
//! subject ([`Measure::Catalog`]) and only while the recorded list is not the one this build
//! serves. Everywhere else the recorded list is read.

use std::path::Path;

use ymp_runtime_api::{Readiness, RuntimeDriver, RuntimeKind};
use ymp_runtime_claude::ClaudeRuntime;
use ymp_runtime_codex::CodexRuntime;
use ymp_runtime_fake::FakeRuntime;
use ymp_runtime_registry::{
    Engine, EngineProperties, EngineRecord, ModelCatalog, ModelSource, Registry,
};

use crate::pages::{Body, Cell, Column, Page, Row};
use crate::style;
use crate::theme;

/// Whether this pass measures the model catalog or reads the one the registry recorded.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Measure {
    /// Read the recorded model list. Readiness is still probed; nothing is measured per model.
    Recorded,
    /// Measure the model list where the recorded one was not measured against the installed build.
    Catalog,
}

/// The registry's state of one engine, as the page and the routing read it.
#[derive(Clone, Debug)]
pub struct EngineFacts {
    pub engine: Engine,
    pub enabled: bool,
    /// Why the engine is disabled, when it is.
    pub disabled_reason: Option<String>,
    pub models: ModelCatalog,
}

/// One runtime profile as the registry holds it and the probe found it.
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
    /// The registry row of this profile. The fixture runtime is not an engine and carries none.
    pub registry: Option<EngineFacts>,
}

impl ProfileFacts {
    /// Whether the registry admits this profile. A profile with no registry row — the fixture — is
    /// admitted by nothing and refused by nothing; it is simply not an engine.
    pub fn admitted(&self) -> bool {
        self.registry
            .as_ref()
            .map(|registry| registry.enabled)
            .unwrap_or(true)
    }

    /// Whether a run can be routed here: the registry admits the engine and its probe found it
    /// ready. A disabled engine is never ready, whatever a probe would have reported.
    pub fn ready(&self) -> bool {
        self.admitted() && self.readiness == Readiness::Ready
    }

    /// What the registry and the probe found, as one word.
    pub fn readiness_text(&self) -> &'static str {
        if !self.admitted() {
            return "disabled";
        }
        match self.readiness {
            Readiness::Ready => "ready",
            Readiness::NotInstalled => "not installed",
            Readiness::Unauthenticated => "unauthenticated",
            Readiness::Incompatible => "incompatible",
            Readiness::Unavailable => "unavailable",
        }
    }

    /// The enabled flag as the page states it.
    pub fn enabled_text(&self) -> &'static str {
        match &self.registry {
            None => "—",
            Some(registry) if registry.enabled => "yes",
            Some(_) => "no",
        }
    }

    /// The model list as one cell: how many routes there are and where the list came from.
    pub fn models_text(&self) -> String {
        match &self.registry {
            None => "—".to_owned(),
            Some(registry) => match registry.models.source {
                ModelSource::Unmeasured => "unmeasured".to_owned(),
                source => format!("{} {}", registry.models.names.len(), source.label()),
            },
        }
    }
}

/// The completed reading of every shipped driver.
#[derive(Clone, Debug)]
pub struct Report {
    pub profiles: Vec<ProfileFacts>,
}

impl Report {
    pub fn ready_count(&self) -> usize {
        self.profiles.iter().filter(|p| p.ready()).count()
    }

    pub fn profile(&self, name: &str) -> Option<&ProfileFacts> {
        self.profiles.iter().find(|profile| profile.name == name)
    }

    /// The engine at one row of the page, when that row is an engine at all.
    pub fn engine_at(&self, row: usize) -> Option<Engine> {
        self.profiles
            .get(row)
            .and_then(|profile| profile.registry.as_ref())
            .map(|registry| registry.engine)
    }
}

/// Read the registry and probe every engine it admits. Runs subprocesses; call it off the drawing
/// thread.
///
/// `root` is the root or the store the invocation addressed. Which registry that reaches is
/// derived from the path itself: a store standing under a root reads the registry that root holds,
/// whichever way the invocation named it.
pub fn probe_all(root: &Path, measure: Measure) -> Report {
    let registry = Registry::addressing(root);
    let mut profiles = vec![fixture_facts()];
    for engine in Engine::ALL {
        profiles.push(engine_facts(&registry, engine, measure));
    }
    Report { profiles }
}

fn fixture_facts() -> ProfileFacts {
    let fake = FakeRuntime::default();
    let mut facts = facts("fake", &fake, None);
    facts.registry = None;
    facts
}

/// One engine: what the registry holds, and — only where it admits the engine — what its probe
/// reports and what the record then records.
fn engine_facts(registry: &Registry, engine: Engine, measure: Measure) -> ProfileFacts {
    let record = match registry.read(engine) {
        Ok(record) => record,
        // A record that cannot be read is not an enabled engine. Nothing is probed and nothing is
        // routed here, and the row states the registry error rather than a readiness.
        Err(error) => {
            return ProfileFacts {
                name: engine.name().to_owned(),
                runtime: engine.name().to_owned(),
                model_route: None,
                executable: String::new(),
                version: None,
                readiness: Readiness::Unavailable,
                detail: error.to_string(),
                registry: Some(EngineFacts {
                    engine,
                    enabled: false,
                    disabled_reason: Some(error.to_string()),
                    models: ModelCatalog::default(),
                }),
            };
        }
    };
    if !record.enabled {
        // The decision is materialised even though nothing is measured, so the registry holds a
        // record for every engine and an operator can read — and edit — what holds this one back.
        record_engine(registry, engine, &record);
        return disabled_facts(engine, &record);
    }
    match engine {
        Engine::ClaudeCode => claude_facts(registry, record, measure),
        Engine::Codex => codex_facts(registry, record),
    }
}

/// A disabled engine, stated from the record alone. Nothing about it is started.
fn disabled_facts(engine: Engine, record: &EngineRecord) -> ProfileFacts {
    ProfileFacts {
        name: engine.name().to_owned(),
        runtime: engine.name().to_owned(),
        model_route: None,
        executable: record.properties.executable.clone().unwrap_or_default(),
        version: record.properties.version.clone(),
        readiness: Readiness::Unavailable,
        detail: format!(
            "disabled in the registry — {}. Nothing was probed and nothing is offered.",
            record.refusal_reason()
        ),
        registry: Some(EngineFacts {
            engine,
            enabled: false,
            disabled_reason: Some(record.refusal_reason()),
            models: record.models.clone(),
        }),
    }
}

fn claude_facts(registry: &Registry, mut record: EngineRecord, measure: Measure) -> ProfileFacts {
    let runtime = ClaudeRuntime::default();
    let profile = runtime.profile().clone();
    let mut facts = facts(
        Engine::ClaudeCode.name(),
        &runtime,
        Some(profile.model.clone()),
    );
    // The digest is computed from the installed file, so it is what says whether a recorded list
    // still describes what is installed. A record's own account of the release it measured decides
    // nothing: a forged one would otherwise suppress the re-measurement that would expose it.
    let digest = runtime.executable_digest().ok();
    record.properties = EngineProperties {
        executable: Some(facts.executable.clone()),
        version: facts.version.clone(),
        executable_digest: digest.clone(),
        credential_origin: runtime.credential_origin().map(str::to_owned),
        max_budget_microusd: Some(profile.max_budget_microusd),
        max_in_flight_overshoot_microusd: Some(profile.max_in_flight_overshoot_microusd),
        wall_time_limit_ms: Some(profile.wall_time_limit_ms),
        output_limit_bytes: Some(profile.output_limit_bytes as u64),
    };
    let current = digest
        .as_deref()
        .is_some_and(|digest| record.models.current_for(digest));
    if measure == Measure::Catalog && facts.readiness == Readiness::Ready && !current {
        record.models = match runtime.measure_model_catalog() {
            Ok(measured) => ModelCatalog {
                source: match measured.unasked {
                    0 => ModelSource::Measured,
                    _ => ModelSource::Filtered,
                },
                measured_for_version: facts.version.clone(),
                measured_for_digest: digest,
                note: (measured.unasked > 0).then(|| {
                    format!(
                        "{} candidates this build carries were not put to it, so this list is \
                         part of what it serves rather than all of it",
                        measured.unasked
                    )
                }),
                names: measured.served,
            },
            Err(error) => ModelCatalog {
                source: ModelSource::Unmeasured,
                measured_for_version: None,
                measured_for_digest: None,
                note: Some(format!("the model catalog was not measured: {error}")),
                names: Vec::new(),
            },
        };
    }
    record_engine(registry, Engine::ClaudeCode, &record);
    facts.registry = Some(EngineFacts {
        engine: Engine::ClaudeCode,
        enabled: true,
        disabled_reason: None,
        models: record.models,
    });
    facts
}

fn codex_facts(registry: &Registry, mut record: EngineRecord) -> ProfileFacts {
    let runtime = CodexRuntime::default();
    let profile = runtime.profile().clone();
    let mut facts = facts(Engine::Codex.name(), &runtime, Some(profile.model.clone()));
    record.properties = EngineProperties {
        executable: Some(facts.executable.clone()),
        version: facts.version.clone(),
        executable_digest: runtime.executable_digest().ok(),
        credential_origin: runtime.credential_origin().map(str::to_owned),
        max_budget_microusd: None,
        max_in_flight_overshoot_microusd: None,
        wall_time_limit_ms: Some(profile.wall_time_limit_ms),
        output_limit_bytes: Some(profile.output_limit_bytes as u64),
    };
    // The installed build lists no routes of its own and the account answers only at request time,
    // so where the engine is reachable the record states the pinned route and calls it pinned.
    // Nothing here is called a measurement. Where the probe did not reach the engine, no route is
    // recorded at all: a list nobody could confirm is not a list of what this host can serve.
    record.models = match facts.readiness == Readiness::Ready {
        true => ModelCatalog {
            source: ModelSource::Pinned,
            measured_for_version: facts.version.clone(),
            measured_for_digest: None,
            note: Some(
                "this build lists no catalog; the route the managed profile pins is recorded \
                 instead"
                    .to_owned(),
            ),
            names: runtime.model_catalog(),
        },
        false => ModelCatalog {
            source: ModelSource::Unmeasured,
            measured_for_version: None,
            measured_for_digest: None,
            note: Some(format!(
                "no route was recorded — the probe did not reach the engine: {}",
                facts.detail
            )),
            names: Vec::new(),
        },
    };
    record_engine(registry, Engine::Codex, &record);
    facts.registry = Some(EngineFacts {
        engine: Engine::Codex,
        enabled: true,
        disabled_reason: None,
        models: record.models,
    });
    facts
}

/// Write what was measured back into the record. A registry that cannot be written is not a reason
/// to refuse the reading that was already taken, so the failure is carried in the record's own note
/// on the next read rather than raised here.
fn record_engine(registry: &Registry, engine: Engine, record: &EngineRecord) {
    let _ = registry.update(engine, |stored| {
        stored.enabled = record.enabled;
        stored.disabled_reason = record.disabled_reason.clone();
        stored.properties = record.properties.clone();
        stored.models = record.models.clone();
    });
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
            registry: None,
        },
        Err(error) => ProfileFacts {
            name: name.to_owned(),
            runtime: runtime_label(driver.kind()).to_owned(),
            model_route,
            executable: driver.executable().display().to_string(),
            version: None,
            readiness: Readiness::Unavailable,
            detail: error.to_string(),
            registry: None,
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
            title: "ENABLED",
            width: 9,
        },
        Column {
            title: "MODEL ROUTE",
            width: 24,
        },
        Column {
            title: "MODELS",
            width: 14,
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
                "reading the registry and probing each engine it admits — a profile appears as \
                 soon as its probe returns"
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
                Cell::new(
                    profile.enabled_text(),
                    if profile.admitted() {
                        theme::dim()
                    } else {
                        theme::red()
                    },
                ),
                Cell::new(
                    profile
                        .model_route
                        .clone()
                        .unwrap_or_else(|| "—".to_owned()),
                    theme::muted(),
                ),
                Cell::new(profile.models_text(), theme::muted()),
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
    let mut notes = vec![
        "an engine is enabled or disabled here; a disabled engine is not probed, not offered and \
         not routed to"
            .to_owned(),
        "readiness is the engine's own probe of this host; the fix line names what the registry or \
         the probe reported"
            .to_owned(),
    ];
    notes.extend(report.profiles.iter().filter_map(catalog_note));
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
        notes,
        footer: style::spans(&status, theme::muted()),
        keys: vec![("Enter", "enable/disable"), ("Esc", "back")],
        selected: 0,
    }
}

/// What one engine can serve, named model by model. An engine with no measured list says so
/// instead of leaving the reader to read an empty column as an empty catalog.
fn catalog_note(profile: &ProfileFacts) -> Option<String> {
    let registry = profile.registry.as_ref()?;
    let models = &registry.models;
    if models.names.is_empty() {
        // A disabled engine says why it is held back. That is what an operator reading an empty
        // catalog needs, and a note about how the list was recorded would stand in its place.
        let reason = registry
            .disabled_reason
            .clone()
            .or_else(|| models.note.clone())
            .unwrap_or_else(|| "the engine has not been measured on this host".to_owned());
        return Some(format!("{} · no model list — {reason}", profile.name));
    }
    let measured_for = models
        .measured_for_version
        .clone()
        .unwrap_or_else(|| "an unrecorded build".to_owned());
    let count = models.names.len();
    Some(format!(
        "{} · {count} {} {} from {measured_for}: {}",
        profile.name,
        match count {
            1 => "model",
            _ => "models",
        },
        models.source.label(),
        models.names.join(", ")
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn engine(enabled: bool, reason: Option<&str>) -> EngineFacts {
        EngineFacts {
            engine: Engine::Codex,
            enabled,
            disabled_reason: reason.map(str::to_owned),
            models: ModelCatalog::default(),
        }
    }

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
                    registry: None,
                },
                ProfileFacts {
                    name: "codex".into(),
                    runtime: "codex".into(),
                    model_route: Some("openai/o4".into()),
                    executable: "/usr/bin/codex".into(),
                    version: None,
                    readiness: Readiness::NotInstalled,
                    detail: "executable not found".into(),
                    registry: Some(engine(true, None)),
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

    /// A disabled engine is never ready, whatever a probe would have said about it, and the page
    /// states the registry's reason rather than a readiness nobody measured.
    #[test]
    fn a_disabled_engine_is_not_ready_and_states_the_registry_reason() {
        let mut report = report();
        report.profiles[1].readiness = Readiness::Ready;
        report.profiles[1].registry = Some(engine(false, Some("usage limit until 2026-09-12")));
        report.profiles[1].detail =
            "disabled in the registry — usage limit until 2026-09-12".to_owned();
        assert!(!report.profiles[1].ready());
        assert_eq!(report.profiles[1].readiness_text(), "disabled");
        assert_eq!(report.ready_count(), 1);

        let page = page(Some(&report), "idle".into());
        let Body::Table { rows, .. } = &page.body else {
            panic!("expected a table");
        };
        let fix = rows[1].fix.as_ref().expect("fix line");
        let text: String = fix.iter().map(|span| span.content.as_ref()).collect();
        assert!(text.contains("usage limit until 2026-09-12"), "{text}");
    }

    #[test]
    fn the_page_names_every_model_an_engine_serves() {
        let mut report = report();
        report.profiles[1].registry = Some(EngineFacts {
            engine: Engine::ClaudeCode,
            enabled: true,
            disabled_reason: None,
            models: ModelCatalog {
                source: ModelSource::Measured,
                measured_for_version: Some("2.1.233 (Claude Code)".into()),
                measured_for_digest: Some("build-digest".into()),
                note: None,
                names: vec!["claude-haiku-4-5".into(), "claude-sonnet-5".into()],
            },
        });
        assert_eq!(report.profiles[1].models_text(), "2 measured");
        let page = page(Some(&report), "idle".into());
        let stated = page.notes.join("\n");
        assert!(stated.contains("claude-haiku-4-5"), "{stated}");
        assert!(stated.contains("claude-sonnet-5"), "{stated}");
        assert!(stated.contains("2.1.233 (Claude Code)"), "{stated}");
    }

    #[test]
    fn the_row_of_an_engine_names_the_engine_and_the_fixture_row_names_none() {
        let report = report();
        assert_eq!(report.engine_at(0), None);
        assert_eq!(report.engine_at(1), Some(Engine::Codex));
        assert_eq!(report.engine_at(9), None);
    }
}
