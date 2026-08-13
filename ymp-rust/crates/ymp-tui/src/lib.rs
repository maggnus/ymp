//! The ymp terminal interface.
//!
//! The main screen is a conversation: one scrolling transcript, an input line, a thin header
//! and a status line. Full-screen data pages open over it through a `:` command line in the k9s
//! tradition, and a decision modal interrupts the conversation only for an irreversible
//! command. The composition follows `ymp-docs/design/ymp_chat_tui.dc.html`; the reasoning is in
//! `ymp-docs/VISUAL_CONCEPT.md`.
//!
//! # From a typed request to a run
//!
//! A store with no run answers a typed line as a request. [`draft`] collects what the kernel
//! cannot infer — the source directory and the acceptance condition — and hands the assembled
//! request to `ymp-application`, which validates it, computes the oracle digest from the
//! verifier program itself and produces the contract bytes. The coverage map shows what would
//! be checked; a typed confirmation stores the contract and starts the run. A request with no
//! acceptance condition is refused by the application, which names the missing part, so the
//! interface and the equivalent command refuse identically.
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

use ymp_runtime_supervisor::ManagedContract;

pub mod app;
pub mod decisions;
pub mod draft;
pub mod frame;
pub mod journal;
pub mod overlay;
pub mod pages;
pub mod projection;
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

/// Start the interface. The signature is the one `ymp-cli` calls.
pub fn run_with_contracts(
    data_root: impl AsRef<Path>,
    contracts: Vec<ManagedContract>,
) -> anyhow::Result<()> {
    let session = Session::open(data_root.as_ref(), &contracts);
    app::run(session, theme::Markers::detect())
}
