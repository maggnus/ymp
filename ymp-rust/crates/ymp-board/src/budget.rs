//! The finite dimensions a participant spends on talking, and the vectors board accounts hold.
//!
//! This is a separate namespace from the control plane's budget vector, and separate on purpose.
//! Capacity to publish is not capacity to start a participant, and neither converts into the other:
//! there is no rate, no exchange and no aggregate scalar in this file. A caller holding a
//! communication allowance holds exactly the right to talk, and a plane that mixed the two vectors
//! would let unspent talk pay for execution.
//!
//! Addition and subtraction are componentwise, and every dimension is enforced on its own. A
//! refusal names the dimension that refused, because no other dimension could have covered it.

use std::fmt;

use serde::{Deserialize, Serialize};

/// One independently enforced dimension of a communication allowance.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Allowance {
    /// Permission to append one further message. A refresh is a publication like any other and
    /// spends one of these too, so repeating a finding is never free.
    Publications,
    /// Permission to keep one earlier finding salient once more. It is counted apart from the
    /// publication so that holding attention costs something beyond saying a thing once.
    SalienceRefreshes,
    /// Permission to open one further audience grant. Spending it is irreversible, so a scope
    /// cannot be reopened to the same participant forever by releasing and regranting.
    MembershipGrants,
    /// One concurrently live membership. It is held by the grant while the grant is live and
    /// returns to the account that funded it when the grant is released or expires.
    ActiveMemberships,
    /// Payload bytes appended by the author.
    PublishedBytes,
    /// Payload bytes made available to a reader. It is charged to the reader, because a read is
    /// what makes bytes cross, and a receipt states only that they were made available.
    DeliveredBytes,
}

pub const ALLOWANCES: [Allowance; 6] = [
    Allowance::Publications,
    Allowance::SalienceRefreshes,
    Allowance::MembershipGrants,
    Allowance::ActiveMemberships,
    Allowance::PublishedBytes,
    Allowance::DeliveredBytes,
];

pub const ALLOWANCE_COUNT: usize = ALLOWANCES.len();

/// What spending a dimension means for settlement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AllowanceKind {
    /// Capacity that moves between accounts and, while it is still unspent, settles back to the
    /// account that funded it.
    Capacity,
    /// One permission to bring an object into existence. It is transferable while unspent, and
    /// spending it never returns.
    CreationAuthority,
}

impl Allowance {
    pub const fn index(self) -> usize {
        match self {
            Self::Publications => 0,
            Self::SalienceRefreshes => 1,
            Self::MembershipGrants => 2,
            Self::ActiveMemberships => 3,
            Self::PublishedBytes => 4,
            Self::DeliveredBytes => 5,
        }
    }

    pub const fn kind(self) -> AllowanceKind {
        match self {
            Self::Publications
            | Self::SalienceRefreshes
            | Self::MembershipGrants
            | Self::PublishedBytes
            | Self::DeliveredBytes => AllowanceKind::CreationAuthority,
            Self::ActiveMemberships => AllowanceKind::Capacity,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Publications => "publications",
            Self::SalienceRefreshes => "salience_refreshes",
            Self::MembershipGrants => "membership_grants",
            Self::ActiveMemberships => "active_memberships",
            Self::PublishedBytes => "published_bytes",
            Self::DeliveredBytes => "delivered_bytes",
        }
    }
}

impl fmt::Display for Allowance {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// A quantity of every communication dimension at once, in integer smallest units.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub struct CommunicationAllowance {
    amounts: [u64; ALLOWANCE_COUNT],
}

impl CommunicationAllowance {
    pub const ZERO: Self = Self {
        amounts: [0; ALLOWANCE_COUNT],
    };

    pub const fn from_amounts(amounts: [u64; ALLOWANCE_COUNT]) -> Self {
        Self { amounts }
    }

    pub fn units(allowance: Allowance, units: u64) -> Self {
        Self::ZERO.with(allowance, units)
    }

    pub fn unit(allowance: Allowance) -> Self {
        Self::units(allowance, 1)
    }

    #[must_use]
    pub const fn with(mut self, allowance: Allowance, units: u64) -> Self {
        self.amounts[allowance.index()] = units;
        self
    }

    pub const fn get(&self, allowance: Allowance) -> u64 {
        self.amounts[allowance.index()]
    }

    pub const fn amounts(&self) -> &[u64; ALLOWANCE_COUNT] {
        &self.amounts
    }

    /// Componentwise addition. The error names the dimension that would overflow.
    pub fn checked_add(&self, other: &Self) -> Result<Self, Allowance> {
        let mut sum = Self::ZERO;
        for allowance in ALLOWANCES {
            let index = allowance.index();
            sum.amounts[index] = self.amounts[index]
                .checked_add(other.amounts[index])
                .ok_or(allowance)?;
        }
        Ok(sum)
    }

    /// Componentwise subtraction. The error names the first dimension that would go negative,
    /// which is exactly the dimension a refusal must report.
    pub fn checked_sub(&self, other: &Self) -> Result<Self, Allowance> {
        let mut difference = Self::ZERO;
        for allowance in ALLOWANCES {
            let index = allowance.index();
            difference.amounts[index] = self.amounts[index]
                .checked_sub(other.amounts[index])
                .ok_or(allowance)?;
        }
        Ok(difference)
    }

    /// Whether this vector holds at least `required` in every dimension.
    pub fn covers(&self, required: &Self) -> bool {
        self.shortfall(required).is_none()
    }

    /// The first dimension in which this vector holds less than `required`.
    pub fn shortfall(&self, required: &Self) -> Option<Allowance> {
        ALLOWANCES
            .into_iter()
            .find(|allowance| self.get(*allowance) < required.get(*allowance))
    }

    pub fn is_zero(&self) -> bool {
        self.amounts.iter().all(|units| *units == 0)
    }
}
