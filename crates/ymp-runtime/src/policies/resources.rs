//! Initial deterministic resource mechanisms. Forecast parameters are assumptions,
//! not measured accuracy. Treasury remains responsible for admission and settlement.
use ymp_domain::{
    Denial, Digest, Proposal, Result,
    journal::{PolicySelection, decode, encode},
    resources::*,
};
use ymp_kernel::ports::resources::*;

pub use ymp_domain::resources::{PriceWeightedParameters, PurposeBoundedParameters};

pub struct PriceWeighted {
    selection: PolicySelection,
    parameters: PriceWeightedParameters,
}
impl PriceWeighted {
    pub fn new(parameters: PriceWeightedParameters) -> Result<Self> {
        parameters.validate()?;
        let selection = PolicySelection::new(
            "CostModel",
            "PriceWeighted",
            "1",
            serde_json::to_value(&parameters)
                .map_err(|_| Denial::new("parameters", "Cannot encode cost parameters"))?,
        )?;
        Ok(Self {
            selection,
            parameters,
        })
    }
    pub fn from_selection(selection: &PolicySelection) -> Result<Self> {
        selection.validate()?;
        if selection.policy.port != "CostModel"
            || selection.policy.implementation != "PriceWeighted"
            || selection.policy.version != "1"
        {
            return Err(Denial::new(
                "policy",
                "Expected CostModel/PriceWeighted version 1",
            ));
        }
        let parameters: PriceWeightedParameters = decode(&encode(&selection.parameters)?)?;
        parameters.validate()?;
        Ok(Self {
            selection: selection.clone(),
            parameters,
        })
    }
}
fn amount(value: f64) -> Result<CostUnits> {
    CostUnits::new(value).map_err(|_| Denial::new("cost_overflow", "Cost is not finite"))
}
fn count(value: u64) -> Result<CostUnits> {
    if value > 1_u64 << 53 {
        return Err(Denial::new(
            "usage_overflow",
            "Usage exceeds exact integer pricing range",
        ));
    }
    amount(value as f64)
}
pub fn weighted_cost(usage: &Usage, rates: &Rates) -> Result<CostUnits> {
    usage.validate()?;
    rates.validate()?;
    let uncached = usage.input - usage.cache_read - usage.cache_write;
    let mut total = amount(0.0)?;
    for (tokens, rate) in [
        (uncached, rates.input),
        (usage.cache_read, rates.cache_read),
        (usage.cache_write, rates.cache_write),
        (usage.output, rates.output),
    ] {
        total = add(total, multiply(count(tokens)?, rate)?)?;
    }
    Ok(total)
}
fn proposal<T>(selection: &PolicySelection, value: T, rationale: &str) -> Proposal<T> {
    Proposal {
        value,
        rationale: rationale.into(),
        basis: vec![],
        policy: selection.policy.clone(),
    }
}
impl CostModel for PriceWeighted {
    fn selection(&self) -> &PolicySelection {
        &self.selection
    }
    fn cost(&self, view: &CostView) -> Result<Proposal<ReceiptPrice>> {
        view.receipt.validate()?;
        view.pricebook.validate()?;
        view.allowance.validate()?;
        let value = if let Some(cost) = view.known_complete_cost {
            nonnegative(cost)?;
            ReceiptPrice::Known(cost)
        } else if view.receipt.coverage == Coverage::Complete {
            ReceiptPrice::Known(weighted_cost(
                &view.receipt.usage,
                view.pricebook
                    .rates(&view.demand.provider, &view.demand.profile.model),
            )?)
        } else {
            match view.unknown_usage {
                UnknownUsage::Stop => ReceiptPrice::Unknown,
                UnknownUsage::Estimate => ReceiptPrice::Estimated(view.allowance.cost),
            }
        };
        Ok(proposal(
            &self.selection,
            value,
            "Use recorded complete-cost evidence or price complete usage; preserve unknown coverage",
        ))
    }
    fn estimate(&self, view: &EstimateView) -> Result<Proposal<CostEstimate>> {
        view.pricebook.validate()?;
        let pricebook_digest = Digest::of_value(&view.pricebook)?;
        let history: Vec<_> = view
            .history
            .iter()
            .filter(|entry| {
                !entry.estimated
                    && entry.pricebook == pricebook_digest
                    && entry.policy == self.selection.policy
                    && entry.demand.profile == view.demand.profile
                    && entry.demand.provider == view.demand.provider
                    && entry.demand.kind == view.demand.kind
                    && entry.demand.difficulty == view.demand.difficulty
            })
            .collect();
        let expected = if history.is_empty() {
            let scale = match view.demand.difficulty {
                Difficulty::Simple => 1,
                Difficulty::Standard => 2,
                Difficulty::Complex => 4,
            };
            let input = self
                .parameters
                .expected_input
                .checked_mul(scale)
                .ok_or_else(|| Denial::new("estimate", "Forecast input overflow"))?;
            let output = self
                .parameters
                .expected_output
                .checked_mul(scale)
                .ok_or_else(|| Denial::new("estimate", "Forecast output overflow"))?;
            weighted_cost(
                &Usage {
                    input,
                    cache_read: 0,
                    cache_write: 0,
                    output,
                    reasoning: None,
                },
                view.pricebook
                    .rates(&view.demand.provider, &view.demand.profile.model),
            )?
        } else {
            let mut total = amount(0.0)?;
            for entry in &history {
                total = add(total, entry.cost)?;
            }
            amount(total.get() / history.len() as f64)?
        };
        let estimate = CostEstimate {
            expected,
            p90: multiply(expected, self.parameters.p90_factor)?,
        };
        estimate.validate()?;
        let mut response = proposal(
            &self.selection,
            estimate,
            "Use matching known history, otherwise the explicit usage forecast and p90 factor",
        );
        response.basis = history.iter().map(|entry| entry.basis.clone()).collect();
        Ok(response)
    }
}

pub struct PurposeBounded {
    selection: PolicySelection,
    parameters: PurposeBoundedParameters,
}
impl PurposeBounded {
    pub fn new(parameters: PurposeBoundedParameters) -> Result<Self> {
        parameters.validate()?;
        let selection = PolicySelection::new(
            "ResourcePolicy",
            "PurposeBounded",
            "1",
            serde_json::to_value(&parameters)
                .map_err(|_| Denial::new("parameters", "Cannot encode resource parameters"))?,
        )?;
        Ok(Self {
            selection,
            parameters,
        })
    }
    pub fn from_selection(selection: &PolicySelection) -> Result<Self> {
        selection.validate()?;
        if selection.policy.port != "ResourcePolicy"
            || selection.policy.implementation != "PurposeBounded"
            || selection.policy.version != "1"
        {
            return Err(Denial::new(
                "policy",
                "Expected ResourcePolicy/PurposeBounded version 1",
            ));
        }
        let parameters: PurposeBoundedParameters = decode(&encode(&selection.parameters)?)?;
        parameters.validate()?;
        Ok(Self {
            selection: selection.clone(),
            parameters,
        })
    }
    fn bounded(
        &self,
        cost: CostUnits,
        scale: u64,
        at: u64,
        deadline: Option<u64>,
    ) -> Result<Allowance> {
        let timeout = self
            .parameters
            .timeout
            .checked_mul(scale)
            .ok_or_else(|| Denial::new("allowance", "Timeout overflow"))?;
        let timeout = deadline
            .map(|deadline| timeout.min(deadline.saturating_sub(at)))
            .unwrap_or(timeout);
        let output_chars = self
            .parameters
            .output_chars
            .checked_mul(scale)
            .ok_or_else(|| Denial::new("allowance", "Output limit overflow"))?;
        let allowance = Allowance {
            cost,
            timeout,
            native_turns: self.parameters.native_turns,
            output_chars,
        };
        allowance.validate()?;
        Ok(allowance)
    }
}
impl ResourcePolicy for PurposeBounded {
    fn selection(&self) -> &PolicySelection {
        &self.selection
    }
    fn allowance(&self, view: &AllowanceView) -> Result<Proposal<Allowance>> {
        view.estimate.validate()?;
        nonnegative(view.remaining)?;
        let cost = amount(
            view.estimate
                .p90
                .get()
                .min(self.parameters.max_cost.get())
                .min(view.remaining.get()),
        )?;
        if cost < view.estimate.expected {
            return Err(Denial::new(
                "budget",
                "Available allowance cannot cover the expected contribution cost",
            ));
        }
        let scale = match (view.demand.kind, view.demand.difficulty) {
            (ContributionKind::Narrate | ContributionKind::Clarify, _)
            | (_, Difficulty::Simple) => 1,
            (_, Difficulty::Standard) => 2,
            (_, Difficulty::Complex) => 4,
        };
        Ok(proposal(
            &self.selection,
            self.bounded(cost, scale, view.at, view.deadline)?,
            "Bound this contribution by purpose, difficulty, deadline and available resources",
        ))
    }
    fn reporting_reserve(&self, view: &ReportingView) -> Result<Proposal<ReportingPlan>> {
        view.task.validate()?;
        let available = remaining(
            view.task.constraints.budget,
            view.task.constraints.verification_reserve,
        )?;
        let required = add(
            self.parameters.report_call_cost,
            self.parameters.report_call_cost,
        )?;
        let plan = if !view.pool.eligible.is_empty()
            && required <= available
            && view
                .task
                .constraints
                .deadline
                .is_none_or(|deadline| deadline > view.at)
        {
            let allowance = self.bounded(
                self.parameters.report_call_cost,
                1,
                view.at,
                view.task.constraints.deadline,
            )?;
            ReportingPlan {
                narration: Some(allowance.clone()),
                correction: Some(allowance),
            }
        } else {
            ReportingPlan::deterministic()
        };
        Ok(proposal(
            &self.selection,
            plan,
            "Reserve one narration and one correction when feasible; otherwise use deterministic reporting",
        ))
    }
}

pub fn cost_response(
    model: &dyn CostModel,
    view: &CostView,
) -> Result<ResourceResponse<ReceiptPrice>> {
    let input = Digest::of_value(view)?;
    let proposal = model.cost(view)?;
    Ok(ResourceResponse { input, proposal })
}
pub fn estimate_response(
    model: &dyn CostModel,
    view: &EstimateView,
) -> Result<ResourceResponse<CostEstimate>> {
    let input = Digest::of_value(view)?;
    let proposal = model.estimate(view)?;
    Ok(ResourceResponse { input, proposal })
}
pub fn allowance_response(
    policy: &dyn ResourcePolicy,
    view: &AllowanceView,
) -> Result<ResourceResponse<Allowance>> {
    let input = Digest::of_value(view)?;
    let proposal = policy.allowance(view)?;
    Ok(ResourceResponse { input, proposal })
}
pub fn reporting_response(
    policy: &dyn ResourcePolicy,
    view: &ReportingView,
) -> Result<ResourceResponse<ReportingPlan>> {
    let input = Digest::of_value(view)?;
    let proposal = policy.reporting_reserve(view)?;
    Ok(ResourceResponse { input, proposal })
}
