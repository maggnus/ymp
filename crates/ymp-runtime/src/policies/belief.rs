//! Neutral log-odds aggregation and an experimental strongest-support rule.
use std::collections::BTreeSet;
use ymp_domain::{
    Denial, Digest, Prob, Proposal, Result,
    journal::PolicySelection,
    task::{EvidenceClass, Real},
    verification::*,
};
use ymp_kernel::ports::planning::{BeliefModel, BeliefPrior, BeliefUpdate, BeliefView, entry_for};

pub fn default_thresholds() -> BeliefThresholds {
    BeliefThresholds {
        behavior: Prob::new(0.9).unwrap(),
        quality: Prob::new(0.6).unwrap(),
        support: Prob::new(0.6).unwrap(),
    }
}
pub struct LikelihoodRatioTable {
    selection: PolicySelection,
    parameters: LikelihoodRatioParameters,
}
impl LikelihoodRatioTable {
    pub fn new(parameters: LikelihoodRatioParameters) -> Result<Self> {
        parameters.validate()?;
        Ok(Self {
            selection: PolicySelection::new(
                "BeliefModel",
                "LikelihoodRatioTable",
                "1",
                serde_json::to_value(&parameters).map_err(|_| {
                    Denial::new("belief_parameters", "Cannot encode belief parameters")
                })?,
            )?,
            parameters,
        })
    }
    pub fn standard() -> Result<Self> {
        Self::new(LikelihoodRatioParameters {
            trusted: Real::new(50.0)?,
            external: Real::new(30.0)?,
            hidden: Real::new(20.0)?,
            independent: Real::new(5.0)?,
            inspection: Real::new(1.5)?,
            producer: Real::new(1.5)?,
            static_read: Real::new(1.1)?,
            thresholds: default_thresholds(),
        })
    }
    fn ratio(&self, view: &BeliefView, evidence: &Evidence) -> f64 {
        let p = &self.parameters;
        if evidence.class == EvidenceClass::ExternalData {
            return p.external.get();
        }
        if matches!(
            evidence.class,
            EvidenceClass::Executed | EvidenceClass::Browser
        ) {
            match evidence.independence {
                Independence::Trusted => return p.trusted.get(),
                Independence::IndependentHidden
                    if discriminating(evidence, &view.criterion, &view.rules) =>
                {
                    return p.hidden.get();
                }
                Independence::IndependentHidden | Independence::IndependentVisible => {
                    return p.independent.get();
                }
                Independence::ProducerAuthored if evidence.class == EvidenceClass::Executed => {
                    return p.producer.get();
                }
                _ => {}
            }
        }
        if evidence.class == EvidenceClass::Inspection
            && matches!(
                evidence.independence,
                Independence::IndependentVisible | Independence::IndependentHidden
            )
        {
            return p.inspection.get();
        }
        if evidence.class == EvidenceClass::StaticRead {
            return p.static_read.get();
        }
        1.0
    }
}
fn proposal(
    selection: &PolicySelection,
    view: &BeliefView,
    belief: Prob,
    prior: BeliefPrior,
    thresholds: &BeliefThresholds,
    rationale: &str,
) -> Result<Proposal<BeliefUpdate>> {
    Ok(Proposal {
        value: BeliefUpdate {
            entry: entry_for(view, belief, thresholds),
            prior,
        },
        rationale: rationale.into(),
        basis: vec![view.criterion.reference()?, view.subject.clone()],
        policy: selection.policy.clone(),
    })
}
impl BeliefModel for LikelihoodRatioTable {
    fn selection(&self) -> &PolicySelection {
        &self.selection
    }
    fn update(&self, view: &BeliefView) -> Result<Proposal<BeliefUpdate>> {
        let mut groups = BTreeSet::new();
        let mut log_odds = 0.0_f64;
        for evidence in view
            .evidence
            .iter()
            .filter(|e| e.polarity == Polarity::Supports)
        {
            if groups.insert(Digest::of_value(&(
                evidence.independence,
                &evidence.class,
                &evidence.discrimination,
            ))?) {
                log_odds += self.ratio(view, evidence).ln();
            }
        }
        let belief = Prob::new(if log_odds >= 0.0 {
            1.0 / (1.0 + (-log_odds).exp())
        } else {
            let odds = log_odds.exp();
            odds / (1.0 + odds)
        })?;
        proposal(
            &self.selection,
            view,
            belief,
            BeliefPrior {
                criterion: view.criterion.reference()?,
                subject: view.subject.clone(),
                value: Prob::new(0.5)?,
                basis: vec![],
                rationale: "Neutral prior about this criterion and present result".into(),
            },
            &self.parameters.thresholds,
            "Apply one likelihood ratio per correlated supporting group",
        )
    }
}
pub struct StrongestSupport {
    selection: PolicySelection,
    parameters: StrongestSupportParameters,
}
impl StrongestSupport {
    pub fn new(parameters: StrongestSupportParameters) -> Result<Self> {
        parameters.validate()?;
        Ok(Self {
            selection: PolicySelection::new(
                "BeliefModel",
                "StrongestSupport",
                "1",
                serde_json::to_value(&parameters).map_err(|_| {
                    Denial::new("belief_parameters", "Cannot encode experimental parameters")
                })?,
            )?,
            parameters,
        })
    }
    pub fn standard() -> Result<Self> {
        Self::new(StrongestSupportParameters {
            prior: Prob::new(0.5)?,
            trusted: Prob::new(0.97)?,
            independent: Prob::new(0.8)?,
            inspection: Prob::new(0.6)?,
            producer: Prob::new(0.6)?,
            static_read: Prob::new(0.55)?,
            thresholds: default_thresholds(),
        })
    }
    fn strength(&self, evidence: &Evidence) -> f64 {
        let p = &self.parameters;
        if evidence.class == EvidenceClass::ExternalData {
            return p.trusted.get();
        }
        if matches!(
            evidence.class,
            EvidenceClass::Executed | EvidenceClass::Browser
        ) {
            match evidence.independence {
                Independence::Trusted => return p.trusted.get(),
                Independence::IndependentHidden | Independence::IndependentVisible => {
                    return p.independent.get();
                }
                Independence::ProducerAuthored if evidence.class == EvidenceClass::Executed => {
                    return p.producer.get();
                }
                _ => {}
            }
        }
        if evidence.class == EvidenceClass::Inspection {
            return p.inspection.get();
        }
        if evidence.class == EvidenceClass::StaticRead {
            return p.static_read.get();
        }
        0.5
    }
}
impl BeliefModel for StrongestSupport {
    fn selection(&self) -> &PolicySelection {
        &self.selection
    }
    fn update(&self, view: &BeliefView) -> Result<Proposal<BeliefUpdate>> {
        let prior = self.parameters.prior;
        let basis = if prior.get() == 0.5 {
            vec![]
        } else {
            let polarity = if prior.get() > 0.5 {
                Polarity::Supports
            } else {
                Polarity::Contradicts
            };
            vec![
                view.prior_basis
                    .iter()
                    .find(|b| b.polarity == polarity)
                    .ok_or_else(|| {
                        Denial::new(
                            "belief_prior_basis",
                            "A non-neutral experimental prior needs a related recorded statement",
                        )
                    })?
                    .source
                    .clone(),
            ]
        };
        // Max does not multiply a review into the estimate twice when the same
        // source supports both the explicit prior assumption and Inspection.
        let belief = view
            .evidence
            .iter()
            .filter(|e| e.polarity == Polarity::Supports)
            .fold(prior.get(), |best, e| best.max(self.strength(e)));
        proposal(&self.selection, view, Prob::new(belief)?, BeliefPrior { criterion: view.criterion.reference()?, subject: view.subject.clone(), value: prior, basis,
            rationale: "Experimental present-result assumption supported by recorded statements; calibration is unproven".into() },
            &self.parameters.thresholds, "Use the strongest applicable support without accumulating weak groups")
    }
}
