//! Explicit fixed methods; routing by learned task features is a later experiment.
use std::collections::BTreeMap;
use ymp_domain::{
    Id, Proposal, Result,
    journal::{EscalationStep, Method, MethodKind, MethodParameters, PolicySelection},
};
use ymp_kernel::{ports::planning::MethodRouter, view::SessionView};
pub struct FixedMethod {
    selection: PolicySelection,
}
impl FixedMethod {
    pub fn new(kind: MethodKind) -> Result<Self> {
        if !matches!(kind, MethodKind::Solo | MethodKind::SoloWithVerifier) {
            return Err(ymp_domain::Denial::new(
                "method_unavailable",
                "Initial fixed methods are Solo and SoloWithVerifier",
            ));
        }
        Ok(Self {
            selection: PolicySelection::new(
                "MethodRouter",
                "FixedMethod",
                "1",
                serde_json::json!({"kind":kind,"ladder":[EscalationStep::StopPreserving]}),
            )?,
        })
    }
}
impl MethodRouter for FixedMethod {
    fn selection(&self) -> &PolicySelection {
        &self.selection
    }
    fn choose(&self, view: &SessionView) -> Result<Proposal<Method>> {
        let p = MethodParameters::from_selection(&self.selection)?;
        Ok(Proposal {
            value: Method {
                id: Id::new("method")?,
                kind: p.kind,
                ladder: p.ladder,
                params: BTreeMap::new(),
                policy: self.selection.policy.clone(),
            },
            rationale: "Use the explicitly selected initial fixed method".into(),
            basis: view
                .contract()
                .map(|c| vec![c.reference()])
                .unwrap_or_default(),
            policy: self.selection.policy.clone(),
        })
    }
}
