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
    /// Start the agent on the open run. Irreversible; type the run id to confirm.
    Attempt {
        /// The run id, typed exactly as the interface requires it to be typed.
        #[arg(long, value_name = "RUN_ID")]
        confirm: Option<String>,
        /// Which runtime profile does the work. Omitted takes the only profile that is ready.
        #[arg(long, value_name = "PROFILE")]
        runtime: Option<String>,
    },
    /// End the live run. Irreversible; type the run id to confirm.
    Cancel {
        /// The run id, typed exactly as the interface requires it to be typed.
        #[arg(long, value_name = "RUN_ID")]
        confirm: Option<String>,
    },
    /// Write this run's candidate and the evidence that judged it out of the store.
    Export {
        /// Where to write it. Omitted writes it beside the project, under the run's own name.
        #[arg(long = "to", value_name = "DIR")]
        destination: Option<PathBuf>,
    },
    /// Admit a runtime engine, or stop admitting it. The decision is durable and is read by every
    /// run under this root; nothing about an open run changes here.
    Runtime {
        #[command(subcommand)]
        command: RuntimeCommand,
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

/// What a command states about a runtime engine.
#[derive(Clone, Debug, Subcommand)]
pub enum RuntimeCommand {
    /// Admit this engine again. It can be routed to once its probe reports it ready.
    Enable {
        /// The engine, spelled as the runtimes page spells it.
        engine: String,
    },
    /// Stop admitting this engine. It is not probed, not offered and not routed to.
    Disable {
        /// The engine, spelled as the runtimes page spells it.
        engine: String,
        /// Why it is held back. A refusal repeats this, so an operator is told what to change.
        #[arg(long, value_name = "TEXT")]
        reason: Option<String>,
    },
}

impl RuntimeCommand {
    /// The line the interface reads this decision from. The value is a value: it is handed to the
    /// session exactly as a typed line, and a name that selects no engine is refused there.
    fn line(&self) -> String {
        match self {
            Self::Enable { engine } => format!("runtime enable {engine}"),
            Self::Disable {
                engine,
                reason: None,
            } => format!("runtime disable {engine}"),
            Self::Disable {
                engine,
                reason: Some(reason),
            } => format!("runtime disable {engine} {reason}"),
        }
    }
}

/// The request a command states.
///
/// The work is the request; the other three are amendments of the draft it opens, and each one
/// is handed to the interface as the line a typed amendment would have been. An omitted value
/// means exactly what typing nothing about it means there: the product supplies it — the project
/// directory for the source, a copy of it for the negative control, and a verifier proposed from
/// the way the project runs its tests.
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
    /// Which runtime profile does the work. Omitted takes the only profile that is ready.
    #[arg(long, value_name = "PROFILE")]
    pub runtime: Option<String>,
}

impl RequestArgs {
    fn stated(&self) -> bool {
        self.prompt.is_some()
    }

    /// The amendments the draft receives, in the order a typed dialogue would state them.
    fn amendments(&self) -> Vec<String> {
        let mut lines = Vec::new();
        for (keyword, value) in [
            ("source", &self.source),
            ("verifier", &self.verifier),
            ("negative control", &self.negative_control),
        ] {
            if let Some(path) = value {
                lines.push(format!("{keyword} {}", path.display()));
            }
        }
        lines
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
///
/// `root` is the product root this invocation addressed, when it addressed one rather than naming
/// an exact store. The engines live under it, so a command reads the same decision about them that
/// the interface reads.
pub fn run(
    data_root: PathBuf,
    root: Option<PathBuf>,
    contracts: Vec<PreparedContract>,
    command: PublicCommand,
) -> Result<()> {
    let markers = Markers::detect();
    let mut session = Session::open(&data_root, &contracts);
    if let Some(root) = &root {
        session = session.with_registry_root(root);
    }
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
        PublicCommand::Attempt { confirm, runtime } => {
            run_attempt(&mut session, &mut app, &markers, confirm, runtime)
        }
        PublicCommand::Cancel { confirm } => run_cancel(&mut session, &mut app, &markers, confirm),
        PublicCommand::Export { destination } => {
            run_export(&mut session, &mut app, &markers, destination)
        }
        PublicCommand::Runtime { command } => {
            run_runtime(&mut session, &mut app, &markers, command)
        }
        PublicCommand::Show { page, candidate } => {
            run_show(&mut session, &mut app, &markers, page, candidate)
        }
    }
}

/// State which runtime profile does the work, as the line the interface reads it from.
///
/// The value is a value: it is handed to the session as the line an operator would type, and a
/// name that selects no profile is refused there rather than resolved to the nearest one.
fn select_runtime(session: &mut Session, app: &mut App, runtime: Option<&str>) {
    if let Some(name) = runtime {
        session.local_turn(format!("runtime {name}"));
        app.adopt(session.projection(None));
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
    select_runtime(session, app, request.runtime.as_deref());
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
    session.set_runtimes(ymp_tui::runtimes::probe_all(
        session.registry_address(),
        ymp_tui::runtimes::Measure::Recorded,
    ));
    app.adopt(session.projection(None));
    app.open_authorize_at(index);
    print_modal(app, markers)?;
    Ok(())
}

/// Start the agent on the open run, behind the interface's own typed confirmation.
///
/// A command's process is the wait, so it settles the attempt here: the run's events, the
/// verification its candidate makes possible and the terminal it reaches all happen before this
/// returns. The interface schedules the same steps instead of blocking on them.
fn run_attempt(
    session: &mut Session,
    app: &mut App,
    markers: &Markers,
    confirm: Option<String>,
    runtime: Option<String>,
) -> Result<()> {
    let errors = known_errors(&app.data.entries);
    select_runtime(session, app, runtime.as_deref());
    reject_new_errors(app, &errors)?;
    // Which profile would do the work is read from this host, exactly where the interface reads
    // it: a confirmation that could not name the profile would be asking for a spend nobody
    // could make.
    session.set_runtimes(ymp_tui::runtimes::probe_all(
        session.registry_address(),
        ymp_tui::runtimes::Measure::Recorded,
    ));
    app.adopt(session.projection(None));
    app.open_attempt_confirm();
    if matches!(app.modal, Modal::None) {
        bail!("no attempt was launched — {}", attempt_refusal(&app.data));
    }
    commit(session, app, markers, confirm)?;
    session.settle_attempt();
    app.adopt(session.projection(None));
    print_transcript(app, markers);
    reject_new_errors(app, &errors)?;
    Ok(())
}

/// Why this store offers no attempt to launch.
fn attempt_refusal(projection: &Projection) -> String {
    match &projection.run {
        None => "this store holds no run".to_owned(),
        Some(run) if !run.is_live() => format!(
            "run {} has already ended with the terminal outcome {}",
            run.run_id,
            ymp_tui::projection::outcome(run.status)
        ),
        Some(_) => projection.route_note.clone(),
    }
}

/// Write the run's candidate and the evidence that judged it out of the store.
fn run_export(
    session: &mut Session,
    app: &mut App,
    markers: &Markers,
    destination: Option<PathBuf>,
) -> Result<()> {
    let errors = known_errors(&app.data.entries);
    session.export_evidence(destination);
    app.adopt(session.projection(None));
    print_transcript(app, markers);
    reject_new_errors(app, &errors)?;
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
    select_runtime(session, app, request.runtime.as_deref());
    if request.stated() {
        draft(session, app, &request);
    }
    reject_new_errors(app, &errors)?;

    let index = select(&app.data, contract_id.as_deref())?;
    // This command prints the confirmation rather than the coverage map, and starting the run
    // starts no agent, so it has nothing to route and does not probe. `ymp authorize` states what
    // this host offers, and `ymp attempt` reads it where it decides.
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

    // Which run this store held before the start, so a store that answers with the run it already
    // held cannot be read as a run this command started.
    let held_before = app.data.run.as_ref().map(|run| run.run_id.clone());

    let ConfirmAction::StartRun { run_id, .. } = commit(session, app, markers, confirm)? else {
        bail!("the interface confirmed something other than the start of a run");
    };

    // A run is identified by its contract and by the store that holds it. This command addresses
    // the store itself, so it knows that identifier before the start and holds the start to it.
    // Where the interface could not name the store in advance, the run the store now holds is
    // required to be one this command did not find there.
    match &app.data.run {
        Some(run)
            if run_id
                .as_ref()
                .is_none_or(|expected| &run.run_id == expected)
                && held_before.as_ref() != Some(&run.run_id) =>
        {
            Ok(())
        }
        _ => match run_id {
            Some(expected) => bail!("no run was started — the store holds no run under {expected}"),
            None => bail!("no run was started — the store this authorization addressed holds none"),
        },
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

/// Admit an engine or stop admitting it, as the interface's own line does.
///
/// Nothing is probed: the decision is about whether an engine may be started at all, and probing
/// to record a decision that forbids probing would be starting the very engine being held back.
/// A name that selects no engine is refused by the session and is a non-zero exit here.
fn run_runtime(
    session: &mut Session,
    app: &mut App,
    markers: &Markers,
    command: RuntimeCommand,
) -> Result<()> {
    let errors = known_errors(&app.data.entries);
    session.local_turn(command.line());
    app.adopt(session.projection(None));
    print_transcript(app, markers);
    reject_new_errors(app, &errors)?;
    Ok(())
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
        // This page is where the engines are looked at, so it is where a model list nobody has
        // measured against the installed build is measured.
        session.set_runtimes(ymp_tui::runtimes::probe_all(
            session.registry_address(),
            ymp_tui::runtimes::Measure::Catalog,
        ));
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
        ConfirmAction::StartAttempt { .. } => Action::StartAttempt,
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
        Action::StartAttempt => session.start_attempt(),
        Action::ExportEvidence(destination) => session.export_evidence(destination),
        Action::LocalTurn(text) => session.local_turn(text),
        Action::SetEngineEnabled {
            engine,
            enabled,
            reason,
        } => session.set_engine_enabled(engine, enabled, reason),
        // A command has no second thread to wait on: its own process is the check, and it has
        // already finished by the time anything could ask for it to be abandoned.
        Action::CancelCheck => session.cancel_check(),
        // The interface rebuilds its projection; a command builds one per invocation and has
        // nothing to commit for it.
        Action::Rebuild => {}
    }
}

/// Hand the stated request to the interface, one line per value it states.
///
/// Each line is the local turn the interface's event loop performs for a completed line, and
/// nothing else: the value is a value, so a slash, a colon, an escape or any other character in
/// it is part of the line rather than a key that could open another surface.
///
/// The interface assembles the draft after every line, and a line supersedes the assembly the
/// line before it started. A command states everything it has at once, so only the last of them
/// is worth the work: the earlier ones are taken and superseded, and the assembly that runs is
/// the one for the request as fully stated. That is the same path the interface takes when an
/// operator types a second line before the first has finished.
fn draft(session: &mut Session, app: &mut App, request: &RequestArgs) {
    let mut pending = None;
    for line in
        std::iter::once(request.prompt.clone().unwrap_or_default()).chain(request.amendments())
    {
        let before = known_errors(&app.data.entries).len();
        pending = session.begin_turn(line);
        app.adopt(session.projection(None));
        // A line naming something this host cannot take ends the request where it was stated.
        // The lines after it would be stated against a draft that already cannot be assembled,
        // and the refusal the operator is shown must be the one that caused it.
        if known_errors(&app.data.entries).len() > before {
            pending = None;
            break;
        }
    }
    if let Some(pending) = pending {
        session.finish_check(pending.run());
    }
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
    use super::{PageName, PathBuf, RequestArgs};
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

    /// An omitted value amends nothing, so the product supplies it. A stated one is the line a
    /// typed amendment would have been, word for word.
    #[test]
    fn an_omitted_request_value_amends_nothing_and_a_stated_one_is_the_line_it_would_be() {
        assert!(
            RequestArgs {
                prompt: Some("keep the replay path idempotent".to_owned()),
                ..RequestArgs::default()
            }
            .amendments()
            .is_empty()
        );
        assert_eq!(
            RequestArgs {
                prompt: Some("keep the replay path idempotent".to_owned()),
                verifier: Some(PathBuf::from("/tmp/verify.sh")),
                ..RequestArgs::default()
            }
            .amendments(),
            vec!["verifier /tmp/verify.sh".to_owned()]
        );
    }
}
