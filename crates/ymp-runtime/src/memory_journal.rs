//! Atomic reference Journal adapter. Contents live only for this process.

use std::{collections::BTreeMap, sync::Mutex};
use ymp_domain::{Denial, Id, Result, journal::Envelope};
use ymp_kernel::{
    events::Event,
    journal::{Journal, JournalRead, ParameterSchemas, validate_append},
};

type Events = Vec<Envelope<Event>>;

pub struct MemoryJournal {
    schemas: ParameterSchemas,
    identity: Option<MemoryIdentity>,
    sessions: Mutex<BTreeMap<Id, Events>>,
}

struct MemoryIdentity {
    file: std::fs::File,
    identity: ymp_domain::workspace::JournalIdentity,
}
impl Drop for MemoryIdentity {
    fn drop(&mut self) {
        use std::os::unix::fs::MetadataExt;
        if std::fs::symlink_metadata(&self.identity.path).is_ok_and(|m| {
            m.is_file()
                && m.dev() == self.identity.file.device
                && m.ino() == self.identity.file.inode
        }) {
            let _ = std::fs::remove_file(&self.identity.path);
        }
    }
}
impl Default for MemoryJournal {
    fn default() -> Self {
        Self::with_schemas(ParameterSchemas::default())
    }
}
impl MemoryJournal {
    pub fn with_schemas(schemas: ParameterSchemas) -> Self {
        Self {
            schemas,
            identity: None,
            sessions: Mutex::new(BTreeMap::new()),
        }
    }
    pub fn new() -> Self {
        Self::default()
    }
    /// Own a real temporary identity marker, while all event history remains in RAM.
    /// Recreating a process cannot recover this journal from the marker.
    pub fn with_binding_identity(schemas: ParameterSchemas) -> Result<Self> {
        use std::{
            io::Write,
            os::unix::fs::{MetadataExt, OpenOptionsExt},
        };
        let mut random = [0; 32];
        getrandom::fill(&mut random).map_err(|_| {
            Denial::new("journal_identity", "Cannot create memory-journal identity")
        })?;
        let id = ymp_domain::Digest::of(random);
        let path = std::env::temp_dir().join(format!("ymp-memory-{}", id));
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .read(true)
            .create_new(true)
            .mode(0o600)
            .open(&path)
            .map_err(|_| {
                Denial::new(
                    "journal_identity",
                    "Cannot create owned memory identity marker",
                )
            })?;
        let original = file.metadata().ok().map(|m| (m.dev(), m.ino()));
        let initialized = (|| -> Result<MemoryIdentity> {
            file.write_all(id.as_str().as_bytes()).map_err(|_| {
                Denial::new("journal_identity", "Cannot initialize memory identity")
            })?;
            let metadata = file
                .metadata()
                .map_err(|_| Denial::new("journal_identity", "Cannot inspect memory identity"))?;
            let path = std::fs::canonicalize(&path)
                .map_err(|_| Denial::new("journal_identity", "Cannot locate memory identity"))?;
            Ok(MemoryIdentity {
                file,
                identity: ymp_domain::workspace::JournalIdentity {
                    id,
                    path: path.to_string_lossy().into_owned(),
                    file: ymp_domain::workspace::FileIdentity {
                        device: metadata.dev(),
                        inode: metadata.ino(),
                    },
                },
            })
        })();
        match initialized {
            Ok(identity) => Ok(Self {
                schemas,
                identity: Some(identity),
                sessions: Mutex::new(BTreeMap::new()),
            }),
            Err(error) => {
                if std::fs::symlink_metadata(&path)
                    .is_ok_and(|m| m.is_file() && original == Some((m.dev(), m.ino())))
                {
                    let _ = std::fs::remove_file(path);
                }
                Err(error)
            }
        }
    }
}

impl Journal for MemoryJournal {
    fn binding_identity(&self) -> Result<ymp_domain::workspace::JournalIdentity> {
        use std::os::unix::fs::MetadataExt;
        let owned = self.identity.as_ref().ok_or_else(|| {
            Denial::new(
                "journal_identity",
                "Memory journal has no owned physical identity",
            )
        })?;
        let original = owned
            .file
            .metadata()
            .map_err(|_| Denial::new("journal_identity", "Memory identity is unavailable"))?;
        let current = std::fs::symlink_metadata(&owned.identity.path).map_err(|_| {
            Denial::new("journal_identity", "Memory identity marker is unavailable")
        })?;
        if !current.is_file() || current.dev() != original.dev() || current.ino() != original.ino()
        {
            return Err(Denial::new(
                "journal_identity",
                "Memory identity marker was replaced",
            ));
        }
        Ok(owned.identity.clone())
    }
    fn workspace_binding(
        &self,
        root: &ymp_domain::workspace::WorkspaceLocation,
    ) -> Result<Option<ymp_domain::workspace::WorkspaceBinding>> {
        let sessions = self
            .sessions
            .lock()
            .map_err(|_| Denial::new("journal_unavailable", "Journal lock is poisoned"))?;
        let mut found = None;
        for (id, events) in sessions.iter() {
            let read = JournalRead {
                revision: events.len() as u64,
                events: events.clone(),
            };
            let view = read.view_with_schemas(id, None, &self.schemas)?;
            for binding in view.workspace_bindings().values().filter(|b| {
                (b.root.device == root.device && b.root.inode == root.inode)
                    || ymp_kernel::journal::root_paths_overlap(&b.root.root, &root.root)
            }) {
                if found.as_ref().is_some_and(|prior| prior != binding) {
                    return Err(Denial::new(
                        "workspace_binding",
                        "Conflicting memory root bindings",
                    ));
                }
                found = Some(binding.clone());
            }
        }
        Ok(found)
    }
    fn session_control(
        &self,
        session: &Id,
        at: u64,
        change: ymp_kernel::session::SessionChange,
    ) -> Result<ymp_domain::Ref> {
        let mut sessions = self
            .sessions
            .lock()
            .map_err(|_| Denial::new("journal_unavailable", "Journal lock is poisoned"))?;
        let events = sessions.get(session).cloned().unwrap_or_default();
        let current = JournalRead {
            revision: events.len() as u64,
            events,
        };
        let (batch, reference) =
            ymp_kernel::session::control_events(&current, session, at, change, &self.schemas)?;
        if !batch.is_empty() {
            let state = inventory(&sessions, session, &self.schemas)?;
            ymp_kernel::journal::validate_workspace_append(
                &state,
                session,
                current.revision,
                &batch,
                &self.schemas,
            )?;
            sessions.entry(session.clone()).or_default().extend(batch);
        }
        Ok(reference)
    }
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
        if self.identity.is_some() {
            for event in batch {
                if let Event::WorkspaceBound { binding, .. } = &event.payload
                    && binding.journal != self.binding_identity()?
                {
                    return Err(Denial::new(
                        "workspace_binding",
                        "Root binding targets another memory journal",
                    ));
                }
            }
        }

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
