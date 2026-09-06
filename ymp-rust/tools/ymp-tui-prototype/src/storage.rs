//! Atomic, single-writer prototype checkpoints. Real ~/.ymp state and project source are untouched.
use crate::{entities::Export, resources::Kind, simulation::Simulation};
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

const SCHEMA: u32 = 1;
const MAX_STATE: u64 = 128 * 1024 * 1024;
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Bookmark {
    pub kind: Option<Kind>,
    pub query: String,
    pub selected_id: Option<String>,
    pub sort_column: Option<usize>,
    pub descending: bool,
    pub filter: String,
    pub show_archived: bool,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct UiState {
    pub draft: String,
    pub cursor: usize,
    pub chat_filter: String,
    pub sidebar: bool,
    pub current: Option<Bookmark>,
    pub tables: Vec<Bookmark>,
    pub detail_id: Option<String>,
    pub expanded_tools: Vec<String>,
    pub paused: bool,
    pub drafts: std::collections::BTreeMap<String, (String, usize)>,
}
impl Default for UiState {
    fn default() -> Self {
        Self {
            draft: String::new(),
            cursor: 0,
            chat_filter: String::new(),
            sidebar: true,
            current: None,
            tables: vec![],
            detail_id: None,
            expanded_tools: vec![],
            paused: false,
            drafts: Default::default(),
        }
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Checkpoint {
    schema: u32,
    saved_at: u64,
    simulation: Simulation,
    ui: UiState,
}
pub struct Store {
    path: PathBuf,
    _lock: File,
}
pub fn clock_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis().min(u64::MAX as u128) as u64)
}
impl Store {
    pub fn default_path() -> Result<PathBuf> {
        let home = std::env::var_os("HOME").context("HOME is unavailable; specify --state FILE")?;
        Ok(PathBuf::from(home).join(".ymp-prototype/state.json"))
    }
    pub fn open(path: PathBuf) -> Result<Self> {
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        fs::create_dir_all(parent)
            .with_context(|| format!("Cannot create {}", parent.display()))?;
        let mut lockname = path.as_os_str().to_os_string();
        lockname.push(".lock");
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(PathBuf::from(lockname))?;
        fs2::FileExt::try_lock_exclusive(&lock)
            .with_context(|| format!("Another ymp instance owns {}", path.display()))?;
        Ok(Self { path, _lock: lock })
    }
    fn parent(&self) -> &Path {
        self.path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."))
    }
    pub fn load(&self) -> Result<Option<(Simulation, UiState)>> {
        let mut file = match File::open(&self.path) {
            Ok(f) => f,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e.into()),
        };
        if file.metadata()?.len() > MAX_STATE {
            bail!("Checkpoint exceeds the supported size; the file was not changed.");
        }
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)?;
        let mut state: Checkpoint = serde_json::from_slice(&bytes).with_context(|| {
            format!(
                "Cannot read checkpoint {}; original file preserved",
                self.path.display()
            )
        })?;
        if state.schema != SCHEMA {
            bail!(
                "Unsupported checkpoint schema {}; original file preserved",
                state.schema
            );
        }
        state
            .simulation
            .validate()
            .context("Invalid checkpoint; original file preserved")?;
        if state.ui.cursor > state.ui.draft.len()
            || !state.ui.draft.is_char_boundary(state.ui.cursor)
        {
            bail!("Invalid saved cursor; original file preserved");
        }
        if state
            .ui
            .draft
            .chars()
            .any(|c| c.is_control() && c != '\n' && c != '\t')
        {
            bail!("Invalid control character in saved draft");
        }
        for (text, cursor) in state.ui.drafts.values() {
            if *cursor > text.len() || !text.is_char_boundary(*cursor) {
                bail!("Invalid stored draft cursor");
            }
        }
        fn safe_text(v: &serde_json::Value) -> bool {
            match v {
                serde_json::Value::String(s) => {
                    !s.chars().any(|c| c.is_control() && c != '\n' && c != '\t')
                }
                serde_json::Value::Array(a) => a.iter().all(safe_text),
                serde_json::Value::Object(o) => o.values().all(safe_text),
                _ => true,
            }
        }
        if !safe_text(&serde_json::to_value(&state.ui)?) {
            bail!("Invalid control character in saved interface state");
        }
        state
            .simulation
            .recover(clock_ms().saturating_sub(state.saved_at));
        Ok(Some((state.simulation, state.ui)))
    }
    pub fn save(&self, simulation: &Simulation, ui: &UiState) -> Result<()> {
        simulation.validate()?;
        let state = Checkpoint {
            schema: SCHEMA,
            saved_at: clock_ms(),
            simulation: simulation.clone(),
            ui: ui.clone(),
        };
        let bytes = serde_json::to_vec(&state)?;
        if bytes.len() as u64 > MAX_STATE {
            bail!("Checkpoint is too large; the previous file was preserved.");
        }
        let mut tmp = tempfile::NamedTempFile::new_in(self.parent())?;
        tmp.write_all(&bytes)?;
        tmp.as_file().sync_all()?;
        tmp.persist(&self.path)
            .map_err(|e| e.error)
            .context("Checkpoint replacement failed; previous file preserved")?;
        File::open(self.parent())?.sync_all()?;
        Ok(())
    }
    pub fn export(&self, simulation: &mut Simulation, kind: &str, id: &str) -> Result<PathBuf> {
        let (task, value) = simulation.report(kind, id).map_err(anyhow::Error::msg)?;
        let document = serde_json::json!({"format":"ymp-simulation-report-v1","simulation":true,"kind":kind,"entity":id,"data":value});
        let bytes = serde_json::to_vec_pretty(&document)?;
        let digest = format!("{:x}", Sha256::digest(&bytes));
        let export_id = format!("E-{:04}", simulation.exports.len() + 1);
        let directory = self.parent().join("exports");
        fs::create_dir_all(&directory)?;
        let meta = fs::symlink_metadata(&directory)?;
        if !meta.file_type().is_dir() {
            bail!("Export directory is not an owned regular directory");
        }
        let mut file = tempfile::Builder::new()
            .prefix(&format!("{export_id}-"))
            .suffix(".json")
            .tempfile_in(&directory)?;
        file.write_all(&bytes)?;
        file.as_file().sync_all()?;
        let (_, path) = file.keep().map_err(|e| e.error)?;
        File::open(&directory)?.sync_all()?;
        let relative_path = path
            .strip_prefix(self.parent())?
            .to_string_lossy()
            .into_owned();
        simulation.exports.push(Export {
            id: export_id.clone(),
            task,
            kind: kind.into(),
            entity: id.into(),
            relative_path,
            created: simulation.now,
            removed: false,
            digest,
        });
        simulation.record(
            "operator",
            "Report exported",
            export_id,
            path.display().to_string(),
        );
        simulation.changed();
        Ok(path)
    }
    pub fn remove_export(&self, simulation: &mut Simulation, id: &str) -> Result<()> {
        let e = simulation
            .exports
            .iter()
            .find(|e| e.id == id)
            .context("Export no longer exists")?;
        if e.removed {
            bail!("This export was already removed.");
        }
        let relative = Path::new(&e.relative_path);
        if relative.components().count() != 2
            || relative.components().next()
                != Some(std::path::Component::Normal("exports".as_ref()))
            || relative.file_name().is_none()
        {
            bail!("Invalid export path; nothing removed");
        }
        let directory = self.parent().join("exports");
        if fs::symlink_metadata(&directory).is_ok_and(|m| !m.file_type().is_dir()) {
            bail!("Export directory was replaced; nothing removed");
        }
        let path = self.parent().join(relative);
        match fs::symlink_metadata(&path) {
            Ok(meta) => {
                if !meta.file_type().is_file() {
                    bail!("Export path no longer names a regular owned file; nothing removed");
                }
                let bytes = fs::read(&path)?;
                if format!("{:x}", Sha256::digest(&bytes)) != e.digest {
                    bail!("Export was modified outside ymp; nothing removed");
                }
                fs::remove_file(&path)?;
                File::open(path.parent().unwrap())?.sync_all()?;
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => return Err(err.into()),
        }
        simulation
            .exports
            .iter_mut()
            .find(|e| e.id == id)
            .unwrap()
            .removed = true;
        simulation.record(
            "operator",
            "Export removed",
            id,
            "Source records, evidence and project files retained",
        );
        simulation.changed();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        app::App,
        entities::{Family, Stage, Status},
    };
    #[test]
    fn checkpoints_are_atomic_exclusive_and_restore_the_draft() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("state.json");
        let store = Store::open(path.clone()).unwrap();
        assert!(Store::open(path.clone()).is_err());
        let mut s = Simulation::default();
        s.add_provider("Личный OpenAI", Family::OpenAi, true)
            .unwrap();
        s.create_task("Морской бой", "").unwrap();
        let ui = UiState {
            draft: "Черновик /tmp/世界?".into(),
            cursor: 26,
            ..UiState::default()
        };
        let ui = UiState {
            cursor: ui.draft.len(),
            ..ui
        };
        store.save(&s, &ui).unwrap();
        let old = fs::read(&path).unwrap();
        s.next_task = 1;
        assert!(store.save(&s, &ui).is_err());
        assert_eq!(fs::read(&path).unwrap(), old);
        drop(store);
        let store = Store::open(path).unwrap();
        let (s, restored) = store.load().unwrap().unwrap();
        assert_eq!(restored, ui);
        assert_eq!(s.providers[0].name, "Личный OpenAI");
        assert!(s.agents.is_empty());
        let app = App::from_checkpoint(s, restored);
        assert_eq!(app.draft, "Черновик /tmp/世界?");
    }
    #[test]
    fn interrupted_work_does_not_reappear_running_after_restart() {
        let root = tempfile::tempdir().unwrap();
        let store = Store::open(root.path().join("state.json")).unwrap();
        let app = App::scenario_scene("working").unwrap();
        let s = app.simulation.as_ref().unwrap();
        let agents = s.agents.len();
        store.save(s, &UiState::default()).unwrap();
        let (mut restored, _) = store.load().unwrap().unwrap();
        assert_eq!(restored.agents.len(), agents);
        assert_eq!(restored.task().unwrap().stage, Stage::Stopped);
        assert!(
            restored
                .agents
                .iter()
                .filter(|a| a.run > 0)
                .all(|a| a.ended.is_some())
        );
        assert!(restored.tools.iter().all(|t| t.ended.is_some()));
        restored.start(true).unwrap();
        assert_eq!(restored.task().unwrap().run, 2);
        assert_eq!(restored.agents.last().unwrap().state, Status::Starting);
        restored.validate().unwrap();
    }
    #[test]
    fn corrupt_or_future_checkpoints_are_not_replaced() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("state.json");
        let store = Store::open(path.clone()).unwrap();
        for bytes in [b"not json".as_slice(), br#"{"schema":9000}"#.as_slice()] {
            fs::write(&path, bytes).unwrap();
            assert!(store.load().is_err());
            assert_eq!(fs::read(&path).unwrap(), bytes);
        }
    }
    #[test]
    fn report_cleanup_only_removes_the_unchanged_owned_export() {
        let root = tempfile::tempdir().unwrap();
        let store = Store::open(root.path().join("state.json")).unwrap();
        let mut app = App::scenario_scene("review").unwrap();
        let s = app.simulation.as_mut().unwrap();
        let id = s.candidates.last().unwrap().id.clone();
        let path = store.export(s, "candidate", &id).unwrap();
        let report = fs::read(&path).unwrap();
        let export = s.exports.last().unwrap().id.clone();
        let checks = s.checks.len();
        fs::write(&path, b"operator changed this file").unwrap();
        assert!(store.remove_export(s, &export).is_err());
        assert_eq!(fs::read(&path).unwrap(), b"operator changed this file");
        assert!(!s.exports.last().unwrap().removed);
        fs::write(&path, report).unwrap();
        store.remove_export(s, &export).unwrap();
        assert!(!path.exists());
        assert_eq!(s.checks.len(), checks);
        assert!(s.exports.last().unwrap().removed);
        store.save(s, &UiState::default()).unwrap();
        let (loaded, _) = store.load().unwrap().unwrap();
        assert!(loaded.exports.last().unwrap().removed);
    }
    #[cfg(unix)]
    #[test]
    fn export_directory_symlinks_cannot_redirect_cleanup() {
        use std::os::unix::fs::symlink;
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let store = Store::open(root.path().join("state.json")).unwrap();
        let mut app = App::scenario_scene("review").unwrap();
        let s = app.simulation.as_mut().unwrap();
        let id = s.candidates.last().unwrap().id.clone();
        let exported = store.export(s, "candidate", &id).unwrap();
        let export = s.exports.last().unwrap().id.clone();
        let bytes = fs::read(&exported).unwrap();
        let filename = exported.file_name().unwrap();
        fs::rename(
            root.path().join("exports"),
            root.path().join("saved-exports"),
        )
        .unwrap();
        symlink(outside.path(), root.path().join("exports")).unwrap();
        let foreign = outside.path().join(filename);
        fs::write(&foreign, &bytes).unwrap();
        assert!(store.remove_export(s, &export).is_err());
        assert_eq!(fs::read(&foreign).unwrap(), bytes);
        assert!(store.export(s, "candidate", &id).is_err());
    }
}
