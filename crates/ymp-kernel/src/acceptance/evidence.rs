//! Shared applicability and attributed observations, without belief or acceptance.
use super::*;
use std::collections::{BTreeMap, BTreeSet};
use ymp_domain::{
    assignment::{Assignment, ContributionKind, ContributionSubject},
    task::{Criterion, EvidenceClass},
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceScope {
    pub result: Ref,
    pub criterion: Ref,
    pub check: Option<Ref>,
    pub environment: Option<Digest>,
}
/// Expected assessment context supplied by the consumer, including review bases.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApplicabilityContext {
    pub result: Ref,
    pub criteria: BTreeSet<Ref>,
    #[serde(with = "environments")]
    pub environments: BTreeMap<Ref, BTreeSet<Digest>>,
}
// JSON object keys cannot represent the versioned Ref key. A sorted sequence
// preserves the exact check/environment binding in acceptance and ledger events.
mod environments {
    use super::*;
    pub fn serialize<S: serde::Serializer>(
        value: &BTreeMap<Ref, BTreeSet<Digest>>,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        value.iter().collect::<Vec<_>>().serialize(serializer)
    }
    pub fn deserialize<'de, D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<BTreeMap<Ref, BTreeSet<Digest>>, D::Error> {
        let pairs = Vec::<(Ref, BTreeSet<Digest>)>::deserialize(deserializer)?;
        let mut result = BTreeMap::new();
        for (check, environments) in pairs {
            if result.insert(check, environments).is_some() {
                return Err(serde::de::Error::custom("Duplicate assessment check"));
            }
        }
        Ok(result)
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceRecorded {
    pub evidence: Evidence,
    pub scope: EvidenceScope,
    pub reviews: Vec<Ref>,
    pub at: u64,
}
impl EvidenceRecorded {
    pub fn reference(&self) -> Result<Ref> {
        Ok(Ref {
            id: self.evidence.id.erased(),
            version: Digest::of_value(self)?,
        })
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewRecorded {
    pub review: Review,
    pub assignment: Id<Assignment>,
    pub result: Ref,
    pub criteria: Vec<Ref>,
    pub at: u64,
}
impl ReviewRecorded {
    pub fn reference(&self) -> Result<Ref> {
        Ok(Ref {
            id: self.review.id.erased(),
            version: Digest::of_value(self)?,
        })
    }
}
pub struct EvidenceRequest {
    pub expected_revision: u64,
    pub at: u64,
    pub id: Id<Evidence>,
    pub criterion: Ref,
    pub result: Ref,
    pub runs: Vec<Id<CheckRun>>,
    pub reviews: Vec<Id<Review>>,
}
pub struct ReviewRequest {
    pub expected_revision: u64,
    pub at: u64,
    pub id: Id<Review>,
    pub result: Ref,
    pub criteria: Vec<Ref>,
    pub verdict: ReviewVerdict,
    pub findings: Vec<Finding>,
    pub basis: Vec<Id<Evidence>>,
}
fn result(view: &SessionView, reference: &Ref) -> Result<super::subjects::Subject> {
    super::subjects::resolve(view, reference)
}
fn criterion<'a>(view: &'a SessionView, reference: &Ref) -> Result<&'a Criterion> {
    view.criteria()
        .iter()
        .find(|c| c.reference().is_ok_and(|r| r == *reference))
        .ok_or_else(|| Denial::new("evidence_criterion", "Criterion version is not current"))
}
fn current_check<'a>(view: &'a SessionView, reference: &Ref) -> Option<&'a Check> {
    view.checks().values().find(|check| {
        check.reference() == *reference
            && view
                .contract()
                .is_some_and(|c| c.checks.contains(&check.id.erased()))
            && view.criteria().iter().any(|c| {
                c.id == check.criterion
                    && c.reference()
                        .is_ok_and(|r| r.version == check.criterion_version)
            })
    })
}
fn targets(view: &SessionView, result: &super::subjects::Subject) -> Result<Vec<Ref>> {
    result
        .targets
        .iter()
        .map(|id| {
            view.criteria()
                .iter()
                .find(|c| &c.id == id)
                .ok_or_else(|| {
                    Denial::new("review_criteria", "A result target is no longer current")
                })?
                .reference()
        })
        .collect()
}
fn source_independence(
    view: &SessionView,
    check: &Check,
    result: &super::subjects::Subject,
) -> Result<(Independence, Option<Id<ymp_domain::identity::Agent>>)> {
    match &check.author {
        CheckAuthor::User => Ok((Independence::Trusted, None)),
        CheckAuthor::Agent(id) => {
            let author = &view
                .admission()
                .assignments()
                .get(id)
                .ok_or_else(|| {
                    Denial::new("check_author", "Check has no recorded assignment author")
                })?
                .intent
                .assignment;
            Ok((
                if result.producers.contains(&author.agent) {
                    Independence::ProducerAuthored
                } else {
                    check.independence
                },
                Some(author.agent.clone()),
            ))
        }
    }
}
fn bound_run<'a>(
    view: &'a SessionView,
    id: &Id<CheckRun>,
    result: &super::subjects::Subject,
    role: CheckRunRole,
    check: &Check,
) -> Result<&'a CheckRun> {
    let run = view
        .check_runs()
        .get(id)
        .ok_or_else(|| Denial::new("evidence_run", "No recorded check run"))?;
    let snapshot = if role == CheckRunRole::Baseline {
        result.before.as_ref().ok_or_else(|| {
            Denial::new("evidence_baseline", "This subject has no single baseline")
        })?
    } else {
        &result.after
    };
    let target = view
        .snapshots()
        .get(snapshot)
        .ok_or_else(|| Denial::new("snapshot_missing", "No retained result snapshot"))?;
    if run.role != role
        || run.target != *snapshot
        || run.target_version != target.reference()?.version
        || run.check != check.id
        || run.check_version != check.version
    {
        return Err(Denial::new(
            "evidence_run",
            "Run belongs to another role, result snapshot or check version",
        ));
    }
    Ok(run)
}
fn derive_evidence(view: &SessionView, request: &EvidenceRequest) -> Result<EvidenceRecorded> {
    let result = result(view, &request.result)?;
    let criterion = criterion(view, &request.criterion)?;
    if !result.targets.contains(&criterion.id) {
        return Err(Denial::new(
            "evidence_target",
            "Criterion is outside this result's work definition",
        ));
    }
    let (class, independence, author, polarity, discrimination, check, environment, reviews) =
        if request.reviews.is_empty() && !request.runs.is_empty() && request.runs.len() <= 2 {
            if request.runs.iter().collect::<BTreeSet<_>>().len() != request.runs.len() {
                return Err(Denial::new("evidence_run", "A run cannot be counted twice"));
            }
            let runs: Vec<_> = request
                .runs
                .iter()
                .map(|id| {
                    view.check_runs()
                        .get(id)
                        .ok_or_else(|| Denial::new("evidence_run", "No recorded check run"))
                })
                .collect::<Result<_>>()?;
            let candidates: Vec<_> = runs
                .iter()
                .filter(|r| r.role == CheckRunRole::Candidate)
                .collect();
            if candidates.len() != 1
                || runs
                    .iter()
                    .any(|r| !matches!(r.role, CheckRunRole::Candidate | CheckRunRole::Baseline))
            {
                return Err(Denial::new(
                    "evidence_run",
                    "Evidence needs one candidate and at most one baseline",
                ));
            }
            let candidate = candidates[0];
            let check = view
                .checks()
                .get(&candidate.check)
                .and_then(|c| current_check(view, &c.reference()))
                .ok_or_else(|| Denial::new("evidence_check", "Check is not current"))?;
            if check.criterion != criterion.id
                || check.criterion_version != request.criterion.version
            {
                return Err(Denial::new(
                    "evidence_criterion",
                    "Check covers another criterion version",
                ));
            }
            let candidate =
                bound_run(view, &candidate.id, &result, CheckRunRole::Candidate, check)?;
            let polarity = match candidate.outcome {
                CheckOutcome::Pass => Polarity::Supports,
                CheckOutcome::Fail => Polarity::Contradicts,
                CheckOutcome::Error(_) => {
                    return Err(Denial::new(
                        "evidence_inconclusive",
                        "Check error is neither support nor contradiction",
                    ));
                }
            };
            let baseline = runs
                .iter()
                .find(|r| r.role == CheckRunRole::Baseline)
                .map(|run| {
                    let run = bound_run(view, &run.id, &result, CheckRunRole::Baseline, check)?;
                    if run.env != candidate.env {
                        return Err(Denial::new(
                            "evidence_environment",
                            "Baseline and candidate use different environments",
                        ));
                    }
                    Ok(match run.outcome {
                        CheckOutcome::Pass => Some(false),
                        CheckOutcome::Fail => Some(true),
                        CheckOutcome::Error(_) => None,
                    })
                })
                .transpose()?
                .flatten();
            let (independence, author) = source_independence(view, check, &result)?;
            // Executed describes this specific programmatic CheckRun. ExactBytes
            // proves a byte comparison, not that the candidate program was run.
            (
                EvidenceClass::Executed,
                independence,
                author,
                polarity,
                Discrimination {
                    baseline_fails: baseline,
                    candidate_passes: polarity == Polarity::Supports,
                    mutation_score: None,
                },
                Some(check.reference()),
                Some(candidate.env.clone()),
                vec![],
            )
        } else if request.runs.is_empty() && request.reviews.len() == 1 {
            if let Some(review) = view
                .finalization()
                .reviews
                .iter()
                .find(|r| r.verdict.id == request.reviews[0])
            {
                let context = crate::finalization::context(
                    view.finalization()
                        .aggregate
                        .as_ref()
                        .ok_or_else(|| Denial::new("final_scope", "No final scope"))?,
                )?;
                if request.result != review.verdict.aggregate
                    || !crate::finalization::review::applicable(view, review, &context)?
                {
                    return Err(Denial::new(
                        "evidence_review",
                        "Final review is not applicable",
                    ));
                }
                let polarity = match review.verdict.verdict {
                    ReviewVerdict::Approve => Polarity::Supports,
                    ReviewVerdict::Reject => Polarity::Contradicts,
                    ReviewVerdict::NeedsEvidence => {
                        return Err(Denial::new(
                            "evidence_inconclusive",
                            "Final review requests evidence",
                        ));
                    }
                };
                return Ok(EvidenceRecorded {
                    evidence: Evidence {
                        id: request.id.clone(),
                        criterion: criterion.id.clone(),
                        result: None,
                        class: EvidenceClass::Inspection,
                        independence: Independence::IndependentVisible,
                        runs: vec![],
                        discrimination: Discrimination {
                            baseline_fails: None,
                            candidate_passes: false,
                            mutation_score: None,
                        },
                        author: Some(review.reviewer.clone()),
                        polarity,
                    },
                    scope: EvidenceScope {
                        result: request.result.clone(),
                        criterion: request.criterion.clone(),
                        check: None,
                        environment: None,
                    },
                    reviews: vec![review.reference()?],
                    at: request.at,
                });
            }
            let review = view
                .reviews()
                .get(&request.reviews[0])
                .ok_or_else(|| Denial::new("evidence_review", "No recorded review"))?;
            if !review_applicable(view, review, &recorded_context(view, &request.result)?)? {
                return Err(Denial::new(
                    "evidence_review",
                    "Review is not applicable to this result",
                ));
            }
            let polarity = match review.review.verdict {
                ReviewVerdict::Approve => Polarity::Supports,
                ReviewVerdict::Reject => Polarity::Contradicts,
                ReviewVerdict::NeedsEvidence => {
                    return Err(Denial::new(
                        "evidence_inconclusive",
                        "A request for evidence establishes no polarity",
                    ));
                }
            };
            (
                EvidenceClass::Inspection,
                Independence::IndependentVisible,
                Some(review.review.reviewer.clone()),
                polarity,
                Discrimination {
                    baseline_fails: None,
                    candidate_passes: false,
                    mutation_score: None,
                },
                None,
                None,
                vec![review.reference()?],
            )
        } else {
            return Err(Denial::new(
                "evidence_sources",
                "Keep check evidence and review statements separately attributed",
            ));
        };
    let mut runs = request.runs.clone();
    runs.sort();
    let evidence = Evidence {
        id: request.id.clone(),
        criterion: criterion.id.clone(),
        result: result.result.clone(),
        class,
        independence,
        runs,
        discrimination,
        author,
        polarity,
    };
    evidence.validate()?;
    Ok(EvidenceRecorded {
        evidence,
        scope: EvidenceScope {
            result: request.result.clone(),
            criterion: request.criterion.clone(),
            check,
            environment,
        },
        reviews,
        at: request.at,
    })
}

/// The caller names its current result, criterion, check and expected environment.
/// No latest-ID or last-environment heuristic can silently select that context.
pub fn evidence_applicable(
    view: &SessionView,
    evidence: &EvidenceRecorded,
    context: &ApplicabilityContext,
) -> Result<bool> {
    if view.evidence().get(&evidence.evidence.id) != Some(evidence) {
        return Ok(false);
    }
    sources_current(
        view,
        vec![Source::Evidence(evidence.evidence.id.clone())],
        context,
    )
}
pub fn applicable_evidence<'a>(
    view: &'a SessionView,
    context: &ApplicabilityContext,
) -> Result<Vec<&'a EvidenceRecorded>> {
    let mut applicable = vec![];
    for evidence in view.evidence().values() {
        if evidence_applicable(view, evidence, context)? {
            applicable.push(evidence);
        }
    }
    Ok(applicable)
}
pub fn review_applicable(
    view: &SessionView,
    review: &ReviewRecorded,
    context: &ApplicabilityContext,
) -> Result<bool> {
    if view.reviews().get(&review.review.id) != Some(review) {
        return Ok(false);
    }
    sources_current(
        view,
        vec![Source::Review(review.review.id.clone())],
        context,
    )
}
enum Source {
    Evidence(Id<Evidence>),
    Review(Id<Review>),
}
fn sources_current(
    view: &SessionView,
    mut pending: Vec<Source>,
    context: &ApplicabilityContext,
) -> Result<bool> {
    let mut evidence_seen = BTreeSet::new();
    let mut review_seen = BTreeSet::new();
    while let Some(source) = pending.pop() {
        match source {
            Source::Evidence(id) => {
                if !evidence_seen.insert(id.clone()) {
                    continue;
                }
                let Some(evidence) = view.evidence().get(&id) else {
                    return Ok(false);
                };
                let scope = &evidence.scope;
                if scope.result != context.result
                    || !context.criteria.contains(&scope.criterion)
                    || result(view, &scope.result).is_err()
                    || criterion(view, &scope.criterion).is_err()
                {
                    return Ok(false);
                }
                match (&scope.check, &scope.environment) {
                    (Some(check), Some(environment)) => {
                        let Some(check) = current_check(view, check) else {
                            return Ok(false);
                        };
                        if check.criterion.erased() != scope.criterion.id
                            || check.criterion_version != scope.criterion.version
                            || !context
                                .environments
                                .get(&check.reference())
                                .is_some_and(|expected| expected.contains(environment))
                            || evidence.evidence.runs.is_empty()
                            || !evidence.evidence.runs.iter().all(|id| {
                                view.check_runs().get(id).is_some_and(|r| {
                                    r.check == check.id
                                        && r.check_version == check.version
                                        && r.env == *environment
                                })
                            })
                        {
                            return Ok(false);
                        }
                    }
                    (None, None) if evidence.reviews.len() == 1 => {
                        if let Some(review) = view.finalization().reviews.iter().find(|r| {
                            r.reference()
                                .is_ok_and(|reference| reference == evidence.reviews[0])
                        }) {
                            if review.verdict.aggregate != scope.result {
                                return Ok(false);
                            }
                            pending.push(Source::Review(review.verdict.id.clone()));
                            continue;
                        }
                        let Some(review) = view
                            .reviews()
                            .values()
                            .find(|r| r.reference().is_ok_and(|r| r == evidence.reviews[0]))
                        else {
                            return Ok(false);
                        };
                        if review.result != scope.result {
                            return Ok(false);
                        }
                        pending.push(Source::Review(review.review.id.clone()));
                    }
                    _ => return Ok(false),
                }
            }
            Source::Review(id) => {
                if !review_seen.insert(id.clone()) {
                    continue;
                }
                if let Some(review) = view
                    .finalization()
                    .reviews
                    .iter()
                    .find(|r| r.verdict.id == id)
                {
                    let Ok(subject) = result(view, &review.verdict.aggregate) else {
                        return Ok(false);
                    };
                    if review.verdict.aggregate != context.result
                        || subject.producers.contains(&review.reviewer)
                        || review.criteria.iter().cloned().collect::<BTreeSet<_>>()
                            != context.criteria
                    {
                        return Ok(false);
                    }
                    for id in &review.verdict.basis {
                        let Some(evidence) = view.evidence().get(id) else {
                            return Ok(false);
                        };
                        if evidence.scope.result != context.result {
                            return Ok(false);
                        }
                        pending.push(Source::Evidence(id.clone()));
                    }
                    continue;
                }
                let Some(review) = view.reviews().get(&id) else {
                    return Ok(false);
                };
                let Ok(result) = result(view, &review.result) else {
                    return Ok(false);
                };
                let Ok(criteria) = targets(view, &result) else {
                    return Ok(false);
                };
                if review.result != context.result
                    || result.producers.contains(&review.review.reviewer)
                    || review.criteria != criteria
                    || !review
                        .criteria
                        .iter()
                        .all(|reference| context.criteria.contains(reference))
                {
                    return Ok(false);
                }
                for id in &review.review.basis {
                    let Some(evidence) = view.evidence().get(id) else {
                        return Ok(false);
                    };
                    if evidence.scope.result != review.result {
                        return Ok(false);
                    }
                    pending.push(Source::Evidence(id.clone()));
                }
            }
        }
    }
    Ok(true)
}
// Recording a statement validates its declared sources, without choosing a future
// assessment environment. A7/A8/report consumers must pass their own context.
pub(crate) fn recorded_context(
    view: &SessionView,
    reference: &Ref,
) -> Result<ApplicabilityContext> {
    let result = result(view, reference)?;
    let mut environments: BTreeMap<Ref, BTreeSet<Digest>> = BTreeMap::new();
    for evidence in view
        .evidence()
        .values()
        .filter(|e| e.scope.result == *reference)
    {
        if let (Some(check), Some(environment)) =
            (&evidence.scope.check, &evidence.scope.environment)
        {
            environments
                .entry(check.clone())
                .or_default()
                .insert(environment.clone());
        }
    }
    Ok(ApplicabilityContext {
        result: reference.clone(),
        criteria: targets(view, &result)?.into_iter().collect(),
        environments,
    })
}
pub(crate) fn derive_review(
    view: &SessionView,
    assignment: &Assignment,
    request: &ReviewRequest,
) -> Result<ReviewRecorded> {
    let result = result(view, &request.result)?;
    let contribution = &view.coordination().contributions()[&assignment.contribution];
    let criteria = targets(view, &result)?;
    if assignment.role != RoleKind::Reviewer
        || contribution.value.kind != ContributionKind::Review
        || contribution.value.subject
            != Some(ContributionSubject::ResultVersion(request.result.clone()))
        || result.producers.contains(&assignment.agent)
        || request.criteria != criteria
        || contribution.contract != view.contract().unwrap().reference()
        || request
            .findings
            .iter()
            .filter_map(|f| f.criterion.as_ref())
            .any(|id| !contribution.value.targets.contains(id))
    {
        return Err(Denial::new(
            "review_author",
            "Review differs from its independent admitted reviewer, exact result or criterion versions",
        ));
    }
    for id in &request.basis {
        let evidence = view
            .evidence()
            .get(id)
            .ok_or_else(|| Denial::new("review_basis", "No recorded evidence"))?;
        if evidence.scope.result != request.result
            || !evidence_applicable(view, evidence, &recorded_context(view, &request.result)?)?
        {
            return Err(Denial::new(
                "review_basis",
                "Review basis is not applicable",
            ));
        }
    }
    let review = Review {
        id: request.id.clone(),
        result: result.result.clone().ok_or_else(|| {
            Denial::new(
                "review_subject",
                "Final review has its own paid provenance consumer",
            )
        })?,
        reviewer: assignment.agent.clone(),
        profile: assignment.profile.clone(),
        verdict: request.verdict,
        findings: request.findings.clone(),
        basis: request.basis.clone(),
    };
    review.validate()?;
    Ok(ReviewRecorded {
        review,
        assignment: assignment.id.clone(),
        result: request.result.clone(),
        criteria,
        at: request.at,
    })
}
pub(crate) fn evidence_refs(view: &SessionView, data: &EvidenceRecorded) -> Result<Vec<Ref>> {
    let mut refs = vec![data.scope.result.clone(), data.scope.criterion.clone()];
    if let Some(check) = &data.scope.check {
        refs.push(check.clone());
    }
    refs.extend(data.reviews.iter().cloned());
    for id in &data.evidence.runs {
        refs.push(
            view.check_runs()
                .get(id)
                .ok_or_else(|| Denial::new("evidence_run", "No recorded run"))?
                .reference()?,
        );
    }
    refs.sort();
    refs.dedup();
    Ok(refs)
}
pub(crate) fn review_refs(view: &SessionView, data: &ReviewRecorded) -> Result<Vec<Ref>> {
    let source = view
        .admission()
        .assignments()
        .get(&data.assignment)
        .ok_or_else(|| Denial::new("review_author", "No admitted reviewer"))?;
    let mut refs = vec![
        data.result.clone(),
        source.references.last().unwrap().clone(),
    ];
    refs.extend(data.criteria.iter().cloned());
    for id in &data.review.basis {
        refs.push(
            view.evidence()
                .get(id)
                .ok_or_else(|| Denial::new("review_basis", "No evidence"))?
                .reference()?,
        );
    }
    refs.sort();
    refs.dedup();
    Ok(refs)
}
pub(crate) fn validate_evidence(
    view: &SessionView,
    event: &Envelope<Event>,
    data: &EvidenceRecorded,
) -> Result<()> {
    let reviews = data
        .reviews
        .iter()
        .map(|reference| {
            if let Some(review) = view
                .finalization()
                .reviews
                .iter()
                .find(|r| r.reference().is_ok_and(|r| r == *reference))
            {
                return Ok(review.verdict.id.clone());
            }
            view.reviews()
                .values()
                .find(|r| r.reference().is_ok_and(|r| r == *reference))
                .map(|r| r.review.id.clone())
                .ok_or_else(|| Denial::new("evidence_review", "No exact review"))
        })
        .collect::<Result<Vec<_>>>()?;
    let expected = derive_evidence(
        view,
        &EvidenceRequest {
            expected_revision: view.revision(),
            at: event.at,
            id: data.evidence.id.clone(),
            result: data.scope.result.clone(),
            criterion: data.scope.criterion.clone(),
            runs: data.evidence.runs.clone(),
            reviews,
        },
    )?;
    if *data != expected
        || view.evidence().contains_key(&data.evidence.id)
        || view.evidence().len() >= 4096
        || event.at < view.latest_at()
        || event.policy.is_some()
        || event.input != Some(view.digest()?)
        || event.refs != evidence_refs(view, data)?
    {
        return Err(Denial::new(
            "evidence_attribution",
            "Evidence differs from its kernel-derived source, polarity, class or independence",
        ));
    }
    Ok(())
}
pub(crate) fn validate_review(
    view: &SessionView,
    event: &Envelope<Event>,
    data: &ReviewRecorded,
) -> Result<()> {
    let source = view
        .admission()
        .assignments()
        .get(&data.assignment)
        .ok_or_else(|| Denial::new("review_author", "No admitted reviewer"))?;
    crate::gatekeeper::validate_active_grant(view, source, None, event.at)?;
    let expected = derive_review(
        view,
        &source.intent.assignment,
        &ReviewRequest {
            expected_revision: view.revision(),
            at: event.at,
            id: data.review.id.clone(),
            result: data.result.clone(),
            criteria: data.criteria.clone(),
            verdict: data.review.verdict,
            findings: data.review.findings.clone(),
            basis: data.review.basis.clone(),
        },
    )?;
    if *data != expected
        || view.reviews().contains_key(&data.review.id)
        || view.reviews().len() >= 4096
        || event.at < view.latest_at()
        || event.policy.is_some()
        || event.input != Some(view.digest()?)
        || event.refs != review_refs(view, data)?
    {
        return Err(Denial::new(
            "review_attribution",
            "Review differs from its actual assignment or recorded basis",
        ));
    }
    Ok(())
}
impl<J: Journal, C: ContentStore> AcceptanceAuthority<J, C> {
    pub fn evidence(
        &self,
        owner: &SessionControl,
        request: EvidenceRequest,
    ) -> Result<EvidenceRecorded> {
        let session = self.owner.authorize(owner)?;
        let view = self.journal.view(&session, None)?;
        boundary(&view, request.expected_revision, request.at)?;
        let data = derive_evidence(&view, &request)?;
        let event = Envelope {
            seq: next(view.revision())?,
            session: session.clone(),
            at: request.at,
            actor: Actor::Runtime,
            policy: None,
            input: Some(view.digest()?),
            refs: evidence_refs(&view, &data)?,
            payload: Event::EvidenceRecorded {
                version: 1,
                data: Box::new(data.clone()),
            },
        };
        self.append(&session, view.revision(), event)?;
        Ok(data)
    }
    pub fn review(
        &self,
        gate: &Gatekeeper<J, C>,
        token: &GrantToken,
        request: ReviewRequest,
    ) -> Result<ReviewRecorded> {
        gate.require_journal(&self.journal)?;
        let assignment =
            gate.authorize_revision(token, None, request.at, request.expected_revision)?;
        let view = self.journal.view(token.session(), None)?;
        boundary(&view, request.expected_revision, request.at)?;
        let data = derive_review(&view, &assignment, &request)?;
        let event = Envelope {
            seq: next(view.revision())?,
            session: token.session().clone(),
            at: request.at,
            actor: Actor::Runtime,
            policy: None,
            input: Some(view.digest()?),
            refs: review_refs(&view, &data)?,
            payload: Event::ReviewRecorded {
                version: 1,
                data: Box::new(data.clone()),
            },
        };
        self.append(token.session(), view.revision(), event)?;
        Ok(data)
    }
}

pub(crate) fn final_review_applicable(
    view: &SessionView,
    id: &Id<Review>,
    context: &ApplicabilityContext,
) -> Result<bool> {
    sources_current(view, vec![Source::Review(id.clone())], context)
}
