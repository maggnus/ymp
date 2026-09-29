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
    backends::claude::ClaudeStreamJson,
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
    let path = dir.0.join("claude_fixture.py");
    std::fs::write(
        &path,
        include_bytes!("../../ymp-runtime/tests/fixtures/claude_stream_json.py"),
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
    _backend: Arc<ClaudeStreamJson>,
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
        ClaudeStreamJson::new(
            id("provider"),
            ClaudeParameters {
                executable: executable.clone(),
                source: DiscoverySource::ProtocolFixture,
                connect_timeout_ms: 3000,
                frame_bytes: 262144,
                round_trips: 8,
            },
            1,
        )
        .unwrap(),
    );
    let discovery = backend.discover().unwrap();
    assert_eq!(discovery.source, DiscoverySource::ProtocolFixture);
    assert_eq!(discovery.provider.kind, ProviderKind::Claude);
    assert_eq!(discovery.provider.version.as_deref(), Some("0.0.0-fixture"));
    assert_eq!(discovery.default_model.as_deref(), Some("fixture-large"));
    // Aliases collapse to resolved models; a model without native effort offers none.
    assert_eq!(
        discovery
            .offerings
            .iter()
            .map(|o| (o.model.clone().unwrap(), o.efforts.clone().unwrap()))
            .collect::<Vec<_>>(),
        vec![
            ("fixture-haiku".into(), BTreeSet::new()),
            (
                "fixture-large".into(),
                BTreeSet::from(["high".into(), "low".into()])
            ),
        ]
    );
    assert!(!format!("{discovery:?}").contains("example.invalid"));
    ParameterSchemas::default()
        .validate(backend.selection())
        .unwrap();
    assert!(ymp_kernel::registry::mediated_backend(
        &discovery,
        Some(backend.selection())
    ));
    let mut updated = discovery.clone();
    updated.provider.version = Some("0.0.1-fixture".into());
    assert!(!ymp_kernel::registry::mediated_backend(
        &updated,
        Some(backend.selection())
    ));
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
        "claude",
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
        .profile(
            &s.session,
            &id("agent"),
            &ProfileSettings {
                model: Some("fixture-haiku".into()),
                effort: None,
            },
        )
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
        _backend: backend,
    }
}
fn attach(f: &mut NativeFixture, name: &str, files: bool) -> Live {
    attach_after(f, name, files, None)
}
fn attach_after(
    f: &mut NativeFixture,
    name: &str,
    files: bool,
    previous: Option<InvocationContinuation>,
) -> Live {
    f.s.files = files;
    let needs = if files {
        BTreeSet::from([Capability::ReadFiles, Capability::WriteFiles])
    } else {
        BTreeSet::new()
    };
    let award = f.s.award_kind(name, ContributionKind::Plan, needs, None);
    let (revision, at, mut request) = f.s.request_with_gate(&f.gate, name, award, "file");
    request.role = RoleKind::Planner;
    let mut prepared = f.gate.prepare(&f.s.session, revision, at, request).unwrap();
    let admitted = f.gate.admit(&mut prepared).unwrap();
    let view = f.gate.view(&f.s.session).unwrap();
    f.clock.advance_to(view.latest_at()).unwrap();
    let prompt = Prompt {
        text: if files { "files" } else { "text only" }.into(),
        basis: vec![
            view.coordination().contributions()[&id(name)]
                .reference
                .clone(),
        ],
    };
    let (call, receipt) = (id(&format!("call-{name}")), id(&format!("receipt-{name}")));
    match previous {
        Some(prior) => f
            .host
            .attach_continuing(admitted, call, receipt, prompt, prior),
        None => f.host.attach(admitted, call, receipt, prompt),
    }
    .unwrap()
}
fn held(f: &NativeFixture) -> f64 {
    f.gate
        .view(&f.s.session)
        .unwrap()
        .treasury()
        .unwrap()
        .budget
        .held
        .get()
}
fn inferences(f: &NativeFixture) -> usize {
    std::fs::read_to_string(f.executable.with_extension("log"))
        .unwrap_or_default()
        .lines()
        .filter(|line| *line == "inference")
        .count()
}
#[test]
fn claude_process_runs_mediated_files_and_settles_a_normalized_receipt() {
    let mut f = native_fixture();
    let mut live = attach(&mut f, "files", true);
    f.host.start(&mut live).unwrap();
    drive(&f.host, &mut live, &f.executable);
    let record = f.host.snapshot(&live).unwrap();
    assert!(
        record.confirmed_terminal,
        "terminal={:?}; diagnostics={:?}; events={:?}",
        record.terminal, record.diagnostics, record.observations
    );
    assert_eq!(record.terminal, Some(InvocationTerminal::Completed));
    assert_eq!(record.output, "{\"ok\":true}");
    // Four native requests: 4 uncached, 4 cache-read and 1 cache-written input token.
    assert_eq!(
        record.usage,
        Usage {
            input: 9,
            cache_read: 4,
            cache_write: 1,
            output: 1,
            reasoning: Some(1),
        }
    );
    assert_eq!(
        record.receipt.as_ref().unwrap().coverage,
        Coverage::Complete
    );
    let settings = &record.invocation.as_ref().unwrap().settings;
    assert_eq!(settings.sent.model.as_deref(), Some("fixture-haiku"));
    assert_eq!(settings.reported.model.as_deref(), Some("fixture-haiku"));
    assert_eq!(
        (&settings.sent.effort, &settings.reported.effort),
        (&None, &None)
    );
    assert!(
        record.observations.values().any(|(event, _)| matches!(
            event.observation,
            BackendObservation::ToolDenied(Capability::ReadFiles)
        )),
        "{:?}",
        record.observations
    );
    assert_eq!(
        std::fs::read(f.root.0.join("file")).unwrap(),
        b"native fixture"
    );
    assert_eq!(inferences(&f), 1);
    assert_eq!(held(&f), 0.0);
    assert!(f.host.start(&mut live).is_err());
    // Continuation belongs to the full adapter: refused before any process starts,
    // and the refused call keeps its hold as an unconfirmed end.
    let prior = InvocationContinuation {
        invocation: record.dispatch.invocation,
        completion: record.end.unwrap(),
    };
    let mut later = attach_after(&mut f, "later", false, Some(prior));
    f.host.start(&mut later).unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let _ = f.host.poll(&mut later).unwrap();
        let record = f.host.snapshot(&later).unwrap();
        if record.terminal.is_some() && !record.diagnostics.is_empty() {
            assert!(!record.confirmed_terminal);
            assert!(
                record
                    .diagnostics
                    .iter()
                    .any(|d| format!("{d:?}").contains("claude_continuation_unsupported")),
                "{:?}",
                record.diagnostics
            );
            break;
        }
        assert!(Instant::now() < deadline, "{:?}", record.diagnostics);
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(inferences(&f), 1);
    assert!(held(&f) > 0.0);
}

#[test]
fn ambiguous_native_stream_preserves_authority_and_accounting() {
    // Each mode names the refusal the journal must carry; a generic failure would
    // hide which boundary stopped the call.
    for (mode, refusal) in [
        ("foreign", "claude_unmanaged_tool"),
        ("surface", "claude_environment"),
        ("plugin", "claude_environment"),
        ("ambient", "claude_surface"),
        ("model", "claude_model_changed"),
        ("disconnect", "claude_disconnect"),
        ("partial-usage", ""),
        ("pause", "invocation_stopped"),
    ] {
        let mut f = native_fixture();
        let mut live = attach(&mut f, "bounded", false);
        std::fs::write(f.executable.with_extension("mode"), mode).unwrap();
        f.host.start(&mut live).unwrap();
        if mode == "pause" {
            let deadline = Instant::now() + Duration::from_secs(3);
            while !f.executable.with_extension("setup").exists() {
                assert!(Instant::now() < deadline);
                std::thread::sleep(Duration::from_millis(2));
            }
            f.host.cancel(&mut live).unwrap();
            std::fs::write(f.executable.with_extension("release"), "resume").unwrap();
        }
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            let state = f.host.poll(&mut live).unwrap();
            let record = f.host.snapshot(&live).unwrap();
            // The host settles a lower-bound receipt for every usage observation,
            // so only the recorded end of the call is a stopping point.
            if if mode == "partial-usage" {
                state == ExecutionStatus::Blocked("unsettled_usage_or_effects".into())
            } else {
                record.terminal.is_some() && !record.diagnostics.is_empty()
            } {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "{mode}: {state:?} {:?} {:?} {:?} {:?}",
                record.terminal,
                record.receipt,
                record.diagnostics,
                std::fs::read_to_string(f.executable.with_extension("error"))
            );
            std::thread::sleep(Duration::from_millis(2));
        }
        let record = f.host.snapshot(&live).unwrap();
        assert!(
            !f.executable.with_extension("error").exists(),
            "{mode}: the fixture process itself failed: {:?}",
            std::fs::read_to_string(f.executable.with_extension("error"))
        );
        if mode == "partial-usage" {
            // The native result is real but its per-model report does not confirm it.
            assert_eq!(
                record.backend_terminal.as_ref().map(|end| &end.0),
                Some(&InvocationTerminal::Completed),
                "{:?}",
                record.diagnostics
            );
            assert_eq!(record.terminal, None);
            assert_eq!(record.output, "{\"ok\":true}", "{:?}", record.diagnostics);
            assert_eq!(record.receipt.as_ref().unwrap().coverage, Coverage::Partial);
        } else {
            assert!(!record.confirmed_terminal, "{mode}");
            assert_ne!(
                record.terminal,
                Some(InvocationTerminal::Completed),
                "{mode}"
            );
            assert!(record.output.is_empty(), "{mode}");
            assert!(
                record
                    .diagnostics
                    .iter()
                    .any(|d| format!("{d:?}").contains(refusal)),
                "{mode}: {:?}",
                record.diagnostics
            );
        }
        assert!(
            record
                .receipt
                .as_ref()
                .is_none_or(|r| r.coverage != Coverage::Complete),
            "{mode}"
        );
        assert!(held(&f) > 0.0, "{mode}");
        assert_eq!(
            inferences(&f),
            usize::from(!matches!(mode, "ambient" | "pause")),
            "{mode}: only the tool surface and a stop are checked before inference"
        );
        assert!(f.host.start(&mut live).is_err());
        assert!(
            f.host.recover(&f.s.session, &id("call-bounded")).is_err() || mode == "partial-usage"
        );
    }
}

/// Metadata-only development procedure; normal tests never spawn an installed provider.
#[test]
#[ignore = "Requires explicit YMP_CLAUDE_METADATA_EXECUTABLE; no user message or model turn"]
fn installed_claude_metadata_without_inference() {
    let executable = std::env::var_os("YMP_CLAUDE_METADATA_EXECUTABLE")
        .expect("Explicit installed Claude Code path")
        .into();
    let backend = ClaudeStreamJson::new(
        id("metadata-provider"),
        ClaudeParameters {
            executable,
            source: DiscoverySource::Native,
            connect_timeout_ms: 30000,
            frame_bytes: 1_048_576,
            round_trips: 1,
        },
        1,
    )
    .unwrap();
    let facts = backend.discover().unwrap();
    assert_eq!(facts.source, DiscoverySource::Native);
    assert!(!facts.offerings.is_empty());
    for offering in &facts.offerings {
        eprintln!("{:?} efforts={:?}", offering.model, offering.efforts);
    }
    eprintln!(
        "Native metadata only: version={:?}, default={:?}, zero model turns",
        facts.provider.version, facts.default_model
    );
}
