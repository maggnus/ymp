//! Immutable physical-root binding. Only the Journal owns mutable access holds.
use rustix::fs::{self as rfs, AtFlags, Dir, FileType, FlockOperation, Mode, OFlags, RenameFlags};
use std::{
    collections::BTreeSet,
    fs::File,
    io::{Read, Write},
    os::unix::fs::MetadataExt,
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
};
use ymp_domain::{
    Denial, Result,
    journal::{decode, encode},
    workspace::*,
};
use ymp_kernel::journal::Journal;

pub const MARKER: &str = ".ymp-workspace-owner";
const MAX_MARKER_BYTES: usize = 16 * 1024;
fn io(error: impl std::fmt::Display) -> Denial {
    Denial::new(
        "binding_io",
        format!("Workspace binding I/O failed: {error}"),
    )
}
fn identity(file: &File) -> Result<FileIdentity> {
    let m = file.metadata().map_err(io)?;
    Ok(FileIdentity {
        device: m.dev(),
        inode: m.ino(),
    })
}
// rustix's dev_t/ino_t widths differ across the supported Unix targets.
#[allow(clippy::unnecessary_cast)]
pub(super) fn stat_identity(stat: &rfs::Stat) -> FileIdentity {
    FileIdentity {
        device: stat.st_dev as u64,
        inode: stat.st_ino as u64,
    }
}
fn open_dir(parent: &File, name: &str) -> Result<File> {
    Ok(File::from(
        rfs::openat(
            parent,
            name,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(io)?,
    ))
}
fn marker_exists(directory: &File) -> Result<bool> {
    match rfs::statat(directory, MARKER, AtFlags::SYMLINK_NOFOLLOW) {
        Ok(_) => Ok(true),
        Err(rustix::io::Errno::NOENT) => Ok(false),
        Err(e) => Err(io(e)),
    }
}
fn marker_bytes(directory: &File) -> Result<(Vec<u8>, FileIdentity)> {
    let mut marker = File::from(
        rfs::openat(
            directory,
            MARKER,
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(io)?,
    );
    let before = marker.metadata().map_err(io)?;
    if !before.is_file() || before.nlink() != 1 || before.len() > MAX_MARKER_BYTES as u64 {
        return Err(Denial::new(
            "binding_marker",
            "Binding marker must be a bounded regular file without aliases",
        ));
    }
    let mut bytes = vec![];
    Read::by_ref(&mut marker)
        .take(MAX_MARKER_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(io)?;
    let after = marker.metadata().map_err(io)?;
    if bytes.len() > MAX_MARKER_BYTES
        || before.len() != bytes.len() as u64
        || before.dev() != after.dev()
        || before.ino() != after.ino()
        || before.mtime() != after.mtime()
        || before.mtime_nsec() != after.mtime_nsec()
        || before.ctime() != after.ctime()
        || before.ctime_nsec() != after.ctime_nsec()
    {
        return Err(Denial::new(
            "binding_changed",
            "Binding marker changed during inspection",
        ));
    }
    Ok((
        bytes,
        FileIdentity {
            device: before.dev(),
            inode: before.ino(),
        },
    ))
}
pub(super) struct DirectoryLocks(Vec<File>);
impl Drop for DirectoryLocks {
    fn drop(&mut self) {
        for directory in self.0.iter().rev() {
            let _ = rfs::flock(directory, FlockOperation::Unlock);
        }
    }
}
impl DirectoryLocks {
    fn acquire(path: &str, exclusive_root: bool) -> Result<Self> {
        let names: Vec<_> = path.split('/').filter(|s| !s.is_empty()).collect();
        let top = File::from(
            rfs::open(
                "/",
                OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .map_err(io)?,
        );
        let mut held = Self(vec![]);
        let mut next = top;
        let mut seen = BTreeSet::new();
        for index in 0..=names.len() {
            if !seen.insert(identity(&next)?) {
                return Err(Denial::new(
                    "binding_topology",
                    "Repeated physical directory in root ancestry",
                ));
            }
            let operation = if exclusive_root && index == names.len() {
                FlockOperation::NonBlockingLockExclusive
            } else {
                FlockOperation::NonBlockingLockShared
            };
            rfs::flock(&next, operation).map_err(|error| {
                if error == rustix::io::Errno::WOULDBLOCK {
                    Denial::new(
                        "binding_busy",
                        "Another binding operation owns an overlapping root",
                    )
                } else {
                    io(error)
                }
            })?;
            held.0.push(next);
            if index < names.len() {
                next = open_dir(held.0.last().unwrap(), names[index])?;
            } else {
                break;
            }
        }
        Ok(held)
    }
    fn root(&self) -> &File {
        self.0.last().expect("root lock")
    }
}
fn deny_contained_journal(root: &FileIdentity, journal: &JournalIdentity) -> Result<()> {
    let path = Path::new(&journal.path);
    let parent = path
        .parent()
        .ok_or_else(|| Denial::new("journal_identity", "Journal has no parent"))?;
    let mut directory = File::from(
        rfs::open(
            "/",
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(io)?,
    );
    if identity(&directory)? == *root {
        return Err(Denial::new(
            "binding_storage",
            "Journal must be outside the execution root",
        ));
    }
    for name in parent
        .to_str()
        .ok_or_else(|| Denial::new("journal_identity", "Journal path must be UTF-8"))?
        .split('/')
        .filter(|s| !s.is_empty())
    {
        directory = open_dir(&directory, name)?;
        if identity(&directory)? == *root {
            return Err(Denial::new(
                "binding_storage",
                "Journal must be outside the execution root",
            ));
        }
    }
    Ok(())
}
fn scan_descendants(
    directory: &File,
    device: u64,
    depth: usize,
    remaining: &mut usize,
    seen: &mut BTreeSet<FileIdentity>,
    limits: &CaptureLimits,
) -> Result<()> {
    if depth > limits.max_depth {
        return Err(Denial::new(
            "binding_limit",
            "Root binding scan exceeds its depth limit",
        ));
    }
    let before = directory.metadata().map_err(io)?;
    let node = identity(directory)?;
    if node.device != device || !seen.insert(node) {
        return Err(Denial::new(
            "binding_topology",
            "Device crossing or directory alias in binding scan",
        ));
    }
    for entry in Dir::read_from(directory).map_err(io)? {
        let entry = entry.map_err(io)?;
        let name = entry.file_name().to_str().map_err(io)?;
        if name == "." || name == ".." {
            continue;
        }
        *remaining = remaining.checked_sub(1).ok_or_else(|| {
            Denial::new("binding_limit", "Root binding scan exceeds its entry limit")
        })?;
        if name.eq_ignore_ascii_case(MARKER)
            || name
                .to_ascii_lowercase()
                .starts_with(&format!("{MARKER}.tmp-"))
        {
            return Err(Denial::new(
                "binding_nested",
                "An existing binding or ambiguous control file is inside the proposed root",
            ));
        }
        let stat = rfs::statat(directory, name, AtFlags::SYMLINK_NOFOLLOW).map_err(io)?;
        match FileType::from_raw_mode(stat.st_mode) {
            FileType::Directory => {
                let child = open_dir(directory, name)?;
                let m = child.metadata().map_err(io)?;
                if (FileIdentity {
                    device: m.dev(),
                    inode: m.ino(),
                }) != stat_identity(&stat)
                {
                    return Err(Denial::new(
                        "binding_changed",
                        "Directory changed during binding scan",
                    ));
                }
                if marker_exists(&child)? {
                    return Err(Denial::new(
                        "binding_nested",
                        "A descendant workspace is already bound",
                    ));
                }
                scan_descendants(&child, device, depth + 1, remaining, seen, limits)?;
            }
            FileType::RegularFile => {}
            _ => {
                return Err(Denial::new(
                    "binding_topology",
                    "Links and special files cannot be scanned as a binding boundary",
                ));
            }
        }
    }
    let after = directory.metadata().map_err(io)?;
    if before.mtime() != after.mtime()
        || before.mtime_nsec() != after.mtime_nsec()
        || before.ctime() != after.ctime()
        || before.ctime_nsec() != after.ctime_nsec()
    {
        return Err(Denial::new(
            "binding_changed",
            "Directory changed during binding scan",
        ));
    }
    Ok(())
}
fn install_marker(root: &File, bytes: &[u8]) -> Result<()> {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let mut staged = None;
    for _ in 0..128 {
        let name = format!(
            "{MARKER}.tmp-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        );
        match rfs::openat(
            root,
            name.as_str(),
            OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::RUSR | Mode::WUSR,
        ) {
            Ok(fd) => {
                staged = Some((name, File::from(fd)));
                break;
            }
            Err(rustix::io::Errno::EXIST) => continue,
            Err(e) => return Err(io(e)),
        }
    }
    let (name, mut file) = staged.ok_or_else(|| {
        Denial::new(
            "binding_busy",
            "Cannot allocate a private marker staging file",
        )
    })?;
    let own = identity(&file)?;
    let result = (|| {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(std::fs::Permissions::from_mode(0o600))
            .map_err(io)?;
        file.write_all(bytes).map_err(io)?;
        file.sync_all().map_err(io)?;
        rfs::renameat_with(root, name.as_str(), root, MARKER, RenameFlags::NOREPLACE)
            .map_err(io)?;
        root.sync_all().map_err(io)?;
        Ok(())
    })();
    if result.is_err()
        && let Ok(stat) = rfs::statat(root, name.as_str(), AtFlags::SYMLINK_NOFOLLOW)
        && stat_identity(&stat) == own
    {
        let _ = rfs::unlinkat(root, name.as_str(), AtFlags::empty());
    }
    result
}

pub struct RootBinding {
    root: File,
    record: WorkspaceBinding,
    marker: FileIdentity,
}
impl RootBinding {
    pub fn open(path: &Path, journal: &dyn Journal, limits: &CaptureLimits) -> Result<Self> {
        limits.validate()?;
        let journal_id = journal.binding_identity()?;
        journal_id.validate()?;
        let canonical = path.canonicalize().map_err(io)?;
        let text = canonical
            .to_str()
            .ok_or_else(|| Denial::new("binding_path", "Workspace root must be UTF-8"))?;
        let locks = DirectoryLocks::acquire(text, true)?;
        let physical = identity(locks.root())?;
        deny_contained_journal(&physical, &journal_id)?;
        let location = WorkspaceLocation {
            root: text.into(),
            device: physical.device,
            inode: physical.inode,
        };
        let wanted = WorkspaceBinding {
            format: 1,
            root: location.clone(),
            journal: journal_id,
        };
        wanted.validate()?;
        let recorded = journal.workspace_binding(&location)?;
        if recorded.as_ref().is_some_and(|r| r != &wanted) {
            return Err(Denial::new(
                "binding_conflict",
                "An overlapping or replaced workspace has a different persisted binding",
            ));
        }
        for ancestor in &locks.0[..locks.0.len() - 1] {
            if marker_exists(ancestor)? {
                return Err(Denial::new(
                    "binding_nested",
                    "A workspace ancestor is already bound",
                ));
            }
        }
        if !marker_exists(locks.root())? {
            if recorded.is_some() {
                return Err(Denial::new(
                    "binding_missing",
                    "A recorded root lost its binding marker",
                ));
            }
            let mut remaining = limits.max_entries;
            scan_descendants(
                locks.root(),
                physical.device,
                0,
                &mut remaining,
                &mut BTreeSet::new(),
                limits,
            )?;
            let bytes = encode(&wanted)?;
            if bytes.len() > MAX_MARKER_BYTES {
                return Err(Denial::new(
                    "binding_limit",
                    "Binding metadata exceeds its supported size",
                ));
            }
            install_marker(locks.root(), &bytes)?;
        }
        let (bytes, marker) = marker_bytes(locks.root())?;
        let record: WorkspaceBinding = decode(&bytes)?;
        record.validate()?;
        if record != wanted || encode(&record)? != bytes {
            return Err(Denial::new(
                "binding_conflict",
                "Root marker belongs to another journal or physical root",
            ));
        }
        let result = Self {
            root: locks.root().try_clone().map_err(io)?,
            record,
            marker,
        };
        result.verify(journal)?;
        Ok(result)
    }
    pub(super) fn capture_lock(location: &WorkspaceLocation) -> Result<DirectoryLocks> {
        let locks = DirectoryLocks::acquire(&location.root, false)?;
        if identity(locks.root())?
            != (FileIdentity {
                device: location.device,
                inode: location.inode,
            })
        {
            return Err(Denial::new(
                "binding_changed",
                "Capture lock names another physical root",
            ));
        }
        Ok(locks)
    }
    pub(super) fn unbound_capture_allowed(
        location: &WorkspaceLocation,
        limits: &CaptureLimits,
    ) -> Result<DirectoryLocks> {
        // Exclusive root coordination also blocks new descendant bindings while
        // preview I/O runs; shared ancestor locks alone would not do so.
        let locks = DirectoryLocks::acquire(&location.root, true)?;
        if identity(locks.root())?
            != (FileIdentity {
                device: location.device,
                inode: location.inode,
            })
        {
            return Err(Denial::new(
                "binding_changed",
                "Capture lock names another physical root",
            ));
        }
        for directory in &locks.0 {
            if marker_exists(directory)? {
                return Err(Denial::new(
                    "binding_required",
                    "Attach the recorded root binding before capturing this workspace",
                ));
            }
        }
        let mut remaining = limits.max_entries;
        scan_descendants(
            locks.root(),
            location.device,
            0,
            &mut remaining,
            &mut BTreeSet::new(),
            limits,
        )?;
        Ok(locks)
    }
    pub fn record(&self) -> &WorkspaceBinding {
        &self.record
    }
    pub fn verify(&self, journal: &dyn Journal) -> Result<()> {
        if journal.binding_identity()? != self.record.journal {
            return Err(Denial::new(
                "binding_conflict",
                "The journal differs from the bound physical store",
            ));
        }
        self.verify_marker()
    }
    pub(super) fn verify_marker(&self) -> Result<()> {
        let current = File::from(
            rfs::open(
                self.record.root.root.as_str(),
                OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .map_err(io)?,
        );
        let physical = FileIdentity {
            device: self.record.root.device,
            inode: self.record.root.inode,
        };
        if identity(&self.root)? != physical || identity(&current)? != physical {
            return Err(Denial::new(
                "binding_changed",
                "The bound root was replaced or moved",
            ));
        }
        let (bytes, marker) = marker_bytes(&self.root)?;
        if marker != self.marker || bytes != encode(&self.record)? {
            return Err(Denial::new(
                "binding_changed",
                "The immutable root marker was replaced or changed",
            ));
        }
        Ok(())
    }
}
