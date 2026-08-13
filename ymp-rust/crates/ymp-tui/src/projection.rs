//! The data boundary between the application and the screen.
//!
//! Everything the interface draws arrives through the types in this module. They are built
//! from durable application state (`ymp-application`, `ymp-domain`), from runtime probes
//! (`ymp-runtime-api`) and from the process environment — never from the drawing code. A
//! concept the current domain cannot produce has no field here, so a page can only state it as
//! unavailable; it cannot invent one.
//!
//! Two facts have no application projection today and are therefore recorded here, at the
//! boundary, with their source named:
//!
//! * the assurance profile, whose authoritative name lives in `ymp-docs/PROJECT-CONTRACT.md`;
//! * the project directory, which is the process working directory.

use std::path::{Path, PathBuf};

use ymp_domain::{Budget, RunState, RunStatus};
use ymp_runtime_supervisor::ManagedContract;

use crate::pages::Page;
use crate::state::{PageKind, PaletteItem};
use crate::transcript::Entry;

/// The authoritative assurance-profile name.
///
/// `ymp-docs/PROJECT-CONTRACT.md` states that the first release uses `poc_process_isolation`,
/// and the invariants forbid claiming containment the POC does not provide. The chat-first
/// drawing named a different profile; that name is superseded and the decision is recorded in
/// `ymp-docs/VISUAL_CONCEPT.md`.
pub const ASSURANCE_PROFILE: &str = "poc_process_isolation";

/// What the profile does not give the operator, stated wherever the profile is shown (INV-8).
pub const ASSURANCE_LIMIT: &str = "no hostile-code containment; agents run with your user's permissions in private copies of \
     the codebase";

/// Facts about this process and this checkout. None of them is a domain object.
#[derive(Clone, Debug)]
pub struct Environment {
    pub version: String,
    /// The directory the product was started in — the project, in the naming model.
    pub project: String,
    pub project_path: PathBuf,
    /// Where durable state lives.
    pub data_root: PathBuf,
    pub assurance_profile: String,
    pub assurance_limit: String,
}

impl Environment {
    pub fn detect(data_root: &Path) -> Self {
        let project_path = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let project = project_path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| data_root.display().to_string());
        Self {
            version: env!("CARGO_PKG_VERSION").to_owned(),
            project,
            project_path,
            data_root: data_root.to_path_buf(),
            assurance_profile: ASSURANCE_PROFILE.to_owned(),
            assurance_limit: ASSURANCE_LIMIT.to_owned(),
        }
    }
}

/// The five terminal outcomes and the live state, named exactly (INV-7).
pub fn outcome(status: RunStatus) -> &'static str {
    match status {
        RunStatus::Running => "running",
        RunStatus::Accepted => "accepted",
        RunStatus::Exhausted => "exhausted",
        RunStatus::Abstained => "abstained",
        RunStatus::Cancelled => "cancelled",
        RunStatus::InfrastructureError => "infrastructure_error",
    }
}

/// A monochrome marker for the run state. The word is always printed next to it.
pub fn outcome_marker(status: RunStatus) -> &'static str {
    match status {
        RunStatus::Running => "[*]",
        RunStatus::Accepted => "[+]",
        RunStatus::Exhausted => "[-]",
        RunStatus::Abstained => "[?]",
        RunStatus::Cancelled => "[x]",
        RunStatus::InfrastructureError => "[!]",
    }
}

/// The run, exactly as the durable state records it.
#[derive(Clone, Debug)]
pub struct RunFacts {
    pub run_id: String,
    pub status: RunStatus,
    pub budget: Budget,
    pub active_attempts: Vec<String>,
    pub candidate_digest: Option<String>,
    pub last_sequence: u64,
    /// The reason recorded with the terminal event, when the run has ended.
    pub terminal_reason: Option<String>,
}

impl RunFacts {
    pub fn from_state(state: &RunState, terminal_reason: Option<String>) -> Self {
        Self {
            run_id: state.run_id.clone(),
            status: state.status,
            budget: state.budget.clone(),
            active_attempts: state.active_attempts.clone(),
            candidate_digest: state.candidate_digest.clone(),
            last_sequence: state.last_sequence,
            terminal_reason,
        }
    }

    pub fn is_live(&self) -> bool {
        !self.status.is_terminal()
    }
}

/// A managed contract as the operator must judge it before authorizing: what will be run, and
/// what will mechanically check the result. The domain has no contract package object, so this
/// comes from the file `ymp-cli` was pointed at.
#[derive(Clone, Debug)]
pub struct ContractFacts {
    pub contract_id: String,
    pub contract_digest: String,
    pub source: PathBuf,
    pub prompt: String,
    /// The verifier program, its oracle digest and the deliberately broken version it must
    /// reject. Absent when the contract declares no mechanical check.
    pub verifier: Option<VerifierFacts>,
}

#[derive(Clone, Debug)]
pub struct VerifierFacts {
    pub program: PathBuf,
    pub oracle_digest: String,
    pub negative_control: PathBuf,
    pub wall_time_ms: u64,
}

impl ContractFacts {
    pub fn from_managed(contract: &ManagedContract) -> Self {
        Self {
            contract_id: contract.contract_id.clone(),
            contract_digest: contract.contract_digest.clone(),
            source: contract.source.clone(),
            prompt: contract.prompt.clone(),
            verifier: contract.verifier.as_ref().map(|verifier| VerifierFacts {
                program: verifier.program.clone(),
                oracle_digest: verifier.oracle_digest.clone(),
                negative_control: verifier.negative_control.clone(),
                wall_time_ms: verifier.wall_time_ms,
            }),
        }
    }

    /// Authorization is blocked while the contract declares no mechanical check: approval would
    /// grant authority over a result nothing can reject.
    pub fn blocking_items(&self) -> usize {
        usize::from(self.verifier.is_none())
    }
}

/// An immutable candidate, as the journal records it.
#[derive(Clone, Debug)]
pub struct CandidateFacts {
    /// The journal position that published it. The domain has no separate candidate id.
    pub sequence: u64,
    pub attempt_id: String,
    pub base_digest: String,
    pub object_digest: String,
    /// The recorded verdict, when a verification names this candidate.
    pub verdict: Option<CandidateVerdict>,
}

#[derive(Clone, Debug)]
pub struct CandidateVerdict {
    pub sequence: u64,
    pub accepted: bool,
    pub evidence_digest: String,
    pub oracle_digest: String,
    pub contract_digest: String,
}

/// An attempt of the current run. POC-1 records attempts, not participants.
#[derive(Clone, Debug)]
pub struct AttemptFacts {
    pub attempt_id: String,
    pub started_at_sequence: u64,
    pub active: bool,
    pub candidates: usize,
}

/// One journal event, reduced to what a dense row shows.
#[derive(Clone, Debug)]
pub struct EventFacts {
    pub sequence: u64,
    pub plane: crate::transcript::Plane,
    pub kind: &'static str,
    pub subject: String,
}

/// Everything on screen. The drawing layer reads this and nothing else.
#[derive(Clone, Debug, Default)]
pub struct Projection {
    pub environment: Option<Environment>,
    pub run: Option<RunFacts>,
    pub contracts: Vec<ContractFacts>,
    /// The transcript, in journal order.
    pub entries: Vec<Entry>,
    /// Full-screen data pages, in the order the palette offers them.
    pub pages: Vec<(PageKind, Page)>,
    pub commands: Vec<PaletteItem>,
    /// The probe of the shipped runtime drivers, once it has returned.
    pub runtimes: Option<crate::runtimes::Report>,
    /// Left half of the status line.
    pub status: String,
}

impl Projection {
    pub fn page(&self, kind: PageKind) -> Option<&Page> {
        self.pages
            .iter()
            .find(|(page_kind, _)| *page_kind == kind)
            .map(|(_, page)| page)
    }

    pub fn page_mut(&mut self, kind: PageKind) -> Option<&mut Page> {
        self.pages
            .iter_mut()
            .find(|(page_kind, _)| *page_kind == kind)
            .map(|(_, page)| page)
    }
}

/// Shorten a content-addressed digest for a dense row, keeping it recognisable.
pub fn short_digest(digest: &str) -> String {
    let cut = digest
        .char_indices()
        .nth(10)
        .map_or(digest.len(), |(index, _)| index);
    if cut == digest.len() {
        digest.to_owned()
    } else {
        format!("{}…", &digest[..cut])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_run_status_is_named_exactly_and_marked_without_colour() {
        let all = [
            RunStatus::Running,
            RunStatus::Accepted,
            RunStatus::Exhausted,
            RunStatus::Abstained,
            RunStatus::Cancelled,
            RunStatus::InfrastructureError,
        ];
        let mut markers = Vec::new();
        for status in all {
            assert!(!outcome(status).is_empty());
            assert_ne!(outcome(status), "done");
            markers.push(outcome_marker(status));
        }
        markers.sort_unstable();
        markers.dedup();
        assert_eq!(markers.len(), all.len(), "markers must be distinguishable");
    }

    #[test]
    fn a_contract_without_a_mechanical_check_blocks_authorization() {
        let facts = ContractFacts {
            contract_id: "demo".into(),
            contract_digest: "0".repeat(64),
            source: PathBuf::from("/tmp/source"),
            prompt: "do the work".into(),
            verifier: None,
        };
        assert_eq!(facts.blocking_items(), 1);
    }
}
