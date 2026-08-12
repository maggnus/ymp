#![forbid(unsafe_code)]

use std::path::PathBuf;

use anyhow::{Result, ensure};
use clap::{Parser, Subcommand};
use ymp_corpus::{check_cache, load_corpus, prepare, report_json, reproduce, write_report};

#[derive(Debug, Parser)]
#[command(about = "Prepare and reproduce an ymp repair-corpus edition")]
struct Cli {
    #[arg(long, default_value = concat!(env!("CARGO_MANIFEST_DIR"), "/corpus"))]
    corpus: PathBuf,

    #[arg(long)]
    cache: PathBuf,

    #[command(subcommand)]
    command: CorpusCommand,
}

#[derive(Debug, Subcommand)]
enum CorpusCommand {
    Prepare,
    Check,
    Verify {
        #[arg(long)]
        report: Option<PathBuf>,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let corpus = load_corpus(&cli.corpus)?;
    match cli.command {
        CorpusCommand::Prepare => {
            let report = prepare(&corpus, &cli.cache)?;
            println!("{}", report_json(&report)?);
        }
        CorpusCommand::Check => {
            check_cache(&corpus, &cli.cache)?;
            println!("corpus {} and cache are intact", corpus.registry.corpus_id);
        }
        CorpusCommand::Verify { report } => {
            let result = reproduce(&corpus, &cli.cache)?;
            if let Some(path) = report {
                write_report(&path, &result)?;
            }
            println!("{}", report_json(&result)?);
            ensure!(result.usable, "one or more corpus packages are unusable");
        }
    }
    Ok(())
}
