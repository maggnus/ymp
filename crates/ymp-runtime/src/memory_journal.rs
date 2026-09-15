//! Atomic reference Journal adapter. Contents live only for this process.

use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};
use ymp_domain::{Denial, Id, Result, journal::Envelope};
use ymp_kernel::{
    events::Event,
    journal::{Journal, JournalRead, ParameterSchemas, validate_append},
};

type Events = Arc<Mutex<Vec<Envelope<Event>>>>;

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
        let sessions = self.sessions.lock().map_err(|_| {
            Denial::new(
                "journal_unavailable",
                "The in-memory journal lock is poisoned",
            )
        })?;
        let events = sessions.get(session).cloned();
        drop(sessions);
        let events = match events {
            None => Vec::new(),
            Some(events) => events
                .lock()
                .map_err(|_| {
                    Denial::new(
                        "journal_unavailable",
                        "The session journal lock is poisoned",
                    )
                })?
                .clone(),
        };
        Ok(JournalRead {
            revision: events.len() as u64,
            events,
        })
    }

    fn append(&self, session: &Id, expected: u64, batch: &[Envelope<Event>]) -> Result<u64> {
        let mut sessions = self.sessions.lock().map_err(|_| {
            Denial::new(
                "journal_unavailable",
                "The in-memory journal lock is poisoned",
            )
        })?;
        let shared = match sessions.get(session).cloned() {
            Some(shared) => shared,
            None => {
                let current = JournalRead {
                    revision: 0,
                    events: Vec::new(),
                };
                let view = validate_append(&current, session, expected, batch, &self.schemas)?;
                sessions.insert(session.clone(), Arc::new(Mutex::new(batch.to_vec())));
                return Ok(view.revision());
            }
        };
        drop(sessions);
        let mut events = shared.lock().map_err(|_| {
            Denial::new(
                "journal_unavailable",
                "The session journal lock is poisoned",
            )
        })?;
        let current = JournalRead {
            revision: events.len() as u64,
            events: events.clone(),
        };
        let view = validate_append(&current, session, expected, batch, &self.schemas)?;
        events.extend_from_slice(batch);
        Ok(view.revision())
    }
}
