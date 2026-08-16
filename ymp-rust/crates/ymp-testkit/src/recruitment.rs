//! Fixtures for the two boundaries recruitment stands on: the measurement that says whether an
//! entry is live on this host, and the managed path an admitted participant is started through.
//!
//! Both are traits in `ymp-domain` because neither is the kernel's work — one is I/O and the other
//! starts processes. A check that wants to exercise the mechanical gates needs neither to happen,
//! so it states what the measurement would have answered and records what the start path was asked
//! for. Nothing here reaches a network, starts a process or reads the operator's own root.

use std::collections::BTreeMap;

use ymp_domain::pool::EntryIdentity;
use ymp_domain::recruitment::{
    AdmittedParticipant, ParticipantStartFailed, ParticipantStartPath, RuntimeAdmission,
    RuntimeRoute, RuntimeUnadmitted,
};

/// A host that serves the entries it was given, each through a stated profile and route, and
/// admits no other.
///
/// The refusal carries the words a measurement would have stated, since a refusal the kernel wrote
/// itself would say more about the kernel than about the host.
#[derive(Clone, Debug, Default)]
pub struct MeasuredHost {
    served: BTreeMap<EntryIdentity, RuntimeRoute>,
    reason: String,
}

impl MeasuredHost {
    /// A host serving nothing, which is what an entry no engine on this machine answers for reads
    /// as.
    pub fn serving_nothing(reason: impl Into<String>) -> Self {
        Self {
            served: BTreeMap::new(),
            reason: reason.into(),
        }
    }

    /// A host serving the named entries, each through the engine of its own name and a route named
    /// after the account and the model. The route is an opaque token: nothing compares two of them
    /// for anything but equality.
    pub fn serving(entries: &[EntryIdentity]) -> Self {
        let mut host = Self::serving_nothing("no runtime on this host answers for that entry");
        for entry in entries {
            host = host.serves(
                entry.clone(),
                RuntimeRoute::new(&entry.engine, route(entry)),
            );
        }
        host
    }

    #[must_use]
    pub fn serves(mut self, entry: EntryIdentity, route: RuntimeRoute) -> Self {
        self.served.insert(entry, route);
        self
    }

    /// Stop serving one entry, as a host whose credential expired between two requests would.
    #[must_use]
    pub fn withdraws(mut self, entry: &EntryIdentity) -> Self {
        self.served.remove(entry);
        self
    }
}

/// The route token one entry is served through in these fixtures.
pub fn route(entry: &EntryIdentity) -> String {
    format!("{}/{}", entry.provider, entry.model)
}

impl RuntimeAdmission for MeasuredHost {
    fn admits(&self, entry: &EntryIdentity) -> Result<RuntimeRoute, RuntimeUnadmitted> {
        self.served
            .get(entry)
            .cloned()
            .ok_or_else(|| RuntimeUnadmitted::new(self.reason.clone()))
    }
}

/// The managed start path, recording what it was asked to start instead of starting it.
///
/// It is the seam the one real start path implements. What a check reads from it is how many
/// participants a run handed over and which ones, which is how "exactly one participant was
/// started" is measured without a process existing.
#[derive(Clone, Debug, Default)]
pub struct RecordedStarts {
    started: Vec<AdmittedParticipant>,
    /// A participant this path refuses to start, as a host that ran out of sandboxes would.
    refuses: Option<String>,
}

impl RecordedStarts {
    pub fn new() -> Self {
        Self::default()
    }

    /// A start path that fails for every participant, with the stated reason.
    pub fn failing(reason: impl Into<String>) -> Self {
        Self {
            started: Vec::new(),
            refuses: Some(reason.into()),
        }
    }

    /// Every participant this path was asked to start, in the order it was asked.
    pub fn started(&self) -> &[AdmittedParticipant] {
        &self.started
    }
}

impl ParticipantStartPath for RecordedStarts {
    fn start(&mut self, admitted: &AdmittedParticipant) -> Result<(), ParticipantStartFailed> {
        if let Some(reason) = &self.refuses {
            return Err(ParticipantStartFailed::new(
                &admitted.participant_id,
                reason.clone(),
            ));
        }
        self.started.push(admitted.clone());
        Ok(())
    }
}
