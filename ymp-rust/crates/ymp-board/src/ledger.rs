//! The board kernel: it decides whether a typed command is admissible, attributed and paid for,
//! and commits every fact of an accepted command together or none of them.
//!
//! Deciding is a pure function of the current board and the command. A refused command therefore
//! leaves the board byte for byte as it was, which is what makes a malformed or cross-scope message
//! a refusal and not a partial write.
//!
//! What this kernel decides is who published, to whom it was addressed, whether the addressee was
//! admitted, whether the author could pay, and in what order it all happened. What it never decides
//! is what any of it means. The inputs it reads are identifiers it compares for equality, integer
//! byte counts it subtracts, deadlines it compares against its clock, and digests of inert content
//! it stores without opening — there is no payload in this file, and therefore nothing here that a
//! payload could ask for.

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;

use crate::budget::{Allowance, CommunicationAllowance};
use crate::protocol::{
    AdvanceClock, BoardCommand, BoardError, BoardEvent, CommitAssessment, GrantAudience,
    LeaveAudience, NoteAncestry, NoteVerdict, OpenScope, Publish, ReadBoard, RecordIntervention,
    RefreshSalience, RegisterParticipant, RequestAudience, RevealContext,
};
use crate::records::{
    AccountRecord, AccountRef, Admissibility, AncestryRecord, AssessmentRecord, Audience,
    AudienceRequestRecord, DeliveryReceipt, GrantRecord, GrantState, InterventionRecord,
    MessageKind, MessageRecord, ReaderRecord, Reference, Relation, ReviewPolicy, ReviewerRecord,
    ReviewerState, Rights, RunKeepingAuthority, ScopeKind, ScopeRecord, VerdictRecord,
};
use crate::{
    BOARD_SCHEMA_VERSION, MAX_DECISION_BASIS, MAX_DELIVERY_BYTES, MAX_DISCOVERY_PAYLOAD_BYTES,
    MAX_DISCOVERY_REFERENCES, MAX_GRANT_MS, MAX_INITIAL_MEMBERS, MAX_PAYLOAD_BYTES, MAX_RECIPIENTS,
    MAX_REFERENCES, MAX_SALIENCE_MS, MAX_WITHHELD_SCOPES, validate_digest, validate_identifier,
};

/// A guarded check, named so that a deliberate removal can be exercised by the suite instead of
/// being asserted in prose.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Check {
    /// A detailed audience is reached only through an explicit grant.
    Admission,
    /// A grant whose expiry has arrived admits nothing.
    GrantExpiry,
    /// A project-discovery notice is bounded in payload and in references.
    DiscoveryBound,
    /// Publishing, refreshing, admitting and receiving are charged.
    Charge,
    /// Nothing on the board reaches a reviewer before its own assessment is durable.
    BlindingOrder,
    /// An account holds what a command is about to move out of it.
    Covering,
}

/// Checks a test build may switch off to prove that each one is load-bearing. The field does not
/// exist outside `cfg(test)`, so a release build has no way to reach a weakened kernel.
#[cfg(test)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct DisabledChecks {
    pub admission: bool,
    pub grant_expiry: bool,
    pub discovery_bound: bool,
    pub charge: bool,
    pub blinding_order: bool,
    pub covering: bool,
}

/// Deliberate readings of a payload, which a test build may switch on one at a time.
///
/// Each one is a board that read the bytes and did something because of what they said. They exist
/// so that the inertness suite has something to catch: a suite that only ever ran against a board
/// which cannot read a payload would prove that this board does not react, and not that the
/// assertions would notice one that did. The field does not exist outside `cfg(test)`, and the only
/// entry point that carries payload bytes at all is likewise test-only, so no release build has a
/// path from content to a fact.
#[cfg(test)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct ContentReactions {
    /// A payload holding a capability-shaped value admits its author to every open scope.
    pub admits_on_capability_text: bool,
    /// A payload holding a consent sentence returns the publication charge to its author.
    pub refunds_on_consent_text: bool,
    /// A payload naming a protected reference records a verdict for the result it names.
    pub notes_verdict_on_protected_reference: bool,
}

/// The scoped collaboration board: append-only messages, expiring audiences, delivery cursors,
/// finite communication allowances, and the observations mirrored in for reading.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct BoardLedger {
    now: u64,
    /// The trusted controller. It mirrors what the control plane recorded and opens the audiences
    /// that plane's work created; it is never a party to a conversation.
    controller: String,
    initial_total: CommunicationAllowance,
    consumed: CommunicationAllowance,
    participants: BTreeMap<String, AccountRecord>,
    scopes: BTreeMap<String, ScopeRecord>,
    requests: BTreeMap<String, AudienceRequestRecord>,
    grants: BTreeMap<String, GrantRecord>,
    /// Every message ever appended, in publication order. Nothing removes an entry and nothing
    /// rewrites one: what expires is the active projection, never the record.
    messages: Vec<MessageRecord>,
    message_index: BTreeMap<String, usize>,
    readers: BTreeMap<String, ReaderRecord>,
    receipts: Vec<DeliveryReceipt>,
    assessments: Vec<AssessmentRecord>,
    ancestry: BTreeMap<String, AncestryRecord>,
    verdicts: BTreeMap<String, VerdictRecord>,
    interventions: BTreeMap<String, InterventionRecord>,
    /// Every committed fact in the order it was committed.
    facts: Vec<BoardEvent>,
    #[cfg(test)]
    #[serde(skip)]
    disabled: DisabledChecks,
    #[cfg(test)]
    #[serde(skip)]
    reactions: ContentReactions,
}

/// One message as it was made available to a reader.
///
/// It carries the identity and the length of the payload and not the payload. Resolving those bytes
/// is the reader's own act against the content-addressed store, in a plane this one cannot reach.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct DeliveredMessage {
    pub sequence: u64,
    pub message_id: String,
    pub author: String,
    pub audience: Audience,
    pub kind: MessageKind,
    pub payload_digest: String,
    pub payload_bytes: u64,
    pub published_at: u64,
    pub salience_expires_at: u64,
    pub references: Vec<Reference>,
    pub relation: Relation,
}

/// What one bounded read made available, and how far it moved the reader's own cursor.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Delivery {
    pub reader: String,
    pub from_cursor: u64,
    pub to_cursor: u64,
    pub messages: Vec<DeliveredMessage>,
    pub bytes: u64,
}

impl BoardLedger {
    /// A board with one funded account, whose whole allowance is the total this plane will ever
    /// hold. Nothing mints communication capacity afterwards.
    pub fn new(
        controller: &str,
        root_participant: &str,
        endowment: CommunicationAllowance,
    ) -> Result<Self, BoardError> {
        validate_identifier("controller", controller)?;
        validate_identifier("participant_id", root_participant)?;
        let participants = BTreeMap::from([(
            root_participant.to_owned(),
            AccountRecord {
                participant_id: root_participant.to_owned(),
                balance: endowment,
            },
        )]);
        Ok(Self {
            now: 0,
            controller: controller.to_owned(),
            initial_total: endowment,
            consumed: CommunicationAllowance::ZERO,
            participants,
            scopes: BTreeMap::new(),
            requests: BTreeMap::new(),
            grants: BTreeMap::new(),
            messages: Vec::new(),
            message_index: BTreeMap::new(),
            readers: BTreeMap::new(),
            receipts: Vec::new(),
            assessments: Vec::new(),
            ancestry: BTreeMap::new(),
            verdicts: BTreeMap::new(),
            interventions: BTreeMap::new(),
            facts: Vec::new(),
            #[cfg(test)]
            disabled: DisabledChecks::default(),
            #[cfg(test)]
            reactions: ContentReactions::default(),
        })
    }

    pub const fn now(&self) -> u64 {
        self.now
    }

    pub fn controller(&self) -> &str {
        &self.controller
    }

    /// Every message ever appended, salient or not. This is the audit record, and it is the reason
    /// expiry costs nothing in evidence: a projection stops carrying a message, and the message
    /// stays exactly where it was published.
    pub fn audit(&self) -> &[MessageRecord] {
        &self.messages
    }

    pub fn message(&self, message_id: &str) -> Option<&MessageRecord> {
        self.message_index
            .get(message_id)
            .and_then(|index| self.messages.get(*index))
    }

    /// What a reader would see now: the messages it is admitted to whose salience has not expired.
    pub fn active_projection(&self, reader: &str) -> Vec<&MessageRecord> {
        self.messages
            .iter()
            .filter(|message| {
                message.is_salient_at(self.now) && self.read_refusal(reader, message).is_none()
            })
            .collect()
    }

    /// Whether this reader may be shown this message. It is the same question every delivery and
    /// every reference check is answered from, so a surface cannot show what a read would refuse.
    pub fn may_read(&self, reader: &str, message: &MessageRecord) -> bool {
        self.read_refusal(reader, message).is_none()
    }

    /// Whether this reader may be shown that somebody else committed this assessment.
    ///
    /// Its own is always its own. Another's is a prior vote, so it reaches a reviewer only once the
    /// reveal order has opened the board to it, and only where it is admitted to the review at all.
    pub fn may_read_assessment(&self, reader: &str, assessment: &AssessmentRecord) -> bool {
        if assessment.reviewer == reader {
            return true;
        }
        if self.blinded_in(reader).is_some() {
            return false;
        }
        self.live_rights(reader, &assessment.scope_id)
            .is_some_and(|rights| rights.read)
    }

    pub fn scope(&self, scope_id: &str) -> Option<&ScopeRecord> {
        self.scopes.get(scope_id)
    }

    pub fn scopes(&self) -> impl Iterator<Item = &ScopeRecord> {
        self.scopes.values()
    }

    pub fn grant(&self, grant_id: &str) -> Option<&GrantRecord> {
        self.grants.get(grant_id)
    }

    pub fn grants(&self) -> impl Iterator<Item = &GrantRecord> {
        self.grants.values()
    }

    pub fn request(&self, request_id: &str) -> Option<&AudienceRequestRecord> {
        self.requests.get(request_id)
    }

    pub fn allowance(&self, participant: &str) -> Option<CommunicationAllowance> {
        self.participants
            .get(participant)
            .map(|account| account.balance)
    }

    pub fn reader(&self, participant: &str) -> Option<&ReaderRecord> {
        self.readers.get(participant)
    }

    pub fn receipts(&self) -> &[DeliveryReceipt] {
        &self.receipts
    }

    pub fn assessments(&self) -> &[AssessmentRecord] {
        &self.assessments
    }

    /// The assessments that may stand in the primary comparison: those committed under blinding,
    /// before their author had anything else.
    pub fn primary_assessments(&self) -> Vec<&AssessmentRecord> {
        self.assessments
            .iter()
            .filter(|assessment| assessment.admissibility == Admissibility::Primary)
            .collect()
    }

    pub fn verdicts(&self) -> impl Iterator<Item = &VerdictRecord> {
        self.verdicts.values()
    }

    pub fn ancestry(&self) -> impl Iterator<Item = &AncestryRecord> {
        self.ancestry.values()
    }

    pub fn interventions(&self) -> impl Iterator<Item = &InterventionRecord> {
        self.interventions.values()
    }

    pub fn intervention(&self, intervention_id: &str) -> Option<&InterventionRecord> {
        self.interventions.get(intervention_id)
    }

    pub fn facts(&self) -> &[BoardEvent] {
        &self.facts
    }

    /// What this plane contributes to keeping a run alive.
    ///
    /// Nothing, and the return type is how that is stated rather than promised:
    /// [`RunKeepingAuthority`] has no variants, so `None` is the only value this signature admits.
    /// An unread message, an unrefreshed projection and a help request nobody answered are not
    /// funded control objects and cannot become one here.
    pub const fn run_keeping_authority(&self) -> Option<RunKeepingAuthority> {
        None
    }

    /// Whether every unit of communication capacity is still where it can be accounted for: held by
    /// an account, held by a live grant, or spent.
    pub fn conserves_allowance(&self) -> bool {
        let mut total = self.consumed;
        for account in self.participants.values() {
            match total.checked_add(&account.balance) {
                Ok(sum) => total = sum,
                Err(_) => return false,
            }
        }
        for grant in self.grants.values() {
            match total.checked_add(&grant.held) {
                Ok(sum) => total = sum,
                Err(_) => return false,
            }
        }
        total == self.initial_total
    }

    /// The whole board as committed data, which is what a refusal is compared against.
    pub fn snapshot(&self) -> String {
        serde_json::to_string(self).expect("a board is representable as committed data")
    }

    /// Decide a command without changing anything, then commit every fact it produced.
    pub fn execute(&mut self, command: &BoardCommand) -> Result<Vec<BoardEvent>, BoardError> {
        let events = self.decide(command)?;
        self.commit(events)
    }

    /// Commit one fact that was decided and recorded earlier, which is how a board is rebuilt from
    /// a durable record. Nothing here re-checks the admission, the deadlines or the funding that
    /// [`Self::decide`] established, because a committed fact is not a request.
    pub fn replay(&mut self, fact: &BoardEvent) -> Result<(), BoardError> {
        self.apply(fact)
    }

    fn commit(&mut self, events: Vec<BoardEvent>) -> Result<Vec<BoardEvent>, BoardError> {
        let mut committed = self.clone();
        for event in &events {
            committed.apply(event)?;
        }
        *self = committed;
        Ok(events)
    }

    /// Take delivery of what lies after this reader's cursor, and record the receipt.
    pub fn deliver(&mut self, command: &ReadBoard) -> Result<Delivery, BoardError> {
        let (delivery, events) = self.decide_read(command)?;
        self.commit(events)?;
        Ok(delivery)
    }

    /// The facts a command would commit, or the reason it may not.
    pub fn decide(&self, command: &BoardCommand) -> Result<Vec<BoardEvent>, BoardError> {
        match command {
            BoardCommand::RegisterParticipant(command) => self.decide_register(command),
            BoardCommand::OpenScope(command) => self.decide_open_scope(command),
            BoardCommand::RequestAudience(command) => self.decide_request_audience(command),
            BoardCommand::GrantAudience(command) => self.decide_grant_audience(command),
            BoardCommand::LeaveAudience(command) => self.decide_leave_audience(command),
            BoardCommand::Publish(command) => self.decide_publish(command),
            BoardCommand::RefreshSalience(command) => self.decide_refresh(command),
            BoardCommand::ReadBoard(command) => self.decide_read(command).map(|(_, events)| events),
            BoardCommand::AdvanceClock(command) => self.decide_advance_clock(command),
            BoardCommand::CommitAssessment(command) => self.decide_commit_assessment(command),
            BoardCommand::RevealContext(command) => self.decide_reveal_context(command),
            BoardCommand::NoteAncestry(command) => self.decide_note_ancestry(command),
            BoardCommand::NoteVerdict(command) => self.decide_note_verdict(command),
            BoardCommand::RecordIntervention(command) => self.decide_record_intervention(command),
        }
    }

    // --- admission -------------------------------------------------------------------------

    /// The scope, if any, whose reveal order currently withholds the whole board from this
    /// participant.
    ///
    /// A reviewer under a blinded policy is withheld from until the review policy has revealed
    /// context to it, which it may do only once the reviewer's own assessment is durable. The
    /// withholding is of the whole board rather than of the scopes the policy lists, because an
    /// agent review is a separate blinded attempt whose one job is that assessment: a reviewer that
    /// could still read everything else would be reading the producer's rationale in another
    /// audience.
    fn blinded_in(&self, participant: &str) -> Option<&ScopeRecord> {
        if self.check_disabled(Check::BlindingOrder) {
            return None;
        }
        self.scopes.values().find(|scope| {
            scope
                .review_policy
                .as_ref()
                .is_some_and(|policy| policy.blinded)
                && scope
                    .reviewers
                    .get(participant)
                    .is_some_and(|reviewer| reviewer.state != ReviewerState::Disclosed)
        })
    }

    /// Whether this participant holds a live grant on this scope, and what it permits.
    fn live_rights(&self, participant: &str, scope_id: &str) -> Option<Rights> {
        let expiry_enforced = !self.check_disabled(Check::GrantExpiry);
        self.grants
            .values()
            .filter(|grant| grant.participant == participant && grant.scope_id == scope_id)
            .filter(|grant| {
                if expiry_enforced {
                    grant.is_live_at(self.now)
                } else {
                    matches!(grant.state, GrantState::Live)
                }
            })
            .map(|grant| grant.rights)
            .reduce(|left, right| Rights {
                read: left.read || right.read,
                publish: left.publish || right.publish,
            })
    }

    /// Why this participant may not read this message, or `None` if it may.
    ///
    /// A reveal order is answered before an audience is, because a blinded reviewer holding a live
    /// grant is exactly the case the order exists for: the grant is real, and the board is still
    /// withheld until the reviewer's own assessment is durable.
    fn read_refusal(&self, reader: &str, message: &MessageRecord) -> Option<BoardError> {
        if let Some(scope) = self.blinded_in(reader) {
            return Some(BoardError::BoardWithheldByReviewOrder {
                scope_id: scope.scope_id.clone(),
                reviewer: reader.to_owned(),
            });
        }
        if message.author == reader {
            return None;
        }
        if !self.participants.contains_key(reader) {
            return Some(BoardError::Unknown {
                kind: "participant",
                id: reader.to_owned(),
            });
        }
        match &message.audience {
            Audience::ProjectDiscovery => None,
            Audience::Scope { scope_id } => self.scope_read_refusal(reader, scope_id),
            Audience::Named {
                scope_id,
                recipients,
            } => {
                if !recipients.iter().any(|named| named == reader) {
                    return Some(BoardError::NotAdmitted {
                        participant: reader.to_owned(),
                        scope_id: scope_id.clone(),
                    });
                }
                self.scope_read_refusal(reader, scope_id)
            }
        }
    }

    fn scope_read_refusal(&self, reader: &str, scope_id: &str) -> Option<BoardError> {
        if self.check_disabled(Check::Admission) {
            return None;
        }
        match self.live_rights(reader, scope_id) {
            Some(rights) if rights.read => None,
            _ => Some(BoardError::NotAdmitted {
                participant: reader.to_owned(),
                scope_id: scope_id.to_owned(),
            }),
        }
    }

    /// Whether this participant may append to this audience.
    fn ensure_may_publish(&self, author: &str, audience: &Audience) -> Result<(), BoardError> {
        match audience {
            Audience::ProjectDiscovery => Ok(()),
            Audience::Scope { scope_id }
            | Audience::Named {
                scope_id,
                recipients: _,
            } => {
                self.scope(scope_id).ok_or_else(|| BoardError::Unknown {
                    kind: "scope",
                    id: scope_id.clone(),
                })?;
                if self.check_disabled(Check::Admission) {
                    return Ok(());
                }
                match self.live_rights(author, scope_id) {
                    Some(rights) if rights.publish => Ok(()),
                    Some(_) => Err(BoardError::PublishNotPermitted {
                        participant: author.to_owned(),
                        scope_id: scope_id.clone(),
                    }),
                    None => Err(BoardError::NotAdmitted {
                        participant: author.to_owned(),
                        scope_id: scope_id.clone(),
                    }),
                }
            }
        }
    }

    fn ensure_may_read_message(&self, reader: &str, message_id: &str) -> Result<(), BoardError> {
        let message = self
            .message(message_id)
            .ok_or_else(|| BoardError::Unknown {
                kind: "message",
                id: message_id.to_owned(),
            })?;
        match self.read_refusal(reader, message) {
            None => Ok(()),
            Some(BoardError::BoardWithheldByReviewOrder { scope_id, reviewer }) => {
                Err(BoardError::BoardWithheldByReviewOrder { scope_id, reviewer })
            }
            Some(_) => Err(BoardError::ReferenceNotAdmitted {
                message_id: message_id.to_owned(),
            }),
        }
    }

    // --- accounting ------------------------------------------------------------------------

    fn participant(&self, participant_id: &str) -> Result<&AccountRecord, BoardError> {
        self.participants
            .get(participant_id)
            .ok_or_else(|| BoardError::Unknown {
                kind: "participant",
                id: participant_id.to_owned(),
            })
    }

    fn balance_of(&self, account: &AccountRef) -> Result<CommunicationAllowance, BoardError> {
        match account {
            AccountRef::Participant { participant_id } => {
                Ok(self.participant(participant_id)?.balance)
            }
            AccountRef::Grant { grant_id } => self
                .grants
                .get(grant_id)
                .map(|grant| grant.held)
                .ok_or_else(|| BoardError::Unknown {
                    kind: "grant",
                    id: grant_id.clone(),
                }),
        }
    }

    fn ensure_covers(
        &self,
        account: &AccountRef,
        required: &CommunicationAllowance,
    ) -> Result<(), BoardError> {
        if self.check_disabled(Check::Covering) {
            return Ok(());
        }
        let held = self.balance_of(account)?;
        match held.shortfall(required) {
            None => Ok(()),
            Some(allowance) => Err(BoardError::InsufficientAllowance {
                account: account_label(account),
                allowance,
            }),
        }
    }

    /// The facts that charge one account, or none at all when charging is switched off in a test
    /// build.
    fn charge(
        &self,
        participant: &str,
        amount: CommunicationAllowance,
    ) -> Result<Vec<BoardEvent>, BoardError> {
        if self.check_disabled(Check::Charge) || amount.is_zero() {
            return Ok(Vec::new());
        }
        let account = AccountRef::Participant {
            participant_id: participant.to_owned(),
        };
        self.ensure_covers(&account, &amount)?;
        Ok(vec![BoardEvent::AllowanceConsumed { account, amount }])
    }

    // --- transitions -----------------------------------------------------------------------

    fn decide_register(
        &self,
        command: &RegisterParticipant,
    ) -> Result<Vec<BoardEvent>, BoardError> {
        validate_identifier("participant_id", &command.participant_id)?;
        let sponsor = self.participant(&command.sponsor)?.participant_id.clone();
        if self.participants.contains_key(&command.participant_id) {
            return Err(BoardError::DuplicateIdentifier {
                kind: "participant",
                id: command.participant_id.clone(),
            });
        }
        let account = AccountRef::Participant {
            participant_id: sponsor,
        };
        self.ensure_covers(&account, &command.endowment)?;
        Ok(vec![
            BoardEvent::ParticipantRegistered {
                participant_id: command.participant_id.clone(),
            },
            BoardEvent::AllowanceTransferred {
                from: account,
                to: AccountRef::Participant {
                    participant_id: command.participant_id.clone(),
                },
                amount: command.endowment,
            },
        ])
    }

    fn decide_open_scope(&self, command: &OpenScope) -> Result<Vec<BoardEvent>, BoardError> {
        self.ensure_controller(&command.controller)?;
        validate_identifier("scope_id", &command.scope_id)?;
        if self.scopes.contains_key(&command.scope_id) {
            return Err(BoardError::DuplicateIdentifier {
                kind: "scope",
                id: command.scope_id.clone(),
            });
        }
        self.participant(&command.sponsor)?;
        if let Some(policy) = &command.review_policy {
            self.validate_review_policy(&command.scope_id, command.kind, policy)?;
        }
        if command.initial_members.len() > MAX_INITIAL_MEMBERS {
            return Err(BoardError::TooManyEntries {
                kind: "initial members",
                limit: MAX_INITIAL_MEMBERS,
            });
        }
        self.validate_grant_window(command.member_expires_at)?;
        let mut seen = BTreeSet::new();
        let mut events = vec![BoardEvent::ScopeOpened {
            scope_id: command.scope_id.clone(),
            kind: command.kind,
            sponsor: command.sponsor.clone(),
            review_policy: command.review_policy.clone(),
        }];
        for member in &command.initial_members {
            validate_identifier("grant_id", &member.grant_id)?;
            if self.grants.contains_key(&member.grant_id) || !seen.insert(&member.grant_id) {
                return Err(BoardError::DuplicateIdentifier {
                    kind: "grant",
                    id: member.grant_id.clone(),
                });
            }
            self.participant(&member.participant)?;
            events.extend(self.admission_facts(
                &member.grant_id,
                &command.scope_id,
                &command.sponsor,
                &member.participant,
                member.rights,
                command.member_expires_at,
            )?);
        }
        self.ensure_membership_funded(&command.sponsor, command.initial_members.len())?;
        Ok(events)
    }

    fn validate_review_policy(
        &self,
        scope_id: &str,
        kind: ScopeKind,
        policy: &ReviewPolicy,
    ) -> Result<(), BoardError> {
        if kind != ScopeKind::CandidateReview {
            return Err(BoardError::NotUnderReview {
                scope_id: scope_id.to_owned(),
            });
        }
        validate_digest("candidate_digest", &policy.candidate_digest)?;
        if policy.withheld_scopes.len() > MAX_WITHHELD_SCOPES {
            return Err(BoardError::TooManyEntries {
                kind: "withheld scopes",
                limit: MAX_WITHHELD_SCOPES,
            });
        }
        for withheld in &policy.withheld_scopes {
            validate_identifier("scope_id", withheld)?;
            if withheld == scope_id {
                return Err(BoardError::InvalidReviewPolicy {
                    scope_id: scope_id.to_owned(),
                });
            }
        }
        Ok(())
    }

    /// The facts one admission commits: the grant itself, the irreversible membership authority it
    /// spends, and the concurrent membership the grant holds until it ends.
    fn admission_facts(
        &self,
        grant_id: &str,
        scope_id: &str,
        sponsor: &str,
        participant: &str,
        rights: Rights,
        expires_at: u64,
    ) -> Result<Vec<BoardEvent>, BoardError> {
        let mut events = vec![BoardEvent::AudienceGranted {
            grant_id: grant_id.to_owned(),
            scope_id: scope_id.to_owned(),
            participant: participant.to_owned(),
            rights,
            granted_by: sponsor.to_owned(),
            expires_at,
        }];
        if self.check_disabled(Check::Charge) {
            return Ok(events);
        }
        events.push(BoardEvent::AllowanceConsumed {
            account: AccountRef::Participant {
                participant_id: sponsor.to_owned(),
            },
            amount: CommunicationAllowance::unit(Allowance::MembershipGrants),
        });
        events.push(BoardEvent::AllowanceTransferred {
            from: AccountRef::Participant {
                participant_id: sponsor.to_owned(),
            },
            to: AccountRef::Grant {
                grant_id: grant_id.to_owned(),
            },
            amount: CommunicationAllowance::unit(Allowance::ActiveMemberships),
        });
        Ok(events)
    }

    /// Whether one sponsor holds the membership authority and the concurrent memberships that a
    /// number of admissions commits at once.
    fn ensure_membership_funded(&self, sponsor: &str, admissions: usize) -> Result<(), BoardError> {
        if self.check_disabled(Check::Charge) {
            return Ok(());
        }
        let count = admissions as u64;
        let required = CommunicationAllowance::units(Allowance::MembershipGrants, count)
            .with(Allowance::ActiveMemberships, count);
        self.ensure_covers(
            &AccountRef::Participant {
                participant_id: sponsor.to_owned(),
            },
            &required,
        )
    }

    fn decide_request_audience(
        &self,
        command: &RequestAudience,
    ) -> Result<Vec<BoardEvent>, BoardError> {
        validate_identifier("request_id", &command.request_id)?;
        self.participant(&command.participant)?;
        self.scope(&command.scope_id)
            .ok_or_else(|| BoardError::Unknown {
                kind: "scope",
                id: command.scope_id.clone(),
            })?;
        if self.requests.contains_key(&command.request_id) {
            return Err(BoardError::DuplicateIdentifier {
                kind: "audience request",
                id: command.request_id.clone(),
            });
        }
        let mut events = self.charge(
            &command.participant,
            CommunicationAllowance::unit(Allowance::Publications),
        )?;
        events.push(BoardEvent::AudienceRequested {
            request_id: command.request_id.clone(),
            scope_id: command.scope_id.clone(),
            participant: command.participant.clone(),
        });
        Ok(events)
    }

    fn decide_grant_audience(
        &self,
        command: &GrantAudience,
    ) -> Result<Vec<BoardEvent>, BoardError> {
        validate_identifier("grant_id", &command.grant_id)?;
        let scope = self
            .scope(&command.scope_id)
            .ok_or_else(|| BoardError::Unknown {
                kind: "scope",
                id: command.scope_id.clone(),
            })?;
        if scope.sponsor != command.sponsor {
            return Err(BoardError::NotAuthorized {
                participant: command.sponsor.clone(),
            });
        }
        self.participant(&command.participant)?;
        if self.grants.contains_key(&command.grant_id) {
            return Err(BoardError::DuplicateIdentifier {
                kind: "grant",
                id: command.grant_id.clone(),
            });
        }
        self.validate_grant_window(command.expires_at)?;
        self.ensure_membership_funded(&command.sponsor, 1)?;
        self.admission_facts(
            &command.grant_id,
            &command.scope_id,
            &command.sponsor,
            &command.participant,
            command.rights,
            command.expires_at,
        )
    }

    fn validate_grant_window(&self, expires_at: u64) -> Result<(), BoardError> {
        if expires_at <= self.now || expires_at.saturating_sub(self.now) > MAX_GRANT_MS {
            return Err(BoardError::InvalidGrantWindow);
        }
        Ok(())
    }

    fn decide_leave_audience(
        &self,
        command: &LeaveAudience,
    ) -> Result<Vec<BoardEvent>, BoardError> {
        let grant = self
            .grants
            .get(&command.grant_id)
            .ok_or_else(|| BoardError::Unknown {
                kind: "grant",
                id: command.grant_id.clone(),
            })?;
        if grant.participant != command.participant {
            return Err(BoardError::NotAuthorized {
                participant: command.participant.clone(),
            });
        }
        if !matches!(grant.state, GrantState::Live) {
            return Err(BoardError::GrantNotLive {
                grant_id: command.grant_id.clone(),
            });
        }
        Ok(release_facts(grant, false))
    }

    fn decide_publish(&self, command: &Publish) -> Result<Vec<BoardEvent>, BoardError> {
        validate_identifier("message_id", &command.message_id)?;
        validate_digest("payload_digest", command.payload.digest())?;
        self.participant(&command.author)?;
        if self.message_index.contains_key(&command.message_id) {
            return Err(BoardError::DuplicateIdentifier {
                kind: "message",
                id: command.message_id.clone(),
            });
        }
        if matches!(command.relation, Relation::Refreshes { .. }) {
            return Err(BoardError::RefreshIsNotPublication);
        }
        if command.salience_ms == 0 || command.salience_ms > MAX_SALIENCE_MS {
            return Err(BoardError::InvalidSalience);
        }
        self.validate_audience_bounds(&command.audience, &command.payload, &command.references)?;
        self.ensure_may_publish(&command.author, &command.audience)?;
        self.validate_recipients(&command.audience)?;
        self.validate_references(&command.author, &command.references)?;
        self.validate_relation(command)?;
        self.validate_decision_basis(command)?;

        let mut events = self.charge(
            &command.author,
            CommunicationAllowance::unit(Allowance::Publications)
                .with(Allowance::PublishedBytes, command.payload.bytes()),
        )?;
        events.push(BoardEvent::MessagePublished {
            message_id: command.message_id.clone(),
            author: command.author.clone(),
            audience: command.audience.clone(),
            kind: command.kind,
            payload_digest: command.payload.digest().to_owned(),
            payload_bytes: command.payload.bytes(),
            published_at: self.now,
            salience_expires_at: self.now.saturating_add(command.salience_ms),
            references: command.references.clone(),
            relation: command.relation.clone(),
            claimed_decision_basis: command.claimed_decision_basis.clone(),
        });
        Ok(events)
    }

    /// What one audience admits at all: discovery announces that work or help exists, and a
    /// detailed audience carries a finding.
    fn validate_audience_bounds(
        &self,
        audience: &Audience,
        payload: &crate::Payload,
        references: &[Reference],
    ) -> Result<(), BoardError> {
        let discovery = audience.is_discovery() && !self.check_disabled(Check::DiscoveryBound);
        let (payload_limit, reference_limit) = if discovery {
            (MAX_DISCOVERY_PAYLOAD_BYTES, MAX_DISCOVERY_REFERENCES)
        } else {
            (MAX_PAYLOAD_BYTES, MAX_REFERENCES)
        };
        if payload.bytes() > payload_limit {
            return Err(BoardError::PayloadTooLarge {
                bytes: payload.bytes(),
                limit: payload_limit,
            });
        }
        if references.len() > reference_limit {
            return Err(BoardError::TooManyEntries {
                kind: "references",
                limit: reference_limit,
            });
        }
        Ok(())
    }

    fn validate_recipients(&self, audience: &Audience) -> Result<(), BoardError> {
        let Audience::Named {
            scope_id,
            recipients,
        } = audience
        else {
            return Ok(());
        };
        if recipients.is_empty() || recipients.len() > MAX_RECIPIENTS {
            return Err(BoardError::TooManyEntries {
                kind: "recipients",
                limit: MAX_RECIPIENTS,
            });
        }
        if self.check_disabled(Check::Admission) {
            return Ok(());
        }
        for recipient in recipients {
            let admitted = self
                .live_rights(recipient, scope_id)
                .is_some_and(|rights| rights.read);
            if !admitted {
                return Err(BoardError::RecipientNotAdmitted {
                    participant: recipient.clone(),
                    scope_id: scope_id.clone(),
                });
            }
        }
        Ok(())
    }

    fn validate_references(
        &self,
        author: &str,
        references: &[Reference],
    ) -> Result<(), BoardError> {
        for reference in references {
            match reference {
                Reference::Message { message_id } => {
                    self.ensure_may_read_message(author, message_id)?;
                }
                Reference::Artifact { object_digest } => {
                    validate_digest("object_digest", object_digest)?;
                }
                Reference::Candidate { candidate_digest } => {
                    validate_digest("candidate_digest", candidate_digest)?;
                }
                Reference::ControlRecord { record_id } => {
                    validate_identifier("record_id", record_id)?;
                }
            }
        }
        Ok(())
    }

    /// A relation stands to a message the author may read, in the audience this message is
    /// addressed to. A challenge names what it challenges and only a challenge does, and a revision
    /// is of the author's own earlier position.
    fn validate_relation(&self, command: &Publish) -> Result<(), BoardError> {
        let is_challenge_relation = matches!(command.relation, Relation::Challenges { .. });
        if is_challenge_relation != (command.kind == MessageKind::Challenge) {
            return Err(BoardError::ChallengeRelationMismatch);
        }
        let Some(target_id) = command.relation.target() else {
            return Ok(());
        };
        self.ensure_may_read_message(&command.author, target_id)?;
        let target = self.message(target_id).ok_or_else(|| BoardError::Unknown {
            kind: "message",
            id: target_id.to_owned(),
        })?;
        if target.audience.scope_id() != command.audience.scope_id() {
            return Err(BoardError::RelationOutsideAudience {
                message_id: target_id.to_owned(),
            });
        }
        if matches!(command.relation, Relation::Revises { .. }) && target.author != command.author {
            return Err(BoardError::RevisionOfAnother {
                message_id: target_id.to_owned(),
                author: target.author.clone(),
            });
        }
        Ok(())
    }

    fn validate_decision_basis(&self, command: &Publish) -> Result<(), BoardError> {
        if command.claimed_decision_basis.is_empty() {
            return Ok(());
        }
        if command.kind != MessageKind::Decision {
            return Err(BoardError::DecisionBasisOnOtherKind);
        }
        if command.claimed_decision_basis.len() > MAX_DECISION_BASIS {
            return Err(BoardError::TooManyEntries {
                kind: "claimed decision basis",
                limit: MAX_DECISION_BASIS,
            });
        }
        for cited in &command.claimed_decision_basis {
            self.ensure_may_read_message(&command.author, cited)?;
        }
        Ok(())
    }

    fn decide_refresh(&self, command: &RefreshSalience) -> Result<Vec<BoardEvent>, BoardError> {
        validate_identifier("message_id", &command.message_id)?;
        self.participant(&command.author)?;
        if self.message_index.contains_key(&command.message_id) {
            return Err(BoardError::DuplicateIdentifier {
                kind: "message",
                id: command.message_id.clone(),
            });
        }
        if command.salience_ms == 0 || command.salience_ms > MAX_SALIENCE_MS {
            return Err(BoardError::InvalidSalience);
        }
        self.ensure_may_read_message(&command.author, &command.refreshes)?;
        let refreshed = self
            .message(&command.refreshes)
            .ok_or_else(|| BoardError::Unknown {
                kind: "message",
                id: command.refreshes.clone(),
            })?
            .clone();
        self.ensure_may_publish(&command.author, &refreshed.audience)?;

        let mut events = self.charge(
            &command.author,
            CommunicationAllowance::unit(Allowance::Publications)
                .with(Allowance::SalienceRefreshes, 1)
                .with(Allowance::PublishedBytes, refreshed.payload_bytes),
        )?;
        events.push(BoardEvent::MessagePublished {
            message_id: command.message_id.clone(),
            author: command.author.clone(),
            audience: refreshed.audience.clone(),
            kind: refreshed.kind,
            payload_digest: refreshed.payload_digest.clone(),
            payload_bytes: refreshed.payload_bytes,
            published_at: self.now,
            salience_expires_at: self.now.saturating_add(command.salience_ms),
            references: Vec::new(),
            relation: Relation::Refreshes {
                message_id: command.refreshes.clone(),
            },
            claimed_decision_basis: Vec::new(),
        });
        Ok(events)
    }

    /// What one bounded read makes available, and the facts that record it.
    ///
    /// The cursor moves over everything examined and not only over what was delivered, so a message
    /// this reader is not admitted to is passed rather than queued forever. What stops the read is
    /// the byte bound the caller asked for or the bytes the reader can still pay for, whichever
    /// arrives first.
    fn decide_read(&self, command: &ReadBoard) -> Result<(Delivery, Vec<BoardEvent>), BoardError> {
        self.participant(&command.reader)?;
        if command.limit_bytes == 0 || command.limit_bytes > MAX_DELIVERY_BYTES {
            return Err(BoardError::InvalidDeliveryLimit);
        }
        if let Some(scope) = self.blinded_in(&command.reader) {
            return Err(BoardError::BoardWithheldByReviewOrder {
                scope_id: scope.scope_id.clone(),
                reviewer: command.reader.clone(),
            });
        }
        let from_cursor = self
            .readers
            .get(&command.reader)
            .map_or(0, |reader| reader.cursor);
        let affordable = if self.check_disabled(Check::Charge) {
            u64::MAX
        } else {
            self.participant(&command.reader)?
                .balance
                .get(Allowance::DeliveredBytes)
        };

        let mut delivered = Vec::new();
        let mut bytes = 0_u64;
        let mut cursor = from_cursor;
        for message in self.messages.iter().skip(from_cursor as usize) {
            let readable = message.is_salient_at(self.now)
                && self.read_refusal(&command.reader, message).is_none();
            if readable {
                let next = bytes.saturating_add(message.payload_bytes);
                if next > command.limit_bytes || next > affordable {
                    break;
                }
                bytes = next;
                delivered.push(deliver(message));
            }
            cursor = message.sequence;
        }

        let delivery = Delivery {
            reader: command.reader.clone(),
            from_cursor,
            to_cursor: cursor,
            messages: delivered,
            bytes,
        };
        if cursor == from_cursor {
            return Ok((delivery, Vec::new()));
        }
        let mut events = self.charge(
            &command.reader,
            CommunicationAllowance::units(Allowance::DeliveredBytes, bytes),
        )?;
        events.push(BoardEvent::DeliveryRecorded {
            reader: command.reader.clone(),
            from_cursor,
            to_cursor: cursor,
            message_ids: delivery
                .messages
                .iter()
                .map(|message| message.message_id.clone())
                .collect(),
            bytes,
        });
        Ok((delivery, events))
    }

    fn decide_advance_clock(&self, command: &AdvanceClock) -> Result<Vec<BoardEvent>, BoardError> {
        if command.to < self.now {
            return Err(BoardError::ClockRegression {
                now: self.now,
                to: command.to,
            });
        }
        let mut events = vec![BoardEvent::ClockAdvanced { to: command.to }];
        if self.check_disabled(Check::GrantExpiry) {
            return Ok(events);
        }
        // Expiry is swept here as well as compared against on every read, because a grant that has
        // ended must also give back the concurrent membership it was holding. The two together are
        // the one check: reading against the clock alone would leave the capacity reserved forever.
        for grant in self.grants.values() {
            if matches!(grant.state, GrantState::Live) && grant.expires_at <= command.to {
                events.extend(release_facts(grant, true));
            }
        }
        Ok(events)
    }

    fn decide_commit_assessment(
        &self,
        command: &CommitAssessment,
    ) -> Result<Vec<BoardEvent>, BoardError> {
        validate_identifier("assessment_id", &command.assessment_id)?;
        validate_digest("assessment_digest", &command.assessment_digest)?;
        self.participant(&command.reviewer)?;
        let scope = self
            .scope(&command.scope_id)
            .ok_or_else(|| BoardError::Unknown {
                kind: "scope",
                id: command.scope_id.clone(),
            })?;
        let policy = match (&scope.kind, &scope.review_policy) {
            (ScopeKind::CandidateReview, Some(policy)) => policy,
            _ => {
                return Err(BoardError::NotUnderReview {
                    scope_id: command.scope_id.clone(),
                });
            }
        };
        if self
            .assessments
            .iter()
            .any(|assessment| assessment.assessment_id == command.assessment_id)
        {
            return Err(BoardError::DuplicateIdentifier {
                kind: "assessment",
                id: command.assessment_id.clone(),
            });
        }
        let admitted = self
            .live_rights(&command.reviewer, &command.scope_id)
            .is_some_and(|rights| rights.read);
        if !admitted && !self.check_disabled(Check::Admission) {
            return Err(BoardError::NotAdmitted {
                participant: command.reviewer.clone(),
                scope_id: command.scope_id.clone(),
            });
        }
        let first_under_blinding = policy.blinded
            && scope
                .reviewers
                .get(&command.reviewer)
                .is_some_and(|reviewer| {
                    reviewer.state == ReviewerState::Blinded && reviewer.assessments.is_empty()
                });
        let admissibility = if first_under_blinding {
            Admissibility::Primary
        } else {
            Admissibility::Secondary
        };
        let mut events = self.charge(
            &command.reviewer,
            CommunicationAllowance::unit(Allowance::Publications),
        )?;
        events.push(BoardEvent::AssessmentCommitted {
            assessment_id: command.assessment_id.clone(),
            scope_id: command.scope_id.clone(),
            reviewer: command.reviewer.clone(),
            assessment_digest: command.assessment_digest.clone(),
            admissibility,
        });
        Ok(events)
    }

    fn decide_reveal_context(
        &self,
        command: &RevealContext,
    ) -> Result<Vec<BoardEvent>, BoardError> {
        let scope = self
            .scope(&command.scope_id)
            .ok_or_else(|| BoardError::Unknown {
                kind: "scope",
                id: command.scope_id.clone(),
            })?;
        let blinded_policy = scope
            .review_policy
            .as_ref()
            .is_some_and(|policy| policy.blinded);
        if !blinded_policy {
            return Err(BoardError::ReviewNotBlinded {
                scope_id: command.scope_id.clone(),
            });
        }
        let reviewer =
            scope
                .reviewers
                .get(&command.reviewer)
                .ok_or_else(|| BoardError::NotAdmitted {
                    participant: command.reviewer.clone(),
                    scope_id: command.scope_id.clone(),
                })?;
        if reviewer.state == ReviewerState::Blinded && !self.check_disabled(Check::BlindingOrder) {
            return Err(BoardError::BoardWithheldByReviewOrder {
                scope_id: command.scope_id.clone(),
                reviewer: command.reviewer.clone(),
            });
        }
        Ok(vec![BoardEvent::ContextRevealed {
            scope_id: command.scope_id.clone(),
            reviewer: command.reviewer.clone(),
        }])
    }

    fn ensure_controller(&self, controller: &str) -> Result<(), BoardError> {
        if controller == self.controller {
            Ok(())
        } else {
            Err(BoardError::NotAuthorized {
                participant: controller.to_owned(),
            })
        }
    }

    fn decide_note_ancestry(&self, command: &NoteAncestry) -> Result<Vec<BoardEvent>, BoardError> {
        self.ensure_controller(&command.controller)?;
        validate_digest("candidate_digest", &command.candidate_digest)?;
        if command.parents.len() > MAX_REFERENCES {
            return Err(BoardError::TooManyEntries {
                kind: "parents",
                limit: MAX_REFERENCES,
            });
        }
        for parent in &command.parents {
            validate_digest("parent_digest", parent)?;
        }
        Ok(vec![BoardEvent::AncestryNoted {
            candidate_digest: command.candidate_digest.clone(),
            parents: command.parents.clone(),
        }])
    }

    fn decide_note_verdict(&self, command: &NoteVerdict) -> Result<Vec<BoardEvent>, BoardError> {
        self.ensure_controller(&command.controller)?;
        validate_digest("candidate_digest", &command.candidate_digest)?;
        Ok(vec![BoardEvent::VerdictNoted {
            candidate_digest: command.candidate_digest.clone(),
            verdict: command.verdict,
        }])
    }

    fn decide_record_intervention(
        &self,
        command: &RecordIntervention,
    ) -> Result<Vec<BoardEvent>, BoardError> {
        self.ensure_controller(&command.controller)?;
        validate_identifier("intervention_id", &command.intervention_id)?;
        if self.interventions.contains_key(&command.intervention_id) {
            return Err(BoardError::DuplicateIdentifier {
                kind: "intervention",
                id: command.intervention_id.clone(),
            });
        }
        if self.message(&command.subject_message).is_none() {
            return Err(BoardError::Unknown {
                kind: "message",
                id: command.subject_message.clone(),
            });
        }
        self.participant(&command.receiver)?;
        Ok(vec![BoardEvent::InterventionRecorded {
            intervention_id: command.intervention_id.clone(),
            kind: command.kind,
            subject_message: command.subject_message.clone(),
            receiver: command.receiver.clone(),
            replications: command.replications,
            matched_budget: command.matched_budget,
        }])
    }

    // --- committing ------------------------------------------------------------------------

    fn apply(&mut self, event: &BoardEvent) -> Result<(), BoardError> {
        match event {
            BoardEvent::ClockAdvanced { to } => self.now = *to,
            BoardEvent::ParticipantRegistered { participant_id } => {
                self.participants.insert(
                    participant_id.clone(),
                    AccountRecord {
                        participant_id: participant_id.clone(),
                        balance: CommunicationAllowance::ZERO,
                    },
                );
            }
            BoardEvent::AllowanceTransferred { from, to, amount } => {
                self.debit(from, amount)?;
                self.credit(to, amount)?;
            }
            BoardEvent::AllowanceConsumed { account, amount } => {
                self.debit(account, amount)?;
                self.consumed = self
                    .consumed
                    .checked_add(amount)
                    .map_err(|allowance| BoardError::AllowanceOverflow { allowance })?;
            }
            BoardEvent::ScopeOpened {
                scope_id,
                kind,
                sponsor,
                review_policy,
            } => {
                self.scopes.insert(
                    scope_id.clone(),
                    ScopeRecord {
                        scope_id: scope_id.clone(),
                        kind: *kind,
                        sponsor: sponsor.clone(),
                        opened_at: self.now,
                        review_policy: review_policy.clone(),
                        reviewers: BTreeMap::new(),
                    },
                );
            }
            BoardEvent::AudienceRequested {
                request_id,
                scope_id,
                participant,
            } => {
                self.requests.insert(
                    request_id.clone(),
                    AudienceRequestRecord {
                        request_id: request_id.clone(),
                        scope_id: scope_id.clone(),
                        participant: participant.clone(),
                        requested_at: self.now,
                    },
                );
            }
            BoardEvent::AudienceGranted {
                grant_id,
                scope_id,
                participant,
                rights,
                granted_by,
                expires_at,
            } => {
                self.grants.insert(
                    grant_id.clone(),
                    GrantRecord {
                        grant_id: grant_id.clone(),
                        scope_id: scope_id.clone(),
                        participant: participant.clone(),
                        rights: *rights,
                        granted_by: granted_by.clone(),
                        expires_at: *expires_at,
                        state: GrantState::Live,
                        held: CommunicationAllowance::ZERO,
                    },
                );
                self.enrol_reviewer(scope_id, participant, *rights);
            }
            BoardEvent::AudienceReleased { grant_id } => {
                if let Some(grant) = self.grants.get_mut(grant_id) {
                    grant.state = GrantState::Released;
                }
            }
            BoardEvent::AudienceExpired { grant_id } => {
                if let Some(grant) = self.grants.get_mut(grant_id) {
                    grant.state = GrantState::Expired;
                }
            }
            BoardEvent::MessagePublished {
                message_id,
                author,
                audience,
                kind,
                payload_digest,
                payload_bytes,
                published_at,
                salience_expires_at,
                references,
                relation,
                claimed_decision_basis,
            } => {
                let sequence = self.messages.len() as u64 + 1;
                self.message_index
                    .insert(message_id.clone(), self.messages.len());
                self.messages.push(MessageRecord {
                    schema_version: BOARD_SCHEMA_VERSION,
                    sequence,
                    message_id: message_id.clone(),
                    author: author.clone(),
                    audience: audience.clone(),
                    kind: *kind,
                    payload_digest: payload_digest.clone(),
                    payload_bytes: *payload_bytes,
                    published_at: *published_at,
                    salience_expires_at: *salience_expires_at,
                    references: references.clone(),
                    relation: relation.clone(),
                    claimed_decision_basis: claimed_decision_basis.clone(),
                });
            }
            BoardEvent::DeliveryRecorded {
                reader,
                from_cursor,
                to_cursor,
                message_ids,
                bytes,
            } => {
                let record = self.readers.entry(reader.clone()).or_default();
                record.cursor = *to_cursor;
                record.delivered_bytes = record.delivered_bytes.saturating_add(*bytes);
                self.receipts.push(DeliveryReceipt {
                    reader: reader.clone(),
                    from_cursor: *from_cursor,
                    to_cursor: *to_cursor,
                    message_ids: message_ids.clone(),
                    bytes: *bytes,
                    delivered_at: self.now,
                });
            }
            BoardEvent::AssessmentCommitted {
                assessment_id,
                scope_id,
                reviewer,
                assessment_digest,
                admissibility,
            } => {
                let sequence = self.assessments.len() as u64 + 1;
                self.assessments.push(AssessmentRecord {
                    assessment_id: assessment_id.clone(),
                    scope_id: scope_id.clone(),
                    reviewer: reviewer.clone(),
                    assessment_digest: assessment_digest.clone(),
                    admissibility: *admissibility,
                    committed_at: self.now,
                    sequence,
                });
                if let Some(scope) = self.scopes.get_mut(scope_id) {
                    let record =
                        scope
                            .reviewers
                            .entry(reviewer.clone())
                            .or_insert_with(|| ReviewerRecord {
                                participant: reviewer.clone(),
                                state: ReviewerState::Disclosed,
                                assessments: Vec::new(),
                            });
                    record.assessments.push(assessment_id.clone());
                    if record.state == ReviewerState::Blinded {
                        record.state = ReviewerState::Committed;
                    }
                }
            }
            BoardEvent::ContextRevealed { scope_id, reviewer } => {
                if let Some(scope) = self.scopes.get_mut(scope_id)
                    && let Some(record) = scope.reviewers.get_mut(reviewer)
                {
                    record.state = ReviewerState::Disclosed;
                }
            }
            BoardEvent::AncestryNoted {
                candidate_digest,
                parents,
            } => {
                self.ancestry.insert(
                    candidate_digest.clone(),
                    AncestryRecord {
                        candidate_digest: candidate_digest.clone(),
                        parents: parents.clone(),
                        observed_at: self.now,
                    },
                );
            }
            BoardEvent::VerdictNoted {
                candidate_digest,
                verdict,
            } => {
                self.verdicts.insert(
                    candidate_digest.clone(),
                    VerdictRecord {
                        candidate_digest: candidate_digest.clone(),
                        verdict: *verdict,
                        observed_at: self.now,
                    },
                );
            }
            BoardEvent::InterventionRecorded {
                intervention_id,
                kind,
                subject_message,
                receiver,
                replications,
                matched_budget,
            } => {
                self.interventions.insert(
                    intervention_id.clone(),
                    InterventionRecord {
                        intervention_id: intervention_id.clone(),
                        kind: *kind,
                        subject_message: subject_message.clone(),
                        receiver: receiver.clone(),
                        replications: *replications,
                        matched_budget: *matched_budget,
                        recorded_at: self.now,
                    },
                );
            }
        }
        self.facts.push(event.clone());
        Ok(())
    }

    /// A participant admitted to read a scope whose policy blinds it starts the reveal order at its
    /// beginning. The scope sponsor organizes the review and is not one of its reviewers.
    fn enrol_reviewer(&mut self, scope_id: &str, participant: &str, rights: Rights) {
        let Some(scope) = self.scopes.get_mut(scope_id) else {
            return;
        };
        let blinded = scope
            .review_policy
            .as_ref()
            .is_some_and(|policy| policy.blinded);
        if !blinded || !rights.read || scope.sponsor == participant {
            return;
        }
        scope
            .reviewers
            .entry(participant.to_owned())
            .or_insert_with(|| ReviewerRecord {
                participant: participant.to_owned(),
                state: ReviewerState::Blinded,
                assessments: Vec::new(),
            });
    }

    fn credit(
        &mut self,
        account: &AccountRef,
        amount: &CommunicationAllowance,
    ) -> Result<(), BoardError> {
        let held = self.balance_mut(account)?;
        *held = held
            .checked_add(amount)
            .map_err(|allowance| BoardError::AllowanceOverflow { allowance })?;
        Ok(())
    }

    fn debit(
        &mut self,
        account: &AccountRef,
        amount: &CommunicationAllowance,
    ) -> Result<(), BoardError> {
        let label = account_label(account);
        let held = self.balance_mut(account)?;
        *held = held
            .checked_sub(amount)
            .map_err(|allowance| BoardError::UncoveredDebit {
                account: label,
                allowance,
            })?;
        Ok(())
    }

    fn balance_mut(
        &mut self,
        account: &AccountRef,
    ) -> Result<&mut CommunicationAllowance, BoardError> {
        match account {
            AccountRef::Participant { participant_id } => self
                .participants
                .get_mut(participant_id)
                .map(|account| &mut account.balance)
                .ok_or_else(|| BoardError::Unknown {
                    kind: "participant",
                    id: participant_id.clone(),
                }),
            AccountRef::Grant { grant_id } => self
                .grants
                .get_mut(grant_id)
                .map(|grant| &mut grant.held)
                .ok_or_else(|| BoardError::Unknown {
                    kind: "grant",
                    id: grant_id.clone(),
                }),
        }
    }

    // --- test-only weakenings ---------------------------------------------------------------

    #[cfg(not(test))]
    #[allow(clippy::unused_self)]
    const fn check_disabled(&self, _check: Check) -> bool {
        false
    }

    #[cfg(test)]
    const fn check_disabled(&self, check: Check) -> bool {
        match check {
            Check::Admission => self.disabled.admission,
            Check::GrantExpiry => self.disabled.grant_expiry,
            Check::DiscoveryBound => self.disabled.discovery_bound,
            Check::Charge => self.disabled.charge,
            Check::BlindingOrder => self.disabled.blinding_order,
            Check::Covering => self.disabled.covering,
        }
    }

    #[cfg(test)]
    pub(crate) fn disable_checks(&mut self, disabled: DisabledChecks) {
        self.disabled = disabled;
    }

    #[cfg(test)]
    pub(crate) fn react_to_content(&mut self, reactions: ContentReactions) {
        self.reactions = reactions;
    }

    /// Execute a command and then let a test build's switched-on reading of the payload commit
    /// whatever that reading calls for.
    ///
    /// With no reaction switched on this is [`Self::execute`] and the bytes are dropped unread,
    /// which is the only behaviour a release build has at all: no entry point outside `cfg(test)`
    /// accepts payload bytes.
    #[cfg(test)]
    pub(crate) fn execute_with_payload(
        &mut self,
        command: &BoardCommand,
        payload: &[u8],
    ) -> Result<Vec<BoardEvent>, BoardError> {
        let mut events = self.execute(command)?;
        let reactions = self.content_reactions(command, payload);
        if !reactions.is_empty() {
            events.extend(self.commit(reactions)?);
        }
        Ok(events)
    }

    #[cfg(test)]
    fn content_reactions(&self, command: &BoardCommand, payload: &[u8]) -> Vec<BoardEvent> {
        use crate::records::ObservedVerdict;

        let BoardCommand::Publish(published) = command else {
            return Vec::new();
        };
        let text = String::from_utf8_lossy(payload).to_lowercase();
        let mut events = Vec::new();
        if self.reactions.admits_on_capability_text && text.contains("capability") {
            for (index, scope) in self.scopes.values().enumerate() {
                events.push(BoardEvent::AudienceGranted {
                    grant_id: format!("reaction-grant-{index}-{}", published.message_id),
                    scope_id: scope.scope_id.clone(),
                    participant: published.author.clone(),
                    rights: Rights::READ_AND_PUBLISH,
                    granted_by: scope.sponsor.clone(),
                    expires_at: self.now.saturating_add(MAX_GRANT_MS),
                });
            }
        }
        if self.reactions.refunds_on_consent_text
            && text.contains("i consent")
            && let Some(payer) = self
                .participants
                .keys()
                .find(|participant| *participant != &published.author)
        {
            events.push(BoardEvent::AllowanceTransferred {
                from: AccountRef::Participant {
                    participant_id: payer.clone(),
                },
                to: AccountRef::Participant {
                    participant_id: published.author.clone(),
                },
                amount: CommunicationAllowance::unit(Allowance::Publications),
            });
        }
        if self.reactions.notes_verdict_on_protected_reference
            && text.contains("oracle:")
            && let Some(candidate) = published.references.iter().find_map(|reference| {
                if let Reference::Candidate { candidate_digest } = reference {
                    Some(candidate_digest.clone())
                } else {
                    None
                }
            })
        {
            events.push(BoardEvent::VerdictNoted {
                candidate_digest: candidate,
                verdict: ObservedVerdict::Passed,
            });
        }
        events
    }
}

/// The facts that end one live grant: the state it moves to, and the concurrent membership going
/// back to the account that funded it.
fn release_facts(grant: &GrantRecord, expired: bool) -> Vec<BoardEvent> {
    let mut events = vec![if expired {
        BoardEvent::AudienceExpired {
            grant_id: grant.grant_id.clone(),
        }
    } else {
        BoardEvent::AudienceReleased {
            grant_id: grant.grant_id.clone(),
        }
    }];
    if !grant.held.is_zero() {
        events.push(BoardEvent::AllowanceTransferred {
            from: AccountRef::Grant {
                grant_id: grant.grant_id.clone(),
            },
            to: AccountRef::Participant {
                participant_id: grant.granted_by.clone(),
            },
            amount: grant.held,
        });
    }
    events
}

fn deliver(message: &MessageRecord) -> DeliveredMessage {
    DeliveredMessage {
        sequence: message.sequence,
        message_id: message.message_id.clone(),
        author: message.author.clone(),
        audience: message.audience.clone(),
        kind: message.kind,
        payload_digest: message.payload_digest.clone(),
        payload_bytes: message.payload_bytes,
        published_at: message.published_at,
        salience_expires_at: message.salience_expires_at,
        references: message.references.clone(),
        relation: message.relation.clone(),
    }
}

fn account_label(account: &AccountRef) -> String {
    match account {
        AccountRef::Participant { participant_id } => participant_id.clone(),
        AccountRef::Grant { grant_id } => format!("grant {grant_id}"),
    }
}
