//! The communication observatory: a typed, read-only projection of what was published, delivered,
//! cited, revised, challenged, carried forward and decided by a protected query.
//!
//! It is an analytical surface and takes part in nothing. It admits no participant, allocates no
//! capacity and accepts no candidate; building a view changes no record, and there is no path from
//! a view back into the board.
//!
//! The distinction the whole module exists for is between what the records state and what they do
//! not. Explicit references and reliable publication order state provenance: this author published
//! that, this reader was sent it, this message cites that one, this position revised an earlier one,
//! this result carries those forward, this query returned that verdict. None of it states that one
//! participant listened to another. That claim is reachable only by repeating an episode with the
//! message changed, under the same total budget, and observing that the receiver's later actions
//! changed — so an edge is labelled causal only where a [controlled intervention
//! record](crate::records::InterventionRecord) supports it, and a view that labels anything else
//! causal fails [`ViewState::check`].

use serde::Serialize;

use crate::ledger::BoardLedger;
use crate::records::{
    Admissibility, AssessmentRecord, Audience, InterventionRecord, MessageKind, MessageRecord,
    ObservedVerdict, Reference, Relation,
};

/// How many stochastic replications a controlled comparison needs before this plane will present
/// its result as causal rather than as one episode.
///
/// One replication is an anecdote under a particular sampling of a stochastic system. The bound is
/// deliberately a floor and not a claim that two is enough for a strong estimate; what it enforces
/// is that a single run never carries a causal label.
pub const MIN_CAUSAL_REPLICATIONS: u32 = 2;

/// One thing a view can point at.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(tag = "node", rename_all = "snake_case")]
pub enum NodeRef {
    Participant { participant_id: String },
    Message { message_id: String },
    Candidate { candidate_digest: String },
    Artifact { object_digest: String },
    ControlRecord { record_id: String },
    Verdict { candidate_digest: String },
}

/// What one edge states. Every kind but [`EdgeKind::Influence`] is a record of something that
/// happened; `Influence` is a claim about why, and it is the only kind an intervention supports.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgeKind {
    /// An author appended a message.
    Publication,
    /// A message was made available to a reader. It states availability and not reading.
    Delivery,
    /// A message points at another message, an artifact, a result or a control record.
    Citation,
    /// A message states which earlier published evidence a decision claims to have used. It is the
    /// author's claim about its own reasoning.
    ClaimedBasis,
    /// An author revised its own earlier position. The earlier one stays exactly where it was.
    Revision,
    /// A message disagrees with an earlier one. Recording it settles nothing.
    Challenge,
    /// An author paid to keep an earlier message salient.
    Refresh,
    /// A result carries another forward.
    Ancestry,
    /// A protected query returned a verdict on a result.
    Verdict,
    /// The receiver's later actions changed when this message changed. It is the only causal kind
    /// here, and it exists only where a controlled intervention supports it.
    Influence,
}

impl EdgeKind {
    /// Whether this kind asserts that something happened *because of* something else.
    pub const fn is_causal(self) -> bool {
        matches!(self, Self::Influence)
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Publication => "publication",
            Self::Delivery => "delivery",
            Self::Citation => "citation",
            Self::ClaimedBasis => "claimed_basis",
            Self::Revision => "revision",
            Self::Challenge => "challenge",
            Self::Refresh => "refresh",
            Self::Ancestry => "ancestry",
            Self::Verdict => "verdict",
            Self::Influence => "influence",
        }
    }
}

/// What an edge rests on.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "basis", rename_all = "snake_case")]
pub enum Basis {
    /// A record states it: an attribution, a reference, a receipt, a recorded order.
    Declared,
    /// A controlled intervention supports it.
    Intervention { intervention_id: String },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Edge {
    pub kind: EdgeKind,
    pub from: NodeRef,
    pub to: NodeRef,
    pub basis: Basis,
}

/// One message as the observatory presents it, with the untrusted-data label it always carries.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct MessageView {
    pub sequence: u64,
    pub message_id: String,
    pub author: String,
    pub audience: Audience,
    pub kind: MessageKind,
    pub payload_digest: String,
    pub payload_bytes: u64,
    pub published_at: u64,
    pub salience_expires_at: u64,
    /// Always true. The board's payloads are participant claims, and a surface that could present
    /// one as anything else would be presenting an untrusted byte string as a finding.
    pub untrusted: bool,
}

/// A read-only reading of the board.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ViewState {
    /// The participant this reading was built for, or `None` for the operator's own surface.
    pub reader: Option<String>,
    pub messages: Vec<MessageView>,
    pub edges: Vec<Edge>,
    pub interventions: Vec<InterventionRecord>,
    pub assessments: Vec<AssessmentRecord>,
    pub verdicts: Vec<(String, ObservedVerdict)>,
}

/// Why a view state may not be presented.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum ViewDefect {
    #[error("a {kind} edge from {from:?} to {to:?} is causal and names no controlled intervention")]
    CausalWithoutIntervention {
        kind: &'static str,
        from: NodeRef,
        to: NodeRef,
    },
    #[error("a {kind} edge is an association and may not rest on intervention {intervention_id}")]
    AssociationLabelledCausal {
        kind: &'static str,
        intervention_id: String,
    },
    #[error("intervention {intervention_id} is not recorded in this view")]
    InterventionUnknown { intervention_id: String },
    #[error(
        "intervention {intervention_id} changed another message, or changed it for another receiver, than the edge between {from:?} and {to:?} it is made to support"
    )]
    InterventionSubjectMismatch {
        intervention_id: String,
        from: NodeRef,
        to: NodeRef,
    },
    #[error("intervention {intervention_id} did not run its replications under a matched budget")]
    InterventionUnmatchedBudget { intervention_id: String },
    #[error(
        "intervention {intervention_id} was replicated {replications} time(s), and a causal label needs at least {MIN_CAUSAL_REPLICATIONS}"
    )]
    InterventionUnreplicated {
        intervention_id: String,
        replications: u32,
    },
}

impl ViewState {
    /// The reading one participant may be shown: the messages it is admitted to, and the edges
    /// among them.
    pub fn for_reader(board: &BoardLedger, reader: &str) -> Self {
        Self::build(board, Some(reader.to_owned()))
    }

    /// The operator's own reading, which is not bounded by a board audience because the operator is
    /// not a participant in one. It is still read-only, and every payload it shows is still an
    /// untrusted claim.
    pub fn for_operator(board: &BoardLedger) -> Self {
        Self::build(board, None)
    }

    fn build(board: &BoardLedger, reader: Option<String>) -> Self {
        let visible: Vec<&MessageRecord> = board
            .audit()
            .iter()
            .filter(|message| {
                reader
                    .as_ref()
                    .is_none_or(|reader| board.may_read(reader, message))
            })
            .collect();
        let is_visible = |message_id: &str| {
            visible
                .iter()
                .any(|message| message.message_id == message_id)
        };

        let mut edges = Vec::new();
        for message in &visible {
            let node = NodeRef::Message {
                message_id: message.message_id.clone(),
            };
            edges.push(Edge {
                kind: EdgeKind::Publication,
                from: NodeRef::Participant {
                    participant_id: message.author.clone(),
                },
                to: node.clone(),
                basis: Basis::Declared,
            });
            for reference in &message.references {
                if let Some(target) = reference_node(reference, &is_visible) {
                    edges.push(Edge {
                        kind: EdgeKind::Citation,
                        from: node.clone(),
                        to: target,
                        basis: Basis::Declared,
                    });
                }
            }
            for cited in &message.claimed_decision_basis {
                if is_visible(cited) {
                    edges.push(Edge {
                        kind: EdgeKind::ClaimedBasis,
                        from: node.clone(),
                        to: NodeRef::Message {
                            message_id: cited.clone(),
                        },
                        basis: Basis::Declared,
                    });
                }
            }
            if let Some((kind, target)) = relation_edge(&message.relation)
                && is_visible(target)
            {
                edges.push(Edge {
                    kind,
                    from: node.clone(),
                    to: NodeRef::Message {
                        message_id: target.to_owned(),
                    },
                    basis: Basis::Declared,
                });
            }
        }

        for receipt in board.receipts() {
            for message_id in &receipt.message_ids {
                if is_visible(message_id) {
                    edges.push(Edge {
                        kind: EdgeKind::Delivery,
                        from: NodeRef::Message {
                            message_id: message_id.clone(),
                        },
                        to: NodeRef::Participant {
                            participant_id: receipt.reader.clone(),
                        },
                        basis: Basis::Declared,
                    });
                }
            }
        }

        for record in board.ancestry() {
            for parent in &record.parents {
                edges.push(Edge {
                    kind: EdgeKind::Ancestry,
                    from: NodeRef::Candidate {
                        candidate_digest: record.candidate_digest.clone(),
                    },
                    to: NodeRef::Candidate {
                        candidate_digest: parent.clone(),
                    },
                    basis: Basis::Declared,
                });
            }
        }

        for record in board.verdicts() {
            edges.push(Edge {
                kind: EdgeKind::Verdict,
                from: NodeRef::Verdict {
                    candidate_digest: record.candidate_digest.clone(),
                },
                to: NodeRef::Candidate {
                    candidate_digest: record.candidate_digest.clone(),
                },
                basis: Basis::Declared,
            });
        }

        // The only causal edges this plane draws, and only from a record that supports one.
        let interventions: Vec<InterventionRecord> = board.interventions().cloned().collect();
        for intervention in &interventions {
            if !supports_causal_label(intervention) || !is_visible(&intervention.subject_message) {
                continue;
            }
            edges.push(Edge {
                kind: EdgeKind::Influence,
                from: NodeRef::Message {
                    message_id: intervention.subject_message.clone(),
                },
                to: NodeRef::Participant {
                    participant_id: intervention.receiver.clone(),
                },
                basis: Basis::Intervention {
                    intervention_id: intervention.intervention_id.clone(),
                },
            });
        }

        let assessments = board
            .assessments()
            .iter()
            .filter(|assessment| {
                reader
                    .as_ref()
                    .is_none_or(|reader| board.may_read_assessment(reader, assessment))
            })
            .cloned()
            .collect();

        Self {
            reader,
            messages: visible.iter().map(|message| view_of(message)).collect(),
            edges,
            interventions,
            assessments,
            verdicts: board
                .verdicts()
                .map(|record| (record.candidate_digest.clone(), record.verdict))
                .collect(),
        }
    }

    /// Whether this reading may be presented: every causal edge names a controlled intervention
    /// that supports exactly it, and no association is dressed up as one.
    ///
    /// The check exists because a view is data. It can be constructed by hand, by a surface that
    /// has one more field than it should, or by a later change that draws an inference the records
    /// do not carry — and a reading built by any of those is exactly as presentable as one built
    /// here unless something refuses it.
    pub fn check(&self) -> Result<(), ViewDefect> {
        for edge in &self.edges {
            match (&edge.basis, edge.kind.is_causal()) {
                (Basis::Declared, false) => {}
                (Basis::Declared, true) => {
                    return Err(ViewDefect::CausalWithoutIntervention {
                        kind: edge.kind.as_str(),
                        from: edge.from.clone(),
                        to: edge.to.clone(),
                    });
                }
                (Basis::Intervention { intervention_id }, false) => {
                    return Err(ViewDefect::AssociationLabelledCausal {
                        kind: edge.kind.as_str(),
                        intervention_id: intervention_id.clone(),
                    });
                }
                (Basis::Intervention { intervention_id }, true) => {
                    self.check_intervention(edge, intervention_id)?;
                }
            }
        }
        Ok(())
    }

    fn check_intervention(&self, edge: &Edge, intervention_id: &str) -> Result<(), ViewDefect> {
        let record = self
            .interventions
            .iter()
            .find(|record| record.intervention_id == intervention_id)
            .ok_or_else(|| ViewDefect::InterventionUnknown {
                intervention_id: intervention_id.to_owned(),
            })?;
        let subject = NodeRef::Message {
            message_id: record.subject_message.clone(),
        };
        let receiver = NodeRef::Participant {
            participant_id: record.receiver.clone(),
        };
        if edge.from != subject || edge.to != receiver {
            return Err(ViewDefect::InterventionSubjectMismatch {
                intervention_id: intervention_id.to_owned(),
                from: edge.from.clone(),
                to: edge.to.clone(),
            });
        }
        if !record.matched_budget {
            return Err(ViewDefect::InterventionUnmatchedBudget {
                intervention_id: intervention_id.to_owned(),
            });
        }
        if record.replications < MIN_CAUSAL_REPLICATIONS {
            return Err(ViewDefect::InterventionUnreplicated {
                intervention_id: intervention_id.to_owned(),
                replications: record.replications,
            });
        }
        Ok(())
    }

    pub fn edges_of(&self, kind: EdgeKind) -> Vec<&Edge> {
        self.edges.iter().filter(|edge| edge.kind == kind).collect()
    }

    /// The assessments that may stand in the primary comparison.
    pub fn primary_assessments(&self) -> Vec<&AssessmentRecord> {
        self.assessments
            .iter()
            .filter(|assessment| assessment.admissibility == Admissibility::Primary)
            .collect()
    }
}

/// Whether one recorded intervention is enough to label anything causal.
fn supports_causal_label(record: &InterventionRecord) -> bool {
    record.matched_budget && record.replications >= MIN_CAUSAL_REPLICATIONS
}

fn relation_edge(relation: &Relation) -> Option<(EdgeKind, &str)> {
    match relation {
        Relation::Standalone => None,
        Relation::ReplyTo { message_id } => Some((EdgeKind::Citation, message_id)),
        Relation::Challenges { message_id } => Some((EdgeKind::Challenge, message_id)),
        Relation::Revises { message_id } => Some((EdgeKind::Revision, message_id)),
        Relation::Refreshes { message_id } => Some((EdgeKind::Refresh, message_id)),
    }
}

fn reference_node(reference: &Reference, is_visible: &impl Fn(&str) -> bool) -> Option<NodeRef> {
    match reference {
        Reference::Message { message_id } => is_visible(message_id).then(|| NodeRef::Message {
            message_id: message_id.clone(),
        }),
        Reference::Artifact { object_digest } => Some(NodeRef::Artifact {
            object_digest: object_digest.clone(),
        }),
        Reference::Candidate { candidate_digest } => Some(NodeRef::Candidate {
            candidate_digest: candidate_digest.clone(),
        }),
        Reference::ControlRecord { record_id } => Some(NodeRef::ControlRecord {
            record_id: record_id.clone(),
        }),
    }
}

fn view_of(message: &MessageRecord) -> MessageView {
    MessageView {
        sequence: message.sequence,
        message_id: message.message_id.clone(),
        author: message.author.clone(),
        audience: message.audience.clone(),
        kind: message.kind,
        payload_digest: message.payload_digest.clone(),
        payload_bytes: message.payload_bytes,
        published_at: message.published_at,
        salience_expires_at: message.salience_expires_at,
        untrusted: true,
    }
}
