//! Descriptor-relative Direct capture. This adapter grants no native write access.
use rustix::fs::{self as rfs, AtFlags, Dir, FileType, Mode, OFlags};
use std::{
    collections::BTreeMap,
    fs::{File, Metadata},
    io::{Read, Write},
    os::unix::fs::MetadataExt,
    path::Path,
    sync::{Arc, Mutex},
};
use ymp_domain::{Denial, Digest, Result, journal::PolicySelection, workspace::*};
use ymp_kernel::{
    journal::{ContentStore, Journal},
    ports::execution::WorkspaceProvider,
};

pub struct Direct {
    root: File,
    location: WorkspaceLocation,
    selection: PolicySelection,
    limits: CaptureLimits,
    binding: Mutex<Option<Arc<super::binding::RootBinding>>>,
}
struct CaptureState {
    tree: SnapshotTree,
    total: usize,
    exclude_marker: bool,
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
            binding: Mutex::new(None),
        };
        provider.location()?;
        Ok(provider)
    }
    fn entries(&self, directory: &File, exclude_marker: bool) -> Result<Vec<String>> {
        let mut names = vec![];
        for entry in Dir::read_from(directory).map_err(io_error)? {
            let entry = entry.map_err(io_error)?;
            let name = entry.file_name().to_str().map_err(io_error)?;
            if name == "." || name == ".." {
                continue;
            }
            if exclude_marker && name == super::binding::MARKER {
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
    fn operation_binding(
        &self,
        access: &ymp_kernel::workspace_guard::FileAccess,
        mode: LockMode,
    ) -> Result<Arc<super::binding::RootBinding>> {
        if access.mode() != mode || access.path() == &WorkspacePath::root() {
            return Err(Denial::new(
                "file_operation",
                "Expected a permitted regular-file operation",
            ));
        }
        let first = access
            .path()
            .as_str()
            .split('/')
            .next()
            .expect("validated path");
        let folded = first.to_ascii_lowercase();
        if folded == super::binding::MARKER
            || folded.starts_with(&format!("{}.tmp-", super::binding::MARKER))
        {
            return Err(Denial::new(
                "binding_metadata",
                "Execution cannot access binding metadata",
            ));
        }
        let binding = self
            .binding
            .lock()
            .map_err(|_| Denial::new("workspace_binding", "Binding state is unavailable"))?
            .clone()
            .ok_or_else(|| Denial::new("binding_required", "Bind the root before file I/O"))?;
        if access.binding() != binding.record() {
            return Err(Denial::new(
                "binding_conflict",
                "File operation targets another workspace binding",
            ));
        }
        self.location()?;
        access.scope().validate(&self.location)?;
        access.target().validate(&self.location)?;
        if access.target().path != *access.path() {
            return Err(Denial::new(
                "path_observation",
                "Operation target does not match its path",
            ));
        }
        if !access.scope().missing.is_empty()
            && (access.scope().missing.len() != 1 || access.path() != &access.scope().path)
        {
            return Err(Denial::new(
                "workspace_changed",
                "An absent scope cannot become a directory",
            ));
        }
        if !access.scope().path.contains(access.path()) {
            return Err(Denial::new(
                "access_scope",
                "File is outside its observed scope",
            ));
        }
        binding.verify_marker()?;
        Ok(binding)
    }
    fn scope_identity<'a>(
        &self,
        access: &'a ymp_kernel::workspace_guard::FileAccess,
        depth: usize,
    ) -> Option<&'a FileIdentity> {
        let root_depth = self
            .location
            .root
            .split('/')
            .filter(|part| !part.is_empty())
            .count();
        access
            .scope()
            .existing
            .get(root_depth + depth)
            .map(|part| &part.identity)
            .or_else(|| {
                (depth == access.path().as_str().split('/').count())
                    .then(|| access.created_file())
                    .flatten()
            })
    }
    fn check_scope_component(
        &self,
        access: &ymp_kernel::workspace_guard::FileAccess,
        depth: usize,
        metadata: &Metadata,
    ) -> Result<()> {
        let root_depth = self
            .location
            .root
            .split('/')
            .filter(|part| !part.is_empty())
            .count();
        if access
            .target()
            .existing
            .get(root_depth + depth)
            .is_some_and(|expected| {
                expected.identity.device != metadata.dev()
                    || expected.identity.inode != metadata.ino()
            })
        {
            return Err(changed());
        }
        if self.scope_identity(access, depth).is_some_and(|expected| {
            expected.device != metadata.dev() || expected.inode != metadata.ino()
        }) {
            return Err(changed());
        }
        Ok(())
    }
    fn file_parent(
        &self,
        access: &ymp_kernel::workspace_guard::FileAccess,
    ) -> Result<(File, String)> {
        let path = access.path();
        let mut components = path.as_str().split('/').collect::<Vec<_>>();
        let name = components.pop().expect("validated file path").to_owned();
        let mut parent = self.root.try_clone().map_err(io_error)?;
        self.check_scope_component(access, 0, &parent.metadata().map_err(io_error)?)?;
        for (index, component) in components.into_iter().enumerate() {
            let next = self.child(&parent, component)?;
            self.check_scope_component(access, index + 1, &next.metadata().map_err(io_error)?)?;
            if !next.metadata().map_err(io_error)?.is_dir() {
                return Err(Denial::new(
                    "workspace_path",
                    "A file ancestor is not a directory",
                ));
            }
            parent = next;
        }
        Ok((parent, name))
    }
    fn check_operation_file(
        &self,
        file: &File,
        binding: &super::binding::RootBinding,
    ) -> Result<Metadata> {
        let metadata = file.metadata().map_err(io_error)?;
        if !metadata.is_file() || metadata.dev() != self.location.device || metadata.nlink() != 1 {
            return Err(Denial::new(
                "workspace_type",
                "File operations require an unaliased regular file on the root device",
            ));
        }
        if binding.is_marker(&metadata) {
            return Err(Denial::new(
                "binding_metadata",
                "Execution cannot access a physical alias of the binding marker",
            ));
        }
        permissions(&metadata)?;
        Ok(metadata)
    }
    fn walk(
        &self,
        directory: &File,
        path: &WorkspacePath,
        depth: usize,
        state: &mut CaptureState,
        store: &dyn ContentStore,
    ) -> Result<()> {
        if depth > self.limits.max_depth {
            return Err(Denial::new(
                "capture_limit",
                "Workspace nesting exceeds the capture limit",
            ));
        }
        let before = directory.metadata().map_err(io_error)?;
        state
            .tree
            .directories
            .insert(path.clone(), permissions(&before)?);
        let exclude_here = state.exclude_marker && path == &WorkspacePath::root();
        let names = self.entries(directory, exclude_here)?;
        for name in &names {
            if state.tree.files.len() + state.tree.directories.len() >= self.limits.max_entries {
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
                self.walk(&file, &relative, depth + 1, state, store)?;
            } else {
                let limit = self
                    .limits
                    .max_file_bytes
                    .min(self.limits.max_total_bytes - state.total);
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
                state.total += bytes.len();
                state.tree.files.insert(
                    relative,
                    SnapshotFile {
                        digest,
                        bytes: bytes.len() as u64,
                        mode: permissions(&metadata)?,
                    },
                );
            }
        }
        if names != self.entries(directory, exclude_here)?
            || !same(&before, &directory.metadata().map_err(io_error)?)
        {
            return Err(changed());
        }
        Ok(())
    }
    fn capture_once(&self, store: &dyn ContentStore, exclude_marker: bool) -> Result<SnapshotTree> {
        self.location()?;
        let mut state = CaptureState {
            tree: SnapshotTree {
                files: BTreeMap::new(),
                directories: BTreeMap::new(),
            },
            total: 0,
            exclude_marker,
        };
        self.walk(&self.root, &WorkspacePath::root(), 0, &mut state, store)?;
        self.location()?;
        state.tree.validate()?;
        Ok(state.tree)
    }
}
struct DirectCoordination {
    _locks: super::binding::DirectoryLocks,
    binding: WorkspaceBinding,
}
impl ymp_kernel::ports::execution::WorkspaceCoordination for DirectCoordination {
    fn binding(&self) -> &WorkspaceBinding {
        &self.binding
    }
}
struct DirectFile {
    file: File,
    binding: Arc<super::binding::RootBinding>,
    mode: LockMode,
    limit: usize,
    observed: Metadata,
}
impl ymp_kernel::ports::execution::WorkspaceFile for DirectFile {
    fn identity(&self) -> FileIdentity {
        FileIdentity {
            device: self.observed.dev(),
            inode: self.observed.ino(),
        }
    }
    fn read(&mut self) -> Result<Vec<u8>> {
        if self.mode != LockMode::Read {
            return Err(Denial::new(
                "file_operation",
                "File was not opened for reading",
            ));
        }
        self.binding.verify_marker()?;
        let before = self.file.metadata().map_err(io_error)?;
        if !same(&self.observed, &before) || before.len() > self.limit as u64 {
            return Err(changed());
        }
        let mut bytes = Vec::new();
        Read::by_ref(&mut self.file)
            .take(self.limit as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(io_error)?;
        if bytes.len() > self.limit
            || bytes.len() as u64 != before.len()
            || !same(&before, &self.file.metadata().map_err(io_error)?)
        {
            return Err(changed());
        }
        self.binding.verify_marker()?;
        Ok(bytes)
    }
    fn write(&mut self, bytes: &[u8]) -> Result<()> {
        if self.mode != LockMode::Write || bytes.len() > self.limit {
            return Err(Denial::new(
                "file_operation",
                "Write exceeds its prepared permission",
            ));
        }
        self.binding.verify_marker()?;
        let now = self.file.metadata().map_err(io_error)?;
        if now.dev() != self.observed.dev()
            || now.ino() != self.observed.ino()
            || now.nlink() != 1
            || !now.is_file()
        {
            return Err(changed());
        }
        permissions(&now)?;
        self.file.set_len(0).map_err(io_error)?;
        self.file.write_all(bytes).map_err(io_error)?;
        self.file.sync_all().map_err(io_error)?;
        self.binding.verify_marker()
    }
}
impl WorkspaceProvider for Direct {
    fn coordinate(
        &self,
        expected: &WorkspaceBinding,
    ) -> Result<Box<dyn ymp_kernel::ports::execution::WorkspaceCoordination + '_>> {
        let locks = super::binding::RootBinding::access_lock(&expected.root)?;
        let binding = self
            .binding
            .lock()
            .map_err(|_| Denial::new("workspace_binding", "Binding state unavailable"))?
            .clone()
            .ok_or_else(|| Denial::new("binding_required", "Bind before I/O"))?;
        if binding.record() != expected || self.location()? != expected.root {
            return Err(Denial::new(
                "binding_conflict",
                "Coordinator differs from operation binding",
            ));
        }
        binding.verify_marker()?;
        Ok(Box::new(DirectCoordination {
            _locks: locks,
            binding: expected.clone(),
        }))
    }
    fn validate_file_request(
        &self,
        access: &ymp_kernel::workspace_guard::FileAccess,
    ) -> Result<()> {
        if access.mode() == LockMode::Write && access.limit() > self.limits.max_file_bytes {
            return Err(Denial::new(
                "file_limit",
                "Write exceeds the bounded file size",
            ));
        }
        self.operation_binding(access, access.mode())?;
        Ok(())
    }
    fn prepare_file(
        &self,
        access: &ymp_kernel::workspace_guard::FileAccess,
    ) -> Result<Box<dyn ymp_kernel::ports::execution::WorkspaceFile>> {
        if access.mode() == LockMode::Write && access.limit() > self.limits.max_file_bytes {
            return Err(Denial::new(
                "file_limit",
                "Write exceeds the bounded file size",
            ));
        }
        let binding = self.operation_binding(access, access.mode())?;
        let (parent, name) = self.file_parent(access)?;
        let depth = access.path().as_str().split('/').count();
        let is_new = !access.target().missing.is_empty();
        if is_new
            && (access.mode() != LockMode::Write
                || access.target().missing.len() != 1
                || self.scope_identity(access, depth).is_some())
        {
            return Err(changed());
        }
        if !access.scope().missing.is_empty() && access.created_file().is_none() && !is_new {
            return Err(changed());
        }
        let flags = match access.mode() {
            LockMode::Read => OFlags::RDONLY,
            LockMode::Write => OFlags::WRONLY,
        };
        let flags = flags
            | OFlags::NOFOLLOW
            | OFlags::CLOEXEC
            | OFlags::NONBLOCK
            | if is_new {
                OFlags::CREATE | OFlags::EXCL
            } else {
                OFlags::empty()
            };
        let file = File::from(
            rfs::openat(
                &parent,
                name.as_str(),
                flags,
                Mode::RUSR | Mode::WUSR | Mode::RGRP | Mode::ROTH,
            )
            .map_err(io_error)?,
        );
        let observed = self.check_operation_file(&file, &binding)?;
        self.check_scope_component(access, depth, &observed)?;
        if is_new {
            file.sync_all().map_err(io_error)?;
            parent.sync_all().map_err(io_error)?;
        }
        binding.verify_marker()?;
        Ok(Box::new(DirectFile {
            file,
            binding,
            mode: access.mode(),
            limit: access.limit().min(self.limits.max_file_bytes),
            observed,
        }))
    }
    fn selection(&self) -> &PolicySelection {
        &self.selection
    }
    fn file_modes(&self) -> Vec<LockMode> {
        vec![LockMode::Read, LockMode::Write]
    }
    fn bind(&self, journal: &dyn Journal) -> Result<WorkspaceBinding> {
        let mut binding = self
            .binding
            .lock()
            .map_err(|_| Denial::new("workspace_binding", "Binding state is unavailable"))?;
        if binding.is_none() {
            *binding = Some(Arc::new(super::binding::RootBinding::open(
                Path::new(&self.location.root),
                journal,
                &self.limits,
            )?));
        }
        let bound = binding.as_ref().expect("initialized binding");
        bound.verify(journal)?;
        Ok(bound.record().clone())
    }
    fn verify_binding(&self, expected: &WorkspaceBinding, journal: &dyn Journal) -> Result<()> {
        if self.bind(journal)? != *expected {
            return Err(Denial::new(
                "workspace_binding",
                "Provider has a different root binding",
            ));
        }
        Ok(())
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
    fn capture(
        &self,
        journal: &Result<JournalIdentity>,
        store: &dyn ContentStore,
    ) -> Result<SnapshotTree> {
        let binding = self
            .binding
            .lock()
            .map_err(|_| Denial::new("workspace_binding", "Binding state is unavailable"))?;
        let _locks = if let Some(bound) = binding.as_ref() {
            if journal.as_ref().map_err(Clone::clone)? != &bound.record().journal {
                return Err(Denial::new(
                    "binding_conflict",
                    "Capture caller is not the bound journal",
                ));
            }
            let locks = super::binding::RootBinding::capture_lock(&self.location)?;
            bound.verify_marker()?;
            locks
        } else {
            super::binding::RootBinding::unbound_capture_allowed(&self.location, &self.limits)?
        };
        let first = self.capture_once(store, binding.is_some())?;
        let second = self.capture_once(store, binding.is_some())?;
        if let Some(bound) = binding.as_ref() {
            bound.verify_marker()?;
        }
        if first != second {
            return Err(changed());
        }
        // Two matching bounded scans detect changes; they are not an atomic OS snapshot.
        Ok(first)
    }
}
