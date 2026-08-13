//! Conserved resource dimensions and the vectors accounts hold.
//!
//! Every dimension is counted in integer smallest units and is enforced on its own. Addition and
//! subtraction are componentwise, so spare capacity in one dimension can never pay for another:
//! there is no rate, no conversion and no aggregate scalar anywhere in this file.

use std::fmt;

use serde::{Deserialize, Serialize};

/// One independently enforced resource dimension of the budget vector.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Dimension {
    /// Model-route cost in millionths of a unit of account.
    MoneyMicros,
    /// Model tokens as the route reports them.
    ModelTokens,
    /// Wall time in milliseconds, which is what a lease is bought with.
    WallTimeMs,
    /// Protected verification queries.
    VerificationQueries,
    /// Externally consequential actions.
    ExternalActions,
    /// Permission to start one further participant.
    ParticipantStarts,
    /// Permission to start one further attempt.
    AttemptStarts,
    /// Permission to run one further supervised process slice, whether it is the first slice of an
    /// attempt or the resumption of one that yielded. It is counted apart from the attempt so that
    /// waking a participant costs something: an attempt that yields and resumes buys each slice.
    InvocationStarts,
    /// Permission to create one further offer.
    OfferCreations,
    /// Permission to create one further work obligation.
    ObligationCreations,
}

pub const DIMENSIONS: [Dimension; 10] = [
    Dimension::MoneyMicros,
    Dimension::ModelTokens,
    Dimension::WallTimeMs,
    Dimension::VerificationQueries,
    Dimension::ExternalActions,
    Dimension::ParticipantStarts,
    Dimension::AttemptStarts,
    Dimension::InvocationStarts,
    Dimension::OfferCreations,
    Dimension::ObligationCreations,
];

pub const DIMENSION_COUNT: usize = DIMENSIONS.len();

/// What spending a dimension means for settlement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DimensionKind {
    /// Capacity moves between accounts and, while it is still unspent, settles back to the
    /// account that funded it.
    Capacity,
    /// One permission to bring an object into existence. It is transferable while unspent, but
    /// spending it is irreversible, so a branch cannot create an endless chain by returning the
    /// same nominal budget.
    CreationAuthority,
}

impl Dimension {
    pub const fn index(self) -> usize {
        match self {
            Self::MoneyMicros => 0,
            Self::ModelTokens => 1,
            Self::WallTimeMs => 2,
            Self::VerificationQueries => 3,
            Self::ExternalActions => 4,
            Self::ParticipantStarts => 5,
            Self::AttemptStarts => 6,
            Self::InvocationStarts => 7,
            Self::OfferCreations => 8,
            Self::ObligationCreations => 9,
        }
    }

    pub const fn kind(self) -> DimensionKind {
        match self {
            Self::MoneyMicros
            | Self::ModelTokens
            | Self::WallTimeMs
            | Self::VerificationQueries
            | Self::ExternalActions => DimensionKind::Capacity,
            Self::ParticipantStarts
            | Self::AttemptStarts
            | Self::InvocationStarts
            | Self::OfferCreations
            | Self::ObligationCreations => DimensionKind::CreationAuthority,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MoneyMicros => "money_micros",
            Self::ModelTokens => "model_tokens",
            Self::WallTimeMs => "wall_time_ms",
            Self::VerificationQueries => "verification_queries",
            Self::ExternalActions => "external_actions",
            Self::ParticipantStarts => "participant_starts",
            Self::AttemptStarts => "attempt_starts",
            Self::InvocationStarts => "invocation_starts",
            Self::OfferCreations => "offer_creations",
            Self::ObligationCreations => "obligation_creations",
        }
    }
}

impl fmt::Display for Dimension {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// A quantity of every dimension at once, in integer smallest units.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub struct BudgetVector {
    amounts: [u64; DIMENSION_COUNT],
}

impl BudgetVector {
    pub const ZERO: Self = Self {
        amounts: [0; DIMENSION_COUNT],
    };

    pub const fn from_amounts(amounts: [u64; DIMENSION_COUNT]) -> Self {
        Self { amounts }
    }

    pub fn units(dimension: Dimension, units: u64) -> Self {
        Self::ZERO.with(dimension, units)
    }

    pub fn unit(dimension: Dimension) -> Self {
        Self::units(dimension, 1)
    }

    #[must_use]
    pub const fn with(mut self, dimension: Dimension, units: u64) -> Self {
        self.amounts[dimension.index()] = units;
        self
    }

    pub const fn get(&self, dimension: Dimension) -> u64 {
        self.amounts[dimension.index()]
    }

    pub const fn amounts(&self) -> &[u64; DIMENSION_COUNT] {
        &self.amounts
    }

    /// Componentwise addition. `None` names the dimension that would overflow.
    pub fn checked_add(&self, other: &Self) -> Result<Self, Dimension> {
        let mut sum = Self::ZERO;
        for dimension in DIMENSIONS {
            let index = dimension.index();
            sum.amounts[index] = self.amounts[index]
                .checked_add(other.amounts[index])
                .ok_or(dimension)?;
        }
        Ok(sum)
    }

    /// Componentwise subtraction. `None` names the first dimension that would go negative, which
    /// is exactly the dimension a refusal must report: no other dimension pays for it.
    pub fn checked_sub(&self, other: &Self) -> Result<Self, Dimension> {
        let mut difference = Self::ZERO;
        for dimension in DIMENSIONS {
            let index = dimension.index();
            difference.amounts[index] = self.amounts[index]
                .checked_sub(other.amounts[index])
                .ok_or(dimension)?;
        }
        Ok(difference)
    }

    pub fn checked_scale(&self, factor: u64) -> Result<Self, Dimension> {
        let mut scaled = Self::ZERO;
        for dimension in DIMENSIONS {
            let index = dimension.index();
            scaled.amounts[index] = self.amounts[index].checked_mul(factor).ok_or(dimension)?;
        }
        Ok(scaled)
    }

    /// Whether this vector holds at least `required` in every dimension.
    pub fn covers(&self, required: &Self) -> bool {
        self.shortfall(required).is_none()
    }

    /// The first dimension in which this vector holds less than `required`.
    pub fn shortfall(&self, required: &Self) -> Option<Dimension> {
        DIMENSIONS
            .into_iter()
            .find(|dimension| self.get(*dimension) < required.get(*dimension))
    }

    pub fn is_zero(&self) -> bool {
        self.amounts.iter().all(|units| *units == 0)
    }
}
