//! Synthetic native wire process through the real Registry, admission, mediator and host.
mod support;
use support::Directory;
use ymp_kernel as kernel;
use ymp_storage as storage;
#[allow(dead_code)]
#[path = "support/admission_fixture.rs"]
mod admission_fixture;
#[allow(dead_code)]
#[path = "../../ymp-kernel/tests/support/workspace_fixture.rs"]
mod fixture;
use std::{
    collections::BTreeSet,
    sync::Arc,
    time::{Duration, Instant},
};
use ymp_domain::{
    Digest, Id, Proposal, assignment::*, coordination::*, identity::*, journal::Capability,
    resources::*, task::Real,
};
use ymp_kernel::{
    gatekeeper::Gatekeeper,
    journal::ParameterSchemas,
    ports::{
        checks::NativeDiscovery,
        execution::*,
        resources::{CostModel, ResourcePolicy},
    },
    registry::{ReadinessResponse, Registry, readiness_views},
};
use ymp_runtime::{
    backends::codex::CodexAppServer,
    clock::ManualClock,
    execution_host::{ExecutionHost, ExecutionStatus},
    policies::{
        award::FirstOffer,
        resources::{PriceWeighted, PurposeBounded, PurposeBoundedParameters},
    },
    readiness::StaticDependencyProbe,
};
use ymp_storage::{content::SqliteContent, journal::SqliteJournal};
fn id<T>(s: &str) -> Id<T> {
    Id::new(s).unwrap()
}
fn real(n: f64) -> Real {
    Real::new(n).unwrap()
}
type Host = ExecutionHost<SqliteJournal, SqliteContent>;
type Live = ymp_runtime::execution_host::LiveInvocation<SqliteJournal>;
fn executable(dir: &Directory) -> std::path::PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let path = dir.0.join("codex_fixture.py");
    std::fs::write(
        &path,
        include_bytes!("../../ymp-runtime/tests/fixtures/codex_app_server.py"),
    )
    .unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
    path
}
fn drive(host: &Host, live: &mut Live, executable: &std::path::Path) {
    let end = Instant::now() + Duration::from_secs(20);
    loop {
        let state = host.poll(live).unwrap();
        if state == ExecutionStatus::Finished {
            return;
        }
        assert!(
            Instant::now() < end,
            "{state:?} {:?}",
            (
                host.snapshot(live).unwrap().diagnostics,
                std::fs::read_to_string(executable.with_extension("error"))
            )
        );
        std::thread::sleep(Duration::from_millis(2));
    }
}
struct NativeFixture {
    root: Directory,
    _database: Directory,
    _adapter: Directory,
    executable: std::path::PathBuf,
    s: admission_fixture::Setup<SqliteJournal>,
    gate: Arc<Gatekeeper<SqliteJournal, SqliteContent>>,
    clock: Arc<ManualClock>,
    host: Host,
    backend: Arc<CodexAppServer>,
}
fn native_fixture() -> NativeFixture {
    let root = Directory::new();
    let database = Directory::new();
    let adapter_dir = Directory::new();
    std::fs::write(root.0.join("file"), b"before").unwrap();
    let executable = executable(&adapter_dir);
    let store = SqliteJournal::open(database.database(), ParameterSchemas::default()).unwrap();
    let journal =
        Arc::new(SqliteJournal::open(database.database(), ParameterSchemas::default()).unwrap());
    let backend = Arc::new(
        CodexAppServer::new(
            id("provider"),
            CodexParameters {
                executable: executable.clone(),
                source: DiscoverySource::ProtocolFixture,
                connect_timeout_ms: 3000,
                frame_bytes: 262144,
            },
            1,
        )
        .unwrap(),
    );
    let discovery = backend.discover().unwrap();
    assert_eq!(discovery.source, DiscoverySource::ProtocolFixture);
    assert_eq!(backend.selection().policy.version, "2");
    let mut legacy = backend.selection().clone();
    legacy.policy.version = "1".into();
    ParameterSchemas::default().validate(&legacy).unwrap();
    let mut historical = discovery.clone();
    historical.method = codex_discovery_method(&legacy);
    assert!(ymp_kernel::registry::mediated_backend(
        &historical,
        Some(&legacy)
    ));
    assert!(!ymp_kernel::registry::mediated_backend(
        &historical,
        Some(backend.selection())
    ));
    assert_eq!(
        discovery.offerings[0].efforts,
        Some(BTreeSet::from(["low".into()]))
    );
    let resource = PurposeBounded::new(PurposeBoundedParameters {
        max_cost: real(100.0),
        timeout: 10000,
        native_turns: 2,
        output_chars: 1000,
        report_call_cost: real(10.0),
    })
    .unwrap();
    let mut s = admission_fixture::Setup::new_with_selections(
        journal.clone(),
        &store,
        &root,
        true,
        "codex",
        None,
        FirstOffer::with_commitment_terms(CommitmentTerms {
            lease_duration: 10000,
            renewal_duration: 10,
            renew_on: BTreeSet::new(),
            renewals: 0,
            release_delta: real(1.0),
        })
        .unwrap(),
        vec![backend.selection().clone(), resource.selection().clone()],
    );
    let view = s.gate.view(&s.session).unwrap();
    let mut facts = view.registry().unwrap().input.facts.clone();
    facts.discoveries = vec![discovery];
    facts.dependencies = vec![
        StaticDependencyProbe::capture(
            id("provider"),
            None,
            &executable,
            DependencyKind::Executable,
            view.latest_at() + 1,
        )
        .unwrap(),
    ];
    let registry = Registry::new(journal.clone());
    let input = registry
        .prepare(&s.session, facts, view.latest_at() + 1)
        .unwrap();
    let policy = view.policies()["ReadinessProbe"].clone();
    let responses = readiness_views(&input)
        .iter()
        .map(|v| ReadinessResponse {
            profile: v.profile.clone(),
            input: Digest::of_value(v).unwrap(),
            proposal: Proposal {
                value: Readiness::Ready,
                rationale: "Real protocol discovery and static executable observation".into(),
                basis: vec![],
                policy: policy.policy.clone(),
            },
        })
        .collect();
    registry
        .record(
            &s.session,
            view.revision(),
            view.latest_at() + 1,
            input,
            policy,
            responses,
        )
        .unwrap();
    s.profile = registry
        .profile(&s.session, &id("agent"), &ProfileSettings::default())
        .unwrap();
    let gate = Arc::new(std::mem::replace(
        &mut s.gate,
        Gatekeeper::new(journal.clone(), Arc::new(store.content_store())),
    ));
    let clock = Arc::new(ManualClock::new(gate.view(&s.session).unwrap().latest_at()));
    let host = ExecutionHost::new(
        journal.clone(),
        gate.clone(),
        backend.clone(),
        Arc::new(PriceWeighted::from_selection(s.cost.selection()).unwrap()),
        clock.clone(),
    )
    .unwrap();
    NativeFixture {
        root,
        _database: database,
        _adapter: adapter_dir,
        executable,
        s,
        gate,
        clock,
        host,
        backend,
    }
}
#[test]
fn codex_process_runs_mediated_files_and_paid_native_continuation() {
    let NativeFixture {
        root,
        _database,
        _adapter,
        executable,
        mut s,
        gate,
        clock,
        host,
        backend: _,
    } = native_fixture();
    let mut previous = None;
    for name in ["files", "no-files"] {
        let needs = if name == "files" {
            BTreeSet::from([Capability::ReadFiles, Capability::WriteFiles])
        } else {
            BTreeSet::new()
        };
        s.files = name == "files";
        let award = s.award_kind(name, ContributionKind::Plan, needs, None);
        let (revision, at, mut request) = s.request_with_gate(&gate, name, award, "file");
        request.role = RoleKind::Planner;
        let mut prepared = gate.prepare(&s.session, revision, at, request).unwrap();
        let admitted = gate.admit(&mut prepared).unwrap();
        if name == "no-files" {
            assert!(admitted.assignment.access.is_empty());
        }
        let view = gate.view(&s.session).unwrap();
        let prompt = Prompt {
            text: if name == "files" {
                "files".into()
            } else {
                "text only".into()
            },
            basis: vec![
                view.coordination().contributions()[&id(name)]
                    .reference
                    .clone(),
            ],
        };
        clock.advance_to(view.latest_at()).unwrap();
        let mut live = if let Some(prior) = previous.take() {
            host.attach_continuing(
                admitted,
                id(&format!("call-{name}")),
                id(&format!("receipt-{name}")),
                prompt,
                prior,
            )
            .unwrap()
        } else {
            host.attach(
                admitted,
                id(&format!("call-{name}")),
                id(&format!("receipt-{name}")),
                prompt,
            )
            .unwrap()
        };
        host.start(&mut live).unwrap();
        drive(&host, &mut live, &executable);
        let record = host.snapshot(&live).unwrap();
        assert_eq!(record.output, "{\"ok\":true}");
        assert_eq!(record.usage.input, 5);
        assert_eq!(record.usage.cache_read, 2);
        assert_eq!(record.usage.cache_write, 1);
        assert_eq!(record.usage.output, 1);
        assert_eq!(record.usage.reasoning, Some(1));
        assert_eq!(
            record.receipt.as_ref().unwrap().coverage,
            Coverage::Complete
        );
        assert!(
            record.confirmed_terminal,
            "terminal={:?}; diagnostics={:?}; events={:?}",
            record.terminal, record.diagnostics, record.observations
        );
        assert_eq!(record.terminal, Some(InvocationTerminal::Completed));
        assert!(
            record
                .invocation
                .as_ref()
                .unwrap()
                .settings
                .reported
                .model
                .is_none()
        );
        assert!(host.start(&mut live).is_err());
        previous = Some(InvocationContinuation {
            invocation: record.dispatch.invocation,
            completion: record.end.unwrap(),
        });
    }
    assert_eq!(
        std::fs::read(root.0.join("file")).unwrap(),
        b"later native fixture"
    );
    let log = std::fs::read_to_string(executable.with_extension("log")).unwrap();
    assert_eq!(log.lines().filter(|line| *line == "turn/start").count(), 2);
    assert_eq!(
        log.lines().filter(|line| *line == "thread/start").count(),
        1
    );
    assert_eq!(
        log.lines().filter(|line| *line == "thread/resume").count(),
        1
    );
    assert_eq!(
        gate.view(&s.session)
            .unwrap()
            .treasury()
            .unwrap()
            .budget
            .held
            .get(),
        0.0
    );
}

#[test]
fn ambiguous_native_protocol_and_stop_preserve_authority_and_accounting() {
    for mode in [
        "foreign",
        "disconnect",
        "bad-final",
        "stale-usage",
        "missing-early",
        "pause",
    ] {
        let mut f = native_fixture();
        f.s.files = mode == "missing-early";
        let needs = if f.s.files {
            BTreeSet::from([Capability::ReadFiles, Capability::WriteFiles])
        } else {
            BTreeSet::new()
        };
        let award =
            f.s.award_kind("bounded", ContributionKind::Plan, needs, None);
        let (revision, at, mut request) = f.s.request_with_gate(&f.gate, "bounded", award, "file");
        request.role = RoleKind::Planner;
        let mut prepared = f.gate.prepare(&f.s.session, revision, at, request).unwrap();
        let admitted = f.gate.admit(&mut prepared).unwrap();
        let view = f.gate.view(&f.s.session).unwrap();
        f.clock.advance_to(view.latest_at()).unwrap();
        let prompt = Prompt {
            text: if f.s.files {
                "files".into()
            } else {
                "text only".into()
            },
            basis: vec![
                view.coordination().contributions()[&id("bounded")]
                    .reference
                    .clone(),
            ],
        };
        let mut live = f
            .host
            .attach(
                admitted,
                id("uncertain-call"),
                id("uncertain-receipt"),
                prompt,
            )
            .unwrap();
        std::fs::write(f.executable.with_extension("mode"), mode).unwrap();
        f.host.start(&mut live).unwrap();
        if mode == "pause" {
            let deadline = Instant::now() + Duration::from_secs(3);
            while !f.executable.with_extension("setup").exists() {
                assert!(Instant::now() < deadline);
                std::thread::sleep(Duration::from_millis(2));
            }
            f.host.cancel(&mut live).unwrap();
            std::fs::write(
                f.executable.with_extension("release"),
                "resume setup after stop",
            )
            .unwrap();
        }
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            let _ = f.host.poll(&mut live).unwrap();
            if mode == "pause" {
                assert_eq!(
                    std::fs::read_to_string(f.executable.with_extension("log"))
                        .unwrap_or_default()
                        .lines()
                        .filter(|line| *line == "turn/start")
                        .count(),
                    0,
                    "Stopped during setup must not begin inference"
                );
            }
            let record = f.host.snapshot(&live).unwrap();
            if (record.terminal.is_some() && !record.diagnostics.is_empty())
                || (matches!(mode, "stale-usage" | "missing-early") && record.receipt.is_some())
            {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "{mode}: {:?}",
                record.diagnostics
            );
            std::thread::sleep(Duration::from_millis(2));
        }
        if mode == "pause" {
            assert_eq!(
                std::fs::read_to_string(f.executable.with_extension("log"))
                    .unwrap_or_default()
                    .lines()
                    .filter(|line| *line == "turn/start")
                    .count(),
                0,
                "Stopped during setup must not begin inference"
            );
        }
        let record = f.host.snapshot(&live).unwrap();
        assert!(!record.confirmed_terminal, "{mode}");
        assert_ne!(record.terminal, Some(InvocationTerminal::Completed));
        assert!(
            record
                .receipt
                .as_ref()
                .is_none_or(|r| r.coverage != Coverage::Complete)
        );
        assert!(
            f.gate
                .view(&f.s.session)
                .unwrap()
                .treasury()
                .unwrap()
                .budget
                .held
                .get()
                > 0.0
        );
        assert!(f.host.start(&mut live).is_err());
        assert!(f.host.recover(&f.s.session, &id("uncertain-call")).is_err());
        if mode == "pause" {
            assert!(
                !std::fs::read_to_string(f.executable.with_extension("log"))
                    .unwrap_or_default()
                    .lines()
                    .any(|line| line == "turn/start")
            );
        }
        if mode == "foreign" {
            assert!(record.output.is_empty());
        }
        drop(f.backend);
    }
}
#[test]
fn malformed_discovery_and_usage_are_not_capability_or_accounting_evidence() {
    let directory = Directory::new();
    let executable = executable(&directory);
    std::fs::write(executable.with_extension("mode"), "bad-guard").unwrap();
    let backend = CodexAppServer::new(
        id("provider"),
        CodexParameters {
            executable: executable.clone(),
            source: DiscoverySource::ProtocolFixture,
            connect_timeout_ms: 1000,
            frame_bytes: 65536,
        },
        1,
    )
    .unwrap();
    assert_eq!(backend.discover().unwrap_err().code, "codex_features");
    for mode in ["host-disabled", "host-fallback"] {
        std::fs::write(executable.with_extension("mode"), mode).unwrap();
        assert_eq!(
            backend.discover().unwrap_err().code,
            "codex_external_surfaces"
        );
    }
    std::fs::write(executable.with_extension("mode"), "orphan-version").unwrap();
    let started = Instant::now();
    assert_eq!(backend.discover().unwrap_err().code, "codex_timeout");
    assert!(started.elapsed() < Duration::from_secs(2));
    let mut value = serde_json::json!({"inputTokens":5,"cachedInputTokens":2,"outputTokens":1,"reasoningOutputTokens":1,"totalTokens":6});
    assert_eq!(
        ymp_runtime::backends::codex::usage::decode(&value)
            .unwrap()
            .cache_write,
        0
    );
    value["inputTokens"] = serde_json::json!(-1);
    assert!(ymp_runtime::backends::codex::usage::decode(&value).is_err());
}

/// Metadata-only development procedure; normal tests never spawn an installed provider.
#[test]
#[ignore = "Requires explicit YMP_CODEX_METADATA_EXECUTABLE; no thread or model turn"]
fn installed_codex_metadata_without_inference() {
    let executable = std::env::var_os("YMP_CODEX_METADATA_EXECUTABLE")
        .expect("Explicit installed Codex path")
        .into();
    let backend = CodexAppServer::new(
        id("metadata-provider"),
        CodexParameters {
            executable,
            source: DiscoverySource::Native,
            connect_timeout_ms: 15000,
            frame_bytes: 1_048_576,
        },
        1,
    )
    .unwrap();
    let facts = backend.discover().unwrap();
    assert_eq!(facts.source, DiscoverySource::Native);
    assert!(!facts.offerings.is_empty());
    eprintln!(
        "Native metadata only: version={:?}, models={}, zero model turns",
        facts.provider.version,
        facts.offerings.len()
    );
}
