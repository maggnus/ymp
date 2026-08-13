//! Decision surfaces, built from projections.
//!
//! A decision modal is the one place the interface interrupts the conversation, so everything
//! it states must be a fact the state produced: the contract it would authorize, the run it
//! would end, the budget already spent. Where the domain records no command for a decision, the
//! action is shown as unavailable with the exact reason — it is never drawn as an enabled
//! control that does nothing.

use crate::projection::{self, ContractFacts, Environment, RunFacts};
use crate::runtimes::Report;
use crate::state::{Authorize, Confirm, ConfirmAction, Requirement, RequirementState};

/// Why authorization cannot be committed in this build.
///
/// The application layer records no contract-authorization command and the domain has no
/// contract-package event, so the coverage map is reviewable and the action is not offered.
pub const AUTHORIZATION_UNAVAILABLE: &str =
    "unavailable — this domain records no contract-authorization command";

/// The coverage map for one managed contract.
pub fn authorize(
    contract: &ContractFacts,
    environment: &Environment,
    runtimes: Option<&Report>,
) -> Authorize {
    let mut facts = vec![
        ("intent".to_owned(), first_line(&contract.prompt)),
        (
            "contract".to_owned(),
            format!(
                "{} · digest {}",
                contract.contract_id,
                projection::short_digest(&contract.contract_digest)
            ),
        ),
        ("source".to_owned(), contract.source.display().to_string()),
    ];
    facts.push((
        "routes".to_owned(),
        match runtimes {
            None => "probing runtime profiles…".to_owned(),
            Some(report) if report.ready_count() == 0 => {
                "no runtime profile on this host is ready".to_owned()
            }
            Some(report) => report
                .profiles
                .iter()
                .filter(|profile| profile.ready())
                .map(|profile| format!("{} ready", profile.name))
                .collect::<Vec<_>>()
                .join(" · "),
        },
    ));
    facts.push((
        "assurance".to_owned(),
        format!(
            "{} ▲ — {}",
            environment.assurance_profile, environment.assurance_limit
        ),
    ));

    let requirements = match &contract.verifier {
        Some(verifier) => vec![
            Requirement {
                state: RequirementState::Covered,
                name: "a candidate is accepted only on verifier evidence".into(),
                checked_by: file_name(&verifier.program),
                negative_control: format!("rejects {}", file_name(&verifier.negative_control)),
                detail: Some(format!(
                    "oracle {} · protected from agents · wall limit {} ms",
                    projection::short_digest(&verifier.oracle_digest),
                    verifier.wall_time_ms
                )),
            },
            Requirement {
                state: RequirementState::Warning,
                name: "the contract text itself".into(),
                checked_by: "no mechanical check".into(),
                negative_control: "—".into(),
                detail: Some(
                    "this domain records no requirement objects — the prompt above is what the \
                     runtime is given, and judging it stays yours"
                        .into(),
                ),
            },
        ],
        None => vec![Requirement {
            state: RequirementState::Blocking,
            name: "a candidate is accepted only on verifier evidence".into(),
            checked_by: "none declared".into(),
            negative_control: "—".into(),
            detail: Some(
                "BLOCKING — the contract declares no verifier, so nothing would reject a wrong \
                 candidate"
                    .into(),
            ),
        }],
    };

    Authorize {
        contract: contract.contract_id.clone(),
        badge: "irreversible · starts spending".into(),
        facts,
        requirements,
        footer_note: "your approval grants authority, not validity — the gaps above stay yours"
            .into(),
        blocking: contract.blocking_items(),
    }
}

/// The typed confirmation that ends a live run.
pub fn cancel_run(run: &RunFacts) -> Confirm {
    let mut consequences = Vec::new();
    consequences.push(match run.active_attempts.len() {
        0 => "no attempt is active — nothing is interrupted".to_owned(),
        1 => format!("attempt {} is interrupted mid-work", run.active_attempts[0]),
        count => format!("{count} active attempts are interrupted mid-work"),
    });
    consequences.push(format!(
        "budget already consumed is not returned · attempts left {} · verification queries left {}",
        run.budget.attempts_remaining, run.budget.verification_queries_remaining
    ));
    consequences
        .push("the journal and every published candidate stay readable afterwards".to_owned());
    consequences.push("terminal outcome recorded: cancelled".to_owned());

    Confirm {
        title: format!("cancel run {}", run.run_id),
        badge: "irreversible".into(),
        consequences,
        prompt_label: "type the run id to confirm:".into(),
        required: run.run_id.clone(),
        typed: String::new(),
        confirm_hint: "confirm — disabled until the id matches exactly".into(),
        cancel_hint: "keep running".into(),
        action: ConfirmAction::CancelRun {
            run_id: run.run_id.clone(),
        },
    }
}

/// The reason recorded with a cancellation. It names who ended the run and from where.
pub fn cancellation_reason() -> String {
    "cancelled by the operator from the terminal interface".to_owned()
}

fn first_line(prompt: &str) -> String {
    prompt.lines().next().unwrap_or_default().to_owned()
}

fn file_name(path: &std::path::Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use ymp_domain::{Budget, RunStatus};

    use super::*;
    use crate::projection::VerifierFacts;

    fn environment() -> Environment {
        Environment {
            version: "0.1.0".into(),
            project: "checkout".into(),
            project_path: PathBuf::from("/tmp/checkout"),
            data_root: PathBuf::from("/tmp/checkout/.ymp-data"),
            assurance_profile: projection::ASSURANCE_PROFILE.to_owned(),
            assurance_limit: projection::ASSURANCE_LIMIT.to_owned(),
        }
    }

    fn contract(verified: bool) -> ContractFacts {
        ContractFacts {
            contract_id: "contract-1".into(),
            contract_digest: "a".repeat(64),
            source: PathBuf::from("/tmp/checkout/contract.json"),
            prompt: "make the replay path idempotent\nsecond line".into(),
            verifier: verified.then(|| VerifierFacts {
                program: PathBuf::from("/tmp/checkout/verify.sh"),
                oracle_digest: "b".repeat(64),
                negative_control: PathBuf::from("/tmp/checkout/broken.patch"),
                wall_time_ms: 60_000,
            }),
        }
    }

    #[test]
    fn a_contract_without_a_verifier_blocks_and_says_why() {
        let modal = authorize(&contract(false), &environment(), None);
        assert_eq!(modal.blocking, 1);
        assert_eq!(modal.requirements[0].state, RequirementState::Blocking);
        let detail = modal.requirements[0].detail.as_ref().expect("detail");
        assert!(detail.contains("BLOCKING"), "{detail}");
    }

    #[test]
    fn a_verified_contract_names_its_check_and_its_negative_control() {
        let modal = authorize(&contract(true), &environment(), None);
        assert_eq!(modal.blocking, 0);
        assert_eq!(modal.requirements[0].checked_by, "verify.sh");
        assert!(
            modal.requirements[0]
                .negative_control
                .contains("broken.patch")
        );
        assert!(
            modal.facts.iter().any(|(label, value)| label == "assurance"
                && value.contains(projection::ASSURANCE_PROFILE))
        );
    }

    #[test]
    fn cancelling_names_the_run_the_attempts_and_the_recorded_outcome() {
        let run = RunFacts {
            run_id: "demo-run".into(),
            status: RunStatus::Running,
            budget: Budget::new(2, 1),
            active_attempts: vec!["attempt-1".into()],
            candidate_digest: None,
            last_sequence: 7,
            terminal_reason: None,
        };
        let confirm = cancel_run(&run);
        assert_eq!(confirm.required, "demo-run");
        assert!(confirm.consequences[0].contains("attempt-1"));
        assert!(
            confirm
                .consequences
                .iter()
                .any(|line| line.contains("cancelled"))
        );
    }
}
