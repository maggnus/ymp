//! A7/A8 and P2 exercised through the retained producer/check/review scenario.
use super::*;
use ymp_domain::Prob;
use ymp_kernel::{
    acceptance::{AcceptanceRequest, CreditResponse, credit_input},
    arbiter::Arbiter,
    gatekeeper::GrantToken,
    ledger::LedgerRequest,
    ports::planning::BeliefResponse,
};
use ymp_runtime::policies::{
    belief::{LikelihoodRatioTable, StrongestSupport},
    credit::{ConfirmedOnly, IncludeDiscriminated},
};

fn rules() -> AssessmentRules {
    AssessmentRules {
        mutation_threshold: Prob::new(0.8).unwrap(),
    }
}
fn model_request(
    s: &Setup<SqliteJournal>,
    authority: &Authority,
    context: &ApplicabilityContext,
    model: &dyn BeliefModel,
) -> LedgerRequest {
    let view = s.gate.view(&s.session).unwrap();
    let at = view.latest_at() + 1;
    let responses = authority
        .ledger_inputs(&s.session, context, &rules(), at)
        .unwrap()
        .into_iter()
        .map(|(id, input)| {
            (
                id,
                BeliefResponse {
                    input: Digest::of_value(&input).unwrap(),
                    proposal: model.update(&input).unwrap(),
                },
            )
        })
        .collect();
    LedgerRequest {
        expected_revision: view.revision(),
        at,
        context: context.clone(),
        rules: rules(),
        responses,
    }
}
fn record_model(
    s: &Setup<SqliteJournal>,
    authority: &Authority,
    context: &ApplicabilityContext,
    model: &dyn BeliefModel,
) -> CriteriaLedger {
    let request = model_request(s, authority, context, model);
    let view = s.gate.view(&s.session).unwrap();
    let replacement =
        (view.policies()["BeliefModel"] != *model.selection()).then(|| model.selection().clone());
    authority
        .record_ledger(&s.control, request, replacement)
        .unwrap()
}
fn high_prior() -> StrongestSupport {
    let standard = StrongestSupport::standard().unwrap();
    let mut parameters: StrongestSupportParameters =
        serde_json::from_value(standard.selection().parameters.clone()).unwrap();
    parameters.prior = Prob::new(0.99).unwrap();
    StrongestSupport::new(parameters).unwrap()
}
pub(super) fn prior_guard(
    s: &Setup<SqliteJournal>,
    gate: &Arc<Gatekeeper<SqliteJournal, ymp_storage::content::SqliteContent>>,
    authority: &Authority,
    reviewer: &GrantToken,
    result: &ResultVersion,
    at: u64,
) {
    let view = gate.view(&s.session).unwrap();
    let context = ApplicabilityContext {
        result: result.reference().unwrap(),
        criteria: BTreeSet::from([view.criteria()[0].reference().unwrap()]),
        environments: BTreeMap::new(),
    };
    assert_eq!(
        authority
            .acceptance_value(&s.session, id("no-approval"), &context, &rules(), at)
            .unwrap()
            .decision,
        AcceptanceDecision::Rejected("no independent approval".into())
    );
    let review = authority
        .review(
            gate,
            reviewer,
            ReviewRequest {
                expected_revision: view.revision(),
                at,
                id: id("unbased-review"),
                result: context.result.clone(),
                criteria: context.criteria.iter().cloned().collect(),
                verdict: ReviewVerdict::Approve,
                findings: vec![],
                basis: vec![],
            },
        )
        .unwrap();
    let model = high_prior();
    let mut wrong_prior = model_request(s, authority, &context, &model);
    wrong_prior
        .responses
        .get_mut(&id("criterion"))
        .unwrap()
        .proposal
        .value
        .prior
        .value = Prob::new(0.98).unwrap();
    assert_eq!(
        authority
            .record_ledger(&s.control, wrong_prior, Some(model.selection().clone()))
            .unwrap_err()
            .code,
        "belief_prior_parameters"
    );
    let mut request = model_request(s, authority, &context, &model);
    let proposal = &mut request
        .responses
        .get_mut(&id("criterion"))
        .unwrap()
        .proposal;
    assert_eq!(
        proposal.value.prior.basis,
        vec![review.reference().unwrap()]
    );
    assert!(proposal.value.entry.evidence.is_empty());
    assert_eq!(proposal.value.entry.status, LedgerStatus::Supported);
    proposal.value.entry.status = LedgerStatus::Satisfied;
    assert_eq!(
        authority
            .record_ledger(&s.control, request, Some(model.selection().clone()))
            .unwrap_err()
            .code,
        "belief_guard"
    );
    let ledger = record_model(s, authority, &context, &model);
    assert_eq!(ledger.entries[&id("criterion")].belief.get(), 0.99);
    assert_eq!(
        ledger.entries[&id("criterion")].status,
        LedgerStatus::Supported
    );
    assert!(ledger.entries[&id("criterion")].evidence.is_empty());
    // A7 is independent of A8 satisfaction and must retain the weak actual grade.
    let value = authority
        .acceptance_value(
            &s.session,
            id("unconfirmed-preview"),
            &context,
            &rules(),
            at,
        )
        .unwrap();
    assert_eq!(value.decision, AcceptanceDecision::Accepted);
    assert_eq!(value.grade, ConfirmationGrade::Unconfirmed);
}

pub(super) fn decide(
    s: &Setup<SqliteJournal>,
    gate: &Arc<Gatekeeper<SqliteJournal, ymp_storage::content::SqliteContent>>,
    authority: &Authority,
    result: &ResultVersion,
    supporting: &EvidenceRecorded,
    number: usize,
) {
    let mut context = assessment(supporting);
    let lr = LikelihoodRatioTable::standard().unwrap();
    let strongest = StrongestSupport::standard().unwrap();
    if number == 1 {
        let first = record_model(s, authority, &context, &lr);
        authority
            .evidence(
                &s.control,
                evidence_request(
                    s,
                    "correlated-support",
                    result,
                    supporting.evidence.runs.clone(),
                    vec![],
                ),
            )
            .unwrap();
        let repeated = record_model(s, authority, &context, &lr);
        assert_eq!(
            first.entries[&id("criterion")].belief,
            repeated.entries[&id("criterion")].belief
        );
        let experimental = record_model(s, authority, &context, &strongest);
        assert_eq!(experimental.entries[&id("criterion")].belief.get(), 0.97);
        assert_ne!(
            first.entries[&id("criterion")].belief,
            experimental.entries[&id("criterion")].belief
        );
        let view = gate.view(&s.session).unwrap();
        let assessments = view.ledger_state().assessments();
        assert_eq!(
            assessments[1].1.decisions[&id("criterion")].effective,
            *lr.selection()
        );
        assert_eq!(
            assessments.last().unwrap().1.decisions[&id("criterion")].effective,
            *strongest.selection()
        );
        assert!(
            assessments.last().unwrap().1.decisions[&id("criterion")]
                .selection_change
                .is_some()
        );
        assert_eq!(assessments.last().unwrap().1.rules, rules());
        let contradiction = &view.evidence()[&id("contradiction")];
        context.environments.insert(
            contradiction.scope.check.clone().unwrap(),
            BTreeSet::from([contradiction.scope.environment.clone().unwrap()]),
        );
        assert_eq!(
            view.reviews()
                .values()
                .filter(|review| review_applicable(&view, review, &context).unwrap()
                    && review.review.verdict == ReviewVerdict::Approve)
                .count(),
            2
        );
        let contradicted = record_model(s, authority, &context, &high_prior());
        assert_eq!(
            contradicted.entries[&id("criterion")].status,
            LedgerStatus::Contradicted
        );
        assert_eq!(contradicted.entries[&id("criterion")].belief.get(), 0.0);
    } else {
        let ledger = record_model(s, authority, &context, &lr);
        assert_eq!(
            ledger.entries[&id("criterion")].status,
            LedgerStatus::Satisfied
        );
        assert!(
            !ledger.entries[&id("criterion")]
                .evidence
                .contains(&id("support-1"))
        );
        assert!(
            !ledger.entries[&id("criterion")]
                .evidence
                .contains(&id("contradiction"))
        );
    }
    let view = gate.view(&s.session).unwrap();
    let at = view.latest_at() + 1;
    let acceptance = authority
        .acceptance_value(
            &s.session,
            id(&format!("acceptance-{number}")),
            &context,
            &rules(),
            at,
        )
        .unwrap();
    let policy: Box<dyn CreditPolicy> = if number == 1 {
        Box::new(ConfirmedOnly::new().unwrap())
    } else {
        Box::new(IncludeDiscriminated::new().unwrap())
    };
    let replacement = (view.policies()["CreditPolicy"] != *policy.selection())
        .then(|| policy.selection().clone());
    let recorded = authority
        .accept(
            &s.control,
            AcceptanceRequest {
                expected_revision: view.revision(),
                at,
                id: acceptance.id.clone(),
                context: context.clone(),
                rules: rules(),
                credit: CreditResponse {
                    input: credit_input(&acceptance).unwrap(),
                    proposal: policy.creditable(acceptance.grade).unwrap(),
                },
            },
            replacement,
        )
        .unwrap();
    assert_eq!(recorded.acceptance, acceptance);
    let current = s.journal.read(&s.session).unwrap();
    let view = gate.view(&s.session).unwrap();
    let mut downgrade=current.events.iter().rev().find(|event|matches!(&event.payload,ymp_kernel::events::Event::AcceptanceRecorded{data,..} if data.acceptance.id==acceptance.id)).unwrap().clone();
    if let ymp_kernel::events::Event::AcceptanceRecorded { version, .. } = &mut downgrade.payload {
        assert_eq!(*version, 2);
        *version = 1;
    }
    downgrade.seq = view.revision() + 1;
    downgrade.at = view.latest_at() + 1;
    downgrade.input = Some(view.digest().unwrap());
    assert_eq!(
        s.journal
            .append(&s.session, view.revision(), &[downgrade])
            .unwrap_err()
            .code,
        "acceptance_version"
    );
    assert_eq!(gate.view(&s.session).unwrap(), view);
    let basis = acceptance.reference().unwrap();
    let arbiter = Arbiter::new(s.journal.clone());
    let view = gate.view(&s.session).unwrap();
    if number == 1 {
        assert_eq!(
            acceptance.decision,
            AcceptanceDecision::Rejected("failing applicable check".into())
        );
        assert_eq!(acceptance.grade, ConfirmationGrade::Refuted);
        assert!(!recorded.credit.outcome);
        assert!(
            arbiter
                .discharge(
                    gate,
                    &s.session,
                    view.revision(),
                    at,
                    &id("production-1"),
                    &basis
                )
                .is_err()
        );
        assert_eq!(gate.view(&s.session).unwrap(), view);
    } else {
        assert_eq!(acceptance.decision, AcceptanceDecision::Accepted);
        assert_eq!(
            acceptance.grade,
            ConfirmationGrade::Confirmed(ConfirmationBasis::TrustedCheck)
        );
        assert!(recorded.credit.outcome);
        assert!(recorded.credit.selection_change.is_some());
        let earlier = &view.acceptances()[&id("acceptance-1")];
        assert_eq!(earlier.acceptance.grade, ConfirmationGrade::Refuted);
        assert_eq!(
            earlier.credit.effective,
            *ConfirmedOnly::new().unwrap().selection()
        );
        // An exact acceptance is still scoped to its production assignment.
        assert!(
            arbiter
                .discharge(
                    gate,
                    &s.session,
                    view.revision(),
                    at,
                    &id("production-1"),
                    &basis
                )
                .is_err()
        );
        for invalid in [
            Ref {
                id: id("forged-acceptance"),
                version: basis.version.clone(),
            },
            Ref {
                id: basis.id.clone(),
                version: Digest::of(b"stale-version"),
            },
            earlier.acceptance.reference().unwrap(),
        ] {
            assert!(
                arbiter
                    .discharge(
                        gate,
                        &s.session,
                        view.revision(),
                        at,
                        &id("production-2"),
                        &invalid
                    )
                    .is_err()
            );
        }
        assert_eq!(gate.view(&s.session).unwrap(), view);
        assert!(
            authority
                .acceptance_value(&s.session, id("replace-accepted"), &context, &rules(), at)
                .is_err()
        );
        arbiter
            .discharge(
                gate,
                &s.session,
                view.revision(),
                at,
                &id("production-2"),
                &basis,
            )
            .unwrap();
        assert_eq!(
            gate.view(&s.session).unwrap().results().items()[&id("item")]
                .accepted
                .as_ref(),
            Some(&result.id)
        );
    }
}

pub(super) fn abandoned(
    s: &Setup<SqliteJournal>,
    authority: &Authority,
    supporting: &EvidenceRecorded,
) {
    let view = s.gate.view(&s.session).unwrap();
    assert_eq!(
        authority.ledger(&s.session).unwrap().entries[&id("criterion")].status,
        LedgerStatus::Unmet
    );
    assert_eq!(
        authority
            .ledger_inputs(
                &s.session,
                &assessment(supporting),
                &rules(),
                view.latest_at()
            )
            .unwrap_err()
            .code,
        "assessment_stale"
    );
    assert!(!view.ledger_state().assessments().is_empty());
}
