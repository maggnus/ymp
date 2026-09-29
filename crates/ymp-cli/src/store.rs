//! Reading the durable store. Nothing here starts a session, calls a model or
//! touches the terminal interface; the non-interactive subcommands use only this.
use std::{
    path::{Component, Path, PathBuf},
    sync::Arc,
};
use ymp_runtime::{
    application::{Application, SessionView},
    backends::scripted_team::ScriptedTeam,
    domain::{Denial, Id, Result},
    kernel::journal::ParameterSchemas,
};
use ymp_storage::journal::SqliteJournal;

const JOURNAL: &str = "journal.sqlite";

fn refused(message: &str) -> Denial {
    Denial::new("store", message)
}
/// Where a directory that may not exist yet would be, with every existing part
/// resolved. Nothing is created.
pub fn resolve(store: &Path) -> Result<PathBuf> {
    let unresolved = || refused("Cannot resolve the store directory");
    let absolute = std::path::absolute(store).map_err(|_| unresolved())?;
    if absolute
        .components()
        .any(|part| matches!(part, Component::ParentDir))
    {
        return Err(unresolved());
    }
    let mut missing = vec![];
    let mut existing = absolute.as_path();
    loop {
        if let Ok(base) = existing.canonicalize() {
            return Ok(missing
                .iter()
                .rev()
                .fold(base, |path, name| path.join(name)));
        }
        missing.push(existing.file_name().ok_or_else(unresolved)?);
        existing = existing.parent().ok_or_else(unresolved)?;
    }
}
/// Open the journal of a store directory. Reading never creates a store.
pub fn journal(store: &Path, create: bool) -> Result<Arc<SqliteJournal>> {
    let file = store.join(JOURNAL);
    if create {
        std::fs::create_dir_all(store).map_err(|_| refused("Cannot create the store directory"))?;
    } else if !std::fs::metadata(&file).is_ok_and(|found| found.is_file() && found.len() > 0) {
        return Err(refused("The store directory holds no journal"));
    }
    let mut schemas = ParameterSchemas::default();
    ScriptedTeam::register(&mut schemas)?;
    Ok(Arc::new(SqliteJournal::open(file, schemas)?))
}
/// A read of the recorded projection. It grants nothing and drives nothing.
pub fn view(journal: &Arc<SqliteJournal>, session: &Id) -> Result<SessionView> {
    Application::new(journal.clone()).view(session, None)
}
/// The recorded status in words; a session without one says so.
pub fn status(view: &SessionView) -> String {
    match serde_json::to_value(view.status()) {
        Ok(serde_json::Value::String(text)) => text,
        Ok(serde_json::Value::Object(parts)) => parts
            .iter()
            .map(|(name, value)| match value.as_str() {
                Some(text) => format!("{name}({text})"),
                None => format!("{name}({value})"),
            })
            .collect::<Vec<_>>()
            .join(" "),
        _ => "not recorded".into(),
    }
}
