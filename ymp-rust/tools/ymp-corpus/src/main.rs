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
    Check {
        #[arg(long)]
        technical_evidence_only: bool,
    },
    Verify {
        #[arg(long)]
        report: Option<PathBuf>,

        #[arg(long)]
        technical_evidence_only: bool,
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
        CorpusCommand::Check {
            technical_evidence_only,
        } => {
            require_technical_evidence_mode(technical_evidence_only)?;
            check_cache(&corpus, &cli.cache)?;
            println!(
                "corpus {} and cache are technically intact; owner authorization is external and was not evaluated",
                corpus.registry.corpus_id
            );
        }
        CorpusCommand::Verify {
            report,
            technical_evidence_only,
        } => {
            require_technical_evidence_mode(technical_evidence_only)?;
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

fn require_technical_evidence_mode(technical_evidence_only: bool) -> Result<()> {
    ensure!(
        technical_evidence_only,
        "owner authorization is external and cannot be authenticated by ymp-corpus; use --technical-evidence-only for technical verification"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use clap::{Parser, error::ErrorKind};

    use super::{Cli, require_technical_evidence_mode};

    #[test]
    fn owner_approval_file_is_not_a_public_input() {
        for command in ["check", "verify"] {
            let error = Cli::try_parse_from([
                "ymp-corpus",
                "--cache",
                "/tmp/cache",
                command,
                "--owner-approval",
                "/tmp/fabricated.json",
            ])
            .unwrap_err();
            assert_eq!(error.kind(), ErrorKind::UnknownArgument);
        }
    }

    #[test]
    fn technical_commands_require_explicit_non_authoritative_mode() {
        assert!(require_technical_evidence_mode(false).is_err());
        assert!(require_technical_evidence_mode(true).is_ok());
    }
}
