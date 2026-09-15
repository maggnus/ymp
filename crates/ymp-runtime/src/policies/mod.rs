//! Strategy implementations: one module per port, default plus alternatives.
//! Owner: W1-0011 (run-level selection).
//! Created by owner decision on 2026-09-16; see ymp-docs/project-worktree.md.
//! A module gains behavior only through its owning task.

pub mod award;
pub mod belief;
pub mod calibration;
pub mod claim_audit;
pub mod context;
pub mod contribution;
pub mod credit;
pub mod diagnosis;
pub mod dispute;
pub mod escalation;
pub mod intake;
pub mod knowledge_curator;
pub mod method;
pub mod mutation;
pub mod narrative;
pub mod planner;
pub mod profile;
pub mod progress;
pub mod reputation;
pub mod resources;
pub mod retrieval;
pub mod reviewer;
pub mod selection;
pub mod team;
pub mod trial;
pub mod verification_designer;
pub mod volunteer;
