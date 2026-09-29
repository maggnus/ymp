//! ymp-runtime: application assembly and port adapters.
//! Owner: W1-0001 (MemoryJournal, composition).
//! Created by owner decision on 2026-09-16; see ymp-docs/project-worktree.md.
//! A module gains behavior only through its owning task.

pub mod admission;
pub mod application;
pub mod attempts;
pub mod backends;
pub mod checks;
pub mod clock;
pub mod consequences;
pub mod dispatcher;
pub mod execution_host;
pub mod experiments;
pub mod live_session;
pub mod memory_journal;
pub mod policies;
pub mod readiness;
pub mod team_operations;
pub mod workspace;

/// Consumers that depend only on the runtime name domain and kernel values through it.
pub use ymp_domain as domain;
pub use ymp_kernel as kernel;
