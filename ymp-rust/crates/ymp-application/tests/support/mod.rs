use std::path::{Path, PathBuf};

use ymp_storage::{ObjectStore, ObjectStoreError};

/// Storage-level fixture access for corruption and recovery checks.
///
/// This helper is compiled into each integration-test crate that names it. It is not part of the
/// application API and cannot be reached by a product consumer.
pub struct FixtureObjects(ObjectStore);

impl FixtureObjects {
    pub fn at(data_root: &Path) -> Self {
        Self(
            ObjectStore::open(data_root.join("objects/sha256")).expect("open fixture object store"),
        )
    }

    pub fn put(&self, bytes: &[u8]) -> String {
        self.0.put(bytes).expect("store fixture object")
    }

    pub fn read(&self, digest: &str) -> Vec<u8> {
        self.0.read(digest).expect("read fixture object")
    }

    pub fn verify(&self, digest: &str) -> Result<(), ObjectStoreError> {
        self.0.verify(digest)
    }

    pub fn path_for(&self, digest: &str) -> PathBuf {
        self.0.path_for(digest).expect("resolve fixture object")
    }
}
