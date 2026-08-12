use std::fs::OpenOptions;
use std::io::Write;
use std::sync::mpsc::TryRecvError;
use tempfile::tempdir;
use ymp_application::{Application, ApplicationConfig, ApplicationError};
use ymp_domain::{Budget, Command, EventEnvelope, EventKind, RunState, RunStatus};
use ymp_storage::{JournalError, JournalLimits, ObjectStoreError};

#[test]
fn duplicate_command_is_replayed_and_state_recovers_from_cursor() {
    let temporary = tempdir().expect("temporary directory");
    let expected_state = {
        let mut app = Application::create(temporary.path(), "run-1", Budget::new(2, 1))
            .expect("create application");
        let candidate = app
            .object_store()
            .put(b"candidate bytes")
            .expect("store candidate");
        let first = app
            .execute(
                "command-1",
                Command::StartAttempt {
                    attempt_id: "attempt-1".to_owned(),
                },
            )
            .expect("start attempt");
        let duplicate = app
            .execute(
                "command-1",
                Command::StartAttempt {
                    attempt_id: "attempt-1".to_owned(),
                },
            )
            .expect("replay duplicate");
        assert!(duplicate.replayed);
        assert_eq!(duplicate.event, first.event);
        assert_eq!(app.state().budget.attempts_remaining, 1);
        assert!(matches!(
            app.execute(
                "command-1",
                Command::StartAttempt {
                    attempt_id: "different-attempt".to_owned(),
                }
            ),
            Err(ApplicationError::IdempotencyConflict { .. })
        ));

        app.execute(
            "command-2",
            Command::SubmitCandidate {
                attempt_id: "attempt-1".to_owned(),
                base_digest: "0".repeat(64),
                object_digest: candidate,
            },
        )
        .expect("submit candidate");
        assert_eq!(app.events_after(1).expect("cursor replay").len(), 2);
        app.state().clone()
    };

    let app = Application::open(temporary.path()).expect("reopen application");
    assert_eq!(app.state(), &expected_state);
    assert_eq!(app.events_after(1).expect("replay after restart").len(), 2);
}

#[test]
fn recovered_command_replay_returns_its_original_result() {
    let temporary = tempdir().expect("temporary directory");
    let original = {
        let mut app = Application::create(temporary.path(), "run-1", Budget::new(1, 0))
            .expect("create application");
        let original = app
            .execute(
                "start",
                Command::StartAttempt {
                    attempt_id: "attempt-1".to_owned(),
                },
            )
            .expect("start attempt");
        assert_eq!(original.status, RunStatus::Running);
        app.execute(
            "cancel",
            Command::Cancel {
                reason: "stop after the recorded result".to_owned(),
            },
        )
        .expect("cancel run");
        assert_eq!(app.state().status, RunStatus::Cancelled);
        original
    };

    let mut app = Application::open(temporary.path()).expect("reopen application");
    let state_before_replay = app.state().clone();
    let event_count = app.events_after(0).expect("read events").len();
    let replay = app
        .execute(
            "start",
            Command::StartAttempt {
                attempt_id: "attempt-1".to_owned(),
            },
        )
        .expect("replay recovered command");

    assert!(replay.replayed);
    assert_eq!(replay.event, original.event);
    assert_eq!(replay.status, original.status);
    assert_eq!(app.state(), &state_before_replay);
    assert_eq!(app.events_after(0).expect("read events").len(), event_count);
}

#[test]
fn recovery_rejects_conflicting_command_digest_before_applying_the_event() {
    let temporary = tempdir().expect("temporary directory");
    let first = {
        let mut app = Application::create(temporary.path(), "run-1", Budget::new(2, 0))
            .expect("create application");
        app.execute(
            "reused-command",
            Command::StartAttempt {
                attempt_id: "attempt-1".to_owned(),
            },
        )
        .expect("start first attempt")
        .event
    };
    let conflicting_command = Command::StartAttempt {
        attempt_id: "attempt-2".to_owned(),
    };
    let conflicting_event = EventEnvelope::new(
        "run-1",
        first.sequence + 1,
        "reused-command",
        conflicting_command
            .digest()
            .expect("digest conflicting command"),
        Some(first.digest),
        EventKind::AttemptStarted {
            attempt_id: "attempt-2".to_owned(),
        },
    )
    .expect("build conflicting event");
    assert!(conflicting_event.has_valid_digest());
    assert_ne!(conflicting_event.command_digest, first.command_digest);

    let journal_path = temporary.path().join("events.jsonl");
    let mut journal = OpenOptions::new()
        .append(true)
        .open(&journal_path)
        .expect("open journal");
    serde_json::to_writer(&mut journal, &conflicting_event).expect("write conflicting event");
    journal.write_all(b"\n").expect("complete event line");
    journal.sync_all().expect("sync conflicting event");
    drop(journal);

    let journal_before_open = std::fs::read(&journal_path).expect("read journal");
    let state_before_open: RunState = serde_json::from_slice(
        &std::fs::read(temporary.path().join("run.json")).expect("read state"),
    )
    .expect("parse state");
    assert!(matches!(
        Application::open(temporary.path()),
        Err(ApplicationError::IdempotencyConflict { command_id })
            if command_id == "reused-command"
    ));

    assert_eq!(
        std::fs::read(&journal_path).expect("reread journal"),
        journal_before_open
    );
    let state_after_open: RunState = serde_json::from_slice(
        &std::fs::read(temporary.path().join("run.json")).expect("read marked state"),
    )
    .expect("parse marked state");
    assert_eq!(state_after_open.status, RunStatus::InfrastructureError);
    assert_eq!(
        state_after_open.last_sequence,
        state_before_open.last_sequence
    );
    assert_eq!(
        state_after_open.last_event_digest,
        state_before_open.last_event_digest
    );
    assert_eq!(state_after_open.budget, state_before_open.budget);
    assert_eq!(
        state_after_open.active_attempts,
        state_before_open.active_attempts
    );
}

#[test]
fn second_writer_is_rejected() {
    let temporary = tempdir().expect("temporary directory");
    let _first = Application::create(temporary.path(), "run-1", Budget::new(1, 1))
        .expect("create first writer");
    assert!(matches!(
        Application::open(temporary.path()),
        Err(ApplicationError::WriterAlreadyActive)
    ));
}

#[test]
fn run_and_command_identifiers_are_bounded_before_commit() {
    let temporary = tempdir().expect("temporary directory");
    assert!(matches!(
        Application::create(temporary.path(), "", Budget::new(1, 1)),
        Err(ApplicationError::InvalidIdentifier { kind: "run_id" })
    ));

    let mut app = Application::create(temporary.path(), "run-1", Budget::new(1, 1))
        .expect("create application");
    assert!(matches!(
        app.execute(
            "",
            Command::StartAttempt {
                attempt_id: "attempt-1".to_owned()
            }
        ),
        Err(ApplicationError::InvalidIdentifier { kind: "command_id" })
    ));
    assert_eq!(app.state().last_sequence, 1);
}

#[test]
fn lagging_notification_receiver_recovers_from_durable_cursor() {
    let temporary = tempdir().expect("temporary directory");
    let mut app = Application::create(temporary.path(), "run-1", Budget::new(1, 1))
        .expect("create application");
    let notifications = app.subscribe(1).expect("subscribe");
    let candidate = app
        .object_store()
        .put(b"candidate bytes")
        .expect("store candidate");

    app.execute(
        "start",
        Command::StartAttempt {
            attempt_id: "attempt-1".to_owned(),
        },
    )
    .expect("start attempt");
    app.execute(
        "submit",
        Command::SubmitCandidate {
            attempt_id: "attempt-1".to_owned(),
            base_digest: "0".repeat(64),
            object_digest: candidate,
        },
    )
    .expect("submit candidate");

    assert_eq!(notifications.try_recv().expect("first notification"), 2);
    assert!(matches!(notifications.try_recv(), Err(TryRecvError::Empty)));
    let recovered = app.events_after(2).expect("recover from durable cursor");
    assert_eq!(recovered.len(), 1);
    assert_eq!(recovered[0].sequence, 3);
}

#[test]
fn journal_capacity_uses_terminal_reserve_and_recovers_as_infrastructure_error() {
    let temporary = tempdir().expect("temporary directory");
    let config = ApplicationConfig {
        journal_limits: JournalLimits {
            max_event_bytes: 1024,
            max_journal_bytes: 4096,
            terminal_reserve_bytes: 1024,
        },
    };
    {
        let mut app =
            Application::create_with_config(temporary.path(), "run-1", Budget::new(1, 1), config)
                .expect("create bounded application");
        let candidate = app
            .object_store()
            .put(b"candidate")
            .expect("store candidate");
        app.execute(
            "start",
            Command::StartAttempt {
                attempt_id: "attempt-1".to_owned(),
            },
        )
        .expect("start attempt");

        let mut index = 0_u64;
        loop {
            let result = app.execute(
                format!("submit-{index}"),
                Command::SubmitCandidate {
                    attempt_id: "attempt-1".to_owned(),
                    base_digest: "0".repeat(64),
                    object_digest: candidate.clone(),
                },
            );
            if matches!(
                result,
                Err(ApplicationError::Journal(
                    JournalError::CapacityExhausted { .. }
                ))
            ) {
                break;
            }
            result.expect("submit before capacity");
            index += 1;
            assert!(index < 32, "test journal capacity was not reached");
        }
        assert_eq!(app.state().status, RunStatus::InfrastructureError);
        assert!(app.state().active_attempts.is_empty());
    }

    let app = Application::open_with_config(temporary.path(), config)
        .expect("recover terminal bounded application");
    assert_eq!(app.state().status, RunStatus::InfrastructureError);
    assert!(matches!(
        app.events_after(0)
            .expect("read journal")
            .last()
            .unwrap()
            .event,
        ymp_domain::EventKind::RunFailed { .. }
    ));
}

#[test]
fn metadata_write_failure_replays_committed_command_and_repairs_summary() {
    let temporary = tempdir().expect("temporary directory");
    let mut app = Application::create(temporary.path(), "run-1", Budget::new(2, 1))
        .expect("create application");
    let metadata = temporary.path().join("run.json");
    std::fs::remove_file(&metadata).expect("remove metadata fixture");
    std::fs::create_dir(&metadata).expect("block metadata rename");

    assert!(matches!(
        app.execute(
            "start",
            Command::StartAttempt {
                attempt_id: "attempt-1".to_owned()
            }
        ),
        Err(ApplicationError::Io(_))
    ));
    assert_eq!(app.state().last_sequence, 2);
    assert_eq!(app.state().budget.attempts_remaining, 1);

    std::fs::remove_dir(&metadata).expect("unblock metadata rename");
    let replay = app
        .execute(
            "start",
            Command::StartAttempt {
                attempt_id: "attempt-1".to_owned(),
            },
        )
        .expect("replay committed command");
    assert!(replay.replayed);
    assert_eq!(app.state().last_sequence, 2);
    assert_eq!(app.state().budget.attempts_remaining, 1);
    let recovered: RunState =
        serde_json::from_slice(&std::fs::read(metadata).expect("repaired metadata"))
            .expect("parse repaired metadata");
    assert_eq!(recovered, *app.state());
    assert_eq!(
        std::fs::read_dir(temporary.path())
            .expect("read data root")
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().starts_with(".run."))
            .count(),
        0
    );
}

#[test]
fn incomplete_journal_tail_is_rejected() {
    let temporary = tempdir().expect("temporary directory");
    drop(
        Application::create(temporary.path(), "run-1", Budget::new(1, 1))
            .expect("create application"),
    );
    let mut journal = OpenOptions::new()
        .append(true)
        .open(temporary.path().join("events.jsonl"))
        .expect("open journal");
    journal.write_all(b"{\"partial\":").expect("damage tail");
    journal.sync_all().expect("sync damage");

    assert!(matches!(
        Application::open(temporary.path()),
        Err(ApplicationError::Journal(JournalError::IncompleteTail))
    ));
    assert_persisted_infrastructure_error(temporary.path());
}

#[test]
fn missing_candidate_object_is_rejected_during_recovery() {
    let temporary = tempdir().expect("temporary directory");
    let object_path = {
        let mut app = Application::create(temporary.path(), "run-1", Budget::new(1, 1))
            .expect("create application");
        let candidate = app
            .object_store()
            .put(b"candidate bytes")
            .expect("store candidate");
        app.execute(
            "command-1",
            Command::StartAttempt {
                attempt_id: "attempt-1".to_owned(),
            },
        )
        .expect("start attempt");
        app.execute(
            "command-2",
            Command::SubmitCandidate {
                attempt_id: "attempt-1".to_owned(),
                base_digest: "0".repeat(64),
                object_digest: candidate.clone(),
            },
        )
        .expect("submit candidate");
        app.object_store()
            .path_for(&candidate)
            .expect("candidate path")
    };
    std::fs::remove_file(object_path).expect("remove candidate object");

    assert!(matches!(
        Application::open(temporary.path()),
        Err(ApplicationError::ObjectStore(ObjectStoreError::Missing(_)))
    ));
    assert_persisted_infrastructure_error(temporary.path());
}

#[test]
fn journal_gap_duplicate_predecessor_and_digest_corruption_are_distinguished() {
    for corruption in ["gap", "duplicate", "predecessor", "digest"] {
        let temporary = tempdir().expect("temporary directory");
        {
            let mut app = Application::create(temporary.path(), "run-1", Budget::new(1, 1))
                .expect("create application");
            app.execute(
                "command-1",
                Command::StartAttempt {
                    attempt_id: "attempt-1".to_owned(),
                },
            )
            .expect("start attempt");
        }
        rewrite_event(temporary.path(), corruption);

        let error = match Application::open(temporary.path()) {
            Ok(_) => panic!("corrupted journal was accepted"),
            Err(error) => error,
        };
        match (corruption, error) {
            ("gap", ApplicationError::Journal(JournalError::Sequence { .. }))
            | ("duplicate", ApplicationError::Journal(JournalError::Sequence { .. }))
            | ("predecessor", ApplicationError::Journal(JournalError::Predecessor { .. }))
            | ("digest", ApplicationError::Journal(JournalError::Digest { .. })) => {}
            (_, unexpected) => panic!("unexpected error for {corruption}: {unexpected}"),
        }
        assert_persisted_infrastructure_error(temporary.path());
    }
}

fn rewrite_event(data_root: &std::path::Path, corruption: &str) {
    let path = data_root.join("events.jsonl");
    let mut events: Vec<serde_json::Value> = std::fs::read_to_string(&path)
        .expect("read journal")
        .lines()
        .map(|line| serde_json::from_str(line).expect("parse event"))
        .collect();
    match corruption {
        "gap" => events[1]["sequence"] = serde_json::json!(4),
        "duplicate" => events[1]["sequence"] = serde_json::json!(1),
        "predecessor" => events[1]["predecessor_digest"] = serde_json::json!("f".repeat(64)),
        "digest" => events[0]["digest"] = serde_json::json!("f".repeat(64)),
        _ => unreachable!("known corruption"),
    }
    let mut bytes = events
        .into_iter()
        .map(|event| serde_json::to_string(&event).expect("serialize event"))
        .collect::<Vec<_>>()
        .join("\n")
        .into_bytes();
    bytes.push(b'\n');
    std::fs::write(path, bytes).expect("write corrupted journal");
}

fn assert_persisted_infrastructure_error(data_root: &std::path::Path) {
    let state: RunState = serde_json::from_slice(
        &std::fs::read(data_root.join("run.json")).expect("read terminal metadata"),
    )
    .expect("parse terminal metadata");
    assert_eq!(state.status, RunStatus::InfrastructureError);
    assert!(state.active_attempts.is_empty());
}
