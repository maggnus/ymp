#![forbid(unsafe_code)]

use clap::Parser;

fn main() -> anyhow::Result<()> {
    ymp_cli::run(ymp_cli::Cli::parse())
}
