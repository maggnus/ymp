#![forbid(unsafe_code)]

mod journal;
mod object_store;
mod root;

pub use journal::{
    Journal, JournalError, JournalLimits, MAX_EVENT_BYTES, MAX_JOURNAL_BYTES,
    TERMINAL_EVENT_RESERVE_BYTES,
};
pub use object_store::{ObjectStore, ObjectStoreError};
pub use root::{DEFAULT_ROOT, DataRoot, LAYOUT_VERSION, LEGACY_STORE, RootError, StoreIntent};

use fs2::FileExt;
use std::fs::{self, File, OpenOptions};
use std::io::{Seek, SeekFrom, Write};
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum LockError {
    #[error("data root is already locked")]
    AlreadyLocked,
    #[error("data-root lock I/O error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug)]
pub struct DataRootLock {
    file: File,
}

impl DataRootLock {
    pub fn acquire(root: &Path) -> Result<Self, LockError> {
        fs::create_dir_all(root)?;
        let path = root.join("writer.lock");
        let mut file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(path)?;
        file.try_lock_exclusive()
            .map_err(|_| LockError::AlreadyLocked)?;
        file.set_len(0)?;
        file.seek(SeekFrom::Start(0))?;
        writeln!(file, "pid={}", std::process::id())?;
        file.sync_data()?;
        Ok(Self { file })
    }
}

impl Drop for DataRootLock {
    fn drop(&mut self) {
        let _ = FileExt::unlock(&self.file);
    }
}
