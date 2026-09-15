//! Session accounting and the reference in-memory treasury.
//!
//! Accounting counts every invocation — completed, failed, cancelled,
//! timed out and still-uncertain — plus the coordination work around them,
//! and aggregates observed usage. Usage a provider does not report is
//! recorded as unknown (a count of unreported components), never as zero.
//!
//! Both the live [`LedgerTreasury`] and the replay projection in
//! [`view`](super::view) fold the same committed session events, so the
//! live totals and the replayed totals agree by construction.

use std::collections::BTreeMap;
use std::time::Duration;

use super::ports::{ReserveRefused, SettleRefused, Treasury};
use super::types::{Grant, InvocationId, ObservedUsage, ResourceAmount, Termination};
use crate::SessionEvent;

/// Aggregated observed usage across invocations.
///
/// Each component keeps the sum over invocations that reported it and the
/// count of invocations that left it unreported; the count is what keeps
/// unknown usage distinct from zero.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct UsageAggregate {
    turns_known: u64,
    turns_unknown_reports: u64,
    output_chars_known: u64,
    output_chars_unknown_reports: u64,
    wall_clock_known: Duration,
    wall_clock_unknown_reports: u64,
}

impl UsageAggregate {
    pub const fn turns_known(&self) -> u64 {
        self.turns_known
    }

    pub const fn turns_unknown_reports(&self) -> u64 {
        self.turns_unknown_reports
    }

    pub const fn output_chars_known(&self) -> u64 {
        self.output_chars_known
    }

    pub const fn output_chars_unknown_reports(&self) -> u64 {
        self.output_chars_unknown_reports
    }

    pub const fn wall_clock_known(&self) -> Duration {
        self.wall_clock_known
    }

    pub const fn wall_clock_unknown_reports(&self) -> u64 {
        self.wall_clock_unknown_reports
    }

    /// Folds one invocation's settled usage into the aggregate.
    pub fn fold_usage(&mut self, usage: &ObservedUsage) {
        match usage.turns() {
            Some(turns) => self.turns_known = self.turns_known.saturating_add(turns),
            None => self.turns_unknown_reports += 1,
        }
        match usage.output_chars() {
            Some(chars) => self.output_chars_known = self.output_chars_known.saturating_add(chars),
            None => self.output_chars_unknown_reports += 1,
        }
        match usage.wall_clock() {
            Some(duration) => {
                self.wall_clock_known = self.wall_clock_known.saturating_add(duration)
            }
            None => self.wall_clock_unknown_reports += 1,
        }
    }
}

/// Session totals derived from committed execution events.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SessionAccounting {
    completed: u64,
    failed: u64,
    cancelled: u64,
    timed_out: u64,
    /// Invocations whose cancellation was requested and that have no
    /// termination observation: they stand uncertain.
    uncertain: u64,
    /// Admission work around invocations: committed admissions and recorded
    /// refusals.
    coordination: u64,
    usage: UsageAggregate,
}

impl SessionAccounting {
    pub const fn completed(&self) -> u64 {
        self.completed
    }

    pub const fn failed(&self) -> u64 {
        self.failed
    }

    pub const fn cancelled(&self) -> u64 {
        self.cancelled
    }

    pub const fn timed_out(&self) -> u64 {
        self.timed_out
    }

    pub const fn uncertain(&self) -> u64 {
        self.uncertain
    }

    pub const fn coordination(&self) -> u64 {
        self.coordination
    }

    pub const fn usage(&self) -> &UsageAggregate {
        &self.usage
    }

    fn record_termination(&mut self, termination: &Termination) {
        match termination {
            Termination::Completed => self.completed += 1,
            Termination::Failed { .. } => self.failed += 1,
            Termination::Cancelled => self.cancelled += 1,
            Termination::TimedOut => self.timed_out += 1,
        }
    }
}

/// The accounting fold over committed session events, shared by the live
/// ledger and the replay projection.
pub(crate) struct AccountingFold {
    accounting: SessionAccounting,
}

impl AccountingFold {
    pub(crate) fn new() -> Self {
        Self {
            accounting: SessionAccounting::default(),
        }
    }

    pub(crate) fn apply(&mut self, event: &SessionEvent) {
        match event {
            SessionEvent::AssignmentAdmitted { .. } => self.accounting.coordination += 1,
            SessionEvent::InvocationObserved { termination, .. } => {
                self.accounting.record_termination(termination);
            }
            SessionEvent::InvocationFailedAtStart { .. } => {
                self.accounting.failed += 1;
            }
            SessionEvent::InvocationUncertain { .. } => self.accounting.uncertain += 1,
            SessionEvent::InvocationAccounted { usage, .. } => {
                self.accounting.usage.fold_usage(usage);
            }
            SessionEvent::InvocationStarted { .. }
            | SessionEvent::InvocationCancellationRequested { .. }
            | SessionEvent::EffectEvidenceRecorded { .. }
            | SessionEvent::SessionOpened { .. }
            | SessionEvent::SessionCancelled { .. } => {}
        }
    }

    pub(crate) fn accounting(&self) -> &SessionAccounting {
        &self.accounting
    }
}

/// The reference in-memory treasury: a bounded capacity plus the set of
/// reservations currently held, rebuilt from replayed history when a
/// session is reattached.
#[derive(Clone, Debug)]
pub struct LedgerTreasury {
    capacity: ResourceAmount,
    holds: BTreeMap<InvocationId, ResourceAmount>,
    settled_usage: BTreeMap<InvocationId, ObservedUsage>,
}

impl LedgerTreasury {
    pub fn new(capacity: ResourceAmount) -> Self {
        Self {
            capacity,
            holds: BTreeMap::new(),
            settled_usage: BTreeMap::new(),
        }
    }

    /// Rebuilds the ledger's holds from replayed history; existing state is
    /// cleared. Settled usage of already-accounted invocations is retained
    /// for [`Treasury::settle`] idempotence checks.
    pub fn rebuild(&mut self, holds: impl IntoIterator<Item = (InvocationId, ResourceAmount)>) {
        self.holds.clear();
        for (invocation, amount) in holds {
            self.holds.insert(invocation, amount);
        }
    }

    /// The reservations currently held, by invocation.
    pub fn holds(&self) -> &BTreeMap<InvocationId, ResourceAmount> {
        &self.holds
    }

    fn held_total(&self) -> u64 {
        self.holds.values().map(|amount| amount.value()).sum()
    }

    fn available(&self) -> ResourceAmount {
        ResourceAmount::new(self.capacity.value().saturating_sub(self.held_total()))
    }
}

impl Treasury for LedgerTreasury {
    fn capacity(&self) -> ResourceAmount {
        self.capacity
    }

    fn held(&self) -> ResourceAmount {
        ResourceAmount::new(self.held_total())
    }

    fn can_hold(&self, amount: ResourceAmount) -> bool {
        self.available().value() >= amount.value()
    }

    fn reserve(&mut self, grant: &Grant) -> Result<(), ReserveRefused> {
        let requested = grant.reservation();
        if !self.can_hold(requested) {
            return Err(ReserveRefused::new(requested, self.available()));
        }
        self.holds.insert(grant.invocation().clone(), requested);
        Ok(())
    }

    fn settle(
        &mut self,
        invocation: &InvocationId,
        usage: &ObservedUsage,
    ) -> Result<ResourceAmount, SettleRefused> {
        match self.holds.remove(invocation) {
            Some(amount) => {
                self.settled_usage.insert(invocation.clone(), *usage);
                Ok(amount)
            }
            None if self.settled_usage.contains_key(invocation) => Err(SettleRefused::new(
                invocation.clone(),
                "invocation is already settled".to_owned(),
            )),
            None => Err(SettleRefused::new(
                invocation.clone(),
                "invocation holds no reservation".to_owned(),
            )),
        }
    }
}
