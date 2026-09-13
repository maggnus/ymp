//! Bounded, read-only previews of single files.
//!
//! The files page shows the start of a file through [`read`] and describes an entry through
//! [`details`], and through nothing else. Neither creates, writes, renames or removes anything,
//! and no content is executed. A read takes at most the limit it is given. A FIFO, socket or
//! device is never opened as a regular file: its type is checked before the open and again on the
//! open handle, and the open does not wait for a writer. A path is used exactly as it is given,
//! so a name keeps every one of its bytes; any text shown for it is derived elsewhere.

use std::fs::{self, Metadata, OpenOptions};
use std::io::{self, Read};
use std::path::{Path, PathBuf};

/// The most a preview reads from one file.
pub const PREVIEW_BYTES: u64 = 256 * 1024;

/// A file that has no contents to read as text, or whose reading has side effects.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Special {
    Fifo,
    Socket,
    BlockDevice,
    CharDevice,
    Other,
}

impl Special {
    pub fn word(self) -> &'static str {
        match self {
            Special::Fifo => "FIFO",
            Special::Socket => "socket",
            Special::BlockDevice => "block device",
            Special::CharDevice => "character device",
            Special::Other => "special file",
        }
    }
}

/// What a path names once any symbolic link is followed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Directory,
    File { size: u64 },
    Special(Special),
}

/// An entry as the files page describes it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Details {
    /// Set when the path is itself a symbolic link, with the target it stores when that could be
    /// read.
    pub link: Option<Option<PathBuf>>,
    /// What the path names, following a link, or why that could not be found out.
    pub kind: Result<Kind, String>,
}

/// What a preview found at a path.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Preview {
    /// UTF-8 text from the start of the file, which is at `location`: the path itself, or where a
    /// link to it leads. `truncated` when the file holds more than was read.
    Text {
        location: PathBuf,
        text: String,
        size: u64,
        truncated: bool,
    },
    Empty,
    /// A NUL byte at `offset` of what was read.
    Binary {
        size: u64,
        offset: usize,
    },
    /// Text in an encoding the preview does not decode, or bytes that are not UTF-8.
    Encoding {
        size: u64,
        reason: String,
    },
    Directory,
    /// A symbolic link to nothing, with the target it stores when that could be read.
    BrokenLink {
        stored: Option<PathBuf>,
    },
    Special(Special),
    Missing,
    Unreadable(String),
}

/// Describe `path` without opening it. Fails only when the path itself cannot be examined.
pub fn details(path: &Path) -> io::Result<Details> {
    let own = fs::symlink_metadata(path)?;
    if !own.file_type().is_symlink() {
        return Ok(Details {
            link: None,
            kind: Ok(kind(&own)),
        });
    }
    Ok(Details {
        link: Some(fs::read_link(path).ok()),
        kind: fs::metadata(path)
            .map(|metadata| kind(&metadata))
            .map_err(|error| error.to_string()),
    })
}

/// Read the start of the file at `path`, at most `limit` bytes of it, following a symbolic link.
pub fn read(path: &Path, limit: u64) -> Preview {
    let own = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Preview::Missing,
        Err(error) => return Preview::Unreadable(error.to_string()),
    };
    let (metadata, location) = if own.file_type().is_symlink() {
        match fs::metadata(path) {
            Ok(metadata) => (
                metadata,
                fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf()),
            ),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Preview::BrokenLink {
                    stored: fs::read_link(path).ok(),
                }
            }
            Err(error) => return Preview::Unreadable(error.to_string()),
        }
    } else {
        (own, path.to_path_buf())
    };
    match kind(&metadata) {
        Kind::Directory => Preview::Directory,
        Kind::Special(special) => Preview::Special(special),
        Kind::File { .. } => read_regular(path, location, limit),
    }
}

/// Open `path` and read at most `limit` bytes, provided it is a regular file once open.
fn read_regular(path: &Path, location: PathBuf, limit: u64) -> Preview {
    let mut options = OpenOptions::new();
    options.read(true);
    // Something swapped in after the path was examined is checked again once it is open.
    // Without this flag a FIFO would hold the open itself until a writer appeared.
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NONBLOCK);
    }
    let mut file = match options.open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Preview::Missing,
        Err(error) => return Preview::Unreadable(error.to_string()),
    };
    let metadata = match file.metadata() {
        Ok(metadata) => metadata,
        Err(error) => return Preview::Unreadable(error.to_string()),
    };
    let size = match kind(&metadata) {
        Kind::File { size } => size,
        Kind::Directory => return Preview::Directory,
        Kind::Special(special) => return Preview::Special(special),
    };
    let mut bytes = Vec::with_capacity(size.min(limit.saturating_add(1)) as usize);
    if let Err(error) = (&mut file)
        .take(limit.saturating_add(1))
        .read_to_end(&mut bytes)
    {
        return Preview::Unreadable(error.to_string());
    }
    decode(location, bytes, size, limit)
}

/// Turn what was read into text, or into the reason it is not shown as text.
fn decode(location: PathBuf, mut bytes: Vec<u8>, size: u64, limit: u64) -> Preview {
    let truncated = bytes.len() as u64 > limit;
    bytes.truncate(limit as usize);
    if bytes.is_empty() {
        return Preview::Empty;
    }
    for (mark, encoding) in [
        (&[0xFF, 0xFE, 0x00, 0x00][..], "UTF-32"),
        (&[0x00, 0x00, 0xFE, 0xFF][..], "UTF-32"),
        (&[0xFF, 0xFE][..], "UTF-16"),
        (&[0xFE, 0xFF][..], "UTF-16"),
    ] {
        if bytes.starts_with(mark) {
            return Preview::Encoding {
                size,
                reason: format!("it starts with a {encoding} byte order mark"),
            };
        }
    }
    if let Some(offset) = bytes.iter().position(|byte| *byte == 0) {
        return Preview::Binary { size, offset };
    }
    let mark = if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        3
    } else {
        0
    };
    let body = &bytes[mark..];
    let text = match std::str::from_utf8(body) {
        Ok(text) => text,
        // The read stopped inside a character: keep the characters before it.
        Err(error) if truncated && error.error_len().is_none() => {
            std::str::from_utf8(&body[..error.valid_up_to()]).unwrap_or_default()
        }
        Err(error) => {
            return Preview::Encoding {
                size,
                reason: format!("byte {} is not valid UTF-8", mark + error.valid_up_to()),
            }
        }
    };
    Preview::Text {
        location,
        text: text.to_owned(),
        size,
        truncated,
    }
}

fn kind(metadata: &Metadata) -> Kind {
    if metadata.is_dir() {
        Kind::Directory
    } else if metadata.is_file() {
        Kind::File {
            size: metadata.len(),
        }
    } else {
        Kind::Special(special(metadata))
    }
}

#[cfg(unix)]
fn special(metadata: &Metadata) -> Special {
    use std::os::unix::fs::FileTypeExt;
    let file_type = metadata.file_type();
    if file_type.is_fifo() {
        Special::Fifo
    } else if file_type.is_socket() {
        Special::Socket
    } else if file_type.is_block_device() {
        Special::BlockDevice
    } else if file_type.is_char_device() {
        Special::CharDevice
    } else {
        Special::Other
    }
}

#[cfg(not(unix))]
fn special(_: &Metadata) -> Special {
    Special::Other
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    /// Every path beneath `root` with its type and contents, to show that reading changed nothing.
    fn snapshot(root: &Path) -> BTreeMap<PathBuf, String> {
        let mut seen = BTreeMap::new();
        for entry in walkdir::WalkDir::new(root).follow_links(false) {
            let entry = entry.unwrap();
            let path = entry.path().strip_prefix(root).unwrap().to_path_buf();
            let file_type = entry.file_type();
            let value = if file_type.is_file() {
                format!("file {:?}", std::fs::read(entry.path()).unwrap())
            } else if file_type.is_symlink() {
                format!("link {:?}", std::fs::read_link(entry.path()).unwrap())
            } else {
                format!("{file_type:?}")
            };
            seen.insert(path, value);
        }
        seen
    }

    #[test]
    fn a_preview_reads_no_more_than_its_limit_and_says_so() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("large.rs");
        let line = "let value = 42; // é\n";
        std::fs::write(&path, line.repeat(40_000)).unwrap();
        match read(&path, 1000) {
            Preview::Text {
                text,
                size,
                truncated,
                location,
            } => {
                assert!(truncated);
                assert_eq!(location, path);
                assert_eq!(size, (line.len() * 40_000) as u64);
                assert!(text.len() <= 1000);
                assert!(line.repeat(48).starts_with(&text));
            }
            other => panic!("{other:?}"),
        }
        // A limit that falls inside the two bytes of `é` keeps the characters before it.
        let cut = line.find('é').unwrap() + 1;
        match read(&path, cut as u64) {
            Preview::Text {
                text, truncated, ..
            } => {
                assert!(truncated);
                assert_eq!(text, &line[..cut - 1]);
            }
            other => panic!("{other:?}"),
        }
        let exact = temp.path().join("exact.txt");
        std::fs::write(&exact, "12345").unwrap();
        assert!(matches!(
            read(&exact, 5),
            Preview::Text { truncated: false, ref text, .. } if text == "12345"
        ));
    }

    #[test]
    fn content_that_is_not_utf8_text_is_named_rather_than_shown() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        let cases: [(&str, &[u8]); 6] = [
            ("empty", b""),
            ("binary", b"ELF\x7f\x00\x01\x02"),
            ("utf16", b"\xFF\xFEh\x00i\x00"),
            ("latin1", b"caf\xE9 au lait"),
            ("bom", b"\xEF\xBB\xBFtext"),
            ("controls", b"red \x1b[31m bell \x07"),
        ];
        for (name, bytes) in cases {
            std::fs::write(root.join(name), bytes).unwrap();
        }
        let preview = |name: &str| read(&root.join(name), PREVIEW_BYTES);
        assert_eq!(preview("empty"), Preview::Empty);
        assert_eq!(preview("binary"), Preview::Binary { size: 7, offset: 4 });
        assert!(
            matches!(preview("utf16"), Preview::Encoding { ref reason, .. } if reason.contains("UTF-16"))
        );
        assert!(
            matches!(preview("latin1"), Preview::Encoding { ref reason, .. } if reason == "byte 3 is not valid UTF-8")
        );
        assert!(matches!(preview("bom"), Preview::Text { ref text, .. } if text == "text"));
        // Control bytes are content: they are kept here and escaped where they are shown.
        assert!(
            matches!(preview("controls"), Preview::Text { ref text, .. } if text == "red \u{1b}[31m bell \u{7}")
        );
        assert_eq!(preview("gone"), Preview::Missing);
        assert_eq!(read(root, PREVIEW_BYTES), Preview::Directory);
    }

    #[cfg(unix)]
    #[test]
    fn links_are_followed_to_what_they_name_and_reported_when_they_lead_nowhere() {
        use std::os::unix::fs::symlink;
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        std::fs::create_dir_all(root.join("src/deep")).unwrap();
        std::fs::write(root.join("src/lib.rs"), "pub fn lib() {}\n").unwrap();
        symlink("src/deep", root.join("to-deep")).unwrap();
        symlink("src/lib.rs", root.join("to-lib")).unwrap();
        symlink("missing", root.join("broken")).unwrap();
        symlink("loop-b", root.join("loop-a")).unwrap();
        symlink("loop-a", root.join("loop-b")).unwrap();
        symlink("/dev/null", root.join("device")).unwrap();
        let before = snapshot(&root);

        assert_eq!(
            read(&root.join("to-lib"), PREVIEW_BYTES),
            Preview::Text {
                location: root.join("src/lib.rs"),
                text: "pub fn lib() {}\n".into(),
                size: 16,
                truncated: false,
            }
        );
        assert_eq!(
            read(&root.join("to-deep"), PREVIEW_BYTES),
            Preview::Directory
        );
        assert_eq!(
            read(&root.join("broken"), PREVIEW_BYTES),
            Preview::BrokenLink {
                stored: Some(PathBuf::from("missing"))
            }
        );
        assert!(
            matches!(
                read(&root.join("loop-a"), PREVIEW_BYTES),
                Preview::Unreadable(_)
            ),
            "a loop is reported as a reason"
        );
        assert_eq!(
            read(&root.join("device"), PREVIEW_BYTES),
            Preview::Special(Special::CharDevice)
        );

        assert_eq!(
            details(&root.join("to-lib")).unwrap(),
            Details {
                link: Some(Some(PathBuf::from("src/lib.rs"))),
                kind: Ok(Kind::File { size: 16 }),
            }
        );
        let broken = details(&root.join("broken")).unwrap();
        assert_eq!(broken.link, Some(Some(PathBuf::from("missing"))));
        assert!(broken.kind.is_err());
        assert_eq!(
            details(&root.join("src")).unwrap(),
            Details {
                link: None,
                kind: Ok(Kind::Directory),
            }
        );
        assert!(details(&root.join("gone")).is_err());
        assert_eq!(snapshot(&root), before, "reading changed the tree");
    }

    #[cfg(unix)]
    #[test]
    fn special_files_are_never_opened_as_regular_ones() {
        use std::os::unix::net::UnixListener;
        // A short root keeps the socket path below the Unix socket path limit.
        let temp = tempfile::tempdir_in("/tmp").unwrap();
        let root = temp.path();
        let fifo = root.join("pipe");
        let c_path = std::ffi::CString::new(fifo.as_os_str().as_encoded_bytes()).unwrap();
        assert_eq!(unsafe { libc::mkfifo(c_path.as_ptr(), 0o600) }, 0);
        let _socket = UnixListener::bind(root.join("sock")).unwrap();
        let started = std::time::Instant::now();
        assert_eq!(read(&fifo, PREVIEW_BYTES), Preview::Special(Special::Fifo));
        // The open itself does not wait for a writer either, should a FIFO appear after the check.
        assert_eq!(
            read_regular(&fifo, fifo.clone(), PREVIEW_BYTES),
            Preview::Special(Special::Fifo)
        );
        assert!(started.elapsed() < std::time::Duration::from_secs(2));
        assert_eq!(
            read(&root.join("sock"), PREVIEW_BYTES),
            Preview::Special(Special::Socket)
        );
        assert_eq!(
            details(&fifo).unwrap().kind,
            Ok(Kind::Special(Special::Fifo))
        );
    }

    #[cfg(unix)]
    #[test]
    fn an_unreadable_file_is_an_ordinary_state() {
        use std::os::unix::fs::PermissionsExt;
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("locked.txt");
        std::fs::write(&path, "secret").unwrap();
        std::fs::set_permissions(&path, PermissionsExt::from_mode(0o000)).unwrap();
        // A superuser reads through permissions, so there is nothing to observe.
        if std::fs::read(&path).is_ok() {
            return;
        }
        assert!(
            matches!(read(&path, PREVIEW_BYTES), Preview::Unreadable(_)),
            "{:?}",
            read(&path, PREVIEW_BYTES)
        );
        assert_eq!(
            details(&path).unwrap().kind,
            Ok(Kind::File { size: 6 }),
            "the file is still described"
        );
    }
}
