//! ymp-kernel: trusted operations, ports and replay.
//! Owner: W1-0001.
//! Created by owner decision on 2026-09-16; see ymp-docs/project-worktree.md.
//! A module gains behavior only through its owning task.

pub mod acceptance;
pub mod arbiter;
pub mod decision;
pub mod events;
pub mod execution;
pub mod experience_vault;
pub mod finalization;
pub mod gatekeeper;
pub mod intake;
pub mod journal;
pub mod ledger;
pub mod plans;
pub mod ports;
pub mod progress;
pub mod registry;
pub mod results;
pub mod session;
pub mod treasury;
pub mod view;
pub mod workspace_guard;

pub mod workspace_locks;
