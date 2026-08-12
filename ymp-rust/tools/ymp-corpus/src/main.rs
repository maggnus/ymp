#![forbid(unsafe_code)]

use std::path::PathBuf;

use anyhow::{Result, ensure};
use clap::{Parser, Subcommand};
use ymp_corpus::{
    UsageApproval, check_cache, load_corpus, prepare, report_json, reproduce,
    require_owner_approval, write_report,
};

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
        #[arg(long, conflicts_with = "unapproved_evidence")]
        owner_approval: Option<PathBuf>,

        #[arg(long)]
        unapproved_evidence: bool,
    },
    Verify {
        #[arg(long)]
        report: Option<PathBuf>,

        #[arg(long, conflicts_with = "unapproved_evidence")]
        owner_approval: Option<PathBuf>,

        #[arg(long)]
        unapproved_evidence: bool,
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
            owner_approval,
            unapproved_evidence,
        } => {
            let approval = usage_approval(&corpus, owner_approval.as_deref(), unapproved_evidence)?;
            check_cache(&corpus, &cli.cache)?;
            match approval {
                UsageApproval::OwnerApproved { .. } => println!(
                    "corpus {} is owner-approved; corpus and cache are intact",
                    corpus.registry.corpus_id
                ),
                UsageApproval::UnapprovedEvidence => println!(
                    "corpus {} and cache are technically intact; corpus is not approved for use",
                    corpus.registry.corpus_id
                ),
            }
        }
        CorpusCommand::Verify {
            report,
            owner_approval,
            unapproved_evidence,
        } => {
            let approval = usage_approval(&corpus, owner_approval.as_deref(), unapproved_evidence)?;
            let result = reproduce(&corpus, &cli.cache, &approval)?;
            if let Some(path) = report {
                write_report(&path, &result)?;
            }
            println!("{}", report_json(&result)?);
            ensure!(result.usable, "one or more corpus packages are unusable");
        }
    }
    Ok(())
}

fn usage_approval(
    corpus: &ymp_corpus::LoadedCorpus,
    approval_path: Option<&std::path::Path>,
    unapproved_evidence: bool,
) -> Result<UsageApproval> {
    if unapproved_evidence {
        return Ok(UsageApproval::UnapprovedEvidence);
    }
    require_owner_approval(corpus, approval_path)
}
