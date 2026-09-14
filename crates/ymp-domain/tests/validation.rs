#![forbid(unsafe_code)]

use ymp_domain::{
    AcceptanceContract, Constraints, Criterion, CriterionId, DomainError, Goal, SessionId, TaskId,
};

#[test]
fn required_domain_text_rejects_blank_values() {
    assert_eq!(
        TaskId::new(" \t\n"),
        Err(DomainError::BlankText { field: "task ID" })
    );
    assert_eq!(
        SessionId::new(""),
        Err(DomainError::BlankText {
            field: "session ID"
        })
    );
    assert_eq!(
        CriterionId::new("  "),
        Err(DomainError::BlankText {
            field: "criterion ID"
        })
    );
    assert_eq!(
        Goal::new("\n"),
        Err(DomainError::BlankText {
            field: "goal request"
        })
    );

    let criterion_id = CriterionId::new("result").expect("valid criterion ID");
    assert_eq!(
        Criterion::new(criterion_id, " \r\n"),
        Err(DomainError::BlankText {
            field: "criterion description"
        })
    );
}

#[test]
fn acceptance_contract_requires_distinct_criteria() {
    assert_eq!(
        AcceptanceContract::new(Vec::new()),
        Err(DomainError::EmptyAcceptanceContract)
    );

    let first = Criterion::new(
        CriterionId::new("checked").expect("valid criterion ID"),
        "The result is checked",
    )
    .expect("valid criterion");
    let duplicate = Criterion::new(
        CriterionId::new("checked").expect("valid criterion ID"),
        "A second description does not make the ID unique",
    )
    .expect("valid criterion");

    assert_eq!(
        AcceptanceContract::new(vec![first, duplicate]),
        Err(DomainError::DuplicateCriterionId(
            CriterionId::new("checked").expect("valid criterion ID")
        ))
    );
}

#[test]
fn constraints_preserve_supplied_text_and_allow_an_empty_sequence() {
    let empty = Constraints::new(Vec::new()).expect("empty constraints are valid");
    assert!(empty.is_empty());
    assert_eq!(empty.conditions(), &[] as &[String]);

    let supplied = vec![
        "  Preserve these spaces.  ".to_owned(),
        "Do not access the network.\n".to_owned(),
    ];
    let constraints = Constraints::new(supplied.clone()).expect("valid constraints");
    assert_eq!(constraints.conditions(), supplied.as_slice());

    assert_eq!(
        Constraints::new(vec!["valid".to_owned(), " \n".to_owned()]),
        Err(DomainError::BlankText {
            field: "constraint condition"
        })
    );
}

#[test]
fn valid_goal_and_criterion_text_are_not_normalized() {
    let goal_text = "  Keep exact punctuation — and spacing.\n";
    let description = "\tObserve the exact result.  ";
    let goal = Goal::new(goal_text).expect("valid goal");
    let criterion = Criterion::new(
        CriterionId::new("exact-text").expect("valid criterion ID"),
        description,
    )
    .expect("valid criterion");

    assert_eq!(goal.request(), goal_text);
    assert_eq!(criterion.description(), description);
}
