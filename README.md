# YMP

YMP is a terminal application for solving tasks with a team of AI agents. The user
defines the goal and constraints; a trusted kernel controls execution, resources
and acceptance.

## Product definition

- [Intent](intent.md) defines the purpose and product principles.
- [Domain](ymp-docs/domain.md) defines the canonical names.
- [Architecture](ymp-docs/architecture.md) defines responsibility and dependency boundaries.
- [Foundation contract](ymp-docs/foundation.md) defines the current executable scope.
- [Roadmap](ymp-docs/roadmap.md) separates delivered foundations from planned capabilities.
- [Contributing](CONTRIBUTING.md) describes development and verification.

## Foundation

The current implementation validates task contracts, opens and cancels in-memory
sessions through the kernel, and rejects malformed journal histories.

The foundation does not execute agents or tasks, discover providers, persist state,
provide a terminal UI, or expose MCP. An empty `Constraints` value means only that
no conditions were supplied; it grants no execution authority.

## Command line

```sh
cargo run --offline -p ymp-cli --bin ymp -- --help
cargo run --offline -p ymp-cli --bin ymp -- --version
```

No arguments are equivalent to `--help`. Every unsupported or additional argument
returns a nonzero exit status.

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
