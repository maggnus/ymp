#![forbid(unsafe_code)]

use std::path::PathBuf;

use anyhow::{Result, ensure};
use clap::{Parser, Subcommand};
use ymp_corpus::admission::AdmissionCommand;
use ymp_corpus::development::DevelopmentCommand;
use ymp_corpus::development_v2::DevelopmentV2Command;
use ymp_corpus::study::{
    analyze_study_records, load_frozen_manifest, load_study_records, negative_controls,
    power_analysis, run_negative_control,
};
use ymp_corpus::{check_cache, load_corpus, prepare, report_json, reproduce, write_report};

#[derive(Debug, Parser)]
#[command(about = "Prepare and reproduce an ymp repair-corpus edition")]
struct Cli {
    #[arg(long)]
    corpus: Option<PathBuf>,

    #[arg(long)]
    cache: Option<PathBuf>,

    #[command(subcommand)]
    command: CorpusCommand,
}

#[derive(Debug, Subcommand)]
enum CorpusCommand {
    Admission {
        #[command(subcommand)]
        command: AdmissionCommand,
    },
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
    StudyCheck {
        #[arg(long)]
        manifest: PathBuf,

        #[arg(long)]
        digest: PathBuf,
    },
    StudyPower {
        #[arg(long)]
        manifest: PathBuf,

        #[arg(long)]
        digest: PathBuf,
    },
    StudyDryRun {
        #[arg(long)]
        manifest: PathBuf,

        #[arg(long)]
        digest: PathBuf,

        #[arg(long)]
        records: PathBuf,
    },
    StudyNegativeControls {
        #[arg(long)]
        manifest: PathBuf,

        #[arg(long)]
        digest: PathBuf,

        #[arg(long)]
        records: PathBuf,
    },
    StudyNegativeControl {
        #[arg(long)]
        manifest: PathBuf,

        #[arg(long)]
        digest: PathBuf,

        #[arg(long)]
        records: PathBuf,

        #[arg(long)]
        case: String,
    },
    Development {
        #[command(subcommand)]
        command: DevelopmentCommand,
    },
    DevelopmentV2 {
        #[command(subcommand)]
        command: DevelopmentV2Command,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    if let CorpusCommand::Admission { command } = &cli.command {
        command.execute()?;
        return Ok(());
    }
    if let CorpusCommand::Development { command } = &cli.command {
        command.execute()?;
        return Ok(());
    }
    if let CorpusCommand::DevelopmentV2 { command } = &cli.command {
        command.execute()?;
        return Ok(());
    }
    let corpus_path = cli
        .corpus
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("--corpus is required for this command"))?;
    let cache = cli
        .cache
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("--cache is required for this command"))?;
    let corpus = load_corpus(corpus_path)?;
    match cli.command {
        CorpusCommand::Prepare => {
            let report = prepare(&corpus, cache)?;
            println!("{}", report_json(&report)?);
        }
        CorpusCommand::Check {
            technical_evidence_only,
        } => {
            require_technical_evidence_mode(technical_evidence_only)?;
            check_cache(&corpus, cache)?;
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
            let result = reproduce(&corpus, cache)?;
            if let Some(path) = report {
                write_report(&path, &result)?;
            }
            println!("{}", report_json(&result)?);
            ensure!(result.usable, "one or more corpus packages are unusable");
        }
        CorpusCommand::StudyCheck { manifest, digest } => {
            let manifest = load_frozen_manifest(&corpus, &manifest, &digest)?;
            println!("{}", report_json(&manifest.verification_report())?);
        }
        CorpusCommand::StudyPower { manifest, digest } => {
            let manifest = load_frozen_manifest(&corpus, &manifest, &digest)?;
            println!("{}", report_json(&power_analysis(&manifest)?)?);
        }
        CorpusCommand::StudyDryRun {
            manifest,
            digest,
            records,
        } => {
            let manifest = load_frozen_manifest(&corpus, &manifest, &digest)?;
            let records = load_study_records(&records)?;
            println!(
                "{}",
                report_json(&analyze_study_records(&corpus, &manifest, &records)?)?
            );
        }
        CorpusCommand::StudyNegativeControls {
            manifest,
            digest,
            records,
        } => {
            let manifest = load_frozen_manifest(&corpus, &manifest, &digest)?;
            let records = load_study_records(&records)?;
            println!(
                "{}",
                report_json(&negative_controls(&corpus, &manifest, &records, &digest,)?)?
            );
        }
        CorpusCommand::StudyNegativeControl {
            manifest,
            digest,
            records,
            case,
        } => {
            let manifest = load_frozen_manifest(&corpus, &manifest, &digest)?;
            let records = load_study_records(&records)?;
            run_negative_control(&corpus, &manifest, &records, &case)?;
        }
        CorpusCommand::Development { .. } => unreachable!(),
        CorpusCommand::DevelopmentV2 { .. } => unreachable!(),
        CorpusCommand::Admission { .. } => unreachable!(),
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

    use super::{Cli, CorpusCommand, require_technical_evidence_mode};

    #[test]
    fn owner_approval_file_is_not_a_public_input() {
        for command in ["check", "verify"] {
            let error = Cli::try_parse_from([
                "ymp-corpus",
                "--corpus",
                "/tmp/corpus",
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

    #[test]
    fn admission_does_not_require_primary_corpus_arguments() {
        let parsed = Cli::try_parse_from([
            "ymp-corpus",
            "admission",
            "check",
            "--manifest",
            "/tmp/manifest.json",
            "--digest",
            "/tmp/manifest.sha256",
        ])
        .expect("parse admission command");
        assert!(matches!(parsed.command, CorpusCommand::Admission { .. }));
    }

    #[test]
    fn development_v2_does_not_require_primary_corpus_arguments() {
        let parsed = Cli::try_parse_from([
            "ymp-corpus",
            "development-v2",
            "check",
            "--manifest",
            "/tmp/manifest.json",
            "--digest",
            "/tmp/manifest.sha256",
        ])
        .expect("parse development-v2 command");
        assert!(matches!(
            parsed.command,
            CorpusCommand::DevelopmentV2 { .. }
        ));
    }
}
