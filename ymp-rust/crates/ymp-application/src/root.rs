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

pub use ymp_storage::{DataRoot, ROOT_DIRECTORY, RootError, StoreIntent, YMP_HOME};

/// The root every durable path lives under when the operator names none: what [`YMP_HOME`] states,
/// and otherwise [`ROOT_DIRECTORY`] in the home directory. It is never the launch directory, which
/// receives nothing the operator did not ask for by name.
pub fn default_root() -> Result<PathBuf, RootError> {
    DataRoot::default_path()
}

/// The store this invocation acts on, addressed under `root` for the directory the product was
/// started in.
pub fn store_under(root: &Path, intent: StoreIntent) -> Result<PathBuf, RootError> {
    DataRoot::open(root)?.store(intent)
}
