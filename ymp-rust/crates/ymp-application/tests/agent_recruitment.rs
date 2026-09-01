#![forbid(unsafe_code)]

use tempfile::tempdir;
use ymp_agent_api::{AgentToolErrorCode, RequestParticipantArguments};
use ymp_application::{Application, ApplicationConfig};
use ymp_domain::commitment::{
    Advertise, Award, BudgetVector, CommitmentCommand, Dimension, FundingSource, OfferPolicy,
    RecordBid, RegisterParticipant, StartAttempt, StartInvocation, WakeCondition, YieldInvocation,
};
use ymp_domain::pool::{EntryIdentity, FrozenEntry};
use ymp_domain::recruitment::RecruitmentPolicy;
use ymp_domain::{Budget, Command};
use ymp_testkit::recruitment::{MeasuredHost, RecordedStarts};

const ROOT: &str = "participant-root";
const CALLER: &str = "participant-caller";
const ATTEMPT: &str = "attempt-caller";
const INVOCATION: &str = "invocation-caller";

fn entry() -> EntryIdentity {
    EntryIdentity::new("anthropic", "claude-code", "claude-opus-5")
}

fn root_budget() -> BudgetVector {
    BudgetVector::ZERO
        .with(Dimension::MoneyMicros, 20_000)
        .with(Dimension::ModelTokens, 20_000)
        .with(Dimension::WallTimeMs, 20_000)
        .with(Dimension::ParticipantStarts, 4)
        .with(Dimension::AttemptStarts, 4)
        .with(Dimension::InvocationStarts, 4)
        .with(Dimension::OfferCreations, 4)
        .with(Dimension::ObligationCreations, 4)
}

fn execution_escrow() -> BudgetVector {
    BudgetVector::ZERO
        .with(Dimension::MoneyMicros, 10_000)
        .with(Dimension::ModelTokens, 10_000)
        .with(Dimension::WallTimeMs, 10_000)
        .with(Dimension::AttemptStarts, 1)
        .with(Dimension::InvocationStarts, 1)
}

fn prepared_yielded_caller() -> (tempfile::TempDir, Application) {
    let temporary = tempdir().expect("temporary directory");
    let mut application = Application::create_with_config(
        temporary.path(),
        "run-agent-recruitment",
        Budget::new(2, 1),
        ApplicationConfig {
            recruitment: RecruitmentPolicy::WORKING,
            ..ApplicationConfig::default()
        },
    )
    .expect("create application");
    application
        .execute(
            "freeze",
            Command::FreezePool {
                pool: "default".to_owned(),
                entries: vec![FrozenEntry::admissible(
                    "anthropic",
                    "claude-code",
                    "claude-opus-5",
                )],
                digest: "b".repeat(64),
            },
        )
        .expect("freeze pool");
    application
        .open_commitment_kernel(
            "kernel",
            ROOT,
            "principal-root",
            "obligation-root",
            root_budget(),
        )
        .expect("open kernel");

    let commands = [
        CommitmentCommand::RegisterParticipant(RegisterParticipant {
            participant_id: CALLER.to_owned(),
            principal_id: "principal-caller".to_owned(),
            sponsor: ROOT.to_owned(),
            endowment: BudgetVector::ZERO
                .with(Dimension::ParticipantStarts, 1)
                .with(Dimension::InvocationStarts, 1),
        }),
        CommitmentCommand::Advertise(Advertise {
            offer_id: "offer-caller".to_owned(),
            sponsor: ROOT.to_owned(),
            parent_obligation: "obligation-root".to_owned(),
            funding_source: FundingSource::Participant,
            task_scope: "scope-caller".to_owned(),
            base_digest: ymp_domain::digest_bytes(b"base"),
            intent_digest: ymp_domain::digest_bytes(b"intent"),
            artifact_class: "source-change".to_owned(),
            dependencies: vec![],
            capability_scope: vec![],
            execution_escrow: execution_escrow(),
            policy: OfferPolicy::Negotiated,
            bid_deadline: 10_000,
            offer_deadline: 10_000,
            max_awards: 1,
        }),
        CommitmentCommand::RecordBid(RecordBid {
            bid_id: "bid-caller".to_owned(),
            offer_id: "offer-caller".to_owned(),
            bidder: CALLER.to_owned(),
            requested_escrow: execution_escrow(),
            artifact_class: "source-change".to_owned(),
            proposal_digest: None,
            expires_at: 10_000,
        }),
        CommitmentCommand::Award(Award {
            contract_id: "contract-caller".to_owned(),
            obligation_id: "obligation-caller".to_owned(),
            lease_id: "lease-caller".to_owned(),
            offer_id: "offer-caller".to_owned(),
            bid_id: "bid-caller".to_owned(),
            sponsor: ROOT.to_owned(),
            lease_ms: 5_000,
        }),
        CommitmentCommand::StartAttempt(StartAttempt {
            attempt_id: ATTEMPT.to_owned(),
            contract_id: "contract-caller".to_owned(),
            participant: CALLER.to_owned(),
            generation: 1,
        }),
        CommitmentCommand::StartInvocation(StartInvocation {
            invocation_id: INVOCATION.to_owned(),
            attempt_id: ATTEMPT.to_owned(),
            contract_id: "contract-caller".to_owned(),
            participant: CALLER.to_owned(),
            generation: 1,
            cursor: 0,
        }),
        CommitmentCommand::YieldInvocation(YieldInvocation {
            invocation_id: INVOCATION.to_owned(),
            participant: CALLER.to_owned(),
            generation: 1,
            cursor: 0,
            conditions: vec![WakeCondition::SubmissionRecorded {
                contract_id: "contract-caller".to_owned(),
            }],
            wake_deadline: 5_000,
        }),
    ];
    for (index, command) in commands.iter().enumerate() {
        application
            .execute_commitment(format!("prepare-{index}"), command)
            .expect("prepare yielded caller");
    }
    (temporary, application)
}

#[test]
fn unbound_and_yielded_agent_sessions_recruit_nobody() {
    let (_temporary, mut application) = prepared_yielded_caller();
    let before_sequence = application.state().last_sequence;
    let before_consumed = application
        .commitments()
        .expect("kernel")
        .consumed()
        .get(Dimension::ParticipantStarts);
    let arguments = RequestParticipantArguments {
        request_id: "request-1".to_owned(),
        entry: entry(),
    };
    let host = MeasuredHost::serving(&[entry()]);

    let mut starts = RecordedStarts::new();
    let unbound = application
        .agent_session("attempt-forged")
        .request_participant(arguments.clone(), &host, &mut starts)
        .expect_err("unbound session is refused");
    assert_eq!(unbound.code, AgentToolErrorCode::Rejected);
    assert!(unbound.message.contains("not live"));

    let yielded = application
        .agent_session(ATTEMPT)
        .request_participant(arguments, &host, &mut starts)
        .expect_err("yielded caller is refused");
    assert_eq!(yielded.code, AgentToolErrorCode::Rejected);
    assert!(yielded.message.contains("has yielded the process slice"));

    assert_eq!(application.state().last_sequence, before_sequence);
    assert!(application.admissions().is_empty());
    assert_eq!(
        application
            .commitments()
            .expect("kernel")
            .consumed()
            .get(Dimension::ParticipantStarts),
        before_consumed
    );
    assert!(starts.started().is_empty());
}
