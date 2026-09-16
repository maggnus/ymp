//! Private synthetic confinement/cessation evidence, through the real consumer.
use super::*;
use crate as kernel;
#[path = "../tests/support/workspace_fixture.rs"]
mod fixture;
use crate::journal::{JournalRead, ParameterSchemas, validate_workspace_append};
use crate::workspace_locks::{Cessation, CessationRecord};
use std::{collections::BTreeMap, sync::Mutex};
use ymp_domain::{identity::ExecutionProfile, journal::PolicySelection};
fn id<T>(value: &str) -> Id<T> {
    Id::new(value).unwrap()
}
#[derive(Default)]
struct TestJournal {
    schemas: ParameterSchemas,
    inventory: Mutex<BTreeMap<Id, JournalRead>>,
}
impl Journal for TestJournal {
    fn schemas(&self) -> &ParameterSchemas {
        &self.schemas
    }
    fn read(&self, session: &Id) -> Result<JournalRead> {
        Ok(self
            .inventory
            .lock()
            .unwrap()
            .get(session)
            .cloned()
            .unwrap_or(JournalRead {
                revision: 0,
                events: vec![],
            }))
    }
    fn workspace_inventory(&self, session: &Id) -> Result<crate::journal::WorkspaceInventory> {
        test_inventory(&self.inventory.lock().unwrap(), session, &self.schemas)
    }
    fn append(&self, session: &Id, expected: u64, events: &[Envelope<Event>]) -> Result<u64> {
        let mut inventory = self.inventory.lock().unwrap();
        let observed = test_inventory(&inventory, session, &self.schemas)?;
        validate_workspace_append(&observed, session, expected, events, &self.schemas)?;
        let current = inventory.entry(session.clone()).or_insert(JournalRead {
            revision: 0,
            events: vec![],
        });
        current.events.extend_from_slice(events);
        current.revision += events.len() as u64;
        Ok(current.revision)
    }
}
fn test_inventory(
    inventory: &BTreeMap<Id, JournalRead>,
    session: &Id,
    schemas: &ParameterSchemas,
) -> Result<crate::journal::WorkspaceInventory> {
    let current = inventory.get(session).cloned().unwrap_or(JournalRead {
        revision: 0,
        events: vec![],
    });
    let other = inventory
        .iter()
        .filter(|(id, _)| *id != session)
        .map(|(id, read)| {
            Ok(crate::workspace_locks::WorkspaceOwnership::from_view(
                &read.view_with_schemas(id, None, schemas)?,
            ))
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(crate::journal::WorkspaceInventory { current, other })
}
#[derive(Default)]
struct Bytes(Mutex<BTreeMap<Digest, Vec<u8>>>);
impl ContentStore for Bytes {
    fn put(&self, bytes: &[u8]) -> Result<Digest> {
        let d = Digest::of(bytes);
        self.0.lock().unwrap().insert(d.clone(), bytes.to_vec());
        Ok(d)
    }
    fn get(&self, d: &Digest, limit: usize) -> Result<Vec<u8>> {
        self.0
            .lock()
            .unwrap()
            .get(d)
            .filter(|b| b.len() <= limit)
            .cloned()
            .ok_or_else(|| Denial::new("content", "No bounded content"))
    }
}
struct ObservedFixture {
    selection: PolicySelection,
}
impl ObservedFixture {
    fn new() -> Self {
        Self {
            selection: PolicySelection::new(
                "WorkspaceProvider",
                "Direct",
                "1",
                serde_json::to_value(CaptureLimits::default()).unwrap(),
            )
            .unwrap(),
        }
    }
}
impl WorkspaceProvider for ObservedFixture {
    fn selection(&self) -> &PolicySelection {
        &self.selection
    }
    fn location(&self) -> Result<WorkspaceLocation> {
        Ok(WorkspaceLocation {
            root: "/fixture".into(),
            device: 1,
            inode: 2,
        })
    }
    fn validate_paths(&self, _: &[WorkspacePath]) -> Result<()> {
        Ok(())
    }
    fn observe_paths(&self, paths: &[WorkspacePath]) -> Result<Vec<PathObservation>> {
        Ok(paths
            .iter()
            .map(|path| PathObservation {
                path: path.clone(),
                existing: vec![
                    PathComponent {
                        name: "/".into(),
                        identity: FileIdentity {
                            device: 1,
                            inode: 1,
                        },
                    },
                    PathComponent {
                        name: "fixture".into(),
                        identity: FileIdentity {
                            device: 1,
                            inode: 2,
                        },
                    },
                ],
                missing: if path.as_str() == "." {
                    vec![]
                } else {
                    path.as_str().split('/').map(str::to_owned).collect()
                },
            })
            .collect())
    }
    fn capture(&self, _: &Result<JournalIdentity>, _: &dyn ContentStore) -> Result<SnapshotTree> {
        Err(Denial::new(
            "fixture",
            "This fixture supplies path observations, not captures",
        ))
    }
}
type Guard = WorkspaceGuard<TestJournal, Bytes>;
fn proof(
    guard: &Guard,
    session: &Id,
    profile: &ExecutionProfile,
    path: &str,
    mode: LockMode,
) -> AccessEvidence {
    let reference = guard.view(session).unwrap().workspaces()[&id("workspace")]
        .reference()
        .unwrap();
    AccessEvidence {
        session: session.clone(),
        issuer: guard.issuer.clone(),
        workspace: reference.clone(),
        profile: profile.clone(),
        actual: vec![(WorkspacePath::new(path).unwrap(), mode)],
        basis: vec![reference],
    }
}
fn acquire(
    guard: &Guard,
    session: &Id,
    profile: &ExecutionProfile,
    provider: &ObservedFixture,
    assignment: &str,
    requested: &str,
    evidence: &AccessEvidence,
) -> Result<u64> {
    guard.lock(
        session,
        guard.view(session)?.revision(),
        5,
        LockRequest {
            assignment: id(assignment),
            workspace: id("workspace"),
            profile: profile.clone(),
            paths: vec![(WorkspacePath::new(requested).unwrap(), evidence.actual[0].1)],
        },
        provider,
        evidence,
    )
}
#[test]
fn actual_root_access_cannot_be_narrowed_and_foreign_evidence_cannot_authorize() {
    let provider = ObservedFixture::new();
    let journal = Arc::new(TestJournal::default());
    let store = Arc::new(Bytes::default());
    let session = id("root");
    let (guard, profile) = fixture::open(journal.clone(), store.clone(), &session, &provider);
    let broad = proof(&guard, &session, &profile, ".", LockMode::Write);
    acquire(&guard, &session, &profile, &provider, "broad", "a", &broad).unwrap();
    let narrower = proof(&guard, &session, &profile, "b", LockMode::Write);
    let before = journal.read(&session).unwrap();
    let conflict = acquire(
        &guard, &session, &profile, &provider, "narrow", "b", &narrower,
    )
    .unwrap_err();
    assert_eq!(conflict.code, "path_conflict");
    assert!(!conflict.refs.is_empty());
    assert_eq!(journal.read(&session).unwrap(), before);
    let another = WorkspaceGuard::new(journal.clone(), store);
    assert_eq!(
        acquire(
            &another, &session, &profile, &provider, "foreign", "b", &narrower
        )
        .unwrap_err()
        .code,
        "access_evidence"
    );
}
#[test]
fn compatible_reads_disjoint_writes_and_retained_revocation_work_across_sessions() {
    let provider = ObservedFixture::new();
    let journal = Arc::new(TestJournal::default());
    let store = Arc::new(Bytes::default());
    let a = id("a");
    let b = id("b");
    let (ga, pa) = fixture::open(journal.clone(), store.clone(), &a, &provider);
    let (gb, pb) = fixture::open(journal.clone(), store.clone(), &b, &provider);
    acquire(
        &ga,
        &a,
        &pa,
        &provider,
        "read-a",
        "shared",
        &proof(&ga, &a, &pa, "shared", LockMode::Read),
    )
    .unwrap();
    acquire(
        &gb,
        &b,
        &pb,
        &provider,
        "read-b",
        "shared",
        &proof(&gb, &b, &pb, "shared", LockMode::Read),
    )
    .unwrap();
    acquire(
        &ga,
        &a,
        &pa,
        &provider,
        "write-a",
        "left",
        &proof(&ga, &a, &pa, "left", LockMode::Write),
    )
    .unwrap();
    acquire(
        &gb,
        &b,
        &pb,
        &provider,
        "write-b",
        "right",
        &proof(&gb, &b, &pb, "right", LockMode::Write),
    )
    .unwrap();
    let stale = ga.never_authorized(&a, &id("write-a")).unwrap();
    ga.authorize_access(
        &a,
        ga.view(&a).unwrap().revision(),
        6,
        id("write-a"),
        id("invocation"),
        &provider,
    )
    .unwrap();
    assert!(
        ga.release(&a, ga.view(&a).unwrap().revision(), 7, &stale)
            .is_err()
    );
    ga.revoke_access(
        &a,
        ga.view(&a).unwrap().revision(),
        7,
        id("write-a"),
        "Transport disconnected".into(),
    )
    .unwrap();
    let view = ga.view(&a).unwrap();
    assert_eq!(
        WorkspaceGuard::new(journal.clone(), store)
            .view(&a)
            .unwrap(),
        view
    );
    assert!(
        acquire(
            &gb,
            &b,
            &pb,
            &provider,
            "blocked",
            "LEFT/file",
            &proof(&gb, &b, &pb, "LEFT/file", LockMode::Write)
        )
        .is_err()
    );
    let prior = &view.path_locks()[&id("write-a")];
    // Test-only scoped cessation. No native process or mediated writer ran here.
    let ended = CessationEvidence {
        session: a.clone(),
        issuer: ga.issuer.clone(),
        record: CessationRecord {
            assignment: id("write-a"),
            workspace: id("workspace"),
            invocation: Some(id("invocation")),
            state: prior.last.clone(),
            kind: Cessation::AccessWithdrawn,
            basis: vec![prior.last.clone()],
        },
    };
    ga.release(&a, view.revision(), 8, &ended).unwrap();
    acquire(
        &gb,
        &b,
        &pb,
        &provider,
        "after-release",
        "left/file",
        &proof(&gb, &b, &pb, "left/file", LockMode::Write),
    )
    .unwrap();
    let unused = gb.never_authorized(&b, &id("write-b")).unwrap();
    gb.release(&b, gb.view(&b).unwrap().revision(), 9, &unused)
        .unwrap();
    assert!(
        gb.authorize_access(
            &b,
            gb.view(&b).unwrap().revision(),
            10,
            id("write-b"),
            id("late"),
            &provider
        )
        .is_err()
    );
}

#[test]
fn exclusive_locking_does_not_grant_a_missing_read_capability() {
    let journal = Arc::new(TestJournal::default());
    let store = Arc::new(Bytes::default());
    let provider = ObservedFixture::new();
    let session = id("write-only");
    let (guard, profile) = fixture::open_with_capabilities(
        journal.clone(),
        store,
        &session,
        &provider,
        std::collections::BTreeSet::from([ymp_domain::journal::Capability::WriteFiles]),
    );
    let evidence = proof(&guard, &session, &profile, ".", LockMode::Write);
    let before = journal.read(&session).unwrap();
    let result = guard.lock(
        &session,
        before.revision,
        5,
        LockRequest {
            assignment: id("reader"),
            workspace: id("workspace"),
            profile,
            paths: vec![(WorkspacePath::root(), LockMode::Read)],
        },
        &provider,
        &evidence,
    );
    assert_eq!(result.unwrap_err().code, "capability_denied");
    assert_eq!(journal.read(&session).unwrap(), before);
}
