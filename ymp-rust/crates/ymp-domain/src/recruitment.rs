//! Recruitment: how a running participant brings another one into the run, and the five mechanical
//! gates that decide whether it may.
//!
//! The proposing participant decides *whom* to recruit and *whether* to recruit at all. Everything
//! decided here is arithmetic over identifiers, sets and integers: is the entry inside the boundary
//! the run was frozen to, does the proposer still hold the authority to start a participant, is
//! there room for one more, does the runtime behind that entry admit work at this moment, and can
//! the offer-stage allowance the newcomer is given be paid for. Nothing in this file reads a goal,
//! a prompt, a bid, a score or any property of an entry beyond the triple it is named by — there is
//! no field on the request that could carry one.
//!
//! **Nothing here ranks, prefers or substitutes.** The request names one entry; it is admitted on
//! that entry or refused. A refusal never proposes another entry, and no ordering over the frozen
//! pool is read: the declared order matters exactly once in this product, in the ignition rule that
//! [`crate::pool::FrozenPool::freeze`] applies, and recruitment is not that rule.
//!
//! **Every refusal names the constraint that stopped it, in plain words.** The gates are applied in
//! one declared order ([`Gate`]), so a request that would fail several is refused by the first: what
//! an operator reads is one limiting constraint rather than whichever check happened to run last. An
//! exhausted budget is stated as an exhausted budget and never as a failure of the participant that
//! asked.
//!
//! **Liveness is measured outside the kernel.** Whether an engine answers and whether a credential
//! is still valid is I/O, and this crate performs none. It enters the decision as the answer to
//! [`RuntimeAdmission::admits`], which an implementation gives from what it has already observed.
//! Starting the admitted participant is likewise not done here: an admission is handed to the one
//! managed start path the product has ([`ParticipantStartPath`]), so recruitment never grows a
//! second way to start a participant.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::commitment::{
    BudgetVector, CommitmentCommand, CommitmentLedger, Dimension, InvocationState,
    RegisterParticipant,
};
use crate::pool::{EntryIdentity, FrozenPool};
use crate::{MAX_IDENTIFIER_CHARS, RunState, digest_bytes};

/// How many hexadecimal characters of the request digest name the participant it admits.
const DERIVED_IDENTIFIER_CHARS: usize = 12;

/// The typed command one participant issues to recruit another.
///
/// It carries three identifiers and nothing else. The proposer is who asks, the entry is which
/// catalog entry it asks for, and the request identity is what makes a repeat recognizable. There
/// is deliberately no field for a reason, a role, a rank, a priority or a quantity: a command that
/// could carry one would be a command a transition could read one out of.
///
/// The identifier the admitted participant is registered under is not supplied either — it is
/// derived from the request, so the same request always names the same participant and no caller
/// can claim an identifier for an admission it did not obtain.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RequestParticipant {
    /// The identity of this request. A second delivery of it is refused as a duplicate rather than
    /// admitting a second participant.
    pub request_id: String,
    /// The participant asking. It must be a participant of this run that is running now.
    pub proposer: String,
    /// The catalog entry named. Containment in the frozen pool is the only thing read from it.
    pub entry: EntryIdentity,
}

impl RequestParticipant {
    pub fn new(
        request_id: impl Into<String>,
        proposer: impl Into<String>,
        entry: EntryIdentity,
    ) -> Self {
        Self {
            request_id: request_id.into(),
            proposer: proposer.into(),
            entry,
        }
    }

    /// The identifier the participant this request admits is registered under.
    ///
    /// It is a function of the request identity alone, so a repeated request names the participant
    /// the first delivery admitted — which is what lets the duplicate refusal say which participant
    /// already stands for it — and two different requests never collide on one.
    pub fn participant_id(&self) -> String {
        format!(
            "participant-{}",
            &digest_bytes(self.request_id.as_bytes())[..DERIVED_IDENTIFIER_CHARS]
        )
    }

    /// The principal the admitted participant acts as: the provider account the entry names.
    ///
    /// A provider record is one account ([`ymp-docs/design/COLLECTIVE-RESOURCES.md`], *Provider*),
    /// and the principal is what the per-principal round robin of the admission queue takes turns
    /// over. Deriving it from the entry keeps that queue a statement about accounts rather than
    /// about participants: recruiting ten participants on one account buys that account no extra
    /// turn.
    pub fn principal_id(&self) -> &str {
        &self.entry.provider
    }

    /// The private writable workspace the admitted participant is given, named after itself.
    ///
    /// It is derived rather than assigned, so the fact that records the admission is complete
    /// before any process exists: a run interrupted between the admission and the start has a
    /// record naming exactly what the start path owes it.
    pub fn workspace(&self) -> String {
        format!("participants/{}", self.participant_id())
    }
}

/// The ceilings a run recruits under.
///
/// Both are boundaries and neither is a target: nothing drives the collective towards a size, and
/// there is no floor. The size a run reaches is an outcome of its own recruitment decisions inside
/// these two numbers.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RecruitmentPolicy {
    /// How many participants this run may hold at once, the ceiling of the pool it was created
    /// under. The root participant counts: it holds a process like any other.
    pub participants: u32,
    /// What one admission transfers to the participant it admits, so that a newcomer can read an
    /// offer and answer it before any contract exists. It is the proposer's own capacity that pays.
    pub offer_allowance: BudgetVector,
}

impl RecruitmentPolicy {
    /// The standing ceiling of a workspace, as owner decision D8 recorded it: six participants,
    /// pending the measurement `W1-EVL-04a` replaces it with.
    ///
    /// The allowance is one process slice. That is exactly what reading one offer and answering it
    /// costs: money, tokens and wall time reach a participant through the escrow of a contract it
    /// has actually taken on, so an admission funds the answer and never the work.
    pub const WORKING: Self = Self {
        participants: 6,
        offer_allowance: BudgetVector::ZERO.with(Dimension::InvocationStarts, 1),
    };
}

impl Default for RecruitmentPolicy {
    fn default() -> Self {
        Self::WORKING
    }
}

/// The mechanical gates, in the order they are applied.
///
/// The order is part of what a refusal states. A request that fails several gates is refused by the
/// first one in this order, so the constraint an operator reads is a stable property of the request
/// rather than of the order the checks happen to be written in.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gate {
    /// The entry is one the run's frozen pool permits.
    FrozenMembership,
    /// The proposer still holds the authority to start one further participant.
    ParticipantStarts,
    /// The run holds room for one more participant.
    Concurrency,
    /// The runtime behind the entry admits work on this host at this moment.
    RuntimeAdmission,
    /// The proposer can pay the offer-stage allowance the newcomer is given.
    OfferStageCharge,
}

/// Every gate, in the order [`Recruitment::admit`] applies them.
pub const GATES: [Gate; 5] = [
    Gate::FrozenMembership,
    Gate::ParticipantStarts,
    Gate::Concurrency,
    Gate::RuntimeAdmission,
    Gate::OfferStageCharge,
];

impl Gate {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::FrozenMembership => "frozen membership",
            Self::ParticipantStarts => "participant starts",
            Self::Concurrency => "concurrency",
            Self::RuntimeAdmission => "runtime admission",
            Self::OfferStageCharge => "offer-stage charge",
        }
    }
}

impl std::fmt::Display for Gate {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Why a proposer may not propose. It is a statement about the state the proposer is in, and never
/// about how well it has been working.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProposerState {
    /// Its process slice ended and it registered a wake. It has no process, so a request arriving
    /// in its name is one it cannot have issued.
    Yielded,
    /// Every process slice it held is closed.
    Closed,
}

impl std::fmt::Display for ProposerState {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Yielded => "yielded",
            Self::Closed => "closed",
        })
    }
}

/// Why one recruitment request was refused.
///
/// Every variant names the one constraint that stopped it, in the words an operator reads. None of
/// them names a second entry, suggests a substitute, or says anything about the entry beyond the
/// mechanics that refused it.
#[derive(Clone, Debug, Deserialize, Eq, Error, PartialEq, Serialize)]
#[serde(tag = "refusal", rename_all = "snake_case")]
pub enum RecruitmentRefusal {
    #[error("{kind} must contain between 1 and {MAX_IDENTIFIER_CHARS} characters")]
    UnacceptableIdentifier { kind: String },
    #[error("this run has ended and starts nothing further")]
    RunEnded,
    #[error("the commitment kernel of this run was stopped and starts nothing further")]
    RunStopped,
    #[error("this run was created against no frozen pool, so it permits no entry at all")]
    NoFrozenPool,
    #[error("{proposer} is not a participant of this run")]
    ProposerUnknown { proposer: String },
    #[error("{proposer} is {state} and a participant that is not running proposes nothing")]
    ProposerNotRunning {
        proposer: String,
        state: ProposerState,
    },
    #[error("request {request_id} already admitted {participant_id}")]
    DuplicateRequest {
        request_id: String,
        participant_id: String,
    },
    #[error(
        "the {pool} pool this run was frozen to does not permit {entry}; of the {permitted} entr{} \
         it holds, {admissible} {} live when this run was created and this is not one of them",
        if *permitted == 1 { "y" } else { "ies" },
        if *admissible == 1 { "was" } else { "were" }
    )]
    EntryNotPermitted {
        entry: EntryIdentity,
        pool: String,
        permitted: usize,
        admissible: usize,
    },
    #[error("{proposer} holds no further permission to start a participant")]
    ParticipantStartsExhausted { proposer: String },
    #[error(
        "this run already holds {live} participant{}, which is the ceiling it was created under",
        if *live == 1 { "" } else { "s" }
    )]
    ConcurrencyCeilingReached { live: u32, ceiling: u32 },
    #[error("{entry} is not admitted on this host at the moment: {reason}")]
    RuntimeUnadmitted {
        entry: EntryIdentity,
        reason: String,
    },
    #[error(
        "{proposer} cannot fund the offer-stage allowance a newly admitted participant is given: \
         it holds no further {dimension}"
    )]
    OfferAllowanceUnaffordable {
        proposer: String,
        dimension: Dimension,
    },
}

impl RecruitmentRefusal {
    /// Which gate refused this request, or `None` when what refused it was not a gate: a malformed
    /// request, an ended run, a proposer that may not propose, or a repeat.
    pub const fn gate(&self) -> Option<Gate> {
        match self {
            Self::EntryNotPermitted { .. } => Some(Gate::FrozenMembership),
            Self::ParticipantStartsExhausted { .. } => Some(Gate::ParticipantStarts),
            Self::ConcurrencyCeilingReached { .. } => Some(Gate::Concurrency),
            Self::RuntimeUnadmitted { .. } => Some(Gate::RuntimeAdmission),
            Self::OfferAllowanceUnaffordable { .. } => Some(Gate::OfferStageCharge),
            _ => None,
        }
    }
}

/// The runtime profile and model route one entry is served through.
///
/// Both are opaque tokens that the kernel stores and compares for equality. Neither carries a
/// measurement, a quality or a cost.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RuntimeRoute {
    pub profile: String,
    pub route: String,
}

impl RuntimeRoute {
    pub fn new(profile: impl Into<String>, route: impl Into<String>) -> Self {
        Self {
            profile: profile.into(),
            route: route.into(),
        }
    }
}

/// Why the runtime behind an entry does not admit work at this moment, in the words the measurement
/// stated it in.
#[derive(Clone, Debug, Deserialize, Eq, Error, PartialEq, Serialize)]
#[error("{reason}")]
pub struct RuntimeUnadmitted {
    pub reason: String,
}

impl RuntimeUnadmitted {
    pub fn new(reason: impl Into<String>) -> Self {
        Self {
            reason: reason.into(),
        }
    }
}

/// Whether the runtime behind one entry admits work on this host at this moment.
///
/// This crate performs no I/O, so liveness is measured outside it and enters the decision here. An
/// implementation answers from what it has already observed about that one entry. It is never asked
/// which entry to use, never given more than one to consider, and never asked to compare two.
pub trait RuntimeAdmission {
    fn admits(&self, entry: &EntryIdentity) -> Result<RuntimeRoute, RuntimeUnadmitted>;
}

/// What the admitted participant is: who asked for it, what it runs as, and where it works.
///
/// Every field is decided before any process exists, which is what makes the record of an admission
/// complete on its own: a run interrupted between the admission and the start holds a fact stating
/// exactly what the start path still owes it.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AdmittedParticipant {
    /// The request this admission answers. One request admits one participant, ever.
    pub request_id: String,
    /// The participant that asked. It is the parentage of the newcomer and nothing else: a proposer
    /// is not a manager, and nothing here gives it authority over what it recruited.
    pub proposer: String,
    pub participant_id: String,
    pub principal_id: String,
    pub entry: EntryIdentity,
    pub profile: String,
    pub route: String,
    pub workspace: String,
}

/// One granted admission: the participant it names, and the commitment command that pays for it.
///
/// The charge is a command rather than a set of facts, so what moves the accounts is decided by the
/// one kernel that owns them. Recruitment states who is admitted; it keeps no second accounting.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Admission {
    pub participant: AdmittedParticipant,
    pub charge: CommitmentCommand,
}

/// Why the managed start path could not start an admitted participant.
///
/// It is not a refusal: the admission was granted, the authority was spent, and the record stands.
/// What failed is the start.
#[derive(Clone, Debug, Deserialize, Eq, Error, PartialEq, Serialize)]
#[error("{participant_id} was admitted and could not be started: {reason}")]
pub struct ParticipantStartFailed {
    pub participant_id: String,
    pub reason: String,
}

impl ParticipantStartFailed {
    pub fn new(participant_id: impl Into<String>, reason: impl Into<String>) -> Self {
        Self {
            participant_id: participant_id.into(),
            reason: reason.into(),
        }
    }
}

/// The one managed path an admitted participant is started through.
///
/// Recruitment decides admission and hands the result here. It deliberately holds no way to start a
/// participant of its own: the origin participant of a run and every participant recruited during it
/// reach a process through the same implementation, so there is no second start path to keep in
/// step with the first.
pub trait ParticipantStartPath {
    fn start(&mut self, admitted: &AdmittedParticipant) -> Result<(), ParticipantStartFailed>;
}

/// Gates a test build may switch off, one at a time, to show that each one is load-bearing.
///
/// The field does not exist outside `cfg(test)`, so a release build has no way to reach a
/// recruitment that skips a gate.
#[cfg(test)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct DisabledGates {
    pub frozen_membership: bool,
    pub participant_starts: bool,
    pub concurrency: bool,
    pub runtime_admission: bool,
    pub offer_stage_charge: bool,
}

/// The recruitment gate of one run.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Recruitment {
    policy: RecruitmentPolicy,
    #[cfg(test)]
    disabled: DisabledGates,
}

impl Recruitment {
    pub const fn new(policy: RecruitmentPolicy) -> Self {
        Self {
            policy,
            #[cfg(test)]
            disabled: DisabledGates {
                frozen_membership: false,
                participant_starts: false,
                concurrency: false,
                runtime_admission: false,
                offer_stage_charge: false,
            },
        }
    }

    pub const fn policy(&self) -> &RecruitmentPolicy {
        &self.policy
    }

    #[cfg(test)]
    pub(crate) const fn without(mut self, disabled: DisabledGates) -> Self {
        self.disabled = disabled;
        self
    }

    #[cfg(test)]
    const fn enforces(&self, gate: Gate) -> bool {
        match gate {
            Gate::FrozenMembership => !self.disabled.frozen_membership,
            Gate::ParticipantStarts => !self.disabled.participant_starts,
            Gate::Concurrency => !self.disabled.concurrency,
            Gate::RuntimeAdmission => !self.disabled.runtime_admission,
            Gate::OfferStageCharge => !self.disabled.offer_stage_charge,
        }
    }

    #[cfg(not(test))]
    #[allow(clippy::unused_self)]
    const fn enforces(&self, _gate: Gate) -> bool {
        true
    }

    /// Decide one recruitment request against the run's own committed record.
    ///
    /// Deciding changes nothing. A refused request therefore leaves the journal, the ledger and the
    /// host exactly as they were: no participant exists, no capacity moved, and no process was
    /// started, which is what makes a refusal something a proposer may retry after the constraint
    /// that stopped it has changed.
    ///
    /// What is read is the run's own record and the answer of one measurement:
    ///
    /// * the pool this run was frozen to, by value, and never the pool the product root holds now;
    /// * the admissions this run has already committed, so a repeat is recognized;
    /// * the ledger — who is a participant, which of them is running, and what the proposer holds;
    /// * whether the runtime behind the named entry admits work at this moment.
    pub fn admit(
        &self,
        request: &RequestParticipant,
        run: &RunState,
        ledger: &CommitmentLedger,
        runtime: &dyn RuntimeAdmission,
    ) -> Result<Admission, RecruitmentRefusal> {
        self.validate(request)?;
        self.ensure_run_creates(run, ledger)?;
        self.ensure_proposer_proposes(request, ledger)?;
        self.ensure_not_repeated(request, run)?;

        let frozen = run
            .frozen_pool
            .as_ref()
            .ok_or(RecruitmentRefusal::NoFrozenPool)?;
        self.gate_frozen_membership(request, frozen)?;
        self.gate_participant_starts(request, ledger)?;
        self.gate_concurrency(ledger)?;
        let route = self.gate_runtime_admission(request, runtime)?;
        self.gate_offer_stage_charge(request, ledger)?;

        Ok(Admission {
            participant: AdmittedParticipant {
                request_id: request.request_id.clone(),
                proposer: request.proposer.clone(),
                participant_id: request.participant_id(),
                principal_id: request.principal_id().to_owned(),
                entry: request.entry.clone(),
                profile: route.profile,
                route: route.route,
                workspace: request.workspace(),
            },
            charge: CommitmentCommand::RegisterParticipant(RegisterParticipant {
                participant_id: request.participant_id(),
                principal_id: request.principal_id().to_owned(),
                sponsor: request.proposer.clone(),
                endowment: self.policy.offer_allowance,
            }),
        })
    }

    fn validate(&self, request: &RequestParticipant) -> Result<(), RecruitmentRefusal> {
        for (kind, value) in [
            ("a request identifier", request.request_id.as_str()),
            ("a proposer identifier", request.proposer.as_str()),
            ("a provider name", request.entry.provider.as_str()),
            ("an engine name", request.entry.engine.as_str()),
            ("a model name", request.entry.model.as_str()),
        ] {
            let length = value.chars().count();
            if !(1..=MAX_IDENTIFIER_CHARS).contains(&length) {
                return Err(RecruitmentRefusal::UnacceptableIdentifier {
                    kind: kind.to_owned(),
                });
            }
        }
        Ok(())
    }

    /// A run that has ended, and a kernel that has been stopped, start nothing further. Both are
    /// stated in the words of the record that ended, because an admission granted after either
    /// would put a process to work for a run nobody is waiting on any more.
    fn ensure_run_creates(
        &self,
        run: &RunState,
        ledger: &CommitmentLedger,
    ) -> Result<(), RecruitmentRefusal> {
        if run.status.is_terminal() {
            return Err(RecruitmentRefusal::RunEnded);
        }
        if ledger.stopped().is_some() {
            return Err(RecruitmentRefusal::RunStopped);
        }
        Ok(())
    }

    /// Only a participant of this run that is running now proposes anything.
    ///
    /// A participant with no process slice at all has not begun one and is running in the only
    /// sense this check has; one whose slices are all yielded or closed has no process, so a request
    /// arriving in its name is one it could not have issued.
    fn ensure_proposer_proposes(
        &self,
        request: &RequestParticipant,
        ledger: &CommitmentLedger,
    ) -> Result<(), RecruitmentRefusal> {
        if !ledger.participants().contains_key(&request.proposer) {
            return Err(RecruitmentRefusal::ProposerUnknown {
                proposer: request.proposer.clone(),
            });
        }
        let slices: Vec<InvocationState> = ledger
            .invocations()
            .values()
            .filter(|invocation| invocation.participant == request.proposer)
            .map(|invocation| invocation.state)
            .collect();
        if slices.is_empty() || slices.contains(&InvocationState::Running) {
            return Ok(());
        }
        Err(RecruitmentRefusal::ProposerNotRunning {
            proposer: request.proposer.clone(),
            state: if slices.contains(&InvocationState::Yielded) {
                ProposerState::Yielded
            } else {
                ProposerState::Closed
            },
        })
    }

    /// One request admits one participant, ever.
    ///
    /// The record of the run is what answers this, not the memory of a process: a repeat delivered
    /// after a restart is recognized by the same fact the first delivery committed.
    fn ensure_not_repeated(
        &self,
        request: &RequestParticipant,
        run: &RunState,
    ) -> Result<(), RecruitmentRefusal> {
        match run
            .admissions
            .iter()
            .find(|admitted| admitted.request_id == request.request_id)
        {
            None => Ok(()),
            Some(admitted) => Err(RecruitmentRefusal::DuplicateRequest {
                request_id: request.request_id.clone(),
                participant_id: admitted.participant_id.clone(),
            }),
        }
    }

    /// Gate 1. The entry is one the run's own frozen pool permits.
    ///
    /// The frozen fact is read and the live pool is not. A provider disabled, an entry removed or a
    /// model discovered since the run was created changes the next run and never this one.
    fn gate_frozen_membership(
        &self,
        request: &RequestParticipant,
        frozen: &FrozenPool,
    ) -> Result<(), RecruitmentRefusal> {
        if !self.enforces(Gate::FrozenMembership) || frozen.permits(&request.entry) {
            return Ok(());
        }
        Err(RecruitmentRefusal::EntryNotPermitted {
            entry: request.entry.clone(),
            pool: frozen.pool.clone(),
            permitted: frozen.entries.len(),
            admissible: frozen.admissible(),
        })
    }

    /// Gate 2. The proposer still holds the authority to start one further participant.
    ///
    /// The authority is the proposer's own and is irreversible once spent, so a run cannot decompose
    /// without bound by returning the same nominal budget along a chain.
    fn gate_participant_starts(
        &self,
        request: &RequestParticipant,
        ledger: &CommitmentLedger,
    ) -> Result<(), RecruitmentRefusal> {
        if !self.enforces(Gate::ParticipantStarts) {
            return Ok(());
        }
        let held = ledger
            .participants()
            .get(&request.proposer)
            .map_or(0, |participant| {
                participant.balance.get(Dimension::ParticipantStarts)
            });
        if held >= 1 {
            return Ok(());
        }
        Err(RecruitmentRefusal::ParticipantStartsExhausted {
            proposer: request.proposer.clone(),
        })
    }

    /// Gate 3. The run holds room for one more participant.
    ///
    /// This is a ceiling on how many participants the run holds at once, and it is a different
    /// question from gate 2: that one reads one account's own authority, this one reads the size of
    /// the whole run. A proposer holding five starts still recruits nobody into a full run, and a
    /// half-empty run still admits nobody for a proposer that holds none.
    fn gate_concurrency(&self, ledger: &CommitmentLedger) -> Result<(), RecruitmentRefusal> {
        if !self.enforces(Gate::Concurrency) {
            return Ok(());
        }
        let live = u32::try_from(ledger.participants().len()).unwrap_or(u32::MAX);
        if live < self.policy.participants {
            return Ok(());
        }
        Err(RecruitmentRefusal::ConcurrencyCeilingReached {
            live,
            ceiling: self.policy.participants,
        })
    }

    /// Gate 4. The runtime behind the entry admits work on this host at this moment.
    ///
    /// The measurement is somebody else's; what happens here is that its answer is required. A
    /// refusal carries the words the measurement stated and adds none of its own.
    fn gate_runtime_admission(
        &self,
        request: &RequestParticipant,
        runtime: &dyn RuntimeAdmission,
    ) -> Result<RuntimeRoute, RecruitmentRefusal> {
        match runtime.admits(&request.entry) {
            Ok(route) => Ok(route),
            Err(unadmitted) if !self.enforces(Gate::RuntimeAdmission) => {
                // The gate is switched off, so the request proceeds on an entry the host does not
                // serve. What it proceeds with is the measurement's own words, because a route
                // invented here would hide which entry the admission actually named.
                Ok(RuntimeRoute::new(
                    "unadmitted",
                    format!("{} ({})", request.entry, unadmitted.reason),
                ))
            }
            Err(unadmitted) => Err(RecruitmentRefusal::RuntimeUnadmitted {
                entry: request.entry.clone(),
                reason: unadmitted.reason,
            }),
        }
    }

    /// Gate 5. The proposer can pay the offer-stage allowance the newcomer is given.
    ///
    /// A participant admitted with nothing could not read the offer it was recruited for, so the
    /// allowance is part of the admission rather than a later favour — and it comes out of the
    /// proposer's own capacity, because delegation transfers capacity and never creates it.
    fn gate_offer_stage_charge(
        &self,
        request: &RequestParticipant,
        ledger: &CommitmentLedger,
    ) -> Result<(), RecruitmentRefusal> {
        if !self.enforces(Gate::OfferStageCharge) {
            return Ok(());
        }
        let balance = ledger
            .participants()
            .get(&request.proposer)
            .map_or(BudgetVector::ZERO, |participant| participant.balance);
        match balance.shortfall(&self.policy.offer_allowance) {
            None => Ok(()),
            Some(dimension) => Err(RecruitmentRefusal::OfferAllowanceUnaffordable {
                proposer: request.proposer.clone(),
                dimension,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Admission, DisabledGates, Gate, ProposerState, Recruitment, RecruitmentPolicy,
        RecruitmentRefusal, RequestParticipant, RuntimeAdmission, RuntimeRoute, RuntimeUnadmitted,
    };
    use crate::commitment::{
        BudgetVector, CommitmentCommand, CommitmentLedger, Dimension, RegisterParticipant,
    };
    use crate::pool::{EntryIdentity, FrozenEntry, FrozenPool};
    use crate::{Budget, EventEnvelope, EventKind, RunState};

    const OPUS: [&str; 3] = ["anthropic", "claude-code", "claude-opus-5"];
    const SONNET: [&str; 3] = ["anthropic", "claude-code", "claude-sonnet-5"];
    const GPT: [&str; 3] = ["openai", "codex", "gpt-5"];

    fn entry(triple: [&str; 3]) -> EntryIdentity {
        EntryIdentity::new(triple[0], triple[1], triple[2])
    }

    /// A host that serves the entries it was given and refuses every other with plain words.
    struct Host {
        served: Vec<EntryIdentity>,
    }

    impl Host {
        fn serving(entries: &[[&str; 3]]) -> Self {
            Self {
                served: entries.iter().map(|triple| entry(*triple)).collect(),
            }
        }
    }

    impl RuntimeAdmission for Host {
        fn admits(&self, named: &EntryIdentity) -> Result<RuntimeRoute, RuntimeUnadmitted> {
            if self.served.contains(named) {
                Ok(RuntimeRoute::new(
                    named.engine.clone(),
                    format!("{}/{}", named.provider, named.model),
                ))
            } else {
                Err(RuntimeUnadmitted::new("the engine answered nothing"))
            }
        }
    }

    /// A run whose pool is frozen to two live Anthropic entries and one the catalog held back.
    fn run() -> RunState {
        let start = EventEnvelope::new(
            "run-1",
            1,
            "bootstrap",
            "0".repeat(64),
            None,
            EventKind::RunStarted {
                budget: Budget::new(1, 1),
            },
        )
        .expect("start envelope");
        let mut state = RunState::from_start(&start).expect("running state");
        let frozen = FrozenPool::freeze(
            "default",
            vec![
                FrozenEntry::admissible(OPUS[0], OPUS[1], OPUS[2]),
                FrozenEntry::admissible(SONNET[0], SONNET[1], SONNET[2]),
                FrozenEntry::unavailable(
                    GPT[0],
                    GPT[1],
                    GPT[2],
                    "the account states no credential",
                ),
            ],
            "b".repeat(64),
        )
        .expect("the pool freezes");
        let freeze = EventEnvelope::new(
            "run-1",
            2,
            "freeze",
            "0".repeat(64),
            None,
            EventKind::PoolFrozen(frozen),
        )
        .expect("freeze envelope");
        state.apply(&freeze);
        state
    }

    /// A ledger whose root participant holds enough to recruit `starts` times.
    fn ledger(starts: u64) -> CommitmentLedger {
        CommitmentLedger::new(
            "participant-root",
            "anthropic",
            "obligation-root",
            BudgetVector::ZERO
                .with(Dimension::ParticipantStarts, starts)
                .with(Dimension::InvocationStarts, starts),
        )
        .expect("a ledger")
    }

    fn request(request_id: &str, triple: [&str; 3]) -> RequestParticipant {
        RequestParticipant::new(request_id, "participant-root", entry(triple))
    }

    /// Fold one granted admission into the run and the ledger, exactly as the application does: the
    /// charge is executed by the kernel that owns the accounts, and the admission is recorded.
    fn commit(
        admission: &Admission,
        state: &mut RunState,
        ledger: &mut CommitmentLedger,
        sequence: u64,
    ) {
        ledger
            .execute(&admission.charge)
            .expect("the accounts honour the charge");
        let envelope = EventEnvelope::new(
            "run-1",
            sequence,
            format!("admit-{}", admission.participant.request_id),
            "0".repeat(64),
            None,
            EventKind::ParticipantAdmitted {
                admitted: admission.participant.clone(),
                facts: Vec::new(),
            },
        )
        .expect("admission envelope");
        state.apply(&envelope);
    }

    /// The whole positive half: a live participant asking for a frozen, live entry under a budget
    /// that still holds one start is admitted on the entry it named, and the charge the admission
    /// carries is the one the kernel that owns the accounts is asked to make.
    #[test]
    fn a_live_proposer_is_admitted_on_the_frozen_entry_it_named() {
        let admission = Recruitment::new(RecruitmentPolicy::WORKING)
            .admit(
                &request("request-1", SONNET),
                &run(),
                &ledger(2),
                &Host::serving(&[OPUS, SONNET]),
            )
            .expect("the request is admitted");

        // The entry admitted is the one the request named, and not the first of the pool: nothing
        // here reads the declared order, so naming the second live entry admits the second.
        assert_eq!(admission.participant.entry, entry(SONNET));
        assert_eq!(admission.participant.proposer, "participant-root");
        assert_eq!(admission.participant.principal_id, "anthropic");
        assert_eq!(admission.participant.profile, "claude-code");
        assert_eq!(admission.participant.route, "anthropic/claude-sonnet-5");
        assert_eq!(
            admission.participant.workspace,
            format!("participants/{}", admission.participant.participant_id)
        );
        assert_eq!(
            admission.charge,
            CommitmentCommand::RegisterParticipant(RegisterParticipant {
                participant_id: admission.participant.participant_id.clone(),
                principal_id: "anthropic".to_owned(),
                sponsor: "participant-root".to_owned(),
                endowment: RecruitmentPolicy::WORKING.offer_allowance,
            })
        );
    }

    /// The charge is what makes an admission cost something: one unit of the authority to start a
    /// participant leaves the accounts for good, and the allowance moves to the newcomer.
    #[test]
    fn an_admission_spends_the_authority_it_used_and_funds_the_participant_it_admitted() {
        let mut state = run();
        let mut ledger = ledger(2);
        let admission = Recruitment::new(RecruitmentPolicy::WORKING)
            .admit(
                &request("request-1", OPUS),
                &state,
                &ledger,
                &Host::serving(&[OPUS]),
            )
            .expect("the request is admitted");
        commit(&admission, &mut state, &mut ledger, 3);

        let proposer = &ledger.participants()["participant-root"];
        assert_eq!(proposer.balance.get(Dimension::ParticipantStarts), 1);
        assert_eq!(proposer.balance.get(Dimension::InvocationStarts), 1);
        assert_eq!(
            ledger.consumed().get(Dimension::ParticipantStarts),
            1,
            "the authority to start a participant is spent, not moved"
        );
        let admitted = &ledger.participants()[&admission.participant.participant_id];
        assert_eq!(admitted.balance.get(Dimension::InvocationStarts), 1);
        assert_eq!(admitted.principal_id, "anthropic");
    }

    /// The gates are applied in one declared order, so a request failing several is refused by the
    /// first: what is read is the constraint that actually stopped it. Each cause is then removed in
    /// turn and the next gate answers.
    #[test]
    fn a_request_failing_several_gates_is_refused_by_the_first_in_the_declared_order() {
        let state = run();
        let exhausted = ledger(0);
        let unserved = Host::serving(&[]);

        // Outside the frozen pool, no starts left, over the ceiling, unserved, unaffordable.
        let over_ceiling = Recruitment::new(RecruitmentPolicy {
            participants: 1,
            offer_allowance: RecruitmentPolicy::WORKING.offer_allowance,
        });
        let refusal = over_ceiling
            .admit(&request("request-1", GPT), &state, &exhausted, &unserved)
            .expect_err("the first gate refuses");
        assert_eq!(refusal.gate(), Some(Gate::FrozenMembership));

        // The entry is now inside the pool; the proposer's authority answers next.
        let refusal = over_ceiling
            .admit(&request("request-1", OPUS), &state, &exhausted, &unserved)
            .expect_err("the second gate refuses");
        assert_eq!(refusal.gate(), Some(Gate::ParticipantStarts));

        // With authority in hand, the run's own ceiling answers.
        let funded = ledger(2);
        let refusal = over_ceiling
            .admit(&request("request-1", OPUS), &state, &funded, &unserved)
            .expect_err("the third gate refuses");
        assert_eq!(refusal.gate(), Some(Gate::Concurrency));

        // With room in the run, the host answers.
        let roomy = Recruitment::new(RecruitmentPolicy::WORKING);
        let refusal = roomy
            .admit(&request("request-1", OPUS), &state, &funded, &unserved)
            .expect_err("the fourth gate refuses");
        assert_eq!(refusal.gate(), Some(Gate::RuntimeAdmission));

        // With the host serving it, the allowance answers.
        let no_allowance = CommitmentLedger::new(
            "participant-root",
            "anthropic",
            "obligation-root",
            BudgetVector::unit(Dimension::ParticipantStarts),
        )
        .expect("a ledger");
        let refusal = roomy
            .admit(
                &request("request-1", OPUS),
                &state,
                &no_allowance,
                &Host::serving(&[OPUS]),
            )
            .expect_err("the fifth gate refuses");
        assert_eq!(refusal.gate(), Some(Gate::OfferStageCharge));
        assert!(
            matches!(
                &refusal,
                RecruitmentRefusal::OfferAllowanceUnaffordable { dimension, .. }
                    if *dimension == Dimension::InvocationStarts
            ),
            "{refusal}"
        );

        // And with everything in hand, it is admitted.
        assert!(
            roomy
                .admit(
                    &request("request-1", OPUS),
                    &state,
                    &funded,
                    &Host::serving(&[OPUS])
                )
                .is_ok()
        );
    }

    /// Each gate refuses in its own words, and the words name the constraint rather than the
    /// participant that ran into it.
    #[test]
    fn every_gate_refuses_in_the_words_of_the_constraint_that_stopped_it() {
        let state = run();
        let roomy = Recruitment::new(RecruitmentPolicy::WORKING);

        let outside = roomy
            .admit(
                &request("request-1", GPT),
                &state,
                &ledger(2),
                &Host::serving(&[OPUS]),
            )
            .expect_err("an entry outside the frozen pool is refused");
        assert_eq!(
            outside.to_string(),
            "the default pool this run was frozen to does not permit openai · codex · gpt-5; of \
             the 3 entries it holds, 2 were live when this run was created and this is not one of \
             them"
        );

        let exhausted = roomy
            .admit(
                &request("request-1", OPUS),
                &state,
                &ledger(0),
                &Host::serving(&[OPUS]),
            )
            .expect_err("a proposer with no start authority is refused");
        assert_eq!(
            exhausted.to_string(),
            "participant-root holds no further permission to start a participant"
        );

        let full = Recruitment::new(RecruitmentPolicy {
            participants: 1,
            offer_allowance: RecruitmentPolicy::WORKING.offer_allowance,
        })
        .admit(
            &request("request-1", OPUS),
            &state,
            &ledger(2),
            &Host::serving(&[OPUS]),
        )
        .expect_err("a full run is refused");
        assert_eq!(
            full.to_string(),
            "this run already holds 1 participant, which is the ceiling it was created under"
        );

        let unadmitted = roomy
            .admit(
                &request("request-1", OPUS),
                &state,
                &ledger(2),
                &Host::serving(&[]),
            )
            .expect_err("an unserved entry is refused");
        assert_eq!(
            unadmitted.to_string(),
            "anthropic · claude-code · claude-opus-5 is not admitted on this host at the moment: \
             the engine answered nothing"
        );

        let unaffordable = roomy
            .admit(
                &request("request-1", OPUS),
                &state,
                &CommitmentLedger::new(
                    "participant-root",
                    "anthropic",
                    "obligation-root",
                    BudgetVector::unit(Dimension::ParticipantStarts),
                )
                .expect("a ledger"),
                &Host::serving(&[OPUS]),
            )
            .expect_err("an unaffordable allowance is refused");
        assert_eq!(
            unaffordable.to_string(),
            "participant-root cannot fund the offer-stage allowance a newly admitted participant \
             is given: it holds no further invocation_starts"
        );
    }

    /// The negative half of gate 1. With the containment check switched off, a run admits and pays
    /// for a participant on an entry its own frozen pool never permitted — which is the violation
    /// the gate exists to prevent, happening.
    #[test]
    fn without_the_containment_check_a_run_starts_an_entry_it_never_permitted() {
        let mut state = run();
        let mut ledger = ledger(2);
        let outside = request("request-1", GPT);

        // With the gate in place there is no participant and no charge at all.
        let refusal = Recruitment::new(RecruitmentPolicy::WORKING)
            .admit(&outside, &state, &ledger, &Host::serving(&[GPT]))
            .expect_err("the frozen pool does not permit this entry");
        assert_eq!(refusal.gate(), Some(Gate::FrozenMembership));
        assert_eq!(ledger.participants().len(), 1);

        let admission = Recruitment::new(RecruitmentPolicy::WORKING)
            .without(DisabledGates {
                frozen_membership: true,
                ..DisabledGates::default()
            })
            .admit(&outside, &state, &ledger, &Host::serving(&[GPT]))
            .expect("the weakened gate admits it");
        commit(&admission, &mut state, &mut ledger, 3);

        assert_eq!(admission.participant.entry, entry(GPT));
        assert!(
            !state
                .frozen_pool
                .as_ref()
                .expect("the run carries a frozen pool")
                .permits(&admission.participant.entry),
            "the entry that was started is one the run's own snapshot does not permit"
        );
        assert_eq!(
            ledger.participants().len(),
            2,
            "a participant exists on an entry outside the boundary of this run"
        );
        assert_eq!(ledger.consumed().get(Dimension::ParticipantStarts), 1);
    }

    /// The negative half of gate 3. With the ceiling check switched off, one run holds more
    /// participants at once than the ceiling it was created under.
    #[test]
    fn without_the_ceiling_check_a_run_holds_more_participants_than_its_ceiling() {
        let policy = RecruitmentPolicy {
            participants: 2,
            offer_allowance: RecruitmentPolicy::WORKING.offer_allowance,
        };
        let mut state = run();
        let mut ledger = ledger(4);
        let host = Host::serving(&[OPUS]);

        let first = Recruitment::new(policy)
            .admit(&request("request-1", OPUS), &state, &ledger, &host)
            .expect("the run has room for a second participant");
        commit(&first, &mut state, &mut ledger, 3);

        // The run is now at its ceiling of two, and the gate says so.
        let refusal = Recruitment::new(policy)
            .admit(&request("request-2", OPUS), &state, &ledger, &host)
            .expect_err("the run is full");
        assert_eq!(refusal.gate(), Some(Gate::Concurrency));
        assert_eq!(ledger.participants().len(), 2);

        let over = Recruitment::new(policy)
            .without(DisabledGates {
                concurrency: true,
                ..DisabledGates::default()
            })
            .admit(&request("request-2", OPUS), &state, &ledger, &host)
            .expect("the weakened gate admits it");
        commit(&over, &mut state, &mut ledger, 4);

        assert_eq!(
            ledger.participants().len(),
            3,
            "the run holds more participants at once than the ceiling it was created under"
        );
    }

    /// The negative half of gate 4. With the admission check switched off, a participant is admitted
    /// and paid for on an entry no runtime on this host serves, so the run spends its authority on a
    /// process that cannot exist.
    #[test]
    fn without_the_admission_check_a_run_pays_for_an_entry_no_host_serves() {
        let mut state = run();
        let mut ledger = ledger(2);
        let unserved = Host::serving(&[]);

        let refusal = Recruitment::new(RecruitmentPolicy::WORKING)
            .admit(&request("request-1", OPUS), &state, &ledger, &unserved)
            .expect_err("no runtime on this host serves the entry");
        assert_eq!(refusal.gate(), Some(Gate::RuntimeAdmission));
        assert_eq!(ledger.consumed().get(Dimension::ParticipantStarts), 0);

        let admission = Recruitment::new(RecruitmentPolicy::WORKING)
            .without(DisabledGates {
                runtime_admission: true,
                ..DisabledGates::default()
            })
            .admit(&request("request-1", OPUS), &state, &ledger, &unserved)
            .expect("the weakened gate admits it");
        commit(&admission, &mut state, &mut ledger, 3);

        assert_eq!(
            ledger.consumed().get(Dimension::ParticipantStarts),
            1,
            "the run spent its authority on a participant no runtime here can start"
        );
        assert_eq!(admission.participant.profile, "unadmitted");
    }

    /// The negative half of gates 2 and 5. Both read what the proposer holds, and the ledger that
    /// owns the accounts refuses the same debit a moment later — so switching either gate off does
    /// not overspend the run. What it costs is the honest refusal: the proposer is handed the
    /// kernel's accounting fault instead of the constraint that stopped it, which is exactly what
    /// the guardrail against presenting an exhausted budget as a participant failure forbids.
    #[test]
    fn without_the_budget_gates_the_refusal_stops_being_honest_and_the_ledger_catches_the_debit() {
        let state = run();
        let host = Host::serving(&[OPUS]);
        // One account holding no authority to start a participant, and one holding no slice to
        // give away. Each is exhausted in exactly the dimension one gate reads.
        let account = |dimension, units| {
            CommitmentLedger::new(
                "participant-root",
                "anthropic",
                "obligation-root",
                BudgetVector::units(dimension, units),
            )
            .expect("a ledger")
        };

        for (gates, exhausted, gate, dimension) in [
            (
                DisabledGates {
                    participant_starts: true,
                    ..DisabledGates::default()
                },
                account(Dimension::InvocationStarts, 1),
                Gate::ParticipantStarts,
                Dimension::ParticipantStarts,
            ),
            (
                DisabledGates {
                    offer_stage_charge: true,
                    ..DisabledGates::default()
                },
                account(Dimension::ParticipantStarts, 1),
                Gate::OfferStageCharge,
                Dimension::InvocationStarts,
            ),
        ] {
            // With the gate in place the request is refused before anything is decided, in the
            // words of the constraint that stopped it.
            let refusal = Recruitment::new(RecruitmentPolicy::WORKING)
                .admit(&request("request-1", OPUS), &state, &exhausted, &host)
                .expect_err("the gate refuses");
            assert_eq!(refusal.gate(), Some(gate));

            // Without it the request is admitted, and what refuses the debit is the kernel that
            // owns the accounts — so the run does not overspend, and what it loses is the honest
            // refusal: the proposer is handed an accounting fault naming an internal account.
            let admission = Recruitment::new(RecruitmentPolicy::WORKING)
                .without(gates)
                .admit(&request("request-1", OPUS), &state, &exhausted, &host)
                .expect("the weakened gate admits it");
            let fault = exhausted
                .clone()
                .execute(&admission.charge)
                .expect_err("the accounts cannot honour the charge");
            assert_eq!(
                fault,
                crate::commitment::CommitmentError::InsufficientBudget {
                    account: "participant participant-root".to_owned(),
                    dimension,
                }
            );
            assert_ne!(fault.to_string(), refusal.to_string());
            // The run is left as it was either way: a decision changes nothing, and a refused
            // command commits none of its facts.
            assert_eq!(exhausted.participants().len(), 1);
            assert_eq!(exhausted.consumed().get(Dimension::ParticipantStarts), 0);
        }
    }

    /// A repeat of one request admits nobody a second time, and the refusal names the participant
    /// the first delivery admitted.
    #[test]
    fn a_repeated_request_is_refused_as_a_duplicate() {
        let mut state = run();
        let mut ledger = ledger(4);
        let host = Host::serving(&[OPUS]);
        let gate = Recruitment::new(RecruitmentPolicy::WORKING);

        let first = gate
            .admit(&request("request-1", OPUS), &state, &ledger, &host)
            .expect("the first delivery is admitted");
        commit(&first, &mut state, &mut ledger, 3);

        let refusal = gate
            .admit(&request("request-1", OPUS), &state, &ledger, &host)
            .expect_err("the second delivery is refused");
        assert_eq!(
            refusal,
            RecruitmentRefusal::DuplicateRequest {
                request_id: "request-1".to_owned(),
                participant_id: first.participant.participant_id.clone(),
            }
        );
        assert!(refusal.gate().is_none());
        assert_eq!(ledger.participants().len(), 2);

        // A second request naming the same entry is a different request and is admitted: what is
        // refused is the repeat, not the entry.
        assert!(
            gate.admit(&request("request-2", OPUS), &state, &ledger, &host)
                .is_ok()
        );
    }

    /// A proposer that is not a participant of this run, and one whose process slice has yielded,
    /// are both refused before any gate is reached.
    #[test]
    fn a_proposer_that_is_not_running_proposes_nothing() {
        let state = run();
        let ledger = ledger(2);
        let host = Host::serving(&[OPUS]);
        let gate = Recruitment::new(RecruitmentPolicy::WORKING);

        let stranger = RequestParticipant::new("request-1", "participant-stranger", entry(OPUS));
        assert_eq!(
            gate.admit(&stranger, &state, &ledger, &host)
                .expect_err("a stranger proposes nothing"),
            RecruitmentRefusal::ProposerUnknown {
                proposer: "participant-stranger".to_owned(),
            }
        );

        let yielded = yielded_proposer();
        let refusal = gate
            .admit(&request("request-1", OPUS), &state, &yielded, &host)
            .expect_err("a yielded proposer proposes nothing");
        assert_eq!(
            refusal,
            RecruitmentRefusal::ProposerNotRunning {
                proposer: "participant-root".to_owned(),
                state: ProposerState::Yielded,
            }
        );
        assert!(refusal.gate().is_none());
        // Nothing was decided, so nothing moved: the ledger holds the participant it held.
        assert_eq!(yielded.participants().len(), 1);
        assert_eq!(yielded.consumed().get(Dimension::ParticipantStarts), 0);
    }

    /// The request carries identifiers and nothing a transition could read a preference out of.
    #[test]
    fn the_request_carries_no_field_a_decision_could_read_a_preference_out_of() {
        let json =
            serde_json::to_value(request("request-1", OPUS)).expect("the request serializes");
        let object = json.as_object().expect("the request is an object");
        let mut keys: Vec<&str> = object.keys().map(String::as_str).collect();
        keys.sort_unstable();
        assert_eq!(keys, ["entry", "proposer", "request_id"]);
        let mut entry_keys: Vec<&str> = object["entry"]
            .as_object()
            .expect("the entry is an object")
            .keys()
            .map(String::as_str)
            .collect();
        entry_keys.sort_unstable();
        assert_eq!(entry_keys, ["engine", "model", "provider"]);
    }

    /// A ledger whose root participant has yielded the one slice it holds.
    fn yielded_proposer() -> CommitmentLedger {
        use crate::commitment::{
            AcceptOpen, Advertise, FundingSource, OfferPolicy, StartAttempt, StartInvocation,
            WakeCondition, YieldInvocation,
        };

        let mut ledger = CommitmentLedger::new(
            "participant-root",
            "anthropic",
            "obligation-root",
            BudgetVector::ZERO
                .with(Dimension::ParticipantStarts, 4)
                .with(Dimension::InvocationStarts, 4)
                .with(Dimension::AttemptStarts, 4)
                .with(Dimension::OfferCreations, 4)
                .with(Dimension::ObligationCreations, 4)
                .with(Dimension::WallTimeMs, 60_000),
        )
        .expect("a ledger");
        ledger
            .execute(&CommitmentCommand::Advertise(Advertise {
                offer_id: "offer-1".to_owned(),
                sponsor: "participant-root".to_owned(),
                parent_obligation: "obligation-root".to_owned(),
                funding_source: FundingSource::Participant,
                task_scope: "scope".to_owned(),
                base_digest: "1".repeat(64),
                intent_digest: "2".repeat(64),
                artifact_class: "class".to_owned(),
                dependencies: Vec::new(),
                capability_scope: Vec::new(),
                execution_escrow: BudgetVector::ZERO
                    .with(Dimension::WallTimeMs, 30_000)
                    .with(Dimension::InvocationStarts, 2)
                    .with(Dimension::AttemptStarts, 1),
                policy: OfferPolicy::OpenAccept,
                bid_deadline: 10_000,
                offer_deadline: 20_000,
                max_awards: 1,
            }))
            .expect("the offer is advertised");
        ledger
            .execute(&CommitmentCommand::AcceptOpen(AcceptOpen {
                contract_id: "contract-1".to_owned(),
                obligation_id: "obligation-1".to_owned(),
                lease_id: "lease-1".to_owned(),
                bid_id: "bid-1".to_owned(),
                offer_id: "offer-1".to_owned(),
                participant: "participant-root".to_owned(),
                requested_escrow: BudgetVector::ZERO
                    .with(Dimension::WallTimeMs, 30_000)
                    .with(Dimension::InvocationStarts, 2)
                    .with(Dimension::AttemptStarts, 1),
                artifact_class: "class".to_owned(),
                proposal_digest: None,
                lease_ms: 30_000,
            }))
            .expect("the open offer is accepted");
        ledger
            .execute(&CommitmentCommand::StartAttempt(StartAttempt {
                attempt_id: "attempt-1".to_owned(),
                contract_id: "contract-1".to_owned(),
                participant: "participant-root".to_owned(),
                generation: 1,
            }))
            .expect("the attempt starts");
        ledger
            .execute(&CommitmentCommand::StartInvocation(StartInvocation {
                invocation_id: "invocation-1".to_owned(),
                attempt_id: "attempt-1".to_owned(),
                contract_id: "contract-1".to_owned(),
                participant: "participant-root".to_owned(),
                generation: 1,
                cursor: 0,
            }))
            .expect("the slice starts");
        ledger
            .execute(&CommitmentCommand::YieldInvocation(YieldInvocation {
                invocation_id: "invocation-1".to_owned(),
                participant: "participant-root".to_owned(),
                generation: 1,
                cursor: ledger.sequence(),
                conditions: vec![WakeCondition::RunStopped],
                wake_deadline: 20_000,
            }))
            .expect("the slice yields");
        ledger
    }
}
