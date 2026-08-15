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

pub use ymp_storage::{DataRoot, LEGACY_STORE, ROOT_DIRECTORY, RootError, StoreIntent, YMP_HOME};

/// The root every durable path lives under when the operator names none: what [`YMP_HOME`] states,
/// and otherwise [`ROOT_DIRECTORY`] in the home directory. It is never the launch directory, which
/// receives nothing the operator did not ask for by name.
pub fn default_root() -> Result<PathBuf, RootError> {
    DataRoot::default_path()
}

/// Refuse to begin beside state an earlier build wrote into the launch directory, naming it and
/// both ways to proceed.
///
/// Such a directory is read where it stands or left alone; it is never copied into the root this
/// build addresses and never opened as if the default had found it.
pub fn refuse_earlier_layout(root: &Path) -> Result<(), RootError> {
    let project = std::env::current_dir().map_err(|source| RootError::Io {
        path: PathBuf::from("."),
        source,
    })?;
    DataRoot::refuse_earlier_layout_beside(&project, root)
}

/// The store this invocation acts on, addressed under `root` for the directory the product was
/// started in.
pub fn store_under(root: &Path, intent: StoreIntent) -> Result<PathBuf, RootError> {
    DataRoot::open(root)?.store(intent)
}
