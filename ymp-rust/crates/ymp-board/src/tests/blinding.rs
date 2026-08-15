//! The reveal order of an agent review.
//!
//! A reviewer commits the digest of its own assessment while the board is still withheld from it,
//! and only then may the review policy reveal context. Three ways of looking first are refused: a
//! read, an explicit reveal, and a message that cites something the reviewer could only have got by
//! reading. An assessment committed after the reveal stays attributable and never enters the
//! primary comparison, which is what keeps the first position an independent one.

use crate::BoardLedger;
use crate::ledger::DisabledChecks;
use crate::observatory::ViewState;
use crate::protocol::{BoardCommand, BoardError, CommitAssessment, ReadBoard, RevealContext};
use crate::records::{
    Admissibility, Audience, Reference, ReviewPolicy, ReviewerState, Rights, ScopeKind,
};
use crate::tests::{
    ALPHA, BETA, GAMMA, READ_LIMIT, REVIEW_SCOPE, TASK_SCOPE, digest, publication, publish, scoped,
    task_audience,
};

const CANDIDATE: &str = "candidate-under-review";

/// A task scope carrying the producer's rationale, and a blinded review of the result it produced.
/// The reviewer is a member of both, which is the case the reveal order exists for: the grant on
/// the task scope is real, and the board is withheld all the same.
fn under_review() -> BoardLedger {
    let mut board = scoped();
    publish(
        &mut board,
        publication("m-rationale", ALPHA, task_audience(), 96),
    );
    crate::tests::open_scope(
        &mut board,
        REVIEW_SCOPE,
        ScopeKind::CandidateReview,
        Some(ReviewPolicy {
            candidate_digest: digest(CANDIDATE),
            blinded: true,
            withheld_scopes: vec![TASK_SCOPE.to_owned()],
        }),
        &[(BETA, Rights::READ_AND_PUBLISH), (GAMMA, Rights::READ)],
    );
    board
}

fn commit(board: &mut BoardLedger, assessment_id: &str, reviewer: &str) -> Vec<crate::BoardEvent> {
    board
        .execute(&BoardCommand::CommitAssessment(CommitAssessment {
            assessment_id: assessment_id.to_owned(),
            scope_id: REVIEW_SCOPE.to_owned(),
            reviewer: reviewer.to_owned(),
            assessment_digest: digest(assessment_id),
        }))
        .expect("an assessment is committed")
}

fn reviewer_state(board: &BoardLedger, reviewer: &str) -> ReviewerState {
    board
        .scope(REVIEW_SCOPE)
        .expect("the review scope stands")
        .reviewers
        .get(reviewer)
        .expect("the reviewer is enrolled in the reveal order")
        .state
}

/// Nothing on the board reaches a reviewer before its own assessment is durable, and each of the
/// three ways of looking first is refused on its own.
///
/// The negative half switches off the reveal order. Without it the first of the three succeeds, so
/// the order is what withholds the board rather than the reviewer's own restraint.
#[test]
fn nothing_reaches_a_reviewer_before_its_own_assessment_is_durable() {
    let mut board = under_review();
    let withheld = BoardError::BoardWithheldByReviewOrder {
        scope_id: REVIEW_SCOPE.to_owned(),
        reviewer: BETA.to_owned(),
    };

    assert_eq!(
        board.deliver(&ReadBoard {
            reader: BETA.to_owned(),
            limit_bytes: READ_LIMIT,
        }),
        Err(withheld.clone()),
        "a blinded reviewer was delivered the board before committing anything"
    );
    assert_eq!(
        board.decide(&BoardCommand::RevealContext(RevealContext {
            scope_id: REVIEW_SCOPE.to_owned(),
            reviewer: BETA.to_owned(),
        })),
        Err(withheld.clone()),
        "a blinded reviewer revealed the context before committing anything"
    );
    let mut citation = publication(
        "m-peek",
        BETA,
        Audience::Scope {
            scope_id: REVIEW_SCOPE.to_owned(),
        },
        32,
    );
    citation.references = vec![Reference::Message {
        message_id: "m-rationale".to_owned(),
    }];
    assert_eq!(
        board.decide(&BoardCommand::Publish(citation)),
        Err(withheld),
        "a blinded reviewer cited the producer's rationale, which it could only have read"
    );
    assert!(
        !board.may_read(
            BETA,
            board.message("m-rationale").expect("the record stands")
        ),
        "the producer's rationale is readable by a reviewer that has committed nothing"
    );

    let mut weakened = under_review();
    weakened.disable_checks(DisabledChecks {
        blinding_order: true,
        ..DisabledChecks::default()
    });
    assert!(
        weakened
            .deliver(&ReadBoard {
                reader: BETA.to_owned(),
                limit_bytes: READ_LIMIT,
            })
            .is_ok(),
        "with the reveal order switched off a blinded reviewer was still refused, so the order \
         under test is not what withholds the board"
    );
}

/// The first assessment is committed under blinding and is the only one that may stand in the
/// primary comparison. What follows a reveal is recorded, attributed, and secondary.
#[test]
fn the_first_assessment_is_committed_before_disclosure_and_is_the_only_primary_one() {
    let mut board = under_review();
    assert_eq!(reviewer_state(&board, BETA), ReviewerState::Blinded);

    commit(&mut board, "assessment-1", BETA);
    assert_eq!(
        reviewer_state(&board, BETA),
        ReviewerState::Committed,
        "committing an assessment did not move the reviewer along the reveal order"
    );
    assert_eq!(
        board.assessments()[0].admissibility,
        Admissibility::Primary,
        "an assessment committed under blinding is not admitted to the primary comparison"
    );
    assert!(
        board
            .deliver(&ReadBoard {
                reader: BETA.to_owned(),
                limit_bytes: READ_LIMIT,
            })
            .is_err(),
        "the board opened to a reviewer that committed but whose context has not been revealed"
    );

    board
        .execute(&BoardCommand::RevealContext(RevealContext {
            scope_id: REVIEW_SCOPE.to_owned(),
            reviewer: BETA.to_owned(),
        }))
        .expect("context is revealed once the commitment is durable");
    assert_eq!(reviewer_state(&board, BETA), ReviewerState::Disclosed);
    let delivered = board
        .deliver(&ReadBoard {
            reader: BETA.to_owned(),
            limit_bytes: READ_LIMIT,
        })
        .expect("the board opens after the reveal");
    assert!(
        delivered
            .messages
            .iter()
            .any(|message| message.message_id == "m-rationale"),
        "after the reveal the reviewer still cannot read what the policy disclosed"
    );

    commit(&mut board, "assessment-2", BETA);
    assert_eq!(
        board.assessments()[1].admissibility,
        Admissibility::Secondary,
        "an assessment committed after the reveal was admitted to the primary comparison"
    );
    let primary: Vec<&str> = board
        .primary_assessments()
        .into_iter()
        .map(|assessment| assessment.assessment_id.as_str())
        .collect();
    assert_eq!(
        primary,
        vec!["assessment-1"],
        "the primary comparison holds something other than the assessments committed under \
         blinding"
    );
    assert_eq!(
        ViewState::for_operator(&board).primary_assessments().len(),
        1,
        "the reading presents a different primary comparison from the board's own"
    );
}

/// A second assessment by a reviewer that never revealed anything is still secondary: what makes an
/// assessment primary is being the first one committed under blinding, and there is one of those.
#[test]
fn only_the_first_assessment_under_blinding_is_primary() {
    let mut board = under_review();
    commit(&mut board, "assessment-1", BETA);
    commit(&mut board, "assessment-2", BETA);
    assert_eq!(
        board.assessments()[1].admissibility,
        Admissibility::Secondary
    );
    assert_eq!(board.primary_assessments().len(), 1);
}

/// A blinded reviewer is not shown another reviewer's committed position, which is the prior vote
/// the reveal order exists to keep out of a first assessment.
#[test]
fn a_prior_vote_does_not_reach_a_reviewer_that_has_not_committed() {
    let mut board = under_review();
    commit(&mut board, "assessment-1", BETA);
    let view = ViewState::for_reader(&board, GAMMA);
    assert!(
        view.assessments.is_empty(),
        "a reviewer that has committed nothing was shown another reviewer's assessment"
    );
    commit(&mut board, "assessment-2", GAMMA);
    let own = ViewState::for_reader(&board, GAMMA);
    assert_eq!(
        own.assessments.len(),
        1,
        "a reviewer cannot see its own committed assessment, or can already see another's"
    );
    assert_eq!(own.assessments[0].reviewer, GAMMA);
    assert_eq!(
        own.assessments[0].admissibility,
        Admissibility::Primary,
        "the second reviewer's own first assessment, committed under blinding, is not primary"
    );
}

/// An assessment is committed by a reviewer admitted to the review, in a scope that is one.
#[test]
fn an_assessment_belongs_to_a_review_a_reviewer_is_admitted_to() {
    let board = under_review();
    assert_eq!(
        board.decide(&BoardCommand::CommitAssessment(CommitAssessment {
            assessment_id: "assessment-x".to_owned(),
            scope_id: TASK_SCOPE.to_owned(),
            reviewer: BETA.to_owned(),
            assessment_digest: digest("assessment-x"),
        })),
        Err(BoardError::NotUnderReview {
            scope_id: TASK_SCOPE.to_owned()
        }),
        "an assessment was committed against a scope that is not a review"
    );
    assert_eq!(
        board.decide(&BoardCommand::CommitAssessment(CommitAssessment {
            assessment_id: "assessment-x".to_owned(),
            scope_id: REVIEW_SCOPE.to_owned(),
            reviewer: ALPHA.to_owned(),
            assessment_digest: digest("assessment-x"),
        })),
        Err(BoardError::NotAdmitted {
            participant: ALPHA.to_owned(),
            scope_id: REVIEW_SCOPE.to_owned(),
        }),
        "a participant that is not part of the review committed an assessment in it"
    );
}
