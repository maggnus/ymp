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

pub mod internal;
pub mod surface;

#[derive(Debug, Parser)]
#[command(
    name = "ymp",
    version,
    about = "Bounded local coordination for coding agents"
)]
pub struct Cli {
    #[arg(long, global = true, default_value = ".ymp-data")]
    pub data_root: PathBuf,
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
    match cli.command {
        None => ymp_tui::run_with_contracts(cli.data_root, load_contracts(&cli.contract)?),
        Some(Command::Public(command)) => {
            surface::run(cli.data_root, load_contracts(&cli.contract)?, command)
        }
        Some(Command::Internal { command }) => internal::run(cli.data_root, &cli.contract, command),
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
    use super::Cli;
    use clap::{CommandFactory, Parser};

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
