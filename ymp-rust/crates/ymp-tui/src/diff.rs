//! Unified and Git patches in agent output: the part each line of a patch plays.
//!
//! A patch is only ever content a message reported. Styling it says nothing about whether it was
//! applied, accepted or recorded as a change, and nothing here reads the files it names.
//!
//! Two entry points differ in how much they trust the text. [`roles`] is for a block its author
//! declared a `diff` or `patch`, and classifies every line. [`scan`] is for text nobody
//! declared, and accepts only a complete patch: a file header pair, then hunks whose line counts
//! the lines under them match exactly. A list of lines that begin with `+` or `-` is not a patch.

use crate::highlight::legible;
use crate::theme::Theme;
use ratatui::style::{Modifier, Style};

/// What a line of a patch is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    /// `diff --git`, `index`, mode, rename, copy and similarity lines.
    Metadata,
    /// `--- a/path`
    OldFile,
    /// `+++ b/path`
    NewFile,
    /// `@@ -1,3 +1,4 @@`
    Hunk,
    Added,
    Removed,
    Context,
    /// `\ No newline at end of file`
    NoNewline,
    Other,
}

impl Role {
    /// The style of the line, on the background code is drawn on. The line keeps its own `+`,
    /// `-` or space, so its role reads without colour too.
    pub fn style(self, theme: &Theme) -> Style {
        let on_code = |color| theme.code().fg(legible(theme, color, theme.raised));
        match self {
            Role::Added => on_code(theme.good),
            Role::Removed => on_code(theme.bad),
            Role::Hunk => on_code(theme.info),
            Role::OldFile | Role::NewFile => on_code(theme.text).add_modifier(Modifier::BOLD),
            Role::Metadata | Role::NoNewline => on_code(theme.muted),
            Role::Context | Role::Other => theme.code(),
        }
    }
}

/// Whether a fence's info string declares a diff or a patch.
pub fn declared(info: &str) -> bool {
    let word = info.split_whitespace().next().unwrap_or_default();
    ["diff", "patch", "udiff"]
        .iter()
        .any(|name| word.eq_ignore_ascii_case(name))
}

/// Lines of a Git extended header, which come between `diff --git` and the file names.
const METADATA: &[&str] = &[
    "diff --git ",
    "index ",
    "old mode ",
    "new mode ",
    "deleted file mode ",
    "new file mode ",
    "similarity index ",
    "dissimilarity index ",
    "rename from ",
    "rename to ",
    "copy from ",
    "copy to ",
    "Binary files ",
];

fn metadata(line: &str) -> bool {
    METADATA.iter().any(|prefix| line.starts_with(prefix))
}

/// The old and new line counts a hunk header announces. A range without a count has one line.
fn hunk(line: &str) -> Option<(usize, usize)> {
    let rest = line.strip_prefix("@@ -")?;
    let (old, rest) = rest.split_once(" +")?;
    let (new, _) = rest.split_once(" @@")?;
    let count = |range: &str| {
        let (start, length) = range.split_once(',').unwrap_or((range, "1"));
        start.parse::<usize>().ok()?;
        length.parse::<usize>().ok()
    };
    Some((count(old)?, count(new)?))
}

/// Consume one line of a hunk with `old` and `new` lines still to come. `None` when the line
/// cannot belong to it.
fn hunk_line(line: &str, old: &mut usize, new: &mut usize) -> Option<Role> {
    match line.as_bytes().first() {
        Some(b'+') if *new > 0 => {
            *new -= 1;
            Some(Role::Added)
        }
        Some(b'-') if *old > 0 => {
            *old -= 1;
            Some(Role::Removed)
        }
        // A blank context line is one whose leading space something trimmed away.
        Some(b' ') | None if *old > 0 && *new > 0 => {
            *old -= 1;
            *new -= 1;
            Some(Role::Context)
        }
        Some(b'\\') => Some(Role::NoNewline),
        _ => None,
    }
}

/// The role of every line of a block declared a diff or a patch. Inside a hunk whose header
/// gives its counts, the counts decide, so a removed line that reads `--- x` stays a removed line.
pub fn roles<S: AsRef<str>>(lines: &[S]) -> Vec<Role> {
    let mut remaining: Option<(usize, usize)> = None;
    let mut roles = Vec::with_capacity(lines.len());
    for line in lines {
        let line = line.as_ref();
        if let Some((mut old, mut new)) = remaining.take() {
            if let Some(role) = hunk_line(line, &mut old, &mut new) {
                if old > 0 || new > 0 {
                    remaining = Some((old, new));
                }
                roles.push(role);
                continue;
            }
        }
        let role = if line.starts_with("@@") {
            remaining = hunk(line).filter(|(old, new)| *old > 0 || *new > 0);
            Role::Hunk
        } else if line.starts_with("--- ") {
            Role::OldFile
        } else if line.starts_with("+++ ") {
            Role::NewFile
        } else if metadata(line) {
            Role::Metadata
        } else {
            match line.as_bytes().first() {
                Some(b'+') => Role::Added,
                Some(b'-') => Role::Removed,
                Some(b' ') => Role::Context,
                Some(b'\\') => Role::NoNewline,
                _ => Role::Other,
            }
        };
        roles.push(role);
    }
    roles
}

/// What [`scan`] found at the start of some lines.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Scan {
    /// How many lines form a complete patch, or `None` when they do not start one. The patch
    /// ends after its last complete hunk, so text may follow it.
    pub patch: Option<usize>,
    /// How many lines were read to decide, at least one. A caller that looks for a patch at
    /// every line skips these, so a long text is read once rather than once per line.
    pub read: usize,
}

/// Whether `lines`, from the first, form a complete unified or Git patch.
pub fn scan<S: AsRef<str>>(lines: &[S]) -> Scan {
    enum Expect {
        Start,
        /// After `diff --git`: its extended header, or the old file name.
        Git,
        /// After `--- `: the new file name.
        NewFile,
        /// After `+++ ` or a complete hunk. `hunked` once this file has a hunk.
        Between {
            hunked: bool,
        },
        Hunk {
            old: usize,
            new: usize,
        },
    }
    let mut expect = Expect::Start;
    let mut end = None;
    let mut read = lines.len().max(1);
    for (index, line) in lines.iter().enumerate() {
        let line = line.as_ref();
        expect = match expect {
            Expect::Start if line.starts_with("diff --git ") => Expect::Git,
            Expect::Start if line.starts_with("--- ") => Expect::NewFile,
            Expect::Start => {
                read = 1;
                break;
            }
            Expect::Git if line.starts_with("--- ") => Expect::NewFile,
            Expect::Git if metadata(line) => Expect::Git,
            Expect::NewFile if line.starts_with("+++ ") => Expect::Between { hunked: false },
            Expect::Between { hunked } => match hunk(line) {
                Some((0, 0)) => {
                    end = Some(index + 1);
                    Expect::Between { hunked: true }
                }
                Some((old, new)) => Expect::Hunk { old, new },
                None if hunked && line.starts_with("diff --git ") => Expect::Git,
                None if hunked && line.starts_with("--- ") => Expect::NewFile,
                None if hunked && line.starts_with('\\') => {
                    end = Some(index + 1);
                    Expect::Between { hunked }
                }
                None => {
                    read = index + 1;
                    break;
                }
            },
            Expect::Hunk { mut old, mut new } => match hunk_line(line, &mut old, &mut new) {
                Some(_) if old == 0 && new == 0 => {
                    end = Some(index + 1);
                    Expect::Between { hunked: true }
                }
                Some(_) => Expect::Hunk { old, new },
                None => {
                    read = index + 1;
                    break;
                }
            },
            Expect::Git | Expect::NewFile => {
                read = index + 1;
                break;
            }
        };
    }
    Scan { patch: end, read }
}

#[cfg(test)]
mod tests {
    use super::*;

    const GIT: &str = "diff --git a/src/lib.rs b/src/lib.rs
index 3b18e51..a9c1f2d 100644
--- a/src/lib.rs
+++ b/src/lib.rs
@@ -1,4 +1,4 @@
 fn main() {
--- decrement
+    println!(\"a  b\");

 }
\\ No newline at end of file";

    fn lines(text: &str) -> Vec<&str> {
        text.split('\n').collect()
    }

    #[test]
    fn every_line_of_a_git_patch_has_its_role() {
        use Role::*;
        assert_eq!(
            roles(&lines(GIT)),
            [
                Metadata, Metadata, OldFile, NewFile, Hunk, Context, Removed, Added, Context,
                Context, NoNewline
            ]
        );
        assert_eq!(scan(&lines(GIT)).patch, Some(11));
    }

    #[test]
    fn a_patch_ends_after_its_last_complete_hunk() {
        let text = "--- a/x\n+++ b/x\n@@ -1 +1,2 @@\n-old\n+new\n+more\nThat is the change.";
        assert_eq!(
            scan(&lines(text)),
            Scan {
                patch: Some(6),
                read: 7
            }
        );
        // Two files, the second in a new header.
        let two = "--- a/x\n+++ b/x\n@@ -1 +1 @@\n-a\n+b\n--- a/y\n+++ b/y\n@@ -2,0 +3 @@\n+c";
        assert_eq!(scan(&lines(two)).patch, Some(9));
        // A file header with no hunk yet ends the patch before it.
        let open = "--- a/x\n+++ b/x\n@@ -1 +1 @@\n-a\n+b\n--- a/y\n+++ b/y";
        assert_eq!(scan(&lines(open)).patch, Some(5));
    }

    #[test]
    fn text_that_only_looks_like_a_patch_is_not_one() {
        for text in [
            "- first item\n+ second item\n- third item",
            "--- a/x\n+++ b/x\nno hunk follows",
            "--- a/x\n+++ b/x\n@@ -1,2 +1,2 @@\n-only one of two old lines\n+new",
            "@@ -1 +1 @@\n-a\n+b",
            "---\ntitle: front matter\n---",
            "diff --git a/x b/x\nsomething else",
            "",
        ] {
            let found = scan(&lines(text));
            assert_eq!(found.patch, None, "{text:?}");
            assert!(found.read >= 1 && found.read <= lines(text).len().max(1));
        }
    }

    #[test]
    fn a_declared_block_is_classified_without_counts() {
        use Role::*;
        assert_eq!(
            roles(&lines("@@\n+added\n-removed\n context\nplain")),
            [Hunk, Added, Removed, Context, Other]
        );
        assert!(declared("diff"));
        assert!(declared("Patch title=\"fix\""));
        assert!(!declared("rust"));
        assert!(!declared(""));
    }

    #[test]
    fn every_role_reads_in_every_theme() {
        for theme in crate::theme::catalog() {
            for role in [
                Role::Added,
                Role::Removed,
                Role::Hunk,
                Role::NewFile,
                Role::Metadata,
            ] {
                let style = role.style(theme);
                let ratio = crate::theme::contrast(style.fg.unwrap(), theme.raised);
                assert!(ratio >= 3.0, "{} {role:?}: {ratio:.2}", theme.id);
            }
        }
    }
}
