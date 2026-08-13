//! Which store a project's next run belongs in.
//!
//! Creating a store and opening one are already this crate's concern; choosing which store an
//! invocation acts on is the same concern, and it is decided here rather than by the command
//! line. The command line passes the root the operator named — or none — and receives the one
//! store directory the interface's session then opens, exactly as it used to receive a directory
//! the operator had typed.
//!
//! The layout itself belongs to `ymp-storage`, which owns every durable path.

use std::path::{Path, PathBuf};

pub use ymp_storage::{DEFAULT_ROOT, DataRoot, LEGACY_STORE, RootError, StoreIntent};

/// The root every durable path lives under when the operator names none.
pub fn default_root() -> PathBuf {
    DataRoot::default_path()
}

/// Refuse to begin beside a store an earlier layout wrote, naming it and both ways to proceed.
///
/// Such a store is read where it stands or left alone; it is never copied into the new root and
/// never opened as if it were one.
pub fn refuse_earlier_layout(root: &Path) -> Result<(), RootError> {
    DataRoot::refuse_legacy_neighbour(root)
}

/// The store this invocation acts on, addressed under `root` for the directory the product was
/// started in.
pub fn store_under(root: &Path, intent: StoreIntent) -> Result<PathBuf, RootError> {
    DataRoot::open(root)?.store(intent)
}
