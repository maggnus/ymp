//! The files page: the `ratatui-explorer` file explorer, and read-only previews of what it lists.
//!
//! The explorer lists a directory, keeps the selection and draws the list; ymp maps the keys,
//! decides what Enter does, and supplies the styles, the names rows are shown by and the filter.
//! Every row keeps the explorer's native [`File::path`], so an entry is always opened by the exact
//! name the directory holds, never by the text it is shown as. The page starts in the directory
//! ymp was started in and may move above it, but moving changes neither the directory runs work
//! in, nor the loaded session, nor any provider setting. Nothing here writes to the filesystem.
//!
//! Two behaviours of the explorer are kept away from: its own `Right` input removes the selected
//! row before it tries to read the directory, so a directory that cannot be read would lose its
//! row, and its movement inputs and its widget index the selected row, which an empty list does
//! not have. Directories are entered through [`FileExplorer::set_cwd`] and left through
//! [`FileExplorer::set_working_file`], which change nothing unless the read succeeds, and an empty
//! list is neither moved through nor drawn.

use crate::highlight;
use crate::table;
use crate::text;
use crate::theme::Theme;
use crate::views::{field, paragraph, size};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{HighlightSpacing, WidgetRef};
use ratatui_explorer::{File, FileExplorer, FileExplorerBuilder, Input, Theme as ExplorerTheme};
use std::ffi::OsStr;
use std::fmt::Write as _;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, PoisonError, RwLock};
use ymp_workspace::preview::{self, Kind, Preview, Special};

/// Names that are not listed in any directory: version control, dependencies, build output and
/// ymp's own metadata directory.
pub const UNLISTED: &[&str] = &[".git", "node_modules", "target", "__pycache__", ".ymp2"];

/// The most lines a preview shows.
pub const PREVIEW_LINES: usize = 5_000;

/// The most characters of one line a preview shows before it says how many more there are.
pub const LINE_CHARS: usize = 1_000;

/// The name the explorer gives the row that leads to the parent directory. No entry a directory
/// lists can have it, because a name never contains `/`.
const PARENT_ROW: &str = "../";

/// A move of the selection, as the explorer performs it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    Up,
    Down,
    Home,
    End,
    PageUp,
    PageDown,
}

/// What Enter acts on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Target {
    Parent,
    Directory(PathBuf),
    File(PathBuf),
}

#[derive(Debug, Default)]
pub struct Files {
    /// Where the page opens: the directory ymp was started in.
    start: PathBuf,
    /// Nothing until the page is opened, and nothing while the start could not be listed.
    explorer: Option<FileExplorer>,
    /// Why the last move or read failed. The list shown is the last one read successfully.
    notice: Option<String>,
    /// The page filter the explorer lists under. Its filter reads this whenever a directory is.
    filter: Arc<RwLock<String>>,
    /// The theme and focus the explorer's styles were last made for.
    styled: Option<(&'static str, bool)>,
}

impl Files {
    /// Open at `start`, forgetting where the page was.
    pub fn open(&mut self, start: &Path) {
        *self = Self {
            start: start.to_path_buf(),
            ..Self::default()
        };
        let filter = Arc::clone(&self.filter);
        match FileExplorerBuilder::default()
            .working_dir(start.to_path_buf())
            .show_hidden(true)
            .filter_map(move |file| listed(file, &filter))
            .build()
        {
            Ok(explorer) => self.explorer = Some(explorer),
            Err(error) => self.notice = Some(cannot_list(start, &error)),
        }
    }

    /// The directory listed, or the one the page could not open at.
    pub fn directory(&self) -> &Path {
        self.explorer
            .as_ref()
            .map_or(self.start.as_path(), |explorer| explorer.cwd().as_path())
    }

    /// Whether the page has a list at all.
    pub fn is_open(&self) -> bool {
        self.explorer.is_some()
    }

    pub fn notice(&self) -> Option<&str> {
        self.notice.as_deref()
    }

    /// Rows the explorer draws, the row for the parent directory included.
    pub fn rows(&self) -> usize {
        self.explorer
            .as_ref()
            .map_or(0, |explorer| explorer.files().len())
    }

    /// Entries the directory lists under the filter, the row for the parent directory left out.
    pub fn shown(&self) -> usize {
        self.explorer.as_ref().map_or(0, |explorer| {
            explorer
                .files()
                .iter()
                .filter(|file| !is_parent_row(explorer, file))
                .count()
        })
    }

    /// The selected row, when the list has one.
    pub fn selected(&self) -> Option<&File> {
        let explorer = self.explorer.as_ref()?;
        explorer.files().get(explorer.selected_idx())
    }

    /// What Enter would act on.
    pub fn target(&self) -> Option<Target> {
        let explorer = self.explorer.as_ref()?;
        let file = explorer.files().get(explorer.selected_idx())?;
        Some(if is_parent_row(explorer, file) {
            Target::Parent
        } else if file.is_dir {
            Target::Directory(file.path.clone())
        } else {
            Target::File(file.path.clone())
        })
    }

    /// Whether the selected row leads to the parent directory.
    pub fn parent_selected(&self) -> bool {
        self.target() == Some(Target::Parent)
    }

    /// Move the selection. The explorer wraps from either end to the other.
    pub fn step(&mut self, step: Step) {
        let input = match step {
            Step::Up => Input::Up,
            Step::Down => Input::Down,
            Step::Home => Input::Home,
            Step::End => Input::End,
            Step::PageUp => Input::PageUp,
            Step::PageDown => Input::PageDown,
        };
        match &mut self.explorer {
            // The explorer's movement indexes the list, so an empty one is left alone.
            Some(explorer) if !explorer.files().is_empty() => {
                if let Err(error) = explorer.handle(input) {
                    self.notice = Some(error.to_string());
                }
            }
            _ => {}
        }
    }

    /// List `directory`, clearing the filter. A symbolic link is listed where it leads, so the
    /// page names the real directory and going up leaves that one. On failure nothing changes
    /// except the notice, and false is returned.
    pub fn enter(&mut self, directory: &Path) -> bool {
        let Some(explorer) = &mut self.explorer else {
            return false;
        };
        let target = match std::fs::symlink_metadata(directory) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                match std::fs::canonicalize(directory) {
                    Ok(target) => target,
                    Err(error) => {
                        self.notice = Some(cannot_list(directory, &error));
                        return false;
                    }
                }
            }
            _ => directory.to_path_buf(),
        };
        let previous = std::mem::take(&mut *write(&self.filter));
        match explorer.set_cwd(target) {
            Ok(()) => {
                self.notice = None;
                true
            }
            Err(error) => {
                *write(&self.filter) = previous;
                self.notice = Some(cannot_list(directory, &error));
                false
            }
        }
    }

    /// List the parent directory with the directory just left selected, clearing the filter.
    /// At the root of the filesystem, or when the parent cannot be listed, nothing changes except
    /// the notice, and false is returned.
    pub fn leave(&mut self) -> bool {
        let Some(explorer) = &mut self.explorer else {
            return false;
        };
        let here = explorer.cwd().clone();
        let Some(parent) = here.parent() else {
            return false;
        };
        let previous = std::mem::take(&mut *write(&self.filter));
        // Listing the parent with this directory as the working file selects its row.
        match explorer.set_working_file(here.clone()) {
            Ok(()) => {
                self.notice = None;
                true
            }
            Err(error) => {
                *write(&self.filter) = previous;
                self.notice = Some(cannot_list(parent, &error));
                false
            }
        }
    }

    /// Read the directory again, keeping the selected entry selected while it is still listed.
    pub fn reload(&mut self) {
        self.read_again(false);
    }

    /// List under `filter`, as the page filter is typed. The selection stays on its entry while
    /// that still matches, and otherwise moves to the first entry that does.
    pub fn set_filter(&mut self, filter: &str) {
        if *read(&self.filter) == filter {
            return;
        }
        *write(&self.filter) = filter.to_owned();
        self.read_again(true);
    }

    fn read_again(&mut self, prefer_entry: bool) {
        let Some(explorer) = &mut self.explorer else {
            let start = self.start.clone();
            self.open(&start);
            return;
        };
        let here = explorer.cwd().clone();
        // While a filter is typed, the row for the parent directory, which every filter keeps,
        // does not hold the selection away from the entries that match.
        let selected = explorer
            .files()
            .get(explorer.selected_idx())
            .map(|file| file.path.clone())
            .filter(|path| !(prefer_entry && here.parent() == Some(path.as_path())));
        if let Err(error) = explorer.set_cwd(here.clone()) {
            self.notice = Some(cannot_list(&here, &error));
            return;
        }
        self.notice = None;
        let files = explorer.files();
        let index = selected
            .and_then(|path| files.iter().position(|file| file.path == path))
            .or_else(|| {
                prefer_entry
                    .then(|| files.iter().position(|file| !is_parent_row(explorer, file)))
                    .flatten()
            });
        if let Some(index) = index {
            explorer.set_selected_idx(index);
        }
    }

    /// Give the explorer the styles of `theme`, marking the selection as the page tables do.
    pub fn style(&mut self, theme: &Theme, focused: bool) {
        let key = (theme.id, focused);
        let Some(explorer) = &mut self.explorer else {
            return;
        };
        if self.styled == Some(key) {
            return;
        }
        let (marker, item, directory) = if focused {
            (theme.markers.selection, theme.selected(), theme.selected())
        } else {
            (theme.markers.activity, theme.text(), theme.accent())
        };
        explorer.set_theme(
            ExplorerTheme::new()
                .with_style(theme.body())
                .with_item_style(theme.text())
                .with_dir_style(theme.accent())
                .with_highlight_item_style(item)
                .with_highlight_dir_style(directory)
                .with_highlight_symbol(&format!("{marker} "))
                .with_highlight_spacing(HighlightSpacing::Always)
                .with_scroll_padding(1),
        );
        self.styled = Some(key);
    }

    /// The explorer's own widget, when it has a row to draw. The widget reads the selected row,
    /// so an empty list is never given to it.
    pub fn widget(&self) -> Option<impl WidgetRef + '_> {
        self.explorer
            .as_ref()
            .filter(|explorer| !explorer.files().is_empty())
            .map(FileExplorer::widget)
    }
}

fn read(filter: &RwLock<String>) -> std::sync::RwLockReadGuard<'_, String> {
    filter.read().unwrap_or_else(PoisonError::into_inner)
}

fn write(filter: &RwLock<String>) -> std::sync::RwLockWriteGuard<'_, String> {
    filter.write().unwrap_or_else(PoisonError::into_inner)
}

fn is_parent_row(explorer: &FileExplorer, file: &File) -> bool {
    explorer.cwd().parent() == Some(file.path.as_path())
}

/// The explorer's filter: leave out unlisted names and the entries the page filter excludes, and
/// give every entry the name its row shows. The path is never changed.
fn listed(mut file: File, filter: &RwLock<String>) -> Option<File> {
    if file.name == PARENT_ROW {
        return Some(file);
    }
    let own = file.path.file_name()?;
    if UNLISTED.iter().any(|name| OsStr::new(name) == own) {
        return None;
    }
    let mut shown = display_name(own);
    if let Ok(stored) = std::fs::read_link(&file.path) {
        let _ = write!(shown, " -> {}", display_name(stored.as_os_str()));
    }
    if file.is_dir {
        shown.push('/');
    }
    if !table::matches_text(&read(filter), [shown.as_str()]) {
        return None;
    }
    file.name = shown;
    Some(file)
}

fn cannot_list(path: &Path, error: &io::Error) -> String {
    format!("{} cannot be listed: {error}.", display_path(path))
}

/// How `name` reads on screen. Control and bidirectional formatting characters, and bytes that
/// are not UTF-8, are written as escapes, and a backslash is doubled so that no escape can be
/// forged, so a name can neither drive the terminal nor pass for a different one. The result is
/// only ever shown.
pub fn display_name(name: &OsStr) -> String {
    let mut shown = String::new();
    for chunk in name.as_encoded_bytes().utf8_chunks() {
        for ch in chunk.valid().chars() {
            match ch {
                '\\' => shown.push_str("\\\\"),
                '\n' => shown.push_str("\\n"),
                '\r' => shown.push_str("\\r"),
                '\t' => shown.push_str("\\t"),
                ch if ch.is_control() || text::is_format_control(ch) => {
                    let _ = write!(shown, "\\u{{{:X}}}", ch as u32);
                }
                ch => shown.push(ch),
            }
        }
        for byte in chunk.invalid() {
            let _ = write!(shown, "\\x{byte:02X}");
        }
    }
    shown
}

/// A whole path as it reads on screen, escaped as [`display_name`] escapes a name.
pub fn display_path(path: &Path) -> String {
    display_name(path.as_os_str())
}

/// Everything the page knows about an entry, read now, for the popup `d` opens.
pub fn details_lines(path: &Path, parent: bool, theme: &Theme, width: usize) -> Vec<Line<'static>> {
    let mut lines = field(theme, "path", &display_path(path), width);
    let mut opens = "Enter explains why this is not opened.";
    match preview::details(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            lines.extend(field(theme, "kind", "no longer exists", width));
            opens = "r reads the directory again.";
        }
        Err(error) => {
            lines.extend(field(theme, "kind", "unreadable", width));
            lines.extend(field(theme, "reason", &error.to_string(), width));
        }
        Ok(details) => {
            let what = match &details.kind {
                Ok(Kind::Directory) => "directory".to_owned(),
                Ok(Kind::File { .. }) => "file".to_owned(),
                Ok(Kind::Special(special)) => special.word().to_owned(),
                Err(_) => String::new(),
            };
            let words = match (&details.link, &details.kind) {
                (None, _) => what,
                (Some(_), Ok(_)) => format!("link to a {what}"),
                (Some(_), Err(_)) => "link that cannot be followed".to_owned(),
            };
            lines.extend(field(theme, "kind", &words, width));
            if let Ok(Kind::File { size: bytes }) = details.kind {
                lines.extend(field(theme, "size", &size(bytes), width));
            }
            if let Some(stored) = &details.link {
                let stored = stored
                    .as_ref()
                    .map_or_else(|| "?".to_owned(), |stored| display_path(stored));
                lines.extend(field(theme, "stores", &stored, width));
            }
            if let Err(reason) = &details.kind {
                lines.extend(field(theme, "reason", reason, width));
            }
            opens = match details.kind {
                Ok(Kind::Directory) if parent => "Enter goes up to this directory.",
                Ok(Kind::Directory) => {
                    "Enter lists this directory, and Backspace comes back to this one."
                }
                Ok(Kind::File { .. }) => {
                    "Enter shows the start of the file, read only. Nothing is run or changed."
                }
                _ => opens,
            };
        }
    }
    if let Some(name) = path.file_name().filter(|name| name.to_str().is_none()) {
        let bytes = name
            .as_encoded_bytes()
            .iter()
            .map(|byte| format!("{byte:02X}"))
            .collect::<Vec<_>>()
            .join(" ");
        lines.extend(field(theme, "name bytes", &bytes, width));
    }
    lines.push(Line::default());
    lines.extend(paragraph(theme, opens, width));
    lines
}

/// The body of the popup that shows `preview`, for a body `width` cells wide.
pub fn preview_lines(preview: &Preview, theme: &Theme, width: usize) -> Vec<Line<'static>> {
    match preview {
        Preview::Text {
            location,
            text,
            size: bytes,
            truncated,
        } => source_lines(location, text, *bytes, *truncated, theme, width),
        Preview::Empty => message(theme, width, &[(theme.body(), "The file is empty.")]),
        Preview::Binary {
            size: bytes,
            offset,
        } => message(
            theme,
            width,
            &[
                (
                    theme.body(),
                    &format!(
                        "Not shown: this looks like binary content, with a NUL byte at offset {offset}."
                    ),
                ),
                (theme.muted(), &format!("Size {}.", size(*bytes))),
            ],
        ),
        Preview::Encoding {
            size: bytes,
            reason,
        } => message(
            theme,
            width,
            &[
                (
                    theme.body(),
                    &format!("Not shown: {reason}. The preview shows UTF-8 text only."),
                ),
                (theme.muted(), &format!("Size {}.", size(*bytes))),
            ],
        ),
        Preview::Directory => message(theme, width, &[(theme.body(), "This is a directory.")]),
        Preview::BrokenLink { stored } => {
            let stored = match stored {
                Some(stored) => format!("It stores {}.", display_path(stored)),
                None => "What it stores could not be read.".into(),
            };
            message(
                theme,
                width,
                &[
                    (theme.body(), "Nothing exists where this link points."),
                    (theme.muted(), &stored),
                ],
            )
        }
        Preview::Special(special) => {
            let sentence = match special {
                Special::Fifo => "A FIFO is not opened: reading one waits for whatever writes to it.",
                Special::Socket => "A socket has no contents to show.",
                Special::BlockDevice | Special::CharDevice => {
                    "A device is not opened: reading one can do more than read."
                }
                Special::Other => "This is not a regular file, so it is not opened.",
            };
            message(theme, width, &[(theme.body(), sentence)])
        }
        Preview::Missing => message(
            theme,
            width,
            &[(
                theme.body(),
                "This no longer exists. The directory has been read again.",
            )],
        ),
        Preview::Unreadable(reason) => message(
            theme,
            width,
            &[(theme.bad(), &format!("This cannot be read: {reason}."))],
        ),
    }
}

fn message(theme: &Theme, width: usize, paragraphs: &[(Style, &str)]) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    for (index, (style, paragraph)) in paragraphs.iter().enumerate() {
        if index > 0 {
            lines.push(Line::default());
        }
        for piece in text::wrap(&text::sanitize(paragraph), width.max(1)) {
            lines.push(Line::from(Span::styled(piece, *style)));
        }
    }
    if lines.is_empty() {
        lines.push(Line::from(Span::styled(String::new(), theme.body())));
    }
    lines
}

/// A text file: what it is and what was left out, then its lines with their numbers.
fn source_lines(
    location: &Path,
    text: &str,
    bytes: u64,
    truncated: bool,
    theme: &Theme,
    width: usize,
) -> Vec<Line<'static>> {
    let file_name = location
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let language = highlight::for_file(&file_name, text.lines().next().unwrap_or_default());
    let (shown, cut) = match text.match_indices('\n').nth(PREVIEW_LINES - 1) {
        Some((end, _)) if end + 1 < text.len() => (&text[..=end], true),
        _ => (text, false),
    };
    let highlighted = match language {
        Some(language) => highlight::highlight(
            shown,
            language,
            theme,
            theme.surface,
            theme.body(),
            highlight::BUDGET,
        ),
        None => highlight::plain(shown, theme.body()),
    };
    let count = highlighted.lines.len();
    let mut notes: Vec<(Style, String)> = vec![(
        theme.muted(),
        format!(
            "{} · {count} {}{} · {}",
            language.map_or("Plain text", highlight::Language::name),
            if count == 1 { "line" } else { "lines" },
            if truncated || cut { " shown" } else { "" },
            size(bytes)
        ),
    )];
    if language.is_none() {
        notes.push((
            theme.faint(),
            "No grammar matches this file's name or first line, so it is not highlighted.".into(),
        ));
    }
    if truncated {
        notes.push((
            theme.warn(),
            format!(
                "Only the first {} of {} were read.",
                size(preview::PREVIEW_BYTES),
                size(bytes)
            ),
        ));
    }
    if cut {
        notes.push((
            theme.warn(),
            format!("Only the first {PREVIEW_LINES} lines are shown."),
        ));
    }
    if let Some(stop) = &highlighted.stopped {
        notes.push((theme.warn(), stop.sentence()));
    }
    let mut lines = Vec::new();
    for (style, note) in &notes {
        for piece in text::wrap(note, width.max(1)) {
            lines.push(Line::from(Span::styled(piece, *style)));
        }
    }
    lines.push(Line::default());
    let digits = count.max(1).to_string().len();
    let gutter = theme.markers.gutter;
    let room = width
        .saturating_sub(digits + text::width(gutter) + 2)
        .max(1);
    for (index, pieces) in highlighted.lines.iter().enumerate() {
        // Tabs and control characters are written out as the transcript writes them in code.
        let (visible, hidden) = text::literal(pieces, theme.warn(), LINE_CHARS);
        for (row, spans) in text::wrap_styled(&visible, room).into_iter().enumerate() {
            let number = if row == 0 {
                (index + 1).to_string()
            } else {
                String::new()
            };
            let mut line = vec![Span::styled(
                format!("{number:>digits$} {gutter} "),
                theme.faint(),
            )];
            line.extend(spans);
            lines.push(Line::from(line));
        }
        if hidden > 0 {
            lines.push(Line::from(vec![
                Span::styled(format!("{:>digits$} {gutter} ", ""), theme.faint()),
                Span::styled(
                    format!("… {hidden} more characters on this line are not shown"),
                    theme.faint(),
                ),
            ]));
        }
    }
    if truncated || cut {
        lines.push(Line::default());
        lines.push(Line::from(Span::styled(
            "The file continues beyond this point.".to_owned(),
            theme.faint(),
        )));
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plain_text(pieces: &[(Style, String)]) -> String {
        pieces.iter().map(|(_, piece)| piece.as_str()).collect()
    }

    fn names(files: &Files) -> Vec<String> {
        files
            .explorer
            .as_ref()
            .unwrap()
            .files()
            .iter()
            .map(|file| file.name.clone())
            .collect()
    }

    fn select(files: &mut Files, name: &str) {
        let explorer = files.explorer.as_mut().unwrap();
        let index = explorer
            .files()
            .iter()
            .position(|file| file.name == name)
            .unwrap_or_else(|| panic!("{name} is not listed"));
        explorer.set_selected_idx(index);
    }

    #[test]
    fn tabs_stop_every_four_columns_and_controls_are_written_out() {
        let theme = crate::theme::theme("ember");
        let pieces = vec![(
            theme.body(),
            "a\tbc\td\u{1b}[31m\u{7}\u{202E}é\u{7f}".to_owned(),
        )];
        let (shown, hidden) = text::literal(&pieces, theme.warn(), LINE_CHARS);
        assert_eq!(hidden, 0);
        assert_eq!(plain_text(&shown), "a   bc  d^[[31m^G<U+202E>é^?");
        assert!(shown
            .iter()
            .filter(|(_, piece)| piece.starts_with('^') || piece.starts_with('<'))
            .all(|(style, _)| *style == theme.warn()));
        // Wide characters count two columns towards the next stop.
        let (shown, _) = text::literal(&[(theme.body(), "日\tx".to_owned())], theme.warn(), 9);
        assert_eq!(plain_text(&shown), "日  x");
    }

    #[test]
    fn a_long_line_keeps_its_first_characters_and_counts_the_rest() {
        let theme = crate::theme::theme("ember");
        let pieces = vec![(theme.body(), "x".repeat(LINE_CHARS + 25))];
        let (shown, hidden) = text::literal(&pieces, theme.warn(), LINE_CHARS);
        assert_eq!(plain_text(&shown).len(), LINE_CHARS);
        assert_eq!(hidden, 25);
    }

    #[test]
    fn a_shown_name_escapes_controls_and_cannot_pass_for_another() {
        assert_eq!(display_name(OsStr::new("\u{1b}[31mred")), "\\u{1B}[31mred");
        assert_eq!(display_name(OsStr::new("line\nbreak")), "line\\nbreak");
        assert_eq!(
            display_name(OsStr::new("évil\u{202E}txt.rs")),
            "évil\\u{202E}txt.rs"
        );
        assert_eq!(display_path(Path::new("src/a\tb")), "src/a\\tb");
        // A real newline and the two characters `\n` are different names and read differently.
        assert_eq!(display_name(OsStr::new("same-a\nb.rs")), "same-a\\nb.rs");
        assert_eq!(display_name(OsStr::new("same-a\\nb.rs")), "same-a\\\\nb.rs");
        assert_eq!(display_name(OsStr::new("same-a b.rs")), "same-a b.rs");
    }

    #[cfg(unix)]
    #[test]
    fn a_name_that_is_not_utf8_is_shown_by_its_bytes() {
        use std::os::unix::ffi::OsStrExt;
        // Some filesystems reject such names, so only the text shown for them is checked.
        let first = OsStr::from_bytes(b"invalid-\xff");
        let second = OsStr::from_bytes(b"invalid-\xfe");
        assert_eq!(first.to_string_lossy(), second.to_string_lossy());
        assert_eq!(display_name(first), "invalid-\\xFF");
        assert_eq!(display_name(second), "invalid-\\xFE");
    }

    #[test]
    fn entering_and_leaving_keep_native_paths_and_select_the_directory_left() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        for directory in ["src/nested", "docs", ".git", "target", "node_modules"] {
            std::fs::create_dir_all(root.join(directory)).unwrap();
        }
        std::fs::write(root.join("src/main.rs"), "fn main() {}\n").unwrap();
        std::fs::write(root.join("b.rs"), "").unwrap();
        std::fs::write(root.join(".env.example"), "").unwrap();
        let mut files = Files::default();
        files.open(root);
        assert_eq!(files.directory(), root);
        // The parent row, then directories and then other entries, each by name; unlisted names
        // are left out and other hidden names are kept.
        assert_eq!(
            names(&files),
            ["../", "docs/", "src/", ".env.example", "b.rs"]
        );
        assert_eq!(files.shown(), 4);
        assert!(files.parent_selected());

        select(&mut files, "src/");
        assert_eq!(files.target(), Some(Target::Directory(root.join("src"))));
        assert!(files.enter(&root.join("src")));
        assert_eq!(files.directory(), root.join("src"));
        assert_eq!(names(&files), ["../", "nested/", "main.rs"]);

        assert!(files.leave());
        assert_eq!(files.directory(), root);
        assert_eq!(files.selected().unwrap().path, root.join("src"));
    }

    #[test]
    fn a_move_that_fails_keeps_the_list_and_says_why() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        std::fs::create_dir(root.join("gone")).unwrap();
        let mut files = Files::default();
        files.open(root);
        select(&mut files, "gone/");
        std::fs::remove_dir(root.join("gone")).unwrap();
        let before = names(&files);
        assert!(!files.enter(&root.join("gone")));
        assert_eq!(files.directory(), root);
        assert_eq!(names(&files), before, "the failed move lost a row");
        assert!(
            files
                .notice()
                .is_some_and(|notice| notice.contains("cannot be listed")),
            "{:?}",
            files.notice()
        );
        files.reload();
        assert_eq!(names(&files), ["../"]);
        assert_eq!(files.notice(), None);

        let mut unopened = Files::default();
        unopened.open(&root.join("missing"));
        assert!(!unopened.is_open());
        assert!(unopened.notice().is_some());
        assert_eq!(unopened.directory(), root.join("missing"));
    }

    #[test]
    fn the_filter_keeps_the_parent_row_and_moves_the_selection_to_a_match() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        for name in ["main.rs", "lib.rs", "README.md"] {
            std::fs::write(root.join(name), "").unwrap();
        }
        let mut files = Files::default();
        files.open(root);
        files.set_filter("MAIN");
        assert_eq!(names(&files), ["../", "main.rs"]);
        assert_eq!(files.selected().unwrap().path, root.join("main.rs"));
        files.set_filter("!main");
        assert_eq!(names(&files), ["../", "README.md", "lib.rs"]);
        // Entering a directory clears the filter it was typed on.
        assert!(files.leave());
        assert!(files.enter(root));
        assert_eq!(names(&files).len(), 4);
    }

    #[test]
    fn an_empty_list_is_neither_moved_through_nor_drawn() {
        // The root of the filesystem has no parent row, so a filter can leave it with no row.
        let mut files = Files::default();
        files.open(Path::new("/"));
        files.set_filter("no entry of the root is named this 7f3c");
        assert_eq!(files.rows(), 0);
        assert!(files.widget().is_none());
        for step in [
            Step::Up,
            Step::Down,
            Step::Home,
            Step::End,
            Step::PageUp,
            Step::PageDown,
        ] {
            files.step(step);
        }
        assert_eq!(files.target(), None);
        assert!(!files.leave(), "the root has no parent to go to");
        assert_eq!(files.directory(), Path::new("/"));
    }

    #[cfg(unix)]
    #[test]
    fn a_link_to_a_directory_is_listed_where_it_leads() {
        use std::os::unix::fs::symlink;
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        std::fs::create_dir_all(root.join("src/deep")).unwrap();
        symlink("src/deep", root.join("to-deep")).unwrap();
        symlink("missing", root.join("broken")).unwrap();
        let mut files = Files::default();
        files.open(&root);
        assert_eq!(
            names(&files),
            ["../", "src/", "to-deep -> src/deep/", "broken -> missing"]
        );
        select(&mut files, "to-deep -> src/deep/");
        assert_eq!(
            files.target(),
            Some(Target::Directory(root.join("to-deep")))
        );
        assert!(files.enter(&root.join("to-deep")));
        assert_eq!(files.directory(), root.join("src/deep"));
        assert!(files.leave());
        assert_eq!(files.selected().unwrap().path, root.join("src/deep"));
    }
}
