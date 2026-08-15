#![forbid(unsafe_code)]

//! The `ymp` executable: one terminal interface and one command for every action it offers.
//!
//! Invoked with no command, the executable opens the interface. Invoked with one of the public
//! commands, it performs the same action the interface performs, through the same session and
//! the same confirmations — see [`surface`]. The [`internal`] namespace is the product's own
//! machinery and is deliberately not part of that correspondence.

use anyhow::{Context, bail};
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use ymp_application::PreparedContract;
use ymp_application::root::{StoreIntent, default_root, refuse_earlier_layout, store_under};

pub mod internal;
pub mod surface;

#[derive(Debug, Parser)]
#[command(
    name = "ymp",
    version,
    about = "Bounded local coordination for coding agents"
)]
pub struct Cli {
    /// The one root every durable path lives under. A project and a run are addressed inside it,
    /// so a second project and a second run need no name from you. Named nowhere, it is your
    /// `$YMP_HOME`, and failing that `~/.ymp`: the directory you start the product in is yours,
    /// and nothing of the product's is written into it.
    #[arg(
        long,
        global = true,
        value_name = "DIR",
        help = "The root every durable path lives under [default: $YMP_HOME, else ~/.ymp]"
    )]
    pub root: Option<PathBuf>,
    /// One exact store, addressed by hand rather than under a root: a store an earlier build
    /// wrote, or one an operator keeps apart on purpose.
    #[arg(
        long,
        global = true,
        value_name = "DIR",
        conflicts_with = "root",
        help = "Act on exactly this store instead of one addressed under the root"
    )]
    pub data_root: Option<PathBuf>,
    #[arg(
        long,
        global = true,
        value_name = "FILE",
        help = "Load a contract package; may be repeated"
    )]
    pub contract: Vec<PathBuf>,
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// The commands that mirror the interface, one per action it offers.
    #[command(flatten)]
    Public(surface::PublicCommand),
    /// The product's own machinery. Nothing here mirrors an interface action.
    Internal {
        #[command(subcommand)]
        command: internal::InternalCommand,
    },
}

pub fn run(cli: Cli) -> anyhow::Result<()> {
    // The agent bridge addresses no store. It reads its capability from the environment and
    // reaches durable state only through the controller that started it, and it runs with the
    // agent's own workspace as its working directory — so addressing a store here would create
    // one inside the very tree the agent is producing, and the candidate captured from that
    // workspace would carry it. `tests/the_bridge_leaves_no_store_in_the_workspace.rs` drives the
    // built product to establish that it does not.
    if let Some(Command::Internal {
        command: internal::InternalCommand::AgentMcp,
    }) = &cli.command
    {
        return internal::run(PathBuf::new(), &[], internal::InternalCommand::AgentMcp);
    }
    let addressed = addressed(&cli)?;
    let store = match &addressed {
        Addressed::Store(store) => store.clone(),
        Addressed::Root(root) => store_under(root, intent(cli.command.as_ref()))?,
    };
    match cli.command {
        // The interface is given the root as well as the store. A store holds one run, so the
        // second run an operator authorizes in one session is addressed under the root rather
        // than refused; an invocation that named one exact store named what it acts on.
        None => match addressed {
            Addressed::Root(root) => {
                ymp_tui::run_under_root(root, store, load_contracts(&cli.contract)?)
            }
            Addressed::Store(_) => {
                ymp_tui::run_with_contracts(store, load_contracts(&cli.contract)?)
            }
        },
        Some(Command::Public(command)) => {
            surface::run(store, load_contracts(&cli.contract)?, command)
        }
        Some(Command::Internal { command }) => internal::run(store, &cli.contract, command),
    }
}

/// What this invocation acts on: a root it addresses a store under, or one exact store it was
/// given by name.
///
/// Everything below the interface and the commands still receives a store directory, exactly as
/// before. What changed is who chooses it: the operator no longer invents a directory per run,
/// because the root addresses a project and a run for them.
enum Addressed {
    Root(PathBuf),
    Store(PathBuf),
}

fn addressed(cli: &Cli) -> anyhow::Result<Addressed> {
    if let Some(store) = &cli.data_root {
        return Ok(Addressed::Store(store.clone()));
    }
    if let Some(root) = &cli.root {
        return Ok(Addressed::Root(root.clone()));
    }
    let root = default_root()?;
    // Only the default root asks this question. An operator who named a root has answered it,
    // and an operator who named a store is already reading one where it stands.
    refuse_earlier_layout(&root)?;
    Ok(Addressed::Root(root))
}

/// Whether this invocation acts on the project's current store or starts a run in a fresh one.
///
/// Only a command that commits a run start needs a store holding none. Everything else — reading
/// a page, drafting a request, reviewing coverage, cancelling — belongs to the run the project is
/// already on, so it must not be handed an empty store.
///
/// The interface reads the current store too. It can start a run there when that store holds
/// none, and refuses a second one when it does; addressing a fresh store for it instead would
/// hide the run the operator opened the interface to look at.
fn intent(command: Option<&Command>) -> StoreIntent {
    match command {
        Some(Command::Public(surface::PublicCommand::Start { .. })) => StoreIntent::New,
        Some(Command::Internal {
            command:
                internal::InternalCommand::ManagedRuntimeSmoke { .. }
                | internal::InternalCommand::ManagedCandidateSmoke { .. },
        }) => StoreIntent::New,
        _ => StoreIntent::Current,
    }
}

/// Every contract the command line named, validated by the one scenario that starts runs.
///
/// A package that states no acceptance condition is refused here, before the interface opens
/// and before any store is touched, with the missing part named.
pub fn load_contracts(paths: &[PathBuf]) -> anyhow::Result<Vec<PreparedContract>> {
    paths
        .iter()
        .map(|path| {
            ymp_application::load_contract_package(path)
                .with_context(|| format!("load contract package {}", path.display()))
        })
        .collect()
}

/// The single contract a run is started against. A start with no contract is refused: a run
/// nothing could judge is not started, not even by an internal command.
pub fn one_contract(paths: &[PathBuf]) -> anyhow::Result<PreparedContract> {
    let mut contracts = load_contracts(paths)?;
    match contracts.len() {
        1 => Ok(contracts.remove(0)),
        0 => bail!("no run started — pass --contract <file>: a run is started against a contract"),
        count => bail!("a run is started against one contract; {count} were given"),
    }
}

#[cfg(test)]
mod tests {
    use super::{Cli, Command, StoreIntent, intent};
    use clap::{CommandFactory, Parser};

    #[test]
    fn cli_default_invocation_names_no_directory_of_its_own() {
        let cli = Cli::try_parse_from(["ymp"]).expect("default invocation");
        assert!(
            cli.root.is_none() && cli.data_root.is_none(),
            "the default invocation carries a directory the operator did not state"
        );
    }

    #[test]
    fn cli_refuses_a_root_and_a_store_in_one_invocation() {
        Cli::try_parse_from(["ymp", "--root", ".ymp", "--data-root", ".ymp-data"])
            .expect_err("a root and a store name two different things to act on");
    }

    #[test]
    fn only_a_run_start_is_addressed_to_a_store_that_holds_none() {
        let start = Cli::try_parse_from(["ymp", "start", "contract-1"]).expect("start invocation");
        assert_eq!(intent(start.command.as_ref()), StoreIntent::New);

        for reader in [
            vec!["ymp", "show", "events"],
            vec!["ymp", "cancel"],
            vec!["ymp", "authorize", "contract-1"],
            vec!["ymp", "request", "--prompt", "work"],
        ] {
            let cli = Cli::try_parse_from(&reader).expect("reader invocation");
            assert_eq!(
                intent(cli.command.as_ref()),
                StoreIntent::Current,
                "`{}` was addressed to a store holding no run",
                reader.join(" ")
            );
        }

        // The interface reads the store the project is on, so an operator who opens it sees the
        // run they started rather than an empty one.
        let interface: Option<&Command> = None;
        assert_eq!(intent(interface), StoreIntent::Current);
    }

    #[test]
    fn cli_public_help_offers_no_surface_outside_the_product_vocabulary() {
        let mut command = Cli::command();
        let help = command.render_long_help().to_string().to_lowercase();
        for forbidden in ["demo", "inspect", "probe", "daemon", "socket", "headless"] {
            assert!(
                !help
                    .split(|character: char| !character.is_alphanumeric() && character != '-')
                    .any(|word| word == forbidden),
                "public help exposes forbidden command {forbidden}:\n{help}"
            );
        }
    }

    #[test]
    fn cli_default_invocation_selects_the_foreground_mode() {
        let cli = Cli::try_parse_from(["ymp"]).expect("default invocation");
        assert!(cli.command.is_none());
    }

    #[test]
    fn cli_internal_children_are_nested_below_the_internal_namespace() {
        let command = Cli::command();
        let internal = command
            .find_subcommand("internal")
            .expect("internal namespace");
        for child in [
            "agent-mcp",
            "runtime-smoke",
            "managed-runtime-smoke",
            "managed-candidate-smoke",
            "verifier",
            "verify-managed-candidate",
        ] {
            assert!(command.find_subcommand(child).is_none());
            assert!(
                internal.find_subcommand(child).is_some(),
                "missing internal child {child}"
            );
        }
    }
}
