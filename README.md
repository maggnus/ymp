# YMP

YMP is a terminal application for solving tasks with a team of AI agents. The user
defines the goal and constraints; a trusted kernel controls execution, resources
and acceptance.

## Product definition

- [Intent](intent.md) defines the purpose and product principles.
- [Self-organizing team domain model](ymp-docs/self-organizing-team-domain-model.md) is the owner-approved, single authoritative architecture and product scope.
- [Domain](ymp-docs/domain.md) is a companion vocabulary and explains foundation API names.
- [Implementation architecture notes](ymp-docs/architecture.md) describe existing responsibility and dependency boundaries under the approved model.
- [Foundation contract](ymp-docs/foundation.md) defines the current executable scope.
- [Roadmap](ymp-docs/roadmap.md) describes ordered product outcomes and links their task records.
- [Historical domain-model amendments](ymp-docs/domain-model-amendments.md) retain earlier proposals without normative authority.
- [Development task workflow](ymp-docs/development-tasks.md) defines the canonical file-based task register.
- [Development task guide](ymp-docs/tasks/README.md) documents bounded task selection and updates.
- [Contributing](CONTRIBUTING.md) describes development and verification.

## Foundation

The current implementation validates task contracts, persists and replays session
history, and supports one bounded native Codex invocation with optional preliminary
command-check evidence through `run` and `show`. The
[foundation contract](ymp-docs/foundation.md) states its limits. The complete
self-organizing team, final result acceptance, experience and interactive TUI
remain development targets. An empty foundation `Constraints` value means only
that no conditions were supplied; it grants no execution authority.

## Command line

```sh
cargo run --offline -p ymp-cli --bin ymp -- --help
cargo run --offline -p ymp-cli --bin ymp -- --version
```

No arguments are equivalent to `--help`. Unsupported or malformed arguments
return a nonzero exit status; supported command forms are listed by `--help`.

## Library example

```rust
use ymp_runtime::{
    AcceptanceContract, Application, Constraints, Criterion, CriterionId, Goal,
    Revision, SessionId, Task, TaskId,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let task = Task::new(
        TaskId::new("report")?,
        Goal::new("Produce a checked report")?,
        AcceptanceContract::new(vec![Criterion::new(
            CriterionId::new("reviewed")?,
            "The report has been reviewed",
        )?])?,
        Constraints::new(vec!["Do not access the network".to_owned()])?,
    );

    let application = Application::in_memory();
    let session_id = SessionId::new("report-session")?;
    let opened = application.open_session(session_id.clone(), task)?;
    assert_eq!(opened.revision(), Revision::new(1));

    let cancelled = application.cancel_session(&session_id, opened.revision())?;
    assert_eq!(cancelled.revision(), Revision::new(2));
    Ok(())
}
```

`MemoryJournal` clones share one synchronized in-process store. All state is lost
when the process exits.

## Verification

```sh
cargo build --workspace --offline
cargo fmt --all --check
cargo clippy --workspace --all-targets --offline -- -D warnings
cargo test --workspace --offline
```

Development work starts with a bounded task query rather than reading every record:

```sh
python3 ymp-docs/tasks/manage.py next
python3 ymp-docs/tasks/manage.py show W1-0004
```
