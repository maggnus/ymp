//! Decision surfaces, built from projections.
//!
//! A decision modal is the one place the interface interrupts the conversation, so everything
//! it states must be a fact the state produced: the contract it would authorize, the run it
//! would end, the budget already spent. Where the domain records no command for a decision, the
//! action is shown as unavailable with the exact reason — it is never drawn as an enabled
//! control that does nothing.

use crate::projection::{self, ContractFacts, Environment, RunFacts};
use crate::runtimes::Report;
use crate::state::{
    Authorize, AuthorizeAction, Confirm, ConfirmAction, Requirement, RequirementState,
};

/// Why authorization cannot be committed for a contract that carries no mechanical check.
///
/// Approving it would grant authority over a result nothing could reject, so the action is not
/// offered and the coverage map stays reviewable.
pub const AUTHORIZATION_UNAVAILABLE: &str =
    "unavailable — nothing declared would reject a wrong candidate";

/// Why authorization cannot be committed while this store already holds a run and no other store
/// can be addressed for the next one.
pub const AUTHORIZATION_HAS_RUN: &str = "unavailable — this store already holds a run, and this invocation names one exact store \
     rather than a root; a second run needs its own store";

/// Why authorization cannot be committed while no runtime profile can do the work.
pub const AUTHORIZATION_HAS_NO_ROUTE: &str =
    "unavailable — no runtime profile on this host would do this work";

/// What the session settled about the run an authorization would start: which profile would do
/// the work, and which store the run would be recorded in.
#[derive(Clone, Copy, Debug)]
pub struct StartFacts<'a> {
    /// The profile that would do the work, when exactly one is settled.
    pub profile: Option<&'a str>,
    /// What is true about the routing, in the words the operator is shown.
    pub note: &'a str,
    /// Whether this run would be given a store of its own, because the one being read already
    /// holds a run and the layout can address the next one.
    pub in_a_store_of_its_own: bool,
}

impl StartFacts<'_> {
    /// A caller that has settled neither — a surface built before this host was probed, or one
    /// that is not about to start anything.
    pub const fn unknown() -> Self {
        Self {
            profile: None,
            note: "",
            in_a_store_of_its_own: false,
        }
    }
}

/// The coverage map for one contract, and the run authorizing it would start.
pub fn authorize(
    contract: &ContractFacts,
    environment: &Environment,
    runtimes: Option<&Report>,
    run: Option<&RunFacts>,
    start: StartFacts<'_>,
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
    // A store holds one run. Where the one being read already holds one, the run this
    // authorization starts is recorded in a store of its own, addressed under the same root by
    // the layout rather than named by the operator — and the run being read is untouched.
    if start.in_a_store_of_its_own {
        facts.push((
            "store".to_owned(),
            "the store you are reading already holds a run · this one is recorded in a store of \
             its own under the same root, and the run you are reading is left exactly as it stands"
                .to_owned(),
        ));
    }
    // Which agent would do the work, and what this host reported about every profile that could
    // have. A profile that is not ready is named here, before anything is stored, and nothing is
    // ever routed to another profile in its place.
    facts.push((
        "route".to_owned(),
        match (start.profile, runtimes) {
            (Some(profile), _) => format!("{profile} — this run's work would be done by it"),
            (None, None) => "probing runtime profiles…".to_owned(),
            (None, Some(_)) => start.note.to_owned(),
        },
    ));
    if let Some(report) = runtimes {
        facts.push((
            "profiles".to_owned(),
            report
                .profiles
                .iter()
                .map(|profile| format!("{} {}", profile.name, profile.readiness_text()))
                .collect::<Vec<_>>()
                .join(" · "),
        ));
    }
    facts.push((
        "assurance".to_owned(),
        format!(
            "{} ▲ — {}",
            environment.assurance_profile, environment.assurance_limit
        ),
    ));

    if let Some(budget) = &contract.budget {
        facts.push((
            "budget".to_owned(),
            format!(
                "attempts {} · verification queries {}",
                budget.attempts_remaining, budget.verification_queries_remaining
            ),
        ));
    }

    let routed = start.profile.is_some() || runtimes.is_none();
    let mut requirements = match &contract.verifier {
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
            detail: Some(format!(
                "BLOCKING — {}",
                contract.blocked.clone().unwrap_or_else(|| {
                    "the contract declares no verifier, so nothing would reject a wrong candidate"
                        .to_owned()
                })
            )),
        }],
    };

    // Who would do the work is part of what is being authorized, so it is named here, before
    // anything is stored. It does not block the run: a run whose work nothing can do yet is a run
    // with an unspent budget, and the attempt is where a profile is refused. What is refused there
    // is never replaced by another profile.
    requirements.push(Requirement {
        state: if routed {
            RequirementState::Covered
        } else {
            RequirementState::Warning
        },
        name: "the work is done by a runtime profile you approved".into(),
        checked_by: start.profile.unwrap_or("none ready").to_owned(),
        negative_control: "—".into(),
        detail: Some(match start.profile {
            Some(profile) => format!(
                "{profile} · a profile that is not ready stops the attempt rather than being \
                 replaced by another"
            ),
            None if runtimes.is_none() => {
                "this host has not been probed yet — the profiles are named once it is".to_owned()
            }
            None => format!("{} · nothing is spent until one can", start.note),
        }),
    });

    let action = contract
        .can_start(run, start.in_a_store_of_its_own)
        .then(|| AuthorizeAction {
            contract_id: contract.contract_id.clone(),
            run_id: contract.run_id.clone().unwrap_or_default(),
            budget: contract
                .budget
                .as_ref()
                .map(|budget| {
                    vec![
                        ("attempts".to_owned(), budget.attempts_remaining),
                        (
                            "verification_queries".to_owned(),
                            budget.verification_queries_remaining,
                        ),
                    ]
                })
                .unwrap_or_default(),
            source: contract.source.display().to_string(),
            verifier: contract
                .verifier
                .as_ref()
                .map(|verifier| file_name(&verifier.program))
                .unwrap_or_default(),
            negative_control: contract
                .verifier
                .as_ref()
                .map(|verifier| file_name(&verifier.negative_control))
                .unwrap_or_default(),
            reauthorization: contract.previously_authorized,
        });
    let blocking = contract.blocking_items();
    let action_note = if action.is_some() && routed && start.in_a_store_of_its_own {
        "authorize and start — the contract is stored, the run begins in a store of its own and \
         the profile above does the work"
            .to_owned()
    } else if action.is_some() && routed {
        "authorize and start — the contract is stored, the run begins and the profile above does \
         the work"
            .to_owned()
    } else if action.is_some() {
        format!(
            "authorize and start — the run begins, and no attempt is launched: {AUTHORIZATION_HAS_NO_ROUTE}"
        )
    } else if contract.blocking_items() > 0 {
        format!(
            "disabled — {} blocking item · {AUTHORIZATION_UNAVAILABLE}",
            contract.blocking_items()
        )
    } else {
        AUTHORIZATION_HAS_RUN.to_owned()
    };

    Authorize {
        contract: contract.contract_id.clone(),
        badge: "irreversible · starts spending".into(),
        facts,
        requirements,
        footer_note: "your approval grants authority, not validity — the gaps above stay yours"
            .into(),
        blocking,
        action,
        action_note,
    }
}

/// The typed confirmation that launches the managed attempt of a live run.
///
/// This is where the agent starts and where spending against the operator's own account begins,
/// so it carries the same weight as the start of the run: the run identifier, typed in full.
pub fn start_attempt(run: &RunFacts, profile: &str) -> Confirm {
    Confirm {
        title: format!("start the attempt of run {}", run.run_id),
        badge: "irreversible · starts spending".into(),
        consequences: vec![
            format!("the {profile} profile is started and does the work of this run"),
            "it works in a private copy of the source, never in your working tree".to_owned(),
            "it is paid for out of your own account with that profile, and what it spends is not \
             returned"
                .to_owned(),
            format!(
                "attempts left {} · verification queries left {}",
                run.budget.attempts_remaining, run.budget.verification_queries_remaining
            ),
            "every launch, output and tool call is recorded under runtime-evidence in this store"
                .to_owned(),
        ],
        prompt_label: "type the run id to confirm:".into(),
        required: run.run_id.clone(),
        typed: String::new(),
        confirm_hint: "confirm — disabled until the id matches exactly".into(),
        cancel_hint: "start nothing".into(),
        action: ConfirmAction::StartAttempt {
            run_id: run.run_id.clone(),
            profile: profile.to_owned(),
        },
    }
}

/// The typed confirmation that stores the contract and starts its run.
pub fn start_run(action: &AuthorizeAction) -> Confirm {
    let mut consequences = vec![
        format!(
            "contract {} is stored immutably and run {} is recorded in the journal",
            action.contract_id, action.run_id
        ),
        format!("the work is done against {}", action.source),
        format!(
            "a candidate is accepted only if {} accepts it, and only while {} still rejects the \
             known-wrong candidate",
            action.verifier, action.verifier
        ),
        format!("negative control: {}", action.negative_control),
    ];
    for (dimension, amount) in &action.budget {
        consequences.push(format!(
            "{dimension} available to this run: {amount} — spending starts here"
        ));
    }

    // Weight follows what is new. A contract this session already authorized, unchanged since,
    // has been read and typed out once; asking for the identifier again buys nothing, so the
    // confirmation is a single one. Everything else — a first authorization, and every contract
    // that changed — is confirmed by typing the identifier, because that is the point at which
    // spending starts.
    let (prompt_label, required, confirm_hint) = if action.reauthorization {
        (
            "this contract was authorized in this session and has not changed since:".to_owned(),
            String::new(),
            "confirm".to_owned(),
        )
    } else {
        (
            "type the contract id to confirm:".to_owned(),
            action.contract_id.clone(),
            "confirm — disabled until the contract id matches exactly".to_owned(),
        )
    };

    Confirm {
        title: format!("start run {}", action.run_id),
        badge: "irreversible · starts spending".into(),
        consequences,
        prompt_label,
        required,
        typed: String::new(),
        confirm_hint,
        cancel_hint: "start nothing".into(),
        action: ConfirmAction::StartRun {
            contract_id: action.contract_id.clone(),
            run_id: action.run_id.clone(),
        },
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
        if !verified {
            return ContractFacts::refused(
                "contract-1".into(),
                PathBuf::from("/tmp/checkout"),
                "make the replay path idempotent".into(),
                "no run started — the request states no acceptance condition".into(),
            );
        }
        ContractFacts {
            contract_id: "contract-1".into(),
            contract_digest: "a".repeat(64),
            source: PathBuf::from("/tmp/checkout"),
            prompt: "make the replay path idempotent\nsecond line".into(),
            verifier: Some(VerifierFacts {
                program: PathBuf::from("/tmp/checkout/verify.sh"),
                oracle_digest: "b".repeat(64),
                negative_control: PathBuf::from("/tmp/checkout/broken-candidate"),
                wall_time_ms: 60_000,
            }),
            budget: Some(Budget::new(1, 1)),
            run_id: Some("run-aaaaaaaaaaaa".into()),
            blocked: None,
            previously_authorized: false,
        }
    }

    #[test]
    fn a_contract_without_a_verifier_blocks_and_says_why() {
        let modal = authorize(
            &contract(false),
            &environment(),
            None,
            None,
            StartFacts::unknown(),
        );
        assert_eq!(modal.blocking, 1);
        assert_eq!(modal.requirements[0].state, RequirementState::Blocking);
        let detail = modal.requirements[0].detail.as_ref().expect("detail");
        assert!(detail.contains("acceptance condition"), "{detail}");
        assert!(modal.action.is_none(), "a blocked contract offered a run");
        assert!(
            modal.action_note.contains("blocking item"),
            "{}",
            modal.action_note
        );
    }

    #[test]
    fn a_verified_contract_names_its_check_and_its_negative_control() {
        let modal = authorize(
            &contract(true),
            &environment(),
            None,
            None,
            StartFacts::unknown(),
        );
        assert_eq!(modal.blocking, 0);
        assert_eq!(modal.requirements[0].checked_by, "verify.sh");
        assert!(
            modal.requirements[0]
                .negative_control
                .contains("broken-candidate")
        );
        assert!(
            modal.facts.iter().any(|(label, value)| label == "assurance"
                && value.contains(projection::ASSURANCE_PROFILE))
        );
    }

    #[test]
    fn a_startable_contract_offers_the_run_and_a_store_with_a_run_does_not() {
        let modal = authorize(
            &contract(true),
            &environment(),
            None,
            None,
            StartFacts::unknown(),
        );
        let action = modal.action.expect("the run is offered");
        assert_eq!(action.run_id, "run-aaaaaaaaaaaa");
        let confirm = start_run(&action);
        assert_eq!(confirm.required, "contract-1");
        assert!(
            confirm.title.contains("run-aaaaaaaaaaaa"),
            "{}",
            confirm.title
        );
        assert!(
            confirm
                .consequences
                .iter()
                .any(|line| line.contains("verify.sh")),
            "{:?}",
            confirm.consequences
        );

        let run = RunFacts {
            run_id: "demo-run".into(),
            status: RunStatus::Running,
            budget: Budget::new(2, 1),
            active_attempts: Vec::new(),
            candidate_digest: None,
            last_sequence: 3,
            terminal_reason: None,
        };
        let modal = authorize(
            &contract(true),
            &environment(),
            None,
            Some(&run),
            StartFacts::unknown(),
        );
        assert!(modal.action.is_none(), "a second run was offered");
        assert!(modal.action_note.contains("already holds a run"));
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
