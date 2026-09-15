//! Descriptor-relative Direct capture. This adapter grants no native write access.
use rustix::fs::{self as rfs, AtFlags, Dir, FileType, Mode, OFlags};
use std::{
    collections::BTreeMap,
    fs::{File, Metadata},
    io::Read,
    os::unix::fs::MetadataExt,
    path::Path,
};
use ymp_domain::{Denial, Digest, Result, journal::PolicySelection, workspace::*};
use ymp_kernel::{journal::ContentStore, ports::execution::WorkspaceProvider};

pub struct Direct {
    root: File,
    location: WorkspaceLocation,
    selection: PolicySelection,
    limits: CaptureLimits,
}
fn io_error(error: impl std::fmt::Display) -> Denial {
    Denial::new("workspace_io", format!("Workspace access failed: {error}"))
}
fn changed() -> Denial {
    Denial::new("workspace_changed", "Workspace changed during inspection")
}
fn same(left: &Metadata, right: &Metadata) -> bool {
    left.dev() == right.dev()
        && left.ino() == right.ino()
        && left.mode() == right.mode()
        && left.len() == right.len()
        && left.nlink() == right.nlink()
        && left.mtime() == right.mtime()
        && left.mtime_nsec() == right.mtime_nsec()
        && left.ctime() == right.ctime()
        && left.ctime_nsec() == right.ctime_nsec()
}
fn permissions(metadata: &Metadata) -> Result<u32> {
    if metadata.mode() & 0o7000 != 0 {
        return Err(Denial::new(
            "workspace_mode",
            "Special permission bits are not supported in snapshots",
        ));
    }
    Ok(metadata.mode() & 0o777)
}
impl Direct {
    pub fn open(root: &Path, limits: CaptureLimits) -> Result<Self> {
        limits.validate()?;
        let canonical = root.canonicalize().map_err(io_error)?;
        let path = canonical
            .to_str()
            .ok_or_else(|| Denial::new("workspace_path", "Workspace path must be UTF-8"))?
            .to_owned();
        let root = File::from(
            rfs::open(
                &canonical,
                OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .map_err(io_error)?,
        );
        let metadata = root.metadata().map_err(io_error)?;
        let location = WorkspaceLocation {
            root: path,
            device: metadata.dev(),
            inode: metadata.ino(),
        };
        location.validate()?;
        let selection = PolicySelection::new(
            "WorkspaceProvider",
            "Direct",
            "1",
            serde_json::to_value(&limits).map_err(io_error)?,
        )?;
        let provider = Self {
            root,
            location,
            selection,
            limits,
        };
        provider.location()?;
        Ok(provider)
    }
    fn entries(&self, directory: &File) -> Result<Vec<String>> {
        let mut names = vec![];
        for entry in Dir::read_from(directory).map_err(io_error)? {
            let entry = entry.map_err(io_error)?;
            let name = entry.file_name().to_str().map_err(io_error)?;
            if name == "." || name == ".." {
                continue;
            }
            WorkspacePath::new(name)?;
            names.push(name.to_owned());
            if names.len() > self.limits.max_entries {
                return Err(Denial::new(
                    "capture_limit",
                    "Directory exceeds the capture entry limit",
                ));
            }
        }
        names.sort();
        Ok(names)
    }
    fn child(&self, parent: &File, name: &str) -> Result<File> {
        let stat = rfs::statat(parent, name, AtFlags::SYMLINK_NOFOLLOW).map_err(io_error)?;
        let kind = FileType::from_raw_mode(stat.st_mode);
        let flags = match kind {
            FileType::Directory => OFlags::DIRECTORY,
            FileType::RegularFile => OFlags::empty(),
            _ => {
                return Err(Denial::new(
                    "workspace_type",
                    "Symlinks and special files are not supported",
                ));
            }
        };
        // NONBLOCK prevents a concurrent replacement with a FIFO from hanging open.
        let file = File::from(
            rfs::openat(
                parent,
                name,
                OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK | flags,
                Mode::empty(),
            )
            .map_err(io_error)?,
        );
        let metadata = file.metadata().map_err(io_error)?;
        if metadata.dev() != self.location.device
            || metadata.dev() != stat.st_dev as u64
            || metadata.ino() != stat.st_ino as u64
            || (!metadata.is_dir() && !metadata.is_file())
        {
            return Err(changed());
        }
        if metadata.is_file() && metadata.nlink() != 1 {
            return Err(Denial::new(
                "workspace_alias",
                "Hard-linked files cannot establish distinct path ownership",
            ));
        }
        Ok(file)
    }
    fn walk(
        &self,
        directory: &File,
        path: &WorkspacePath,
        depth: usize,
        tree: &mut SnapshotTree,
        total: &mut usize,
        store: &dyn ContentStore,
    ) -> Result<()> {
        if depth > self.limits.max_depth {
            return Err(Denial::new(
                "capture_limit",
                "Workspace nesting exceeds the capture limit",
            ));
        }
        let before = directory.metadata().map_err(io_error)?;
        tree.directories.insert(path.clone(), permissions(&before)?);
        let names = self.entries(directory)?;
        for name in &names {
            if tree.files.len() + tree.directories.len() >= self.limits.max_entries {
                return Err(Denial::new(
                    "capture_limit",
                    "Workspace exceeds the capture entry limit",
                ));
            }
            let relative = WorkspacePath::new(if path.as_str() == "." {
                name.clone()
            } else {
                format!("{}/{name}", path.as_str())
            })?;
            let mut file = self.child(directory, name)?;
            let metadata = file.metadata().map_err(io_error)?;
            if metadata.is_dir() {
                self.walk(&file, &relative, depth + 1, tree, total, store)?;
            } else {
                let limit = self
                    .limits
                    .max_file_bytes
                    .min(self.limits.max_total_bytes - *total);
                if metadata.len() > limit as u64 {
                    return Err(Denial::new(
                        "capture_limit",
                        "Workspace file exceeds the byte limit",
                    ));
                }
                let mut bytes = Vec::new();
                Read::by_ref(&mut file)
                    .take(limit as u64 + 1)
                    .read_to_end(&mut bytes)
                    .map_err(io_error)?;
                if bytes.len() > limit {
                    return Err(Denial::new(
                        "capture_limit",
                        "Workspace file grew past the byte limit",
                    ));
                }
                if metadata.len() != bytes.len() as u64
                    || !same(&metadata, &file.metadata().map_err(io_error)?)
                {
                    return Err(changed());
                }
                let digest = Digest::of(&bytes);
                if store.put(&bytes)? != digest {
                    return Err(Denial::new(
                        "content_digest",
                        "Content store returned the wrong file digest",
                    ));
                }
                *total += bytes.len();
                tree.files.insert(
                    relative,
                    SnapshotFile {
                        digest,
                        bytes: bytes.len() as u64,
                        mode: permissions(&metadata)?,
                    },
                );
            }
        }
        if names != self.entries(directory)?
            || !same(&before, &directory.metadata().map_err(io_error)?)
        {
            return Err(changed());
        }
        Ok(())
    }
    fn capture_once(&self, store: &dyn ContentStore) -> Result<SnapshotTree> {
        self.location()?;
        let mut tree = SnapshotTree {
            files: BTreeMap::new(),
            directories: BTreeMap::new(),
        };
        self.walk(
            &self.root,
            &WorkspacePath::root(),
            0,
            &mut tree,
            &mut 0,
            store,
        )?;
        self.location()?;
        tree.validate()?;
        Ok(tree)
    }
}
impl WorkspaceProvider for Direct {
    fn selection(&self) -> &PolicySelection {
        &self.selection
    }
    fn location(&self) -> Result<WorkspaceLocation> {
        let now = File::from(
            rfs::open(
                self.location.root.as_str(),
                OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .map_err(io_error)?,
        )
        .metadata()
        .map_err(io_error)?;
        if now.dev() != self.location.device || now.ino() != self.location.inode {
            return Err(changed());
        }
        Ok(self.location.clone())
    }
    fn validate_paths(&self, paths: &[WorkspacePath]) -> Result<()> {
        self.location()?;
        for path in paths {
            if path.as_str() == "." {
                continue;
            }
            let mut directory = self.root.try_clone().map_err(io_error)?;
            let components: Vec<_> = path.as_str().split('/').collect();
            for (index, name) in components.iter().enumerate() {
                match rfs::statat(&directory, *name, AtFlags::SYMLINK_NOFOLLOW) {
                    Err(rustix::io::Errno::NOENT) => break,
                    Err(error) => return Err(io_error(error)),
                    Ok(_) => {
                        let next = self.child(&directory, name)?;
                        if index + 1 < components.len()
                            && !next.metadata().map_err(io_error)?.is_dir()
                        {
                            return Err(Denial::new(
                                "workspace_path",
                                "A path ancestor is not a directory",
                            ));
                        }
                        directory = next;
                    }
                }
            }
        }
        self.location()?;
        Ok(())
    }
    fn observe_paths(&self, paths: &[WorkspacePath]) -> Result<Vec<PathObservation>> {
        self.location()?;
        let mut observations = vec![];
        for relative in paths {
            let mut directory = File::from(
                rfs::open(
                    "/",
                    OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
                    Mode::empty(),
                )
                .map_err(io_error)?,
            );
            let metadata = directory.metadata().map_err(io_error)?;
            let mut existing = vec![PathComponent {
                name: "/".into(),
                identity: FileIdentity {
                    device: metadata.dev(),
                    inode: metadata.ino(),
                },
            }];
            let full = if relative.as_str() == "." {
                self.location.root.clone()
            } else {
                format!(
                    "{}/{}",
                    self.location.root.trim_end_matches('/'),
                    relative.as_str()
                )
            };
            let names: Vec<_> = full.split('/').filter(|name| !name.is_empty()).collect();
            let root_depth = self
                .location
                .root
                .split('/')
                .filter(|name| !name.is_empty())
                .count();
            let mut missing = vec![];
            for (index, name) in names.iter().enumerate() {
                let stat = match rfs::statat(&directory, *name, AtFlags::SYMLINK_NOFOLLOW) {
                    Ok(stat) => stat,
                    Err(rustix::io::Errno::NOENT) if index >= root_depth => {
                        missing = names[index..].iter().map(|s| (*s).to_owned()).collect();
                        break;
                    }
                    Err(error) => return Err(io_error(error)),
                };
                let kind = FileType::from_raw_mode(stat.st_mode);
                if kind != FileType::Directory
                    && (kind != FileType::RegularFile || index + 1 < names.len())
                {
                    return Err(Denial::new(
                        "workspace_type",
                        "A scoped path cannot traverse links, special files or regular-file ancestors",
                    ));
                }
                let flags = if kind == FileType::Directory {
                    OFlags::DIRECTORY
                } else {
                    OFlags::empty()
                };
                let file = File::from(
                    rfs::openat(
                        &directory,
                        *name,
                        OFlags::RDONLY
                            | OFlags::NOFOLLOW
                            | OFlags::CLOEXEC
                            | OFlags::NONBLOCK
                            | flags,
                        Mode::empty(),
                    )
                    .map_err(io_error)?,
                );
                let metadata = file.metadata().map_err(io_error)?;
                if metadata.dev() != stat.st_dev as u64
                    || metadata.ino() != stat.st_ino as u64
                    || (index >= root_depth && metadata.dev() != self.location.device)
                    || (metadata.is_file() && metadata.nlink() != 1)
                    || (!metadata.is_file() && !metadata.is_dir())
                {
                    return Err(changed());
                }
                existing.push(PathComponent {
                    name: (*name).into(),
                    identity: FileIdentity {
                        device: metadata.dev(),
                        inode: metadata.ino(),
                    },
                });
                directory = file;
            }
            let observation = PathObservation {
                path: relative.clone(),
                existing,
                missing,
            };
            observation.validate(&self.location)?;
            observations.push(observation);
        }
        self.location()?;
        Ok(observations)
    }
    fn capture(&self, store: &dyn ContentStore) -> Result<SnapshotTree> {
        let first = self.capture_once(store)?;
        let second = self.capture_once(store)?;
        if first != second {
            return Err(changed());
        }
        // Two matching bounded scans detect changes; they are not an atomic OS snapshot.
        Ok(first)
    }
}
