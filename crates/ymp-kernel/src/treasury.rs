//! Journal-backed financial authority. Policies propose; this consumer checks the
//! resource envelope, records observations and alone commits financial changes.
use crate::{
    events::Event,
    journal::{Journal, validate_append},
    ports::resources::*,
    view::SessionView,
};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, sync::Arc};
use ymp_domain::{
    Denial, Digest, Id, Ref, Result,
    journal::{Actor, Decision, Envelope, PolicySelection, encode},
    require_text,
    resources::*,
    task::SessionStatus,
};

fn zero() -> CostUnits {
    CostUnits::new(0.0).expect("finite zero")
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BudgetOpening {
    pub budget: Budget,
    pub pricebook: PriceBook,
    pub reporting: Decision<ReportingPlan>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReserveDecision {
    pub reservation: Reservation,
    pub demand: ResourceDemand,
    pub estimate: Decision<CostEstimate>,
    pub allocation: Decision<Allowance>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AccountingFact {
    NeverStarted,
    CompleteCost(CostUnits),
    EnforcedUpperBound(CostUnits),
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AccountingEvidenceRecord {
    pub reservation: Id<Reservation>,
    pub assignment: Id,
    pub invocation: Option<Id>,
    pub state: Ref,
    pub receipt: Option<Digest>,
    pub fact: AccountingFact,
    pub basis: Vec<Ref>,
}
/// Only the kernel can issue this capability. Stored evidence is data, not a way
/// to recreate authority. Executor-derived factories are owned by W1-0017.
/// ```compile_fail
/// use ymp_kernel::treasury::AccountingEvidence;
/// let evidence: AccountingEvidence = serde_json::from_str("{}").unwrap();
/// ```
pub struct AccountingEvidence {
    session: Id,
    issuer: Arc<()>,
    record: AccountingEvidenceRecord,
}
/// Owner control of financial reporting mode; not passed to strategies or adapters.
pub struct BudgetControl {
    session: Id,
    issuer: Arc<()>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReservationChange {
    Reserved(Box<ReserveDecision>),
    Authorized {
        reservation: Id<Reservation>,
        invocation: Id,
    },
    Observed {
        reservation: Id<Reservation>,
        receipt: Receipt,
        evidence: Vec<AccountingEvidenceRecord>,
    },
    Revoked {
        reservation: Id<Reservation>,
        reason: String,
    },
    Released {
        reservation: Id<Reservation>,
        evidence: AccountingEvidenceRecord,
    },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Settlement {
    pub reservation: Id<Reservation>,
    pub receipt: Receipt,
    pub decision: Decision<ReceiptPrice>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReservationAccount {
    pub reservation: Reservation,
    pub demand: ResourceDemand,
    pub allowance: Allowance,
    pub invocation: Option<Id>,
    pub receipt: Option<Receipt>,
    pub complete_cost: Option<CostUnits>,
    pub observed_cost_floor: CostUnits,
    pub upper_bound: Option<CostUnits>,
    pub revoked: bool,
    pub settlement: Option<Settlement>,
    pub last: Ref,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PurposeTotals {
    pub spent: CostUnits,
    pub held: CostUnits,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TreasuryView {
    pub budget: Budget,
    pub pricebook: PriceBook,
    pub reporting_plan: ReportingPlan,
    pub reporting_mode: Option<ReportingMode>,
    pub accounts: BTreeMap<Id<Reservation>, ReservationAccount>,
    pub totals: BTreeMap<Purpose, PurposeTotals>,
    pub unbounded: bool,
    pub unsettled_usage: bool,
}
impl TreasuryView {
    fn recompute(&mut self) -> Result<()> {
        let mut totals = BTreeMap::new();
        for purpose in [
            Purpose::Production,
            Purpose::Verification,
            Purpose::Coordination,
            Purpose::Reporting,
        ] {
            totals.insert(
                purpose,
                PurposeTotals {
                    spent: zero(),
                    held: zero(),
                },
            );
        }
        self.unbounded = false;
        self.unsettled_usage = false;
        for account in self.accounts.values() {
            let total = totals
                .get_mut(&account.reservation.purpose)
                .expect("all purposes");
            let uncertain = account.invocation.is_some()
                && (account.receipt.is_some() || account.revoked)
                && !account
                    .settlement
                    .as_ref()
                    .is_some_and(|s| matches!(s.decision.outcome, ReceiptPrice::Known(_)));
            if uncertain {
                self.unsettled_usage = true;
                if account.upper_bound.is_none() && account.complete_cost.is_none() {
                    self.unbounded = true;
                }
            }
            match account.reservation.state {
                ReservationState::Held => {
                    // A confirmed full cost is exposure even before pricing is committed.
                    // It also dominates a contradictory, smaller executor bound.
                    let amount = [
                        Some(account.reservation.amount),
                        account.upper_bound,
                        account.complete_cost,
                        Some(account.observed_cost_floor),
                    ]
                    .into_iter()
                    .flatten()
                    .fold(account.reservation.amount, |highest, value| {
                        if value > highest { value } else { highest }
                    });
                    total.held = add(total.held, amount)?;
                }
                ReservationState::Settled => {
                    let settlement = account.settlement.as_ref().ok_or_else(|| {
                        Denial::new("accounting", "Settled reservation has no settlement")
                    })?;
                    let (cost, estimated) = match settlement.decision.outcome {
                        ReceiptPrice::Known(cost) => (cost, false),
                        ReceiptPrice::Estimated(cost) => (cost, true),
                        ReceiptPrice::Unknown => {
                            return Err(Denial::new(
                                "accounting",
                                "Unknown cost cannot be settled",
                            ));
                        }
                    };
                    total.spent = add(total.spent, cost)?;
                    if estimated {
                        // Earlier confirmed expense cannot disappear with a later partial receipt.
                        let bound = account.upper_bound.unwrap_or(zero());
                        let retained = if bound > account.observed_cost_floor {
                            bound
                        } else {
                            account.observed_cost_floor
                        };
                        total.held = add(total.held, exposure(retained, cost)?)?;
                    }
                }
                ReservationState::Released => {}
            }
        }
        self.budget.spent = zero();
        self.budget.held = zero();
        for total in totals.values() {
            self.budget.spent = add(self.budget.spent, total.spent)?;
            self.budget.held = add(self.budget.held, total.held)?;
        }
        self.totals = totals;
        Ok(())
    }
    pub fn remaining(&self, purpose: Purpose) -> Result<CostUnits> {
        if self.unbounded {
            return Err(Denial::new(
                "funds_unknown",
                "Prior consumption has no verified upper bound",
            ));
        }
        if self.unsettled_usage
            && self.budget.unknown_usage == UnknownUsage::Stop
            && matches!(purpose, Purpose::Production | Purpose::Coordination)
        {
            return Ok(zero());
        }
        let used = add(self.budget.spent, self.budget.held)?;
        let cap = match purpose {
            Purpose::Production | Purpose::Coordination => remaining(
                self.budget.limit,
                add(
                    self.budget.verification_reserve,
                    self.budget.reporting_reserve,
                )?,
            )?,
            Purpose::Verification => remaining(self.budget.limit, self.budget.reporting_reserve)?,
            Purpose::Reporting => self.budget.limit,
        };
        let available = remaining(cap, used)?;
        if purpose == Purpose::Reporting {
            let own = &self.totals[&Purpose::Reporting];
            let protected = remaining(self.budget.reporting_reserve, add(own.spent, own.held)?)?;
            Ok(if available < protected {
                available
            } else {
                protected
            })
        } else if purpose == Purpose::Verification
            && self.unsettled_usage
            && self.budget.unknown_usage == UnknownUsage::Stop
        {
            let own = &self.totals[&Purpose::Verification];
            let protected = remaining(self.budget.verification_reserve, add(own.spent, own.held)?)?;
            Ok(if available < protected {
                available
            } else {
                protected
            })
        } else {
            Ok(available)
        }
    }
    /// Existing holds are already included; authorization passes zero additional cost.
    fn check_capacity(&self, purpose: Purpose, additional: CostUnits) -> Result<()> {
        if self.unbounded {
            return Err(Denial::new(
                "funds_unknown",
                "Prior consumption has no verified upper bound",
            ));
        }
        if self.unsettled_usage
            && self.budget.unknown_usage == UnknownUsage::Stop
            && matches!(purpose, Purpose::Production | Purpose::Coordination)
        {
            return Err(Denial::new(
                "usage_unknown",
                "Unknown consumption blocks unprotected work",
            ));
        }
        let cap = match purpose {
            Purpose::Production | Purpose::Coordination => remaining(
                self.budget.limit,
                add(
                    self.budget.verification_reserve,
                    self.budget.reporting_reserve,
                )?,
            )?,
            Purpose::Verification => remaining(self.budget.limit, self.budget.reporting_reserve)?,
            Purpose::Reporting => self.budget.limit,
        };
        let used = add(self.budget.spent, self.budget.held)?;
        let own_cap = match purpose {
            Purpose::Reporting => Some(self.budget.reporting_reserve),
            Purpose::Verification
                if self.unsettled_usage && self.budget.unknown_usage == UnknownUsage::Stop =>
            {
                Some(self.budget.verification_reserve)
            }
            _ => None,
        };
        let own = &self.totals[&purpose];
        let own_used = add(add(own.spent, own.held)?, additional)?;
        if add(used, additional)? > cap || own_cap.is_some_and(|cap| own_used > cap) {
            return Err(Denial::new(
                "budget",
                "The protected or overall budget is exhausted",
            ));
        }
        Ok(())
    }
    fn may_reserve(
        &self,
        view: &SessionView,
        demand: &ResourceDemand,
        amount: CostUnits,
    ) -> Result<()> {
        let purpose = demand.kind.purpose();
        if view.status() == Some(SessionStatus::Cancelled) {
            return Err(Denial::new(
                "user_stop",
                "A stopped session cannot admit paid work",
            ));
        }
        if self.reporting_mode.is_some() {
            let learning = demand.kind == ContributionKind::Curate
                && view.status() == Some(SessionStatus::Delivered);
            if !learning
                && (purpose != Purpose::Reporting
                    || self.reporting_mode != Some(ReportingMode::Narrated))
            {
                return Err(Denial::new(
                    "reporting_only",
                    "Reporting mode does not authorize more work",
                ));
            }
        } else if purpose == Purpose::Reporting {
            return Err(Denial::new(
                "reporting_phase",
                "Reporting cannot begin while work is admitted",
            ));
        }
        if demand.kind == ContributionKind::Curate
            && view.status() != Some(SessionStatus::Delivered)
        {
            return Err(Denial::new(
                "learning_order",
                "Learning follows report delivery",
            ));
        }
        self.check_capacity(purpose, amount)?;
        if purpose == Purpose::Reporting {
            if self.reporting_plan.narration.is_none() {
                return Err(Denial::new(
                    "reporting_plan",
                    "Only deterministic reporting was funded",
                ));
            }
            if self
                .accounts
                .values()
                .filter(|a| {
                    a.reservation.purpose == Purpose::Reporting
                        && a.reservation.state != ReservationState::Released
                })
                .count()
                >= 2
            {
                return Err(Denial::new(
                    "reporting_limit",
                    "Only narration and one correction are permitted",
                ));
            }
        }
        Ok(())
    }
}

pub struct BudgetRequest {
    pub id: Id<Budget>,
    pub pricebook: PriceBook,
    pub unknown_usage: UnknownUsage,
    pub reporting: ResourceResponse<ReportingPlan>,
}
pub struct ReserveRequest {
    pub id: Id<Reservation>,
    pub assignment: Id,
    pub demand: ResourceDemand,
    pub estimate: ResourceResponse<CostEstimate>,
    pub allowance: ResourceResponse<Allowance>,
}

fn selected<'a>(view: &'a SessionView, port: &str) -> Result<&'a PolicySelection> {
    view.policies()
        .get(port)
        .ok_or_else(|| Denial::new("policy_selection", format!("No {port} policy is selected")))
}
fn decision<T: Clone + PartialEq + Serialize>(
    view: &SessionView,
    port: &str,
    input: &impl Serialize,
    response: &ResourceResponse<T>,
) -> Result<Decision<T>> {
    let effective = selected(view, port)?;
    if response.input != Digest::of_value(input)? || response.proposal.policy != effective.policy {
        return Err(Denial::new(
            "resource_proposal",
            "Resource proposal belongs to another input or policy",
        ));
    }
    response.proposal.validate()?;
    for reference in &response.proposal.basis {
        view.resolve(reference)?;
    }
    Ok(Decision {
        proposal: response.proposal.clone(),
        effective: effective.clone(),
        input: response.input.clone(),
        outcome: response.proposal.value.clone(),
        selection_change: None,
    })
}
fn verify_decision<T: Clone + PartialEq + Serialize>(
    view: &SessionView,
    port: &str,
    input: &impl Serialize,
    record: &Decision<T>,
) -> Result<()> {
    if decision(
        view,
        port,
        input,
        &ResourceResponse {
            input: record.input.clone(),
            proposal: record.proposal.clone(),
        },
    )? != *record
    {
        return Err(Denial::new(
            "resource_decision",
            "Financial decision differs from the selected proposal and policy",
        ));
    }
    Ok(())
}
fn ledger(view: &SessionView) -> Result<&TreasuryView> {
    view.treasury()
        .ok_or_else(|| Denial::new("budget_missing", "Open the budget first"))
}
fn account<'a>(book: &'a TreasuryView, id: &Id<Reservation>) -> Result<&'a ReservationAccount> {
    book.accounts
        .get(id)
        .ok_or_else(|| Denial::new("reservation_missing", "Reservation does not exist"))
}
fn validate_demand(view: &SessionView, demand: &ResourceDemand) -> Result<()> {
    let record = view
        .registry()
        .ok_or_else(|| Denial::new("registry_missing", "No pool observation is recorded"))?;
    let task = view
        .task()
        .ok_or_else(|| Denial::new("task_missing", "No task is open"))?;
    if task.constraints != record.input.constraints {
        return Err(Denial::new(
            "registry_stale",
            "Refresh the pool after changing constraints",
        ));
    }
    if !record
        .decisions
        .iter()
        .any(|d| d.profile == demand.profile && d.outcome == ymp_domain::identity::Readiness::Ready)
        || !record
            .input
            .facts
            .agents
            .iter()
            .any(|a| a.id == demand.profile.agent && a.provider == demand.provider)
    {
        return Err(Denial::new(
            "profile_excluded",
            "Contribution profile/provider is not eligible",
        ));
    }
    Ok(())
}
pub fn reporting_view(view: &SessionView, at: u64) -> Result<ReportingView> {
    let task = view
        .task()
        .ok_or_else(|| Denial::new("task_missing", "No task is open"))?;
    let pool = view
        .registry()
        .ok_or_else(|| Denial::new("registry_missing", "No pool observation is recorded"))?;
    if task.constraints != pool.input.constraints {
        return Err(Denial::new(
            "registry_stale",
            "Refresh the pool before opening a budget",
        ));
    }
    Ok(ReportingView {
        journal: view.digest()?,
        task: task.clone(),
        pool: pool.outcome.clone(),
        at,
    })
}
pub fn estimate_view(view: &SessionView, demand: &ResourceDemand) -> Result<EstimateView> {
    validate_demand(view, demand)?;
    let book = ledger(view)?;
    let mut history = vec![];
    for entry in book.accounts.values() {
        if let Some(settlement) = &entry.settlement
            && let ReceiptPrice::Known(cost) = settlement.decision.outcome
        {
            history.push(CostHistory {
                demand: entry.demand.clone(),
                pricebook: Digest::of_value(&book.pricebook)?,
                policy: settlement.decision.effective.policy.clone(),
                cost,
                estimated: false,
                basis: entry.last.clone(),
            });
        }
    }
    Ok(EstimateView {
        journal: view.digest()?,
        demand: demand.clone(),
        pricebook: book.pricebook.clone(),
        history,
    })
}
pub fn allowance_view(
    view: &SessionView,
    demand: &ResourceDemand,
    estimate: CostEstimate,
    at: u64,
) -> Result<AllowanceView> {
    validate_demand(view, demand)?;
    estimate.validate()?;
    Ok(AllowanceView {
        journal: view.digest()?,
        demand: demand.clone(),
        estimate,
        remaining: ledger(view)?.remaining(demand.kind.purpose())?,
        deadline: view.task().and_then(|t| t.constraints.deadline),
        at,
    })
}
pub fn cost_view(view: &SessionView, id: &Id<Reservation>) -> Result<CostView> {
    let book = ledger(view)?;
    let entry = account(book, id)?;
    Ok(CostView {
        journal: view.digest()?,
        demand: entry.demand.clone(),
        receipt: entry.receipt.clone().ok_or_else(|| {
            Denial::new(
                "receipt_missing",
                "Record the receipt observation before pricing",
            )
        })?,
        pricebook: book.pricebook.clone(),
        allowance: entry.allowance.clone(),
        unknown_usage: book.budget.unknown_usage,
        known_complete_cost: entry.complete_cost,
    })
}
fn validate_evidence(
    view: &SessionView,
    entry: &ReservationAccount,
    evidence: &AccountingEvidenceRecord,
) -> Result<()> {
    if evidence.reservation != entry.reservation.id
        || evidence.assignment != entry.reservation.assignment
        || evidence.invocation != entry.invocation
        || evidence.state != entry.last
        || evidence.basis.is_empty()
    {
        return Err(Denial::new(
            "accounting_evidence",
            "Evidence does not bind the current reservation, assignment and invocation",
        ));
    }
    for reference in &evidence.basis {
        view.resolve(reference)?;
    }
    match evidence.fact {
        AccountingFact::NeverStarted => {
            if entry.invocation.is_some() || evidence.receipt.is_some() {
                return Err(Denial::new(
                    "execution_uncertain",
                    "An authorized invocation may have started",
                ));
            }
        }
        AccountingFact::CompleteCost(cost) => {
            nonnegative(cost)?;
            if entry.invocation.is_none() || evidence.receipt.is_none() {
                return Err(Denial::new(
                    "accounting_evidence",
                    "Complete cost must bind an invocation and exact receipt",
                ));
            }
        }
        AccountingFact::EnforcedUpperBound(cost) => {
            nonnegative(cost)?;
            if entry.invocation.is_none() || evidence.receipt.is_some() {
                return Err(Denial::new(
                    "accounting_evidence",
                    "A bound must name an invocation rather than a receipt",
                ));
            }
        }
    }
    Ok(())
}

pub(crate) fn apply(view: &SessionView, event: &Envelope<Event>) -> Result<TreasuryView> {
    match &event.payload {
        Event::BudgetOpened { data, .. } => {
            if view.treasury().is_some() {
                return Err(Denial::new(
                    "budget_exists",
                    "A session budget cannot be reset",
                ));
            }
            data.pricebook.validate()?;
            let input = reporting_view(view, event.at)?;
            verify_decision(view, "ResourcePolicy", &input, &data.reporting)?;
            selected(view, "CostModel")?;
            let reserve = data.reporting.outcome.reserve()?;
            if data.budget.session != *view.session()
                || data.budget.limit != input.task.constraints.budget
                || data.budget.verification_reserve != input.task.constraints.verification_reserve
                || data.budget.reporting_reserve != reserve
                || data.budget.spent != zero()
                || data.budget.held != zero()
                || add(reserve, data.budget.verification_reserve)? > data.budget.limit
            {
                return Err(Denial::new(
                    "budget",
                    "Opening budget differs from the task constraints or reporting plan",
                ));
            }
            if input.pool.eligible.is_empty() && data.reporting.outcome.narration.is_some() {
                return Err(Denial::new(
                    "reporting_plan",
                    "No eligible narrator is available",
                ));
            }
            let mut book = TreasuryView {
                budget: data.budget.clone(),
                pricebook: data.pricebook.clone(),
                reporting_plan: data.reporting.outcome.clone(),
                reporting_mode: None,
                accounts: BTreeMap::new(),
                totals: BTreeMap::new(),
                unbounded: false,
                unsettled_usage: false,
            };
            book.recompute()?;
            Ok(book)
        }
        Event::ReservationChanged { change, .. } => {
            let mut book = ledger(view)?.clone();
            match change {
                ReservationChange::Reserved(data) => {
                    data.estimate.outcome.validate()?;
                    data.allocation.outcome.validate()?;
                    verify_decision(
                        view,
                        "CostModel",
                        &estimate_view(view, &data.demand)?,
                        &data.estimate,
                    )?;
                    verify_decision(
                        view,
                        "ResourcePolicy",
                        &allowance_view(
                            view,
                            &data.demand,
                            data.estimate.outcome.clone(),
                            event.at,
                        )?,
                        &data.allocation,
                    )?;
                    let reservation = &data.reservation;
                    let allowance = &data.allocation.outcome;
                    if reservation.budget != book.budget.id
                        || reservation.state != ReservationState::Held
                        || reservation.amount != allowance.cost
                        || reservation.purpose != data.demand.kind.purpose()
                        || book.accounts.contains_key(&reservation.id)
                        || book
                            .accounts
                            .values()
                            .any(|a| a.reservation.assignment == reservation.assignment)
                    {
                        return Err(Denial::new(
                            "reservation",
                            "Reservation identity, purpose, amount or assignment is invalid",
                        ));
                    }
                    if allowance.cost < data.estimate.outcome.expected {
                        return Err(Denial::new(
                            "allowance",
                            "Allowance cannot cover the expected contribution cost",
                        ));
                    }
                    if view
                        .task()
                        .and_then(|t| t.constraints.deadline)
                        .is_some_and(|deadline| {
                            event
                                .at
                                .checked_add(allowance.timeout)
                                .is_none_or(|end| end > deadline)
                        })
                    {
                        return Err(Denial::new(
                            "deadline",
                            "Allowance exceeds the task deadline",
                        ));
                    }
                    book.may_reserve(view, &data.demand, reservation.amount)?;
                    if reservation.purpose == Purpose::Reporting {
                        let prior = book
                            .accounts
                            .values()
                            .filter(|a| {
                                a.reservation.purpose == Purpose::Reporting
                                    && a.reservation.state != ReservationState::Released
                            })
                            .count();
                        let planned = if prior == 0 {
                            &book.reporting_plan.narration
                        } else {
                            &book.reporting_plan.correction
                        };
                        let planned = planned.as_ref().ok_or_else(|| {
                            Denial::new("reporting_plan", "No paid reporting slot is available")
                        })?;
                        if allowance.cost > planned.cost
                            || allowance.timeout > planned.timeout
                            || allowance.native_turns > planned.native_turns
                            || allowance.output_chars > planned.output_chars
                        {
                            return Err(Denial::new(
                                "reporting_plan",
                                "Reporting allowance exceeds its planned slot",
                            ));
                        }
                    }
                    book.accounts.insert(
                        reservation.id.clone(),
                        ReservationAccount {
                            reservation: reservation.clone(),
                            demand: data.demand.clone(),
                            allowance: allowance.clone(),
                            invocation: None,
                            receipt: None,
                            complete_cost: None,
                            observed_cost_floor: zero(),
                            upper_bound: None,
                            revoked: false,
                            settlement: None,
                            last: event.reference()?,
                        },
                    );
                }
                ReservationChange::Authorized {
                    reservation,
                    invocation,
                } => {
                    let entry = account(&book, reservation)?;
                    if view.status() == Some(SessionStatus::Cancelled) {
                        return Err(Denial::new(
                            "user_stop",
                            "A stopped session cannot authorize an invocation",
                        ));
                    }
                    if entry.reservation.state != ReservationState::Held
                        || entry.invocation.is_some()
                        || entry.revoked
                        || book
                            .accounts
                            .values()
                            .any(|a| a.invocation.as_ref() == Some(invocation))
                    {
                        return Err(Denial::new(
                            "invocation_authority",
                            "Invocation funding is single-use and cannot revive revoked work",
                        ));
                    }
                    validate_demand(view, &entry.demand)?;
                    let learning = entry.demand.kind == ContributionKind::Curate
                        && view.status() == Some(SessionStatus::Delivered);
                    if book.reporting_mode.is_some()
                        && !learning
                        && (entry.reservation.purpose != Purpose::Reporting
                            || book.reporting_mode != Some(ReportingMode::Narrated))
                    {
                        return Err(Denial::new(
                            "reporting_only",
                            "Work stopped before this invocation was authorized",
                        ));
                    }
                    book.check_capacity(entry.reservation.purpose, zero())?;
                    if view
                        .task()
                        .and_then(|t| t.constraints.deadline)
                        .is_some_and(|deadline| event.at >= deadline)
                    {
                        return Err(Denial::new("deadline", "The task deadline has passed"));
                    }
                    let entry = book.accounts.get_mut(reservation).expect("checked account");
                    if let Some(deadline) = view.task().and_then(|t| t.constraints.deadline) {
                        entry.allowance.timeout = entry.allowance.timeout.min(deadline - event.at);
                    }
                    entry.invocation = Some(invocation.clone());
                    entry.last = event.reference()?;
                }
                ReservationChange::Observed {
                    reservation,
                    receipt,
                    evidence,
                } => {
                    receipt.validate()?;
                    let entry = account(&book, reservation)?;
                    if entry.reservation.state != ReservationState::Held
                        || entry.invocation.as_ref() != Some(&receipt.invocation)
                    {
                        return Err(Denial::new(
                            "receipt_binding",
                            "Receipt does not name a funded, unsettled invocation",
                        ));
                    }
                    if book.accounts.values().any(|a| {
                        a.reservation.id != *reservation
                            && a.receipt.as_ref().is_some_and(|r| r.id == receipt.id)
                    }) {
                        return Err(Denial::new(
                            "receipt_binding",
                            "Receipt identity belongs to another reservation",
                        ));
                    }
                    if let Some(prior) = &entry.receipt
                        && (prior.id != receipt.id
                            || !receipt.usage.includes(&prior.usage)
                            || (prior.coverage == Coverage::Complete
                                && encode(prior)? != encode(receipt)?))
                    {
                        return Err(Denial::new(
                            "receipt_conflict",
                            "A completed or nonmonotone receipt cannot be replaced",
                        ));
                    }
                    let mut full = None;
                    let mut bound = entry.upper_bound;
                    let mut full_seen = false;
                    let mut bound_seen = false;
                    for proof in evidence {
                        validate_evidence(view, entry, proof)?;
                        if matches!(proof.fact, AccountingFact::CompleteCost(_))
                            && proof.receipt.as_ref() != Some(&Digest::of_value(receipt)?)
                        {
                            return Err(Denial::new(
                                "accounting_evidence",
                                "Complete-cost evidence belongs to another receipt observation",
                            ));
                        }
                        match proof.fact {
                            AccountingFact::CompleteCost(value) if !full_seen => {
                                full = Some(value);
                                full_seen = true;
                            }
                            AccountingFact::EnforcedUpperBound(value) if !bound_seen => {
                                bound = Some(value);
                                bound_seen = true;
                            }
                            _ => {
                                return Err(Denial::new(
                                    "accounting_evidence",
                                    "Duplicate or irrelevant receipt evidence",
                                ));
                            }
                        }
                    }
                    let entry = book.accounts.get_mut(reservation).expect("checked account");
                    entry.receipt = Some(receipt.clone());
                    entry.complete_cost = full;
                    if let Some(cost) = full {
                        if cost < entry.observed_cost_floor {
                            return Err(Denial::new(
                                "cost_evidence",
                                "Complete cost cannot erase previously confirmed expense",
                            ));
                        }
                        entry.observed_cost_floor = cost;
                    }
                    // A disproved bound cannot certify later incomplete observations.
                    entry.upper_bound = bound.filter(|cost| *cost >= entry.observed_cost_floor);
                    entry.last = event.reference()?;
                }
                ReservationChange::Revoked {
                    reservation,
                    reason,
                } => {
                    require_text(reason, 4096)?;
                    let entry = book.accounts.get_mut(reservation).ok_or_else(|| {
                        Denial::new("reservation_missing", "Reservation does not exist")
                    })?;
                    if entry.reservation.state != ReservationState::Held || entry.revoked {
                        return Err(Denial::new("revocation", "Reservation is no longer active"));
                    }
                    entry.revoked = true;
                    entry.last = event.reference()?;
                }
                ReservationChange::Released {
                    reservation,
                    evidence,
                } => {
                    let entry = account(&book, reservation)?;
                    validate_evidence(view, entry, evidence)?;
                    if entry.reservation.state != ReservationState::Held
                        || evidence.fact != AccountingFact::NeverStarted
                        || entry.invocation.is_some()
                    {
                        return Err(Denial::new(
                            "execution_uncertain",
                            "Financial release requires verified never-started work",
                        ));
                    }
                    let entry = book.accounts.get_mut(reservation).expect("checked account");
                    entry.reservation.state = ReservationState::Released;
                    entry.last = event.reference()?;
                }
            }
            book.recompute()?;
            Ok(book)
        }
        Event::ReceiptSettled { data, .. } => {
            let mut book = ledger(view)?.clone();
            let entry = account(&book, &data.reservation)?;
            if entry.reservation.state != ReservationState::Held
                || entry.receipt.as_ref().map(encode).transpose()?.as_ref()
                    != Some(&encode(&data.receipt)?)
            {
                return Err(Denial::new(
                    "receipt_conflict",
                    "Settlement does not match the current unsettled observation",
                ));
            }
            verify_decision(
                view,
                "CostModel",
                &cost_view(view, &data.reservation)?,
                &data.decision,
            )?;
            match data.decision.outcome {
                ReceiptPrice::Known(cost) => {
                    nonnegative(cost)?;
                    if cost < entry.observed_cost_floor {
                        return Err(Denial::new(
                            "cost_evidence",
                            "Settlement cannot erase previously confirmed expense",
                        ));
                    }
                    if let Some(known) = entry.complete_cost {
                        if cost != known {
                            return Err(Denial::new(
                                "cost_evidence",
                                "Pricing cannot override a verified complete cost",
                            ));
                        }
                    } else if data.receipt.coverage != Coverage::Complete {
                        return Err(Denial::new(
                            "cost_unknown",
                            "Incomplete usage cannot become a known cost",
                        ));
                    }
                }
                ReceiptPrice::Estimated(cost) => {
                    if book.budget.unknown_usage != UnknownUsage::Estimate
                        || data.receipt.coverage == Coverage::Complete
                        || entry.complete_cost.is_some()
                        || cost != entry.allowance.cost
                    {
                        return Err(Denial::new(
                            "cost_estimate",
                            "Only unknown consumption may settle at the labeled allowance estimate",
                        ));
                    }
                }
                ReceiptPrice::Unknown => {
                    return Err(Denial::new(
                        "cost_unknown",
                        "Unknown consumption retains its hold",
                    ));
                }
            }
            let entry = book
                .accounts
                .get_mut(&data.reservation)
                .expect("checked account");
            entry.reservation.state = ReservationState::Settled;
            entry.settlement = Some((**data).clone());
            entry.last = event.reference()?;
            book.recompute()?;
            Ok(book)
        }
        Event::ReportingStarted { mode, .. } => {
            let mut book = ledger(view)?.clone();
            if *mode == ReportingMode::Narrated && view.status() == Some(SessionStatus::Cancelled) {
                return Err(Denial::new(
                    "user_stop",
                    "A stopped session permits only deterministic reporting",
                ));
            }
            if book.reporting_mode.is_some()
                && !(*mode == ReportingMode::Deterministic
                    && book.reporting_mode == Some(ReportingMode::Narrated))
            {
                return Err(Denial::new(
                    "reporting_phase",
                    "Reporting mode cannot be reopened or upgraded",
                ));
            }
            if *mode == ReportingMode::Narrated
                && book.accounts.values().any(|a| {
                    a.reservation.state == ReservationState::Held
                        && a.invocation.is_some()
                        && !a.revoked
                })
            {
                return Err(Denial::new(
                    "work_active",
                    "Stop active work before paid reporting",
                ));
            }
            book.reporting_mode = Some(*mode);
            Ok(book)
        }
        _ => Err(Denial::new("accounting", "Not a Treasury event")),
    }
}

pub fn attribution(
    event: &Event,
) -> Result<(Option<ymp_domain::PolicyRef>, Option<Digest>, Vec<Ref>)> {
    let mut refs = vec![];
    let (policy, input) = match event {
        Event::BudgetOpened { data, .. } => {
            refs.extend(data.reporting.proposal.basis.clone());
            (
                Some(data.reporting.effective.policy.clone()),
                Some(data.reporting.input.clone()),
            )
        }
        Event::ReservationChanged {
            change: ReservationChange::Reserved(data),
            ..
        } => {
            refs.extend(data.estimate.proposal.basis.clone());
            refs.extend(data.allocation.proposal.basis.clone());
            (
                Some(data.allocation.effective.policy.clone()),
                Some(data.allocation.input.clone()),
            )
        }
        Event::ReservationChanged {
            change: ReservationChange::Observed { evidence, .. },
            ..
        } => {
            for record in evidence {
                refs.push(record.state.clone());
                refs.extend(record.basis.clone());
            }
            (None, None)
        }
        Event::ReservationChanged {
            change: ReservationChange::Released { evidence, .. },
            ..
        } => {
            refs.push(evidence.state.clone());
            refs.extend(evidence.basis.clone());
            (None, None)
        }
        Event::ReceiptSettled { data, .. } => {
            refs.extend(data.decision.proposal.basis.clone());
            (
                Some(data.decision.effective.policy.clone()),
                Some(data.decision.input.clone()),
            )
        }
        _ => (None, None),
    };
    refs.sort();
    refs.dedup();
    Ok((policy, input, refs))
}

pub struct Treasury<J: Journal> {
    journal: Arc<J>,
    issuer: Arc<()>,
}
impl<J: Journal> Treasury<J> {
    pub fn new(journal: Arc<J>) -> Self {
        Self {
            journal,
            issuer: Arc::new(()),
        }
    }
    pub fn view(&self, session: &Id) -> Result<SessionView> {
        self.journal
            .read(session)?
            .view_with_schemas(session, None, self.journal.schemas())
    }
    fn commit(&self, session: &Id, expected: u64, at: u64, payload: Event) -> Result<u64> {
        let current = self.journal.read(session)?;
        let (policy, input, refs) = attribution(&payload)?;
        let event = Envelope {
            seq: expected
                .checked_add(1)
                .ok_or_else(|| Denial::new("revision_overflow", "Journal sequence exhausted"))?,
            session: session.clone(),
            at,
            actor: Actor::Runtime,
            policy,
            input,
            refs,
            payload,
        };
        let events = [event];
        let validated =
            validate_append(&current, session, expected, &events, self.journal.schemas())?;
        let committed = self.journal.append(session, expected, &events)?;
        if committed != validated.revision() {
            return Err(Denial::new(
                "journal_append",
                "Journal returned an unexpected revision",
            ));
        }
        Ok(committed)
    }
    pub fn open(
        &self,
        session: &Id,
        expected: u64,
        at: u64,
        request: BudgetRequest,
    ) -> Result<BudgetControl> {
        let view = self.view(session)?;
        let input = reporting_view(&view, at)?;
        let reporting = decision(&view, "ResourcePolicy", &input, &request.reporting)?;
        let budget = Budget {
            id: request.id,
            session: session.clone(),
            limit: input.task.constraints.budget,
            verification_reserve: input.task.constraints.verification_reserve,
            reporting_reserve: reporting.outcome.reserve()?,
            spent: zero(),
            held: zero(),
            unknown_usage: request.unknown_usage,
        };
        self.commit(
            session,
            expected,
            at,
            Event::BudgetOpened {
                version: 1,
                data: Box::new(BudgetOpening {
                    budget,
                    pricebook: request.pricebook,
                    reporting,
                }),
            },
        )?;
        Ok(BudgetControl {
            session: session.clone(),
            issuer: self.issuer.clone(),
        })
    }
    pub fn reserve(
        &self,
        session: &Id,
        expected: u64,
        at: u64,
        request: ReserveRequest,
    ) -> Result<u64> {
        let view = self.view(session)?;
        let estimate = decision(
            &view,
            "CostModel",
            &estimate_view(&view, &request.demand)?,
            &request.estimate,
        )?;
        let allocation = decision(
            &view,
            "ResourcePolicy",
            &allowance_view(&view, &request.demand, estimate.outcome.clone(), at)?,
            &request.allowance,
        )?;
        let reservation = Reservation {
            id: request.id,
            budget: ledger(&view)?.budget.id.clone(),
            assignment: request.assignment,
            amount: allocation.outcome.cost,
            purpose: request.demand.kind.purpose(),
            state: ReservationState::Held,
        };
        self.commit(
            session,
            expected,
            at,
            Event::ReservationChanged {
                version: 1,
                change: ReservationChange::Reserved(Box::new(ReserveDecision {
                    reservation,
                    demand: request.demand,
                    estimate,
                    allocation,
                })),
            },
        )
    }
    pub fn authorize(
        &self,
        session: &Id,
        expected: u64,
        at: u64,
        reservation: Id<Reservation>,
        invocation: Id,
    ) -> Result<u64> {
        self.commit(
            session,
            expected,
            at,
            Event::ReservationChanged {
                version: 1,
                change: ReservationChange::Authorized {
                    reservation,
                    invocation,
                },
            },
        )
    }
    pub fn observe(
        &self,
        session: &Id,
        expected: u64,
        at: u64,
        reservation: Id<Reservation>,
        receipt: Receipt,
        evidence: &[&AccountingEvidence],
    ) -> Result<u64> {
        let view = self.view(session)?;
        let entry = account(ledger(&view)?, &reservation)?;
        if let Some(prior) = &entry.receipt
            && encode(prior)? == encode(&receipt)?
            && evidence.is_empty()
        {
            return Ok(view.revision());
        }
        let mut records = vec![];
        for proof in evidence {
            if proof.session != *session || !Arc::ptr_eq(&proof.issuer, &self.issuer) {
                return Err(Denial::new(
                    "accounting_evidence",
                    "Evidence belongs to another accounting authority",
                ));
            }
            validate_evidence(&view, entry, &proof.record)?;
            records.push(proof.record.clone());
        }
        self.commit(
            session,
            expected,
            at,
            Event::ReservationChanged {
                version: 1,
                change: ReservationChange::Observed {
                    reservation,
                    receipt,
                    evidence: records,
                },
            },
        )
    }
    pub fn settle(
        &self,
        session: &Id,
        expected: u64,
        at: u64,
        reservation: Id<Reservation>,
        response: ResourceResponse<ReceiptPrice>,
    ) -> Result<u64> {
        let view = self.view(session)?;
        let entry = account(ledger(&view)?, &reservation)?;
        if let Some(settled) = &entry.settlement {
            if encode(&response.proposal)? == encode(&settled.decision.proposal)?
                && response.input == settled.decision.input
            {
                return Ok(view.revision());
            }
            return Err(Denial::new(
                "receipt_conflict",
                "A committed settlement cannot be repriced",
            ));
        }
        let price = decision(
            &view,
            "CostModel",
            &cost_view(&view, &reservation)?,
            &response,
        )?;
        let receipt = entry
            .receipt
            .clone()
            .ok_or_else(|| Denial::new("receipt_missing", "Observe usage before settlement"))?;
        self.commit(
            session,
            expected,
            at,
            Event::ReceiptSettled {
                version: 1,
                data: Box::new(Settlement {
                    reservation,
                    receipt,
                    decision: price,
                }),
            },
        )
    }
    pub fn revoke(
        &self,
        session: &Id,
        expected: u64,
        at: u64,
        reservation: Id<Reservation>,
        reason: String,
    ) -> Result<u64> {
        self.commit(
            session,
            expected,
            at,
            Event::ReservationChanged {
                version: 1,
                change: ReservationChange::Revoked {
                    reservation,
                    reason,
                },
            },
        )
    }
    pub fn never_started(
        &self,
        session: &Id,
        reservation: &Id<Reservation>,
    ) -> Result<AccountingEvidence> {
        let view = self.view(session)?;
        let entry = account(ledger(&view)?, reservation)?;
        if entry.invocation.is_some() || entry.reservation.state != ReservationState::Held {
            return Err(Denial::new(
                "execution_uncertain",
                "This reservation has authorized execution or is no longer held",
            ));
        }
        Ok(AccountingEvidence {
            session: session.clone(),
            issuer: self.issuer.clone(),
            record: AccountingEvidenceRecord {
                reservation: reservation.clone(),
                assignment: entry.reservation.assignment.clone(),
                invocation: None,
                state: entry.last.clone(),
                receipt: None,
                fact: AccountingFact::NeverStarted,
                basis: vec![entry.last.clone()],
            },
        })
    }
    pub fn release_unstarted(
        &self,
        session: &Id,
        expected: u64,
        at: u64,
        evidence: &AccountingEvidence,
    ) -> Result<u64> {
        if evidence.session != *session || !Arc::ptr_eq(&evidence.issuer, &self.issuer) {
            return Err(Denial::new(
                "accounting_evidence",
                "Evidence belongs to another accounting authority",
            ));
        }
        self.commit(
            session,
            expected,
            at,
            Event::ReservationChanged {
                version: 1,
                change: ReservationChange::Released {
                    reservation: evidence.record.reservation.clone(),
                    evidence: evidence.record.clone(),
                },
            },
        )
    }
    pub fn start_reporting(
        &self,
        control: &BudgetControl,
        expected: u64,
        at: u64,
        mode: ReportingMode,
    ) -> Result<u64> {
        if !Arc::ptr_eq(&control.issuer, &self.issuer) {
            return Err(Denial::new(
                "owner_authority",
                "Budget control belongs to another authority",
            ));
        }
        self.commit(
            &control.session,
            expected,
            at,
            Event::ReportingStarted { version: 1, mode },
        )
    }
}

#[cfg(test)]
#[path = "treasury_tests.rs"]
mod tests;
