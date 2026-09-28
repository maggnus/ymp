//! Shared exact subject projection. Final aggregates are never fabricated results.
use crate::view::SessionView;
use std::collections::BTreeSet;
use ymp_domain::{
    Denial, Id, Ref, Result, identity::Agent, result::ResultVersion, task::Criterion,
    workspace::Snapshot,
};
#[derive(Clone, Debug)]
pub(crate) struct Subject {
    pub result: Option<Id<ResultVersion>>,
    pub after: Id<Snapshot>,
    pub before: Option<Id<Snapshot>>,
    pub targets: BTreeSet<Id<Criterion>>,
    pub producers: BTreeSet<Id<Agent>>,
}
pub(crate) fn resolve(view: &SessionView, reference: &Ref) -> Result<Subject> {
    if let Some(result) = view
        .results()
        .results()
        .values()
        .find(|r| r.reference().is_ok_and(|r| r == *reference))
    {
        return Ok(Subject {
            result: Some(result.id.clone()),
            after: result.after.clone(),
            before: Some(result.before.clone()),
            targets: view.results().items()[&result.item].targets.clone(),
            producers: BTreeSet::from([result.producer.clone()]),
        });
    }
    let aggregate = view
        .finalization()
        .aggregate
        .as_ref()
        .filter(|a| a.reference().is_ok_and(|r| r == *reference))
        .ok_or_else(|| {
            Denial::new(
                "assessment_subject",
                "No exact retained result or final aggregate",
            )
        })?;
    crate::finalization::aggregate_current(view, aggregate)?;
    let snapshot = view
        .snapshots()
        .values()
        .find(|s| s.reference().is_ok_and(|r| r == aggregate.snapshot))
        .ok_or_else(|| Denial::new("final_snapshot", "Missing final snapshot"))?;
    let before = aggregate
        .baseline
        .as_ref()
        .and_then(|r| {
            view.snapshots()
                .values()
                .find(|s| s.reference().is_ok_and(|actual| actual == *r))
        })
        .map(|s| s.id.clone());
    Ok(Subject {
        result: None,
        after: snapshot.id.clone(),
        before,
        targets: view.criteria().iter().map(|c| c.id.clone()).collect(),
        producers: aggregate.producers.clone(),
    })
}
