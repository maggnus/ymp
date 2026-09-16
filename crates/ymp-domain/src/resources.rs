//! Normalized resource accounting values; no I/O or inferred billing.
pub use crate::task::CostUnits;
use crate::{
    Denial, Id, Result,
    identity::{ExecutionProfile, Provider},
    require_text,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Purpose {
    Production,
    Verification,
    Coordination,
    Reporting,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum UnknownUsage {
    Stop,
    Estimate,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Coverage {
    Complete,
    Partial,
    Unknown,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReservationState {
    Held,
    Settled,
    Released,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Difficulty {
    Simple,
    Standard,
    Complex,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContributionKind {
    Plan,
    DesignChecks,
    Produce,
    Verify,
    Review,
    Research,
    Alternative,
    Diagnose,
    Decompose,
    Integrate,
    Clarify,
    Curate,
    Narrate,
    Judge,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Competence {
    Planning,
    CheckDesign,
    Implementation,
    Verification,
    Research,
    Synthesis,
}
impl ContributionKind {
    pub fn competence(self) -> Competence {
        match self {
            Self::Plan | Self::Decompose | Self::Clarify => Competence::Planning,
            Self::DesignChecks => Competence::CheckDesign,
            Self::Produce | Self::Alternative | Self::Integrate => Competence::Implementation,
            Self::Verify | Self::Review | Self::Judge => Competence::Verification,
            Self::Research | Self::Diagnose => Competence::Research,
            Self::Narrate | Self::Curate => Competence::Synthesis,
        }
    }
    pub fn purpose(self) -> Purpose {
        match self {
            Self::Produce | Self::Alternative | Self::Integrate => Purpose::Production,
            Self::DesignChecks | Self::Verify | Self::Review | Self::Judge => Purpose::Verification,
            Self::Narrate => Purpose::Reporting,
            _ => Purpose::Coordination,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Budget {
    pub id: Id<Budget>,
    pub session: Id,
    pub limit: CostUnits,
    pub verification_reserve: CostUnits,
    pub reporting_reserve: CostUnits,
    pub spent: CostUnits,
    pub held: CostUnits,
    pub unknown_usage: UnknownUsage,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rates {
    pub input: CostUnits,
    pub cache_read: CostUnits,
    pub cache_write: CostUnits,
    pub output: CostUnits,
}
impl Rates {
    pub fn fallback() -> Self {
        Self {
            input: units(1.0),
            cache_read: units(0.1),
            cache_write: units(1.25),
            output: units(4.0),
        }
    }
    pub fn validate(&self) -> Result<()> {
        for value in [self.input, self.cache_read, self.cache_write, self.output] {
            nonnegative(value)?;
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PriceRate {
    pub provider: Id<Provider>,
    pub model: String,
    pub rates: Rates,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PriceBook {
    pub version: String,
    pub rates: Vec<PriceRate>,
    pub fallback: Rates,
}
impl PriceBook {
    pub fn validate(&self) -> Result<()> {
        require_text(&self.version, 256)?;
        self.fallback.validate()?;
        let mut keys = BTreeSet::new();
        for rate in &self.rates {
            require_text(&rate.model, 256)?;
            rate.rates.validate()?;
            if !keys.insert((&rate.provider, &rate.model)) {
                return Err(Denial::new("pricebook", "Duplicate provider/model rate"));
            }
        }
        Ok(())
    }
    pub fn rates(&self, provider: &Id<Provider>, model: &str) -> &Rates {
        self.rates
            .iter()
            .find(|entry| &entry.provider == provider && entry.model == model)
            .map(|entry| &entry.rates)
            .unwrap_or(&self.fallback)
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Usage {
    pub input: u64,
    pub cache_read: u64,
    pub cache_write: u64,
    pub output: u64,
    pub reasoning: Option<u64>,
}
impl Usage {
    /// Normalize a cumulative native report against an established baseline.
    /// A counter reset is a refusal; the execution adapter must retain partial/unknown coverage.
    pub fn since(&self, baseline: &Self) -> Result<Self> {
        self.validate()?;
        baseline.validate()?;
        let difference = |total: u64, before: u64| {
            total.checked_sub(before).ok_or_else(|| {
                Denial::new(
                    "usage_baseline",
                    "A cumulative counter reset or disagrees with its baseline",
                )
            })
        };
        let usage = Self {
            input: difference(self.input, baseline.input)?,
            cache_read: difference(self.cache_read, baseline.cache_read)?,
            cache_write: difference(self.cache_write, baseline.cache_write)?,
            output: difference(self.output, baseline.output)?,
            reasoning: match (self.reasoning, baseline.reasoning) {
                (Some(total), Some(before)) => Some(difference(total, before)?),
                _ => None,
            },
        };
        usage.validate()?;
        Ok(usage)
    }
    pub fn validate(&self) -> Result<()> {
        if self
            .cache_read
            .checked_add(self.cache_write)
            .is_none_or(|cached| cached > self.input)
            || self
                .reasoning
                .is_some_and(|reasoning| reasoning > self.output)
        {
            return Err(Denial::new(
                "usage",
                "Cache counts must be included in input, and reasoning in output",
            ));
        }
        Ok(())
    }
    pub fn includes(&self, earlier: &Self) -> bool {
        self.input >= earlier.input
            && self.cache_read >= earlier.cache_read
            && self.cache_write >= earlier.cache_write
            && self.output >= earlier.output
            && earlier
                .reasoning
                .is_none_or(|prior| self.reasoning.is_some_and(|now| now >= prior))
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Receipt {
    pub id: Id<Receipt>,
    pub invocation: Id,
    pub usage: Usage,
    pub coverage: Coverage,
    pub cost: Option<CostUnits>,
}
impl Receipt {
    pub fn validate(&self) -> Result<()> {
        self.usage.validate()?;
        if let Some(cost) = self.cost {
            nonnegative(cost)?;
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Allowance {
    pub cost: CostUnits,
    pub timeout: u64,
    pub native_turns: u32,
    pub output_chars: u64,
}
impl Allowance {
    pub fn validate(&self) -> Result<()> {
        nonnegative(self.cost)?;
        if self.timeout == 0 || self.native_turns == 0 || self.output_chars == 0 {
            return Err(Denial::new(
                "allowance",
                "Execution limits must be positive",
            ));
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reservation {
    pub id: Id<Reservation>,
    pub budget: Id<Budget>,
    pub assignment: Id,
    pub amount: CostUnits,
    pub purpose: Purpose,
    pub state: ReservationState,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CostEstimate {
    pub expected: CostUnits,
    pub p90: CostUnits,
}
impl CostEstimate {
    pub fn validate(&self) -> Result<()> {
        nonnegative(self.expected)?;
        nonnegative(self.p90)?;
        if self.p90 < self.expected {
            return Err(Denial::new(
                "estimate",
                "The p90 estimate must cover the expected cost",
            ));
        }
        Ok(())
    }
}
/// Resource inputs for one proposed contribution. Admission binds these to the actual Contribution in W1-0006.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResourceDemand {
    pub contribution: Id,
    pub kind: ContributionKind,
    pub difficulty: Difficulty,
    pub provider: Id<Provider>,
    pub profile: ExecutionProfile,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReportingPlan {
    pub narration: Option<Allowance>,
    pub correction: Option<Allowance>,
}
impl ReportingPlan {
    pub fn deterministic() -> Self {
        Self {
            narration: None,
            correction: None,
        }
    }
    pub fn reserve(&self) -> Result<CostUnits> {
        match (&self.narration, &self.correction) {
            (None, None) => Ok(units(0.0)),
            (Some(first), Some(second)) => {
                first.validate()?;
                second.validate()?;
                add(first.cost, second.cost)
            }
            _ => Err(Denial::new(
                "reporting_plan",
                "Plan both bounded narration and correction, or deterministic reporting",
            )),
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReceiptPrice {
    Known(CostUnits),
    Estimated(CostUnits),
    Unknown,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReportingMode {
    Narrated,
    Deterministic,
}

pub fn nonnegative(value: CostUnits) -> Result<()> {
    if value.get() < 0.0 {
        Err(Denial::new(
            "negative_cost",
            "Cost units must be nonnegative",
        ))
    } else {
        Ok(())
    }
}
/// Exact constant construction for the finite literals used by the accounting implementation.
fn units(value: f64) -> CostUnits {
    CostUnits::new(value).expect("finite accounting constant")
}
pub fn add(left: CostUnits, right: CostUnits) -> Result<CostUnits> {
    nonnegative(left)?;
    nonnegative(right)?;
    // TwoSum recovers addition roundoff. Round positive error upward so admission
    // never gains capacity from a rounded-down sum of positive charges.
    let a = left.get();
    let b = right.get();
    let sum = a + b;
    if !sum.is_finite() {
        return Err(Denial::new("cost_overflow", "Cost sum is not finite"));
    }
    let virtual_b = sum - a;
    let error = (a - (sum - virtual_b)) + (b - virtual_b);
    let sum = if error > 0.0 { sum.next_up() } else { sum };
    CostUnits::new(sum).map_err(|_| Denial::new("cost_overflow", "Cost sum is not finite"))
}
pub fn remaining(limit: CostUnits, used: CostUnits) -> Result<CostUnits> {
    nonnegative(limit)?;
    nonnegative(used)?;
    let difference = limit.get() - used.get();
    if difference <= 0.0 {
        return Ok(units(0.0));
    }
    // The subtraction's exact rounding residual prevents an optimistic remainder.
    let virtual_used = limit.get() - difference;
    let error = (limit.get() - (difference + virtual_used)) + (virtual_used - used.get());
    Ok(units(if error < 0.0 {
        difference.next_down().max(0.0)
    } else {
        difference
    }))
}

pub fn multiply(left: CostUnits, right: CostUnits) -> Result<CostUnits> {
    nonnegative(left)?;
    nonnegative(right)?;
    let product = left.get() * right.get();
    if !product.is_finite() {
        return Err(Denial::new("cost_overflow", "Cost product is not finite"));
    }
    let residual = left.get().mul_add(right.get(), -product);
    let tiny_inexact = left.get() > 0.0
        && right.get() > 0.0
        && residual == 0.0
        && product < f64::MIN_POSITIVE * (1_u64 << 53) as f64;
    let rounded = if residual > 0.0 || tiny_inexact {
        product.next_up()
    } else {
        product
    };
    CostUnits::new(rounded).map_err(|_| Denial::new("cost_overflow", "Cost product is not finite"))
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PriceWeightedParameters {
    pub expected_input: u64,
    pub expected_output: u64,
    pub p90_factor: CostUnits,
}
impl PriceWeightedParameters {
    pub fn validate(&self) -> Result<()> {
        if self.expected_input > 1_u64 << 53
            || self.expected_output > 1_u64 << 53
            || self.p90_factor.get() < 1.0
        {
            return Err(Denial::new(
                "price_parameters",
                "Forecast counts must be exactly representable and p90 factor must be at least one",
            ));
        }
        Ok(())
    }
}
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PurposeBoundedParameters {
    pub max_cost: CostUnits,
    pub timeout: u64,
    pub native_turns: u32,
    pub output_chars: u64,
    pub report_call_cost: CostUnits,
}
impl PurposeBoundedParameters {
    pub fn validate(&self) -> Result<()> {
        nonnegative(self.max_cost)?;
        nonnegative(self.report_call_cost)?;
        Allowance {
            cost: self.max_cost,
            timeout: self.timeout,
            native_turns: self.native_turns,
            output_chars: self.output_chars,
        }
        .validate()
    }
}

impl Default for PriceWeightedParameters {
    fn default() -> Self {
        Self {
            expected_input: 1000,
            expected_output: 250,
            p90_factor: units(2.0),
        }
    }
}
impl Default for PurposeBoundedParameters {
    fn default() -> Self {
        Self {
            max_cost: units(10_000.0),
            timeout: 60_000,
            native_turns: 4,
            output_chars: 16_000,
            report_call_cost: units(2000.0),
        }
    }
}

/// A nonnegative difference rounded upward for retained uncertainty exposure.
pub fn exposure(limit: CostUnits, accounted: CostUnits) -> Result<CostUnits> {
    nonnegative(limit)?;
    nonnegative(accounted)?;
    if limit <= accounted {
        return Ok(units(0.0));
    }
    let difference = limit.get() - accounted.get();
    let virtual_used = limit.get() - difference;
    let error = (limit.get() - (difference + virtual_used)) + (virtual_used - accounted.get());
    CostUnits::new(if error > 0.0 {
        difference.next_up()
    } else {
        difference
    })
}
