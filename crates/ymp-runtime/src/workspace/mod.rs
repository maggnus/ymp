//! WorkspaceProvider implementations.
//! Owner: W1-0005, W4-0001.
//! Created by owner decision on 2026-09-16; see ymp-docs/project-worktree.md.
//! A module gains behavior only through its owning task.

pub mod copy_on_write;
pub mod direct;

pub mod binding;
