pub mod repository;

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
        let mut paths = Vec::new();
        visit_project_files(&self.directory, &self.root, |relative, _| {
            paths.push(relative);
            Ok(())
        })?;
        // Match the relative UTF-8 key ordering used by workspace fingerprints.
        paths.sort_unstable();
        Ok(paths.into_iter().map(|p| self.directory.join(p)).collect())
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

fn visit_project_files(
    directory: &Path,
    metadata: &Path,
    mut visit: impl FnMut(String, &walkdir::DirEntry) -> Result<()>,
) -> Result<()> {
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
        let relative = relative_filename(directory, entry.path())?;
        if entry.file_type().is_symlink() || entry.file_type().is_file() {
            visit(relative, &entry)?;
        }
    }
    Ok(())
}

fn relative_filename(directory: &Path, path: &Path) -> Result<String> {
    Ok(path
        .strip_prefix(directory)?
        .to_str()
        .context("Non-UTF8 project filename")?
        .to_owned())
}

fn fingerprint(directory: &Path, metadata: &Path) -> Result<BTreeMap<String, String>> {
    let mut files = BTreeMap::new();
    visit_project_files(directory, metadata, |relative, entry| {
        let mut hash = Sha256::new();
        if entry.file_type().is_symlink() {
            hash.update(b"symlink:");
            hash.update(
                std::fs::read_link(entry.path())?
                    .to_string_lossy()
                    .as_bytes(),
            );
        } else {
            let mut file = std::fs::File::open(entry.path())?;
            let mut buffer = [0u8; 65536];
            loop {
                let count = file.read(&mut buffer)?;
                #[cfg(test)]
                tests::REGULAR_FILE_BYTES_READ.with(|bytes| bytes.set(bytes.get() + count));
                if count == 0 {
                    break;
                }
                hash.update(&buffer[..count]);
            }
        }
        files.insert(relative, format!("{:x}", hash.finalize()));
        Ok(())
    })?;
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;

    thread_local! {
        pub(super) static REGULAR_FILE_BYTES_READ: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    }

    #[test]
    fn listing_skips_regular_file_contents_but_change_detection_reads_them() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("project");
        let state = temp.path().join("state");
        std::fs::create_dir(&source).unwrap();
        let contents = vec![b'a'; 128 * 1024 + 1];
        std::fs::write(source.join("large.bin"), &contents).unwrap();
        REGULAR_FILE_BYTES_READ.set(0);
        let workspace = Workspace::open(&source, &state).unwrap();
        assert_eq!(REGULAR_FILE_BYTES_READ.get(), contents.len());

        REGULAR_FILE_BYTES_READ.set(0);
        assert_eq!(
            workspace.files().unwrap(),
            vec![workspace.directory.join("large.bin")]
        );
        assert_eq!(REGULAR_FILE_BYTES_READ.get(), 0);

        std::fs::write(source.join("large.bin"), vec![b'b'; contents.len()]).unwrap();
        let changes = workspace.changes().unwrap();
        assert_eq!(REGULAR_FILE_BYTES_READ.get(), contents.len());
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].path, workspace.directory.join("large.bin"));
        assert_eq!(changes[0].status, "modified");
    }

    #[test]
    fn listing_preserves_sorted_names_and_excludes_ignored_paths_and_nested_metadata() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("project");
        let state = source.join("nested/session-state");
        for relative in [
            "z.txt",
            "a/child.txt",
            "a.rs",
            ".ordinary",
            "nested/session-state-sibling/keep.txt",
            "nested/session-state/private.json",
            "café.txt",
            "a space.txt",
        ] {
            let path = source.join(relative);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, "fixture").unwrap();
        }
        for ignored in [
            ".git",
            "node_modules",
            "target",
            "__pycache__",
            ".DS_Store",
            ".ymp2",
        ] {
            for prefix in ["", "nested/"] {
                let directory = source.join(format!("{prefix}{ignored}"));
                std::fs::create_dir_all(&directory).unwrap();
                std::fs::write(directory.join("excluded.txt"), "ignored").unwrap();
            }
            let file = source.join("ignored-files").join(ignored);
            std::fs::create_dir_all(file.parent().unwrap()).unwrap();
            std::fs::write(file, "ignored").unwrap();
        }
        let workspace = Workspace::open(&source, &state).unwrap();
        let expected: Vec<_> = [
            ".ordinary",
            "a space.txt",
            "a.rs",
            "a/child.txt",
            "café.txt",
            "nested/session-state-sibling/keep.txt",
            "z.txt",
        ]
        .map(|relative| workspace.directory.join(relative))
        .into();
        assert_eq!(workspace.files().unwrap(), expected);
        assert_eq!(
            fingerprint(&workspace.directory, &workspace.root)
                .unwrap()
                .keys()
                .map(|relative| workspace.directory.join(relative))
                .collect::<Vec<_>>(),
            expected
        );
        assert!(workspace.changes().unwrap().is_empty());

        std::fs::remove_file(source.join("z.txt")).unwrap();
        std::fs::write(source.join("new.txt"), "new").unwrap();
        let changes = workspace.changes().unwrap();
        assert_eq!(changes.len(), 2);
        assert_eq!(changes[0].path, workspace.directory.join("new.txt"));
        assert_eq!(changes[0].status, "created");
        assert_eq!(changes[1].path, workspace.directory.join("z.txt"));
        assert_eq!(changes[1].status, "deleted");
    }

    #[cfg(unix)]
    #[test]
    fn listing_keeps_symlinks_without_following_them_and_hashes_link_targets() {
        use std::os::unix::{fs::symlink, net::UnixListener};

        // A short root also keeps socket paths below the Unix socket path limit.
        let temp = tempfile::tempdir_in("/tmp").unwrap();
        let source = temp.path().join("project");
        std::fs::create_dir(&source).unwrap();
        let external = temp.path().join("external");
        std::fs::create_dir(&external).unwrap();
        std::fs::write(external.join("target.txt"), "first").unwrap();
        symlink(&external, source.join("directory-link")).unwrap();
        symlink(external.join("target.txt"), source.join("file-link")).unwrap();
        symlink("missing", source.join("broken-link")).unwrap();
        let _socket = UnixListener::bind(source.join("socket")).unwrap();
        let workspace = Workspace::open(&source, &temp.path().join("state")).unwrap();
        assert_eq!(
            workspace.files().unwrap(),
            ["broken-link", "directory-link", "file-link"]
                .map(|relative| workspace.directory.join(relative))
        );
        assert_eq!(workspace.initial.len(), 3);

        std::fs::write(external.join("target.txt"), "second").unwrap();
        assert!(workspace.changes().unwrap().is_empty());
        std::fs::remove_file(source.join("file-link")).unwrap();
        symlink(external.join("other.txt"), source.join("file-link")).unwrap();
        let changes = workspace.changes().unwrap();
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].path, workspace.directory.join("file-link"));
        assert_eq!(changes[0].status, "modified");
    }

    #[cfg(unix)]
    #[test]
    fn filename_validation_rejects_non_utf8_relative_paths() {
        use std::{ffi::OsString, os::unix::ffi::OsStringExt};

        // Some filesystems reject non-UTF8 names at creation, so validate paths directly.
        let source = Path::new("/project");
        let invalid = source.join(OsString::from_vec(b"invalid-\xff".to_vec()));
        for path in [&invalid, &invalid.join("child.txt")] {
            assert_eq!(
                relative_filename(source, path).unwrap_err().to_string(),
                "Non-UTF8 project filename"
            );
        }
    }

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
