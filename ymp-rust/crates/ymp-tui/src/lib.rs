//! The ymp terminal interface.
//!
//! The main screen is a conversation: one scrolling transcript, an input line, a thin header
//! and a status line. Full-screen data pages open over it through a `/` command line in the k9s
//! tradition, and a decision modal interrupts the conversation only for an irreversible
//! command. The composition follows `ymp-docs/design/ymp_chat_tui.dc.html`; the reasoning is in
//! `ymp-docs/VISUAL_CONCEPT.md`.
//!
//! # From a typed request to a run
//!
//! A store with no run answers a typed line as a request. [`draft`] assembles the rest of it
//! from the project — a copy of the project as the negative control, and a verifier proposed
//! from the way it already runs its tests — asks that verifier to reject the copy and to accept
//! a sample built to satisfy it, and hands the assembled request to `ymp-application`, which
//! validates it, computes the oracle digest from the verifier program itself and produces the
//! contract bytes. While the draft is unauthorized, the next line amends it. The coverage map
//! shows what would be checked; a confirmation stores the contract and starts the run. What the
//! product cannot propose, it says it cannot propose, and it never invents an acceptance
//! condition, so the interface and the equivalent command refuse identically.
//!
//! # From a started run to exported evidence
//!
//! Starting the run and starting the agent grant different things and are authorized separately.
//! [`attempt`] settles which runtime profile does the work — the one the operator named, or the
//! only one this host can start — and a profile that is not ready stops the attempt rather than
//! being replaced by another. A second confirmation launches it through `ymp-runtime-supervisor`,
//! which holds the private workspace, the runtime evidence and the kernel record of the process
//! slice; the interface holds that attempt, which is what makes both records of how a run ended
//! reachable from here. The candidate the agent commits is judged by the verifier the stored
//! contract names, the verdict and the terminal reach the journal, and an export writes the exact
//! candidate and the evidence that judged it out of the store.
//!
//! # Layers
//!
//! * [`projection`], [`journal`], [`runtimes`], [`decisions`] read the application, the domain
//!   and the runtime drivers and produce every value that reaches the screen;
//! * [`state`] holds the operator's position — surface, selection, scroll, input;
//! * [`ui`] and [`overlay`] compose those into frame specifications;
//! * [`frame`] is the only module that draws.
//!
//! # Deliberate divergence from the drawn reference
//!
//! The drawing shows the target system; this domain lags it, and the gaps are stated rather
//! than filled in. There is no intent, contract package, interview, participant, board message
//! or five-dimension budget here, so no screen shows one. Journal events carry a sequence and
//! no wall-clock time, so rows are keyed by sequence. The participants page is named
//! `attempts`, because attempts are what the journal records. The palette is drawn as one
//! bordered box rather than two.

use std::path::Path;

use ymp_application::PreparedContract;

pub mod app;
pub mod attempt;
pub mod decisions;
pub mod draft;
pub mod frame;
pub mod journal;
pub mod overlay;
pub mod pages;
pub mod pools;
pub mod projection;
pub mod providers;
pub mod runtimes;
pub mod scenario;
pub mod state;
pub mod style;
pub mod terminal;
pub mod text;
pub mod theme;
pub mod transcript;
pub mod ui;

pub use app::Session;
pub use state::{App, Follow, Modal, PageKind, Surface};
pub use transcript::{Entry, MessageKind, Plane};

/// Start the interface against a data root, with no managed contracts configured.
pub fn run(data_root: impl AsRef<Path>) -> anyhow::Result<()> {
    run_with_contracts(data_root, Vec::new())
}

/// Start the interface over contracts the application already validated.
///
/// The command line hands over prepared contracts rather than files, so a package reaches the
/// kernel through the same scenario a typed request does and is refused before the interface
/// opens if it states no acceptance condition.
pub fn run_with_contracts(
    data_root: impl AsRef<Path>,
    contracts: Vec<PreparedContract>,
) -> anyhow::Result<()> {
    let session = Session::open(data_root.as_ref(), &contracts);
    app::run(session, theme::Markers::detect())
}

/// Start the interface over a store addressed under a root.
///
/// A store holds one run. Knowing the root is what lets the second run an operator authorizes in
/// one session be addressed rather than refused: the product chooses the next store, and the run
/// the session was reading is left exactly as it stands.
pub fn run_under_root(
    root: impl AsRef<Path>,
    data_root: impl AsRef<Path>,
    contracts: Vec<PreparedContract>,
) -> anyhow::Result<()> {
    let session = Session::open_under_root(root.as_ref(), data_root.as_ref(), &contracts);
    app::run(session, theme::Markers::detect())
}
