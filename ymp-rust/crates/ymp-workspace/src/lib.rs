use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    io::Read,
    path::{Path, PathBuf},
};

/// The user's actual working directory. The metadata directory contains hashes
/// and change records only; project files are never copied into the application home.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workspace {
    pub root: PathBuf,
    pub directory: PathBuf,
    initial: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileChange {
    pub path: PathBuf,
    pub status: String,
}

impl Workspace {
    pub fn open(directory: &Path, metadata: &Path) -> Result<Self> {
        let directory = directory.canonicalize()?;
        if metadata.join("workspace.json").exists() {
            let workspace = Self::load(metadata)?;
            if workspace.directory != directory {
                bail!("Stored workspace belongs to a different working directory");
            }
            return Ok(workspace);
        }
        std::fs::create_dir_all(metadata)?;
        let root = metadata.canonicalize()?;
        let initial = fingerprint(&directory, &root)?;
        let workspace = Self {
            root,
            directory,
            initial,
        };
        std::fs::write(
            workspace.root.join("workspace.json"),
            serde_json::to_vec_pretty(&workspace)?,
        )?;
        Ok(workspace)
    }
    pub fn load(metadata: &Path) -> Result<Self> {
        Ok(serde_json::from_slice(&std::fs::read(
            metadata.join("workspace.json"),
        )?)?)
    }
    pub fn files(&self) -> Result<Vec<PathBuf>> {
        Ok(fingerprint(&self.directory, &self.root)?
            .keys()
            .map(|p| self.directory.join(p))
            .collect())
    }
    pub fn changes(&self) -> Result<Vec<FileChange>> {
        let current = fingerprint(&self.directory, &self.root)?;
        let mut changes = Vec::new();
        for (path, hash) in &current {
            if self.initial.get(path) != Some(hash) {
                changes.push(FileChange {
                    path: self.directory.join(path),
                    status: if self.initial.contains_key(path) {
                        "modified"
                    } else {
                        "created"
                    }
                    .into(),
                });
            }
        }
        for path in self.initial.keys().filter(|p| !current.contains_key(*p)) {
            changes.push(FileChange {
                path: self.directory.join(path),
                status: "deleted".into(),
            });
        }
        Ok(changes)
    }
    pub fn diff(&self) -> Result<String> {
        let changes = self.changes()?;
        if changes.is_empty() {
            return Ok("No file changes recorded for this session.".into());
        }
        Ok(changes
            .iter()
            .map(|c| format!("{}  {}", c.status, c.path.display()))
            .collect::<Vec<_>>()
            .join("\n"))
    }
    pub fn save_changes(&self) -> Result<()> {
        std::fs::write(
            self.root.join("changes.json"),
            serde_json::to_vec_pretty(&self.changes()?)?,
        )?;
        Ok(())
    }
}

fn fingerprint(directory: &Path, metadata: &Path) -> Result<BTreeMap<String, String>> {
    let mut files = BTreeMap::new();
    for entry in walkdir::WalkDir::new(directory)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| {
            e.depth() == 0
                || (!e.path().starts_with(metadata)
                    && ![
                        ".git",
                        "node_modules",
                        "target",
                        "__pycache__",
                        ".DS_Store",
                        ".ymp2",
                    ]
                    .contains(&e.file_name().to_string_lossy().as_ref()))
        })
    {
        let entry = entry?;
        if entry.depth() == 0 || entry.file_type().is_dir() {
            continue;
        }
        let relative = entry
            .path()
            .strip_prefix(directory)?
            .to_str()
            .context("Non-UTF8 project filename")?
            .to_owned();
        let mut hash = Sha256::new();
        if entry.file_type().is_symlink() {
            hash.update(b"symlink:");
            hash.update(
                std::fs::read_link(entry.path())?
                    .to_string_lossy()
                    .as_bytes(),
            );
        } else if entry.file_type().is_file() {
            let mut file = std::fs::File::open(entry.path())?;
            let mut buffer = [0u8; 65536];
            loop {
                let count = file.read(&mut buffer)?;
                if count == 0 {
                    break;
                }
                hash.update(&buffer[..count]);
            }
        } else {
            continue;
        }
        files.insert(relative, format!("{:x}", hash.finalize()));
    }
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn writes_stay_in_the_working_directory_and_home_contains_only_metadata() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("project");
        let state = temp.path().join("state");
        std::fs::create_dir(&source).unwrap();
        std::fs::write(source.join("existing.txt"), "original").unwrap();
        let workspace = Workspace::open(&source, &state).unwrap();
        std::fs::write(
            workspace.directory.join("index.html"),
            "<!doctype html><h1>Hello</h1>",
        )
        .unwrap();
        std::fs::write(workspace.directory.join("existing.txt"), "updated").unwrap();
        workspace.save_changes().unwrap();
        assert!(source.join("index.html").exists());
        assert!(!source.join(".git").exists());
        assert!(!state.join("index.html").exists());
        assert!(!state.join("existing.txt").exists());
        let changes = workspace.changes().unwrap();
        assert_eq!(changes.len(), 2);
        assert!(changes.iter().any(|c| c.status == "created"
            && c.path == source.canonicalize().unwrap().join("index.html")));
        assert!(!std::fs::read_to_string(state.join("workspace.json"))
            .unwrap()
            .contains("original"));
        assert_eq!(
            Workspace::open(&source, &state)
                .unwrap()
                .changes()
                .unwrap()
                .len(),
            2
        );
    }
}
