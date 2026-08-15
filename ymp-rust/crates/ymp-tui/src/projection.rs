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

use ymp_application::PreparedContract;
use ymp_domain::commitment::{
    BudgetVector, ContractState, DIMENSIONS, ObligationRecord, ObligationState, Outcome,
    TaskContractRecord,
};
use ymp_domain::contract::ContractDocument;
use ymp_domain::{Budget, RunState, RunStatus};

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

    /// The budget as dimensions, each carrying the class the domain gives it.
    ///
    /// Both dimensions this domain records are enforced: `RunState::decide` refuses to start an
    /// attempt or to record a verification once the corresponding remainder reaches zero, and
    /// turns the request into a terminal `exhausted` instead. Observed and estimated classes
    /// exist in the vocabulary because a later dimension may carry one; nothing produces them
    /// today, so nothing shows one.
    pub fn budget_dimensions(&self, initial: Option<&Budget>) -> Vec<BudgetDimension> {
        vec![
            BudgetDimension {
                name: "attempts",
                class: BudgetClass::Enforced,
                total: initial.map(|budget| budget.attempts_remaining),
                remaining: self.budget.attempts_remaining,
            },
            BudgetDimension {
                name: "verification_queries",
                class: BudgetClass::Enforced,
                total: initial.map(|budget| budget.verification_queries_remaining),
                remaining: self.budget.verification_queries_remaining,
            },
        ]
    }
}

/// How a dimension acts on the run.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BudgetClass {
    /// Reaching zero stops the work the dimension governs.
    Enforced,
    /// Recorded, never a gate.
    Observed,
    /// Derived rather than measured, never a gate.
    Estimated,
}

impl BudgetClass {
    pub fn label(self) -> &'static str {
        match self {
            Self::Enforced => "enforced",
            Self::Observed => "observed",
            Self::Estimated => "estimated",
        }
    }

    pub fn gates(self) -> bool {
        matches!(self, Self::Enforced)
    }
}

/// One dimension of the run budget.
#[derive(Clone, Debug)]
pub struct BudgetDimension {
    pub name: &'static str,
    pub class: BudgetClass,
    /// The amount the run started with, when the journal still carries its first event.
    pub total: Option<u32>,
    pub remaining: u32,
}

impl BudgetDimension {
    pub fn used(&self) -> Option<u32> {
        self.total.map(|total| total.saturating_sub(self.remaining))
    }
}

/// A contract as the operator must judge it before authorizing: what will be run, what will
/// mechanically check the result, and the run it would start.
///
/// Every field is read from what the application prepared from the request — the digests, the
/// resolved paths and the derived identifiers are its values, not the interface's.
#[derive(Clone, Debug)]
pub struct ContractFacts {
    pub contract_id: String,
    pub contract_digest: String,
    pub source: PathBuf,
    pub prompt: String,
    /// The verifier program, its oracle digest and the deliberately wrong candidate it must
    /// reject. Absent when the request states no mechanical check.
    pub verifier: Option<VerifierFacts>,
    /// The budget the run would start with, as the application derived it.
    pub budget: Option<Budget>,
    /// The identifier the run would carry in the store this session addresses, when this contract
    /// can start one. A run is identified by its contract and its store together, so this is the
    /// identifier of a start into that store and of no other.
    pub run_id: Option<String>,
    /// Why no run can be started from this contract, in the application's own words.
    pub blocked: Option<String>,
    /// Whether this exact contract was already authorized in this session and has not changed
    /// since. Authorizing it again is one confirmation; a first authorization, and anything that
    /// changed, is authorized by typing the contract id.
    pub previously_authorized: bool,
}

#[derive(Clone, Debug)]
pub struct VerifierFacts {
    pub program: PathBuf,
    pub oracle_digest: String,
    pub negative_control: PathBuf,
    pub wall_time_ms: u64,
}

impl ContractFacts {
    /// The contract the application prepared: startable, with every value read from it.
    ///
    /// `store` is the store a run authorized now would be recorded in, because the identifier of
    /// that run is derived from the contract and that store together.
    pub fn from_prepared(prepared: &PreparedContract, store: &Path) -> Self {
        let verifier = prepared.verifier();
        Self {
            contract_id: prepared.contract_id().to_owned(),
            contract_digest: prepared.contract_digest.clone(),
            source: prepared.document.source.clone(),
            prompt: prepared.document.prompt.clone(),
            verifier: Some(VerifierFacts {
                program: verifier.program.clone(),
                oracle_digest: verifier.oracle_digest.clone(),
                negative_control: verifier.negative_control.clone(),
                wall_time_ms: verifier.wall_time_ms,
            }),
            budget: Some(prepared.budget.clone()),
            run_id: Some(prepared.run_id_in(store)),
            blocked: None,
            previously_authorized: false,
        }
    }

    /// A contract the application refused to prepare, carrying the refusal it reported.
    pub fn refused(contract_id: String, source: PathBuf, prompt: String, reason: String) -> Self {
        Self {
            contract_id,
            contract_digest: String::new(),
            source,
            prompt,
            verifier: None,
            budget: None,
            run_id: None,
            blocked: Some(reason),
            previously_authorized: false,
        }
    }

    /// A contract read back from the store, with no run left for it to start.
    pub fn from_document(document: &ContractDocument, digest: String) -> Self {
        Self {
            contract_id: document.contract_id.clone(),
            contract_digest: digest,
            source: document.source.clone(),
            prompt: document.prompt.clone(),
            verifier: Some(VerifierFacts {
                program: document.verifier.program.clone(),
                oracle_digest: document.verifier.oracle_digest.clone(),
                negative_control: document.verifier.negative_control.clone(),
                wall_time_ms: document.verifier.wall_time_ms,
            }),
            budget: None,
            run_id: None,
            blocked: None,
            previously_authorized: false,
        }
    }

    /// Authorization is blocked while the contract declares no mechanical check: approval would
    /// grant authority over a result nothing can reject.
    pub fn blocking_items(&self) -> usize {
        usize::from(self.verifier.is_none())
    }

    /// Whether authorizing this contract would start a run.
    ///
    /// A store holds one run, and that is a rule about stores rather than about sessions: where
    /// the next store can be addressed — under a root, by the layout and not by the operator —
    /// the run this authorization starts belongs there, and the store being read is left exactly
    /// as it stands. Where it cannot, because the invocation named one exact store, there is
    /// nowhere for a second run to go and the authorization is not offered.
    pub fn can_start(&self, run: Option<&RunFacts>, in_a_store_of_its_own: bool) -> bool {
        (run.is_none() || in_a_store_of_its_own)
            && self.blocking_items() == 0
            && self.run_id.is_some()
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

/// The state of a task contract, named exactly as the commitment kernel records it.
pub fn contract_state(state: ContractState) -> &'static str {
    match state {
        ContractState::Active => "active",
        ContractState::Returned => "returned",
        ContractState::Cancelled => "cancelled",
    }
}

/// The state of a work obligation, named exactly as the commitment kernel records it.
pub fn obligation_state(state: ObligationState) -> &'static str {
    match state {
        ObligationState::Active => "active",
        ObligationState::Terminal => "terminal",
    }
}

/// How outstanding work ended. A returned obligation names one of these and never "done": a
/// dead end, a decline and an exhausted budget are outcomes of their own, not failures of the
/// same kind.
pub fn commitment_outcome(outcome: &Outcome) -> &'static str {
    match outcome {
        Outcome::Result { .. } => "result",
        Outcome::DeadEnd => "dead_end",
        Outcome::Declined => "declined",
        Outcome::Exhausted => "exhausted",
        Outcome::Cancelled => "cancelled",
        Outcome::InfrastructureError => "infrastructure_error",
    }
}

/// One task contract of the run's commitment kernel, as the journal's facts left it.
#[derive(Clone, Debug)]
pub struct CommitmentFacts {
    pub contract_id: String,
    pub sponsor: String,
    pub contractor: String,
    pub obligation_id: String,
    pub state: &'static str,
    /// Whether the contract can still be advanced. A returned or cancelled one cannot.
    pub active: bool,
    /// The fencing generation of the lease, which every state-changing command must repeat.
    pub generation: u64,
    /// What the contract still holds, dimension by dimension. Only the dimensions that hold
    /// something appear; a dimension at zero is not stated, because it authorizes nothing.
    pub escrow: Vec<(&'static str, u64)>,
    pub candidate_digest: Option<String>,
}

impl CommitmentFacts {
    pub fn from_record(record: &TaskContractRecord) -> Self {
        Self {
            contract_id: record.contract_id.clone(),
            sponsor: record.sponsor.clone(),
            contractor: record.contractor.clone(),
            obligation_id: record.obligation_id.clone(),
            state: contract_state(record.state),
            active: record.state == ContractState::Active,
            generation: record.lease.generation,
            escrow: held(&record.escrow),
            candidate_digest: record.candidate_digest.clone(),
        }
    }
}

/// One work obligation of the run's commitment kernel.
#[derive(Clone, Debug)]
pub struct ObligationFacts {
    pub obligation_id: String,
    pub owner: String,
    /// The obligation this one hangs under. The root of the run hangs under nothing.
    pub parent: Option<String>,
    pub state: &'static str,
    pub outcome: Option<&'static str>,
}

impl ObligationFacts {
    pub fn from_record(record: &ObligationRecord) -> Self {
        Self {
            obligation_id: record.obligation_id.clone(),
            owner: record.owner.clone(),
            parent: record.parent.clone(),
            state: obligation_state(record.state),
            outcome: record.outcome.as_ref().map(commitment_outcome),
        }
    }
}

/// The dimensions a vector actually holds, named by the domain and never by this layer.
fn held(escrow: &BudgetVector) -> Vec<(&'static str, u64)> {
    DIMENSIONS
        .into_iter()
        .filter_map(|dimension| {
            let units = escrow.get(dimension);
            (units > 0).then_some((dimension.as_str(), units))
        })
        .collect()
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
    /// The provider level as the session last read it from the records. It is here so a key on the
    /// provider surfaces resolves the account it acts on from the view state alone, exactly as a
    /// key on the runtimes page resolves its engine.
    pub providers: Option<crate::providers::Report>,
    /// The pool level as the session last read it from the records, here for the same reason: a
    /// key on the pool surfaces resolves the pool and the entry it acts on from the view state
    /// alone, so it can act only on what the operator is standing on.
    pub pools: Option<crate::pools::Report>,
    /// Whether a run authorized now would be given a store of its own, addressed under the root
    /// by the layout, because the store this session is reading already holds one. It is the
    /// session's fact: which stores a root holds is not something the journal records.
    pub addresses_a_store_of_its_own: bool,
    /// The runtime profile this run's work would be done by, when exactly one is settled.
    pub route: Option<String>,
    /// What is true about that routing, in the words the operator is shown: the profile that
    /// would do the work, or why none would.
    pub route_note: String,
    /// The answer the interface is waiting for while a request is being drafted. It is what
    /// makes an empty Enter meaningful: it accepts what the question offers.
    pub awaiting: Option<String>,
    /// What the interface is waiting for away from the thread that draws, while it is. The
    /// screen keeps redrawing under it, and Esc ends it.
    pub working: Option<String>,
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
        let facts = ContractFacts::refused(
            "demo".into(),
            PathBuf::from("/tmp/source"),
            "do the work".into(),
            "the request states no acceptance condition".into(),
        );
        assert_eq!(facts.blocking_items(), 1);
        assert!(!facts.can_start(None, false));
        assert!(
            !facts.can_start(None, true),
            "a store of its own does not make a contract nothing could judge startable"
        );
    }
}
