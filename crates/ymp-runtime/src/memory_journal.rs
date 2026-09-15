//! Atomic reference Journal adapter. Contents live only for this process.

use std::{collections::BTreeMap, sync::Mutex};
use ymp_domain::{Denial, Id, Result, journal::Envelope};
use ymp_kernel::{
    events::Event,
    journal::{Journal, JournalRead, ParameterSchemas, validate_append},
};

type Events = Vec<Envelope<Event>>;

#[derive(Default)]
pub struct MemoryJournal {
    schemas: ParameterSchemas,
    sessions: Mutex<BTreeMap<Id, Events>>,
}

impl MemoryJournal {
    pub fn with_schemas(schemas: ParameterSchemas) -> Self {
        Self {
            schemas,
            sessions: Mutex::new(BTreeMap::new()),
        }
    }
    pub fn new() -> Self {
        Self::default()
    }
}

impl Journal for MemoryJournal {
    fn schemas(&self) -> &ParameterSchemas {
        &self.schemas
    }
    fn read(&self, session: &Id) -> Result<JournalRead> {
        let sessions = self
            .sessions
            .lock()
            .map_err(|_| Denial::new("journal_unavailable", "Journal lock is poisoned"))?;
        let events = sessions.get(session).cloned().unwrap_or_default();
        Ok(JournalRead {
            revision: events.len() as u64,
            events,
        })
    }
    fn workspace_inventory(&self, session: &Id) -> Result<ymp_kernel::journal::WorkspaceInventory> {
        let sessions = self
            .sessions
            .lock()
            .map_err(|_| Denial::new("journal_unavailable", "Journal lock is poisoned"))?;
        inventory(&sessions, session, &self.schemas)
    }
    fn append(&self, session: &Id, expected: u64, batch: &[Envelope<Event>]) -> Result<u64> {
        let mut sessions = self
            .sessions
            .lock()
            .map_err(|_| Denial::new("journal_unavailable", "Journal lock is poisoned"))?;
        let events = sessions.get(session).cloned().unwrap_or_default();
        let current = JournalRead {
            revision: events.len() as u64,
            events,
        };
        let view = validate_append(&current, session, expected, batch, &self.schemas)?;
        if ymp_kernel::workspace_locks::touches_ownership(batch) {
            let inventory = inventory(&sessions, session, &self.schemas)?;
            ymp_kernel::journal::validate_workspace_append(
                &inventory,
                session,
                expected,
                batch,
                &self.schemas,
            )?;
        }
        sessions
            .entry(session.clone())
            .or_default()
            .extend_from_slice(batch);
        Ok(view.revision())
    }
}

fn inventory(
    sessions: &BTreeMap<Id, Events>,
    session: &Id,
    schemas: &ParameterSchemas,
) -> Result<ymp_kernel::journal::WorkspaceInventory> {
    let mut current = JournalRead {
        revision: 0,
        events: vec![],
    };
    let mut other = vec![];
    let mut weight = (0usize, 0usize);
    for (id, events) in sessions {
        let read = JournalRead {
            revision: events.len() as u64,
            events: events.clone(),
        };
        if id == session {
            current = read;
        } else {
            let owner = ymp_kernel::workspace_locks::WorkspaceOwnership::from_view(
                &read.view_with_schemas(id, None, schemas)?,
            );
            if !owner.is_empty() {
                let (bytes, count) = owner.retained_weight()?;
                weight.0 += bytes;
                weight.1 += count;
                ymp_kernel::workspace_locks::validate_inventory_size(weight.0, weight.1)?;
                other.push(owner);
            }
        }
    }
    Ok(ymp_kernel::journal::WorkspaceInventory { current, other })
}
