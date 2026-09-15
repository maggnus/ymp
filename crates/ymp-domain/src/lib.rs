//! ymp-domain: pure values and validation, no I/O.
//! Owner: W1-0001 (Id<T>, Digest, Ref, PolicyRef, Proposal, Denial).
//! Created by owner decision on 2026-09-16; see ymp-docs/project-worktree.md.
//! A module gains behavior only through its owning task.

pub mod assignment;
pub mod coordination;
pub mod experience;
pub mod identity;
pub mod journal;
pub mod plan;
pub mod report;
pub mod resources;
pub mod result;
pub mod task;
pub mod verification;
pub mod workspace;
