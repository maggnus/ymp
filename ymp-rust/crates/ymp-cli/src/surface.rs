//! Every action the terminal interface offers, as a command of the same executable.
//!
//! The terminal interface is a convenient way to reach the system core, not the only way. What
//! this module adds is a second way to *reach* the actions, never a second implementation of
//! them: a command opens the interface's own session over the same data root, hands it the
//! answers the operator stated, and commits an irreversible action only behind the interface's
//! own decision surface. The result is that a command cannot start a run the interface would
//! refuse, cannot skip a confirmation the interface enforces, and cannot reach the journal by
//! any path the interface does not take.
//!
//! An argument value is a value, never a key press. The interface's input row reads a leading
//! slash as its command line and a control character as a key of its own, so a value carried
//! through that row could open a surface the command never asked for and complete a decision
//! standing behind it. Nothing here goes through the input row: an answer is passed to
//! [`ymp_tui::Session::local_turn`], which is exactly what the interface's event loop passes it,
//! and a confirmation is compared by the modal's own predicate. `tests/one_command_path.rs`
//! rejects a module of this surface that names the key channel at all.
//!
//! Three properties hold by construction rather than by review:
//!
//! * [`perform`] matches the interface's action vocabulary exhaustively, so an action added to
//!   the interface stops this crate from compiling until a command serves it;
//! * an irreversible action is committed only from the interface's own confirmation, and only
//!   while that confirmation reports the typed identifier as exact;
//! * this module never names the kernel writer. Durable state is reached only through
//!   `ymp_tui::Session`, and `tests/one_command_path.rs` rejects a module of the public surface
//!   that names the writer, the journal or the object store directly.
//!
//! Reporting is the interface's own composition too: a command prints the surface the interface
//! would have drawn, so the two surfaces cannot drift into showing different facts. A refusal
//! the interface states as an error becomes a non-zero exit here, because a command is judged by
//! its exit rather than read by an operator.

use std::collections::BTreeSet;
use std::path::PathBuf;

use anyhow::{Result, bail};
use clap::{Args, Subcommand, ValueEnum};
use ratatui::layout::Rect;
use ratatui::text::Line;
use ymp_application::PreparedContract;
use ymp_domain::RunStatus;
use ymp_tui::app::Action;
use ymp_tui::projection::{ContractFacts, Projection};
use ymp_tui::state::{App, ConfirmAction, Modal, PageKind, Surface};
use ymp_tui::theme::Markers;
use ymp_tui::transcript::Entry;
use ymp_tui::{Session, decisions, overlay, ui};

/// The width a command lays its surfaces out at. The interface uses the terminal; a command
/// writes to a pipe, so the width is fixed and wide enough for the widest column set.
pub const WIDTH: u16 = 120;

/// The height a conversation is laid out at: one screenful, so a command shows the same tail of
/// the transcript the interface shows after the same action.
pub const TRANSCRIPT_HEIGHT: u16 = 40;

/// The height a data page is laid out at. A page windows its rows around the selection; a
/// command has no way to scroll, so it lays the page out tall enough that no row is left outside
/// the window.
pub const PAGE_HEIGHT: u16 = 4_000;

/// The commands that mirror the interface, one per action it offers.
#[derive(Debug, Subcommand)]
pub enum PublicCommand {
    /// Draft a contract from a request. Nothing is started and nothing is spent.
    Request {
        #[command(flatten)]
        request: RequestArgs,
    },
    /// Review what a contract would have checked, before anything is spent.
    Authorize {
        /// Which contract to review, when this invocation carries more than one.
        contract_id: Option<String>,
        #[command(flatten)]
        request: RequestArgs,
    },
    /// Store a contract and start the run it names. Irreversible; type the contract id to confirm.
    Start {
        /// Which contract to start, when this invocation carries more than one.
        contract_id: Option<String>,
        /// The contract id, typed exactly as the interface requires it to be typed.
        #[arg(long, value_name = "CONTRACT_ID")]
        confirm: Option<String>,
        #[command(flatten)]
        request: RequestArgs,
    },
    /// End the live run. Irreversible; type the run id to confirm.
    Cancel {
        /// The run id, typed exactly as the interface requires it to be typed.
        #[arg(long, value_name = "RUN_ID")]
        confirm: Option<String>,
    },
    /// Print one data page, exactly as the interface lays it out.
    Show {
        /// Which page to print.
        page: PageName,
        /// Which candidate the describe page states.
        #[arg(long, value_name = "INDEX")]
        candidate: Option<usize>,
    },
}

/// The request a command states, in the order the interface asks for it.
///
/// The four values are the four the interface's draft collects and no others: the work, the
/// directory it is done in, the program that decides a candidate, and the deliberately wrong
/// candidate that program must reject. Each one is handed to the draft as the answer a typed
/// line would have given, so an omitted value means exactly what an empty answer means there —
/// the project directory for the source, and no acceptance condition for the other two.
#[derive(Args, Clone, Debug, Default)]
pub struct RequestArgs {
    /// The work, in your own words.
    #[arg(long)]
    pub prompt: Option<String>,
    /// The directory the work is done in. Omitted accepts the project directory.
    #[arg(long)]
    pub source: Option<PathBuf>,
    /// The program that decides whether a candidate is accepted.
    #[arg(long)]
    pub verifier: Option<PathBuf>,
    /// The deliberately wrong candidate that program must reject.
    #[arg(long = "negative-control")]
    pub negative_control: Option<PathBuf>,
}

impl RequestArgs {
    fn stated(&self) -> bool {
        self.prompt.is_some()
    }

    /// The answers the draft receives, in the order it asks its questions.
    fn answers(&self) -> [String; 3] {
        let text = |value: &Option<PathBuf>| {
            value
                .as_ref()
                .map(|path| path.display().to_string())
                .unwrap_or_default()
        };
        [
            text(&self.source),
            text(&self.verifier),
            text(&self.negative_control),
        ]
    }
}

/// The data pages, named as the interface names them in its command palette.
#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum PageName {
    Runtimes,
    Candidates,
    Events,
    Budgets,
    Attempts,
    Describe,
}

impl PageName {
    pub fn kind(self) -> PageKind {
        match self {
            Self::Runtimes => PageKind::Runtimes,
            Self::Candidates => PageKind::Candidates,
            Self::Events => PageKind::Events,
            Self::Budgets => PageKind::Budgets,
            Self::Attempts => PageKind::Attempts,
            Self::Describe => PageKind::Describe,
        }
    }

    /// The command for one page of the interface. The match is exhaustive: a page added to the
    /// interface stops this crate from compiling until a command shows it.
    pub fn of(kind: PageKind) -> Self {
        match kind {
            PageKind::Runtimes => Self::Runtimes,
            PageKind::Candidates => Self::Candidates,
            PageKind::Events => Self::Events,
            PageKind::Budgets => Self::Budgets,
            PageKind::Attempts => Self::Attempts,
            PageKind::Describe => Self::Describe,
        }
    }
}

/// Run one command of the public surface.
pub fn run(
    data_root: PathBuf,
    contracts: Vec<PreparedContract>,
    command: PublicCommand,
) -> Result<()> {
    let markers = Markers::detect();
    let mut session = Session::open(&data_root, &contracts);
    let mut app = App::new(session.projection(None));

    match command {
        PublicCommand::Request { request } => {
            run_request(&mut session, &mut app, &markers, request)
        }
        PublicCommand::Authorize {
            contract_id,
            request,
        } => run_authorize(&mut session, &mut app, &markers, contract_id, request),
        PublicCommand::Start {
            contract_id,
            confirm,
            request,
        } => run_start(
            &mut session,
            &mut app,
            &markers,
            contract_id,
            confirm,
            request,
        ),
        PublicCommand::Cancel { confirm } => run_cancel(&mut session, &mut app, &markers, confirm),
        PublicCommand::Show { page, candidate } => {
            run_show(&mut session, &mut app, &markers, page, candidate)
        }
    }
}

/// Draft a contract from a request, as a typed line does in the interface.
fn run_request(
    session: &mut Session,
    app: &mut App,
    markers: &Markers,
    request: RequestArgs,
) -> Result<()> {
    if !request.stated() {
        bail!("nothing was drafted — state the work with --prompt");
    }
    let known = contract_ids(&app.data);
    let errors = known_errors(&app.data.entries);
    draft(session, app, &request);
    print_transcript(app, markers);
    reject_new_errors(app, &errors)?;
    if drafted(&app.data, &known).is_none() {
        bail!("no contract was drafted from this request");
    }
    Ok(())
}

/// Review the coverage of one contract, as the interface's authorization surface states it.
fn run_authorize(
    session: &mut Session,
    app: &mut App,
    markers: &Markers,
    contract_id: Option<String>,
    request: RequestArgs,
) -> Result<()> {
    let errors = known_errors(&app.data.entries);
    if request.stated() {
        draft(session, app, &request);
    } else {
        app.adopt(session.projection(None));
    }
    reject_new_errors(app, &errors)?;

    let index = select(&app.data, contract_id.as_deref())?;
    // The review surface names the runtime profiles a run would be routed to, so the command
    // probes them where the interface does — and only once there is a review to state, since
    // probing starts subprocesses and a refused request has nothing to route.
    session.set_runtimes(ymp_tui::runtimes::probe_all());
    app.adopt(session.projection(None));
    app.open_authorize_at(index);
    print_modal(app, markers)?;
    Ok(())
}

/// Store a contract and start its run, behind the interface's own typed confirmation.
fn run_start(
    session: &mut Session,
    app: &mut App,
    markers: &Markers,
    contract_id: Option<String>,
    confirm: Option<String>,
    request: RequestArgs,
) -> Result<()> {
    let errors = known_errors(&app.data.entries);
    if request.stated() {
        draft(session, app, &request);
    }
    reject_new_errors(app, &errors)?;

    let index = select(&app.data, contract_id.as_deref())?;
    app.open_authorize_at(index);
    // The coverage map gives way to the typed confirmation only when the projection says this
    // contract can start a run. When it cannot, the map states why and nothing is started. This
    // is the transition the interface makes on Enter, made here without a key.
    let Modal::Authorize(authorize) = &app.modal else {
        bail!("the interface offers no authorization surface for this contract");
    };
    let Some(action) = authorize.action.clone() else {
        bail!("no run was started — {}", authorize.action_note);
    };
    app.modal = Modal::Confirm(decisions::start_run(&action));

    let ConfirmAction::StartRun { run_id, .. } = commit(session, app, markers, confirm)? else {
        bail!("the interface confirmed something other than the start of a run");
    };

    match &app.data.run {
        Some(run) if run.run_id == run_id => Ok(()),
        _ => bail!("no run was started — the store holds no run under {run_id}"),
    }
}

/// End the live run, behind the interface's own typed confirmation.
fn run_cancel(
    session: &mut Session,
    app: &mut App,
    markers: &Markers,
    confirm: Option<String>,
) -> Result<()> {
    app.open_cancel_confirm();
    if matches!(app.modal, Modal::None) {
        bail!("nothing was cancelled — this store holds no live run");
    }
    commit(session, app, markers, confirm)?;

    match &app.data.run {
        Some(run) if run.status == RunStatus::Cancelled => Ok(()),
        _ => bail!("the run was not cancelled"),
    }
}

/// Print one data page.
fn run_show(
    session: &mut Session,
    app: &mut App,
    markers: &Markers,
    page: PageName,
    candidate: Option<usize>,
) -> Result<()> {
    let kind = page.kind();
    if kind == PageKind::Runtimes {
        session.set_runtimes(ymp_tui::runtimes::probe_all());
    }
    let describe = (kind == PageKind::Describe).then(|| candidate.unwrap_or(0));
    app.adopt(session.projection(describe));
    app.surface = Surface::Page(kind);
    let missing = app.page(kind).is_none();
    print_surface(app, markers, PAGE_HEIGHT);
    if missing {
        bail!(
            "nothing was shown — this store has no {} page",
            kind.command_name()
        );
    }
    Ok(())
}

/// Put the stated confirmation to the interface's own modal and commit only what it accepts.
///
/// The modal decides: it reports the identifier as exact or it does not, and an absent or wrong
/// confirmation commits nothing here for the same reason it commits nothing in the interface.
fn commit(
    session: &mut Session,
    app: &mut App,
    markers: &Markers,
    confirm: Option<String>,
) -> Result<ConfirmAction> {
    let Modal::Confirm(pending) = &mut app.modal else {
        bail!("the interface offers no confirmation for this action");
    };
    pending.typed = confirm.unwrap_or_default();
    let required = pending.required.clone();
    let confirmed = pending.action.clone();
    let exact = pending.is_exact();
    print_modal(app, markers)?;

    if !exact {
        bail!(
            "nothing was committed — confirm with --confirm {required}: the identifier must match \
             exactly, as it must be typed in the interface"
        );
    }
    let errors = known_errors(&app.data.entries);
    perform(session, committed(&confirmed));
    app.adopt(session.projection(None));
    print_transcript(app, markers);
    reject_new_errors(app, &errors)?;
    Ok(confirmed)
}

/// The action an exact confirmation commits, as the interface commits it.
///
/// The match is exhaustive over the confirmations the interface can raise, so a third one cannot
/// be added there without being decided here.
fn committed(confirmed: &ConfirmAction) -> Action {
    match confirmed {
        ConfirmAction::StartRun { contract_id, .. } => Action::StartRun(contract_id.clone()),
        ConfirmAction::CancelRun { .. } => Action::CancelRun,
    }
}

/// Execute one action of the interface's vocabulary.
///
/// The match is exhaustive over that vocabulary. Adding an action there without adding a command
/// here is a compilation failure, not a review finding.
fn perform(session: &mut Session, action: Action) {
    match action {
        Action::CancelRun => session.cancel_run(),
        Action::StartRun(contract_id) => session.start_run(&contract_id),
        Action::LocalTurn(text) => session.local_turn(text),
        // The interface rebuilds its projection; a command builds one per invocation and has
        // nothing to commit for it.
        Action::Rebuild => {}
    }
}

/// Hand the stated request to the interface, one answer per question it asks.
///
/// Each answer is the local turn the interface's event loop performs for a completed line, and
/// nothing else: the value is a value, so a colon, a slash, an escape or any other character in
/// it is part of the answer rather than a key that could open another surface. The session
/// decides what the answer means, and the loop stops as soon as it is no longer awaiting one.
fn draft(session: &mut Session, app: &mut App, request: &RequestArgs) {
    answer(session, app, request.prompt.clone().unwrap_or_default());
    for stated in request.answers() {
        if app.data.awaiting.is_none() {
            break;
        }
        answer(session, app, stated);
    }
}

fn answer(session: &mut Session, app: &mut App, text: String) {
    perform(session, Action::LocalTurn(text));
    app.adopt(session.projection(None));
}

/// Which contract a command acts on: the one it names, or the only one it carries.
fn select(projection: &Projection, contract_id: Option<&str>) -> Result<usize> {
    let available: Vec<&str> = projection
        .contracts
        .iter()
        .map(|contract| contract.contract_id.as_str())
        .collect();
    match contract_id {
        Some(wanted) => available
            .iter()
            .position(|identifier| *identifier == wanted)
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "no contract named {wanted} is available to this command; available: {}",
                    list(&available)
                )
            }),
        None => match available.len() {
            1 => Ok(0),
            0 => bail!(
                "no contract is available to this command — state a request with --prompt or name \
                 a package with --contract"
            ),
            _ => bail!(
                "this command carries {} contracts; name the one it acts on: {}",
                available.len(),
                list(&available)
            ),
        },
    }
}

fn list(available: &[&str]) -> String {
    available.join(", ")
}

fn contract_ids(projection: &Projection) -> BTreeSet<String> {
    projection
        .contracts
        .iter()
        .map(|contract| contract.contract_id.clone())
        .collect()
}

/// The contract a request drafted, when it produced one that can still start a run.
fn drafted<'a>(projection: &'a Projection, known: &BTreeSet<String>) -> Option<&'a ContractFacts> {
    projection.contracts.iter().find(|contract| {
        !known.contains(&contract.contract_id)
            && contract.blocked.is_none()
            && contract.run_id.is_some()
    })
}

fn known_errors(entries: &[Entry]) -> Vec<String> {
    entries
        .iter()
        .filter_map(|entry| match entry {
            Entry::AppError { text } => Some(text.clone()),
            _ => None,
        })
        .collect()
}

/// A refusal the interface states as an error is a non-zero exit here.
fn reject_new_errors(app: &App, before: &[String]) -> Result<()> {
    let after = known_errors(&app.data.entries);
    if after.len() > before.len() {
        bail!("{}", after[after.len() - 1]);
    }
    Ok(())
}

fn print_transcript(app: &mut App, markers: &Markers) {
    app.surface = Surface::Transcript;
    print_surface(app, markers, TRANSCRIPT_HEIGHT);
}

fn print_surface(app: &App, markers: &Markers, height: u16) {
    let spec = ui::surface_spec(app, Rect::new(0, 0, WIDTH, height), markers);
    println!(
        "{}  {}",
        Line::from(spec.header_left),
        Line::from(spec.header_right)
    );
    for line in spec.body {
        println!("{line}");
    }
    println!("{}", Line::from(spec.status_left));
}

fn print_modal(app: &App, markers: &Markers) -> Result<()> {
    let Some(spec) = overlay::modal_spec(app, Rect::new(0, 0, WIDTH, PAGE_HEIGHT), markers) else {
        bail!("the interface offers no decision surface for this command");
    };
    println!("{} · {}", spec.title, spec.badge);
    for line in spec.body {
        println!("{line}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{PageName, RequestArgs};
    use clap::ValueEnum;
    use ymp_tui::state::PageKind;

    #[test]
    fn every_page_of_the_interface_is_named_by_the_command_that_shows_it() {
        for kind in PageKind::ALL {
            let name = PageName::of(kind);
            assert_eq!(name.kind(), kind);
            let value = name
                .to_possible_value()
                .expect("every page name is selectable");
            assert_eq!(
                value.get_name(),
                kind.command_name(),
                "the command names this page differently from the interface"
            );
        }
    }

    #[test]
    fn an_omitted_request_value_is_the_empty_answer_the_interface_receives() {
        let answers = RequestArgs {
            prompt: Some("keep the replay path idempotent".to_owned()),
            ..RequestArgs::default()
        }
        .answers();
        assert_eq!(answers, ["".to_owned(), "".to_owned(), "".to_owned()]);
    }
}
