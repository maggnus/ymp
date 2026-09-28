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
use std::{collections::BTreeSet, fs, sync::Arc};
use ymp_domain::{
    Digest, Id, Proposal, Result,
    assignment::ErrorClass,
    journal::{Capability, PolicySelection},
    verification::*,
    workspace::*,
};
use ymp_kernel::{
    acceptance::{AcceptanceAuthority, RegisterCheck, RunCheck},
    events::Event,
    journal::{ContentStore, Journal, ParameterSchemas},
    ports::checks::{CheckExecution, CheckObservation, CheckRunner},
};
use ymp_runtime::{
    checks::{process::ProcessRunner, retained::RetainedBytes},
    workspace::direct::Direct,
};
use ymp_storage::{content::SqliteContent, journal::SqliteJournal};
fn id<T>(s: &str) -> Id<T> {
    Id::new(s).unwrap()
}
fn path(s: &str) -> WorkspacePath {
    WorkspacePath::new(s).unwrap()
}
struct Setup {
    root: support::Directory,
    _database: support::Directory,
    journal: Arc<SqliteJournal>,
    store: Arc<SqliteContent>,
    provider: Direct,
    opening: fixture::Opening<SqliteJournal, SqliteContent>,
    authority: AcceptanceAuthority<SqliteJournal, SqliteContent>,
}
impl Setup {
    fn new(script: &str) -> Self {
        let root = support::Directory::new();
        let database = support::Directory::new();
        fs::write(root.0.join("artifact"), b"wrong\n").unwrap();
        fs::write(root.0.join("expected"), b"correct\n").unwrap();
        fs::write(root.0.join("verify"), script).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(root.0.join("verify"), fs::Permissions::from_mode(0o755)).unwrap();
        }
        let provider = Direct::open(&root.0, CaptureLimits::default()).unwrap();
        let journal = Arc::new(
            SqliteJournal::open(database.database(), ParameterSchemas::default()).unwrap(),
        );
        let store = Arc::new(journal.content_store());
        let caps = BTreeSet::from([Capability::ReadFiles, Capability::RunProcess]);
        let opening = fixture::open_with_options(
            journal.clone(),
            store.clone(),
            &id("checks"),
            &provider,
            caps.clone(),
            vec![
                PolicySelection::new(
                    "VerificationDesigner",
                    "ExplicitVisible",
                    "1",
                    serde_json::json!({}),
                )
                .unwrap(),
            ],
            fixture::default_constraints(caps),
        );
        let authority = opening.intake.acceptance(store.clone());
        Self {
            root,
            _database: database,
            journal,
            store,
            provider,
            opening,
            authority,
        }
    }
    fn snapshot(&self, name: &str) -> Snapshot {
        let view = self.journal.view(&id("checks"), None).unwrap();
        self.opening
            .guard
            .snapshot(
                &id("checks"),
                view.revision(),
                view.latest_at() + 1,
                &id("workspace"),
                id(name),
                &self.provider,
            )
            .unwrap();
        self.opening
            .guard
            .retained(&id("checks"), &id(name))
            .unwrap()
    }
    fn check(&self, name: &str, spec: CheckSpec, verifier: Option<&Snapshot>) -> Check {
        let view = self.journal.view(&id("checks"), None).unwrap();
        let mut check = Check {
            id: id(name),
            criterion: id("criterion"),
            criterion_version: view.criteria()[0].reference().unwrap().version,
            spec,
            author: CheckAuthor::User,
            independence: Independence::Trusted,
            visibility: CheckVisibility::Visible,
            needs: BTreeSet::from([Capability::ReadFiles]),
            verifier: verifier.map(|v| v.reference().unwrap()),
            version: Digest::of(b"unsealed"),
        };
        if matches!(check.spec, CheckSpec::Command { .. }) {
            check.needs.insert(Capability::RunProcess);
        }
        check.version = check.content_version().unwrap();
        check
    }
    fn register(&self, check: Check) -> Check {
        let view = self.journal.view(&id("checks"), None).unwrap();
        self.authority
            .register_check(
                &self.opening.control,
                RegisterCheck {
                    expected_revision: view.revision(),
                    at: view.latest_at() + 1,
                    effective: PolicySelection::new(
                        "VerificationDesigner",
                        "ExplicitVisible",
                        "1",
                        serde_json::json!({}),
                    )
                    .unwrap(),
                    proposal: Proposal {
                        value: check,
                        rationale: "Explicit owner check".into(),
                        basis: vec![],
                        policy: PolicySelection::new(
                            "VerificationDesigner",
                            "ExplicitVisible",
                            "1",
                            serde_json::json!({}),
                        )
                        .unwrap()
                        .policy,
                    },
                },
            )
            .unwrap()
    }
    fn run(
        &self,
        name: &str,
        check: &Check,
        target: &Snapshot,
        role: CheckRunRole,
        runner: &dyn CheckRunner,
    ) -> Result<CheckRun> {
        let view = self.journal.view(&id("checks"), None)?;
        self.authority.run(
            &self.opening.control,
            RunCheck {
                expected_revision: view.revision(),
                at: view.latest_at() + 1,
                id: id(name),
                check: check.reference(),
                target: target.id.clone(),
                role,
            },
            runner,
        )
    }
}
fn command(program: &str, args: &[&str]) -> CheckSpec {
    CheckSpec::Command {
        program: path(program),
        args: args.iter().map(|s| (*s).into()).collect(),
        inputs: BTreeSet::from([path("expected")]),
    }
}
fn process() -> ProcessRunner {
    ProcessRunner::new(CheckLimits {
        timeout_ms: 1000,
        output_bytes: 4096,
    })
    .unwrap()
}
const VERIFY: &str = "#!/bin/sh\nIFS= read -r actual < artifact\nIFS= read -r expected < \"$YMP_CHECK_INPUTS/expected\"\nprintf 'observed=%s\\n' \"$actual\"\nprintf 'checked pinned input\\n' >&2\n[ \"$actual\" = \"$expected\" ]\n";

#[test]
fn real_snapshots_discriminate_and_retain_replayable_output_with_two_runners() {
    let setup = Setup::new(VERIFY);
    let before = setup.snapshot("before");
    let cmd = setup.register(setup.check("command", command("verify", &[]), Some(&before)));
    let exact = setup.register(setup.check(
        "exact",
        CheckSpec::ExactBytes {
            path: path("artifact"),
            digest: Digest::of(b"correct\n"),
        },
        None,
    ));
    fs::write(setup.root.0.join("artifact"), b"correct\n").unwrap();
    // The candidate cannot replace the pinned verifier or its expected input.
    fs::write(setup.root.0.join("verify"), b"#!/bin/sh\nexit 0\n").unwrap();
    fs::write(setup.root.0.join("expected"), b"wrong\n").unwrap();
    let after = setup.snapshot("after");
    fs::write(setup.root.0.join("artifact"), b"changed after capture\n").unwrap();
    let runner = process();
    #[cfg(target_os = "macos")]
    {
        let red = setup
            .run("red", &cmd, &before, CheckRunRole::Baseline, &runner)
            .unwrap();
        let green = setup
            .run("green", &cmd, &after, CheckRunRole::Candidate, &runner)
            .unwrap();
        assert_eq!(red.outcome, CheckOutcome::Fail, "{red:?}");
        assert_eq!(green.outcome, CheckOutcome::Pass, "{green:?}");
        assert_eq!(red.env, green.env);
        assert_eq!(
            setup.store.get(&red.stdout, 4096).unwrap(),
            b"observed=wrong\n"
        );
        assert_eq!(
            setup.store.get(&green.stdout, 4096).unwrap(),
            b"observed=correct\n"
        );
        assert_eq!(
            setup.store.get(&green.stderr, 4096).unwrap(),
            b"checked pinned input\n"
        );
        let environment: CheckEnvironment =
            ymp_domain::journal::decode(&setup.store.get(&green.env, 64 * 1024).unwrap()).unwrap();
        assert_eq!(environment, runner.environment().unwrap());
        println!(
            "real Command: before={:?} exit={:?}; after={:?} exit={:?}; retained stdout/stderr/environment verified",
            red.outcome, red.exit, green.outcome, green.exit
        );
    }
    #[cfg(not(target_os = "macos"))]
    assert_eq!(
        setup
            .run(
                "unsupported",
                &cmd,
                &before,
                CheckRunRole::Baseline,
                &runner
            )
            .unwrap()
            .outcome,
        CheckOutcome::Error(ErrorClass::Environment)
    );
    for (name, runner) in [
        ("process-bytes", &runner as &dyn CheckRunner),
        ("retained-bytes", &RetainedBytes),
    ] {
        let red = setup
            .run(
                &format!("{name}-red"),
                &exact,
                &before,
                CheckRunRole::Control,
                runner,
            )
            .unwrap();
        let green = setup
            .run(
                &format!("{name}-green"),
                &exact,
                &after,
                CheckRunRole::Candidate,
                runner,
            )
            .unwrap();
        assert_eq!(red.outcome, CheckOutcome::Fail);
        assert_eq!(green.outcome, CheckOutcome::Pass);
    }
    let view = setup.journal.view(&id("checks"), None).unwrap();
    let reopened =
        SqliteJournal::open(setup._database.database(), ParameterSchemas::default()).unwrap();
    assert_eq!(reopened.view(&id("checks"), None).unwrap(), view);
    assert_eq!(
        view.contract().unwrap().checks,
        vec![cmd.id.erased(), exact.id.erased()]
    );
}

#[test]
#[cfg(target_os = "macos")]
fn process_boundary_blocks_host_reads_writes_verifier_substitution_and_forks() {
    let protected = support::Directory::new();
    let secret = protected.0.join("secret");
    fs::write(&secret, b"private\n").unwrap();
    let script = format!(
        "#!/bin/sh\ncase \"$1\" in\nmarker-link) printf 'marker output\\n'; printf 'marker error\\n' >&2; printf started > \"$TMPDIR/other\"; exec /usr/bin/perl -e 'unlink $ARGV[0] or die $!; symlink $ARGV[1], $ARGV[0] or die $!;' \"$TMPDIR/started\" \"$TMPDIR/other\" ;;\nmarker-fifo) printf 'marker output\\n'; printf 'marker error\\n' >&2; exec /usr/bin/perl -MPOSIX -e 'unlink $ARGV[0] or die $!; POSIX::mkfifo($ARGV[0],0600) == 0 or die $!;' \"$TMPDIR/started\" ;;\nmarker-large) printf 'marker output\\n'; printf 'marker error\\n' >&2; exec /usr/bin/perl -e 'open my $file, \">\", $ARGV[0] or die $!; print $file \"started\" x 8192;' \"$TMPDIR/started\" ;;\nread-host) IFS= read -r x < '{}' ;;\nwrite-host) printf changed > '{}' ;;\nwrite-target) printf changed > artifact ;;\nwrite-verifier) printf changed > \"$YMP_CHECK_INPUTS/expected\" ;;\nfork) (printf forked) ;;\nexec-fork) exec /bin/sh -c '(printf forked)' ;;\ntimeout) while :; do :; done ;;\noutput) while :; do printf 'xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx'; done ;;\nesac\n",
        secret.display(),
        secret.display()
    );
    let setup = Setup::new(&script);
    let snapshot = setup.snapshot("boundary");
    for mode in [
        "read-host",
        "write-host",
        "write-target",
        "write-verifier",
        "fork",
        "exec-fork",
    ] {
        let check = setup.register(setup.check(mode, command("verify", &[mode]), Some(&snapshot)));
        let run = setup
            .run(mode, &check, &snapshot, CheckRunRole::Control, &process())
            .unwrap();
        assert_ne!(run.outcome, CheckOutcome::Pass, "{mode}: {run:?}");
        let err = setup.store.get(&run.stderr, 4096).unwrap();
        assert!(
            String::from_utf8_lossy(&err).contains("Operation not permitted"),
            "{mode}: {err:?}"
        );
    }
    assert_eq!(fs::read(secret).unwrap(), b"private\n");
    assert_eq!(fs::read(setup.root.0.join("artifact")).unwrap(), b"wrong\n");
    for mode in ["timeout", "output"] {
        let check = setup.register(setup.check(mode, command("verify", &[mode]), Some(&snapshot)));
        let runner = ProcessRunner::new(CheckLimits {
            timeout_ms: 50,
            output_bytes: 128,
        })
        .unwrap();
        let start = std::time::Instant::now();
        let run = setup
            .run(mode, &check, &snapshot, CheckRunRole::Control, &runner)
            .unwrap();
        assert_eq!(run.outcome, CheckOutcome::Error(ErrorClass::Infrastructure));
        assert_eq!(run.exit, None);
        assert!(start.elapsed() < std::time::Duration::from_secs(3));
        assert!(setup.store.get(&run.stdout, 128).unwrap().len() <= 128);
    }
    for mode in ["marker-link", "marker-fifo", "marker-large"] {
        let check = setup.register(setup.check(mode, command("verify", &[mode]), Some(&snapshot)));
        let start = std::time::Instant::now();
        let run = setup
            .run(mode, &check, &snapshot, CheckRunRole::Control, &process())
            .unwrap();
        assert_eq!(
            run.outcome,
            CheckOutcome::Error(ErrorClass::Environment),
            "{mode}: {run:?}"
        );
        assert_eq!(run.exit, None);
        assert!(start.elapsed() < std::time::Duration::from_secs(3));
        assert_eq!(
            setup.store.get(&run.stdout, 4096).unwrap(),
            b"marker output\n"
        );
        assert_eq!(
            setup.store.get(&run.stderr, 4096).unwrap(),
            b"marker error\n"
        );
    }
    let missing = setup.register(setup.check("missing", command("absent", &[]), Some(&snapshot)));
    let run = setup
        .run(
            "missing",
            &missing,
            &snapshot,
            CheckRunRole::Control,
            &process(),
        )
        .unwrap();
    assert_eq!(run.outcome, CheckOutcome::Error(ErrorClass::Environment));
    assert_eq!(run.exit, None);
}

struct Substitution(u8);
impl CheckRunner for Substitution {
    fn environment(&self) -> Result<CheckEnvironment> {
        RetainedBytes.environment()
    }
    fn run(
        &self,
        request: &CheckExecution<'_>,
        store: &dyn ContentStore,
    ) -> Result<CheckObservation> {
        let mut observation = RetainedBytes.run(request, store)?;
        match self.0 {
            0 => observation.check.version = Digest::of(b"foreign check"),
            1 => observation.target.version = Digest::of(b"foreign snapshot"),
            2 => observation.environment = Digest::of(b"foreign environment"),
            3 => {
                observation.kind = CheckObservationKind::ExactBytes(Some(Digest::of(b"correct\n")))
            }
            _ => observation.kind = CheckObservationKind::Exited(0),
        }
        Ok(observation)
    }
}
#[test]
fn kernel_rejects_foreign_bindings_forged_success_and_foreign_owner_and_preserves_criteria_versions()
 {
    let setup = Setup::new(VERIFY);
    let target = setup.snapshot("target");
    let check = setup.register(setup.check(
        "exact",
        CheckSpec::ExactBytes {
            path: path("artifact"),
            digest: Digest::of(b"correct\n"),
        },
        None,
    ));
    let before = setup.journal.read(&id("checks")).unwrap();
    let view = setup.journal.view(&id("checks"), None).unwrap();
    let unknown = PolicySelection::new(
        "VerificationDesigner",
        "Unknown",
        "1",
        serde_json::json!({}),
    )
    .unwrap();
    let mut proposed = check.clone();
    proposed.id = id("unknown-policy");
    proposed.version = proposed.content_version().unwrap();
    assert!(
        setup
            .authority
            .register_check(
                &setup.opening.control,
                RegisterCheck {
                    expected_revision: view.revision(),
                    at: view.latest_at() + 1,
                    effective: unknown.clone(),
                    proposal: Proposal {
                        value: proposed,
                        rationale: "Unknown implementation must not register".into(),
                        basis: vec![],
                        policy: unknown.policy
                    }
                }
            )
            .is_err()
    );
    assert_eq!(setup.journal.read(&id("checks")).unwrap(), before);
    for mutation in 0..5 {
        assert!(
            setup
                .run(
                    "forged",
                    &check,
                    &target,
                    CheckRunRole::Candidate,
                    &Substitution(mutation)
                )
                .is_err()
        );
        assert_eq!(setup.journal.read(&id("checks")).unwrap(), before);
    }
    let mut foreign_check = check.clone();
    foreign_check.version = Digest::of(b"unregistered");
    assert!(
        setup
            .run(
                "unknown-check",
                &foreign_check,
                &target,
                CheckRunRole::Candidate,
                &RetainedBytes
            )
            .is_err()
    );
    let mut foreign_target = target.clone();
    foreign_target.id = id("unknown-snapshot");
    assert!(
        setup
            .run(
                "unknown-target",
                &check,
                &foreign_target,
                CheckRunRole::Candidate,
                &RetainedBytes
            )
            .is_err()
    );
    let foreign = Setup::new(VERIFY);
    let view = setup.journal.view(&id("checks"), None).unwrap();
    assert!(
        setup
            .authority
            .run(
                &foreign.opening.control,
                RunCheck {
                    expected_revision: view.revision(),
                    at: view.latest_at() + 1,
                    id: id("foreign-owner"),
                    check: check.reference(),
                    target: target.id.clone(),
                    role: CheckRunRole::Candidate
                },
                &RetainedBytes
            )
            .is_err()
    );
    let run = setup
        .run(
            "real",
            &check,
            &target,
            CheckRunRole::Baseline,
            &RetainedBytes,
        )
        .unwrap();
    let read = setup.journal.read(&id("checks")).unwrap();
    let mut forged = read.events.last().unwrap().clone();
    if let Event::CheckRunRecorded { data, .. } = &mut forged.payload {
        data.run.outcome = CheckOutcome::Pass;
    } else {
        panic!("expected check run");
    }
    let mut events = read.events.clone();
    *events.last_mut().unwrap() = forged;
    assert!(ymp_kernel::view::SessionView::replay(id("checks"), &events).is_err());
    let view = setup.journal.view(&id("checks"), None).unwrap();
    let mut constraints = view.task().unwrap().constraints.clone();
    constraints.allowed.clear();
    let mut criteria = view.criteria().to_vec();
    criteria[0].text = "A changed criterion".into();
    setup
        .opening
        .intake
        .refine(
            &setup.opening.control,
            ymp_kernel::intake::IntakeRefinement {
                expected_revision: view.revision(),
                at: view.latest_at() + 1,
                constraints,
                criteria,
                reason: "Owner changed criterion".into(),
                note: ymp_kernel::intake::IntakeNote::Clarification(
                    ymp_domain::task::Clarification {
                        question: "Which behavior?".into(),
                        answer: "The revised one".into(),
                        at: view.latest_at() + 1,
                    },
                ),
            },
        )
        .unwrap();
    let refined = setup.journal.view(&id("checks"), None).unwrap();
    assert!(refined.contract().unwrap().checks.is_empty());
    assert_eq!(refined.check_runs()[&run.id], run);
    assert_eq!(refined.checks()[&check.id], check);
    assert_eq!(
        setup
            .run(
                "denied-capability",
                &check,
                &target,
                CheckRunRole::Candidate,
                &NeverRun
            )
            .unwrap_err()
            .code,
        "check_scope"
    );
    let mut constraints = refined.task().unwrap().constraints.clone();
    constraints.allowed.insert(Capability::ReadFiles);
    constraints.deadline = Some(refined.latest_at() + 2);
    setup
        .opening
        .intake
        .refine(
            &setup.opening.control,
            ymp_kernel::intake::IntakeRefinement {
                expected_revision: refined.revision(),
                at: refined.latest_at() + 1,
                constraints,
                criteria: refined.criteria().to_vec(),
                reason: "Owner imposed a deadline".into(),
                note: ymp_kernel::intake::IntakeNote::Clarification(
                    ymp_domain::task::Clarification {
                        question: "When does work stop?".into(),
                        answer: "At the stated deadline".into(),
                        at: refined.latest_at() + 1,
                    },
                ),
            },
        )
        .unwrap();
    assert_eq!(
        setup
            .run(
                "denied-deadline",
                &check,
                &target,
                CheckRunRole::Candidate,
                &NeverRun
            )
            .unwrap_err()
            .code,
        "check_scope"
    );
}
struct NeverRun;
impl CheckRunner for NeverRun {
    fn environment(&self) -> Result<CheckEnvironment> {
        panic!("Unauthorized check cannot inspect or execute its runner")
    }
    fn run(&self, _: &CheckExecution<'_>, _: &dyn ContentStore) -> Result<CheckObservation> {
        panic!("Unauthorized check cannot execute")
    }
}

#[test]
fn agent_checks_require_a_live_grant_and_cannot_claim_user_or_trusted_independence() {
    use ymp_domain::{assignment::TeamOperation, coordination::CommitmentTerms, task::Real};
    use ymp_runtime::policies::award::FirstOffer;
    let root = Directory::new();
    fs::write(root.0.join("file"), b"value").unwrap();
    let database = Directory::new();
    let journal =
        Arc::new(SqliteJournal::open(database.database(), ParameterSchemas::default()).unwrap());
    let selection = PolicySelection::new(
        "VerificationDesigner",
        "ExplicitVisible",
        "1",
        serde_json::json!({}),
    )
    .unwrap();
    let fixture = admission_fixture::Setup::new_with_selections(
        journal.clone(),
        journal.as_ref(),
        &root,
        true,
        "agent-checks",
        None,
        FirstOffer::with_commitment_terms(CommitmentTerms {
            lease_duration: 20,
            renewal_duration: 20,
            renew_on: BTreeSet::new(),
            renewals: 0,
            release_delta: Real::new(1.0).unwrap(),
        })
        .unwrap(),
        vec![selection.clone()],
    );
    let award = fixture.award("produce");
    let (expected, at, mut request) = fixture.request("producer", award);
    request.operations.insert(TeamOperation::CheckPropose);
    let mut prepared = fixture
        .gate
        .prepare(&fixture.session, expected, at, request)
        .unwrap();
    let admitted = fixture.gate.admit(&mut prepared).unwrap();
    let authority = fixture.intake.acceptance(Arc::new(journal.content_store()));
    let view = journal.view(&fixture.session, None).unwrap();
    let mut check = Check {
        id: id("agent-exact"),
        criterion: id("criterion"),
        criterion_version: view.criteria()[0].reference().unwrap().version,
        spec: CheckSpec::ExactBytes {
            path: path("file"),
            digest: Digest::of(b"value"),
        },
        author: CheckAuthor::Agent(admitted.assignment.id.clone()),
        independence: Independence::ProducerAuthored,
        visibility: CheckVisibility::Visible,
        needs: BTreeSet::from([Capability::ReadFiles]),
        verifier: None,
        version: Digest::of(b"unsealed"),
    };
    check.version = check.content_version().unwrap();
    let registration = |check: Check, at| RegisterCheck {
        expected_revision: view.revision(),
        at,
        effective: selection.clone(),
        proposal: Proposal {
            value: check,
            rationale: "Visible producer check proposal".into(),
            basis: vec![],
            policy: selection.policy.clone(),
        },
    };
    for mutation in 0..3 {
        let mut forged = check.clone();
        match mutation {
            0 => forged.author = CheckAuthor::User,
            1 => forged.independence = Independence::Trusted,
            _ => forged.independence = Independence::IndependentVisible,
        }
        forged.version = forged.content_version().unwrap();
        assert!(
            authority
                .register_agent_check(
                    &fixture.gate,
                    &admitted.grant,
                    registration(forged, view.latest_at() + 1)
                )
                .is_err()
        );
        assert_eq!(journal.view(&fixture.session, None).unwrap(), view);
    }
    let recorded = authority
        .register_agent_check(
            &fixture.gate,
            &admitted.grant,
            registration(check, view.latest_at() + 1),
        )
        .unwrap();
    assert_eq!(recorded.independence, Independence::ProducerAuthored);
    let events = journal.read(&fixture.session).unwrap();
    let mut expired = events.events.clone();
    expired.last_mut().unwrap().at = view.admission().assignments()[&admitted.assignment.id]
        .intent
        .grant
        .expires;
    assert!(ymp_kernel::view::SessionView::replay(fixture.session.clone(), &expired).is_err());
    let current = journal.view(&fixture.session, None).unwrap();
    fixture
        .gate
        .revoke(
            &fixture.session,
            current.revision(),
            current.latest_at() + 1,
            &admitted.assignment.id,
            "Owner revoked work".into(),
        )
        .unwrap();
    let current = journal.view(&fixture.session, None).unwrap();
    let mut check = recorded;
    check.id = id("revoked-check");
    check.version = check.content_version().unwrap();
    let mut request = registration(check, current.latest_at() + 1);
    request.expected_revision = current.revision();
    assert!(
        authority
            .register_agent_check(&fixture.gate, &admitted.grant, request)
            .is_err()
    );
}
