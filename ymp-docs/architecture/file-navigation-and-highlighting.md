# File navigation and syntax highlighting

Implemented for YMP-139. This record compares the libraries that were considered for the `/files`
page and for syntax highlighting, states what was chosen and why, and describes how the chosen
libraries are integrated, with the limits that follow from them. The user-facing behaviour is
described in the [interface guide](../guides/interface.md#files).

## Requirements

- `/files` is a read-only file navigator. It starts in the directory ymp was started in, may move
  above it, and never changes the directory runs work in, the loaded session or any provider
  setting. Nothing on the page writes, renames or removes anything, executes content or asks a
  provider anything.
- The owner required a reusable Ratatui file-explorer library for navigation. A navigator written
  for ymp does not satisfy the request; the table layout the page had before could change.
- An entry keeps its native `Path` identity independently of the text shown for it. Empty,
  unreadable, removed and changed paths are ordinary states, and symbolic links are explicit.
- Reads and highlighting are bounded. A FIFO, socket or device is never opened as a regular file.
  Binary content, unsupported encodings and unknown languages have truthful fallbacks, and
  truncation is visible. No byte of a name or a file reaches the terminal as a control sequence.
- Source previews are highlighted in the semantic colours of every ymp theme, light and dark, and
  the same highlighter serves fenced code where that is natural.

## File navigation

### Candidates

Every candidate was read in its published source. Popularity was not a criterion.

| Library | Version | Ratatui | Licence | Findings |
| --- | --- | --- | --- | --- |
| [ratatui-explorer](https://crates.io/crates/ratatui-explorer/0.3.0) ([source](https://github.com/tatounee/ratatui-explorer)) | 0.3.0 | 0.30 | MIT | A small widget and state. The builder takes a working directory or a working file, a `filter_map` over entries, and a `Theme` of styles, symbol and title closures. Each `File` keeps the native `PathBuf` beside a display `name`. It has no operation that writes. Two defects need care: `Input::Right` removes the selected row with `swap_remove` before it tries to read the directory, and the movement inputs and the widget index the selected row, which panics on an empty list. |
| [tui-file-explorer](https://crates.io/crates/tui-file-explorer/2.3.0) ([source](https://github.com/sorinirimies/tui-file-explorer)) | 2.3.0 | 0.30 | MIT | A file manager rather than a navigator: its filesystem abstraction creates and removes files and directories (`src/filesystem.rs` lines 15–21), which a read-only page would have to keep disabled. Path shortening slices a string at a computed byte offset (`&path_str[skip..]`, `src/render.rs` line 335), which panics when the offset falls inside a multi-byte character. |
| [ratatui-interact](https://crates.io/crates/ratatui-interact/0.5.3) ([source](https://github.com/Brainwires/ratatui-interact)) | 0.5.3 | 0.30 | MIT | The file explorer is one of about thirty components in a general toolkit. It discards directory load errors (`let _ = self.load_entries()`, `src/components/file_explorer.rs` lines 206, 214 and 285), so an unreadable directory would look empty, and its default styles are fixed terminal colours. |
| [ratatree](https://crates.io/crates/ratatree/0.4.0) ([source](https://github.com/namil-k/ratatree)) | 0.4.0 | 0.30 | MIT | A credible picker with list and tree views, fuzzy search, multi-select and mouse handling. Those modes exceed what the page needs and would each need a read-only review. |
| [ratatui-async-explorer](https://crates.io/crates/ratatui-async-explorer/0.3.4) ([source](https://github.com/caelansar/ratatui-explorer)) | 0.3.4 | 0.29 | MIT | A fork of ratatui-explorer. The published crate and upstream `master` (`be87d96`, the only branch) depend on Ratatui 0.29, so it would add a second Ratatui whose widget trait a 0.30 frame cannot draw. Its `FileSystem` trait and `FileEntry.path` are `String`s built with `to_string_lossy`, which loses the identity of a name that is not UTF-8, and the trait includes `delete`, which its crossterm input conversion binds to `d`. `LocalFileSystem` does use `tokio::fs` with 5 s and 2 s timeouts, but `handle(..).await` runs inline, so the interface still waits unless the host schedules the work; there is no cancellation or handling of a result that arrives after the reader moved on. Its `Right` input keeps the same `swap_remove` defect. |

An earlier draft built a navigator on cap-std directory handles. cap-std is a capability
filesystem library, not a navigator, and the owner's correction superseded that design.

### Decision

ratatui-explorer 0.3.0. Its interface matches a read-only navigator: nothing in it writes, the
native path of every entry is available, `filter_map` lets ymp choose what is listed and how each
name reads without touching the path, and the theme accepts the semantic styles of every ymp
theme. Its two defects lie in inputs ymp does not need to send and in a state ymp can check
before sending input or drawing. ratatree was the strongest alternative, and the async fork was
rejected on Ratatui compatibility and path identity before scheduling was even considered.

### Integration

`ymp-tui/src/files.rs` owns a `FileExplorer` and nothing else about the directory.

- **Entering** a directory calls `set_cwd`, and **leaving** calls `set_working_file` with the
  directory being left, which lists the parent and selects that directory's row. Both assign the
  new list only after the directory was read, so a failure leaves the list as it was and ymp shows
  the reason above it. `Input::Right` and `Input::Left` are never sent. A symbolic link to a
  directory is canonicalised first, so the header names the real directory and going up leaves it.
- **Movement** sends `Up`, `Down`, `Home`, `End`, `PageUp` and `PageDown` to the explorer only when
  its list has a row, and the widget is drawn only then. The list can be empty at the filesystem
  root, which has no parent row, under a filter.
- **Names and filtering** go through one `filter_map`. It keeps the parent row, leaves out `.git`,
  `node_modules`, `target`, `__pycache__` and `.ymp2`, and replaces `File.name` with an escaped
  name: control and bidirectional formatting characters as `\u{..}`, newline, tab and carriage
  return as `\n`, `\t` and `\r`, bytes that are not UTF-8 as `\xFF`, and a backslash doubled so no
  escape can be forged. A link shows ` -> ` and the target it stores; a directory ends in `/`. The
  page filter is matched against that name without regard to case, with a leading `!` inverting
  it. `File.path` is never changed, and Enter, `d` and previews use it.
- **Ordering** is the explorer's own: the parent row, then directories, then every other entry,
  each group sorted by the name as shown. Hidden entries are listed.
- **Styles** come from the ymp theme: directory rows in the accent colour, other rows in the text
  colour, and the selection marked with the theme's selection marker and selected style, or the
  activity marker when the page does not own the keyboard, as page tables mark it.
- **Enter** on a file reads it through `ymp_workspace::preview`, which examines the path with
  `symlink_metadata`, follows a link only to learn what it names, opens a regular file with
  `O_NONBLOCK`, checks the type again on the open handle and reads at most 256 KiB. A FIFO, socket
  or device is described and never opened; a file that became a directory is entered.
- **Reading again** happens when the page opens, on every move, filter change and `r`, and when a
  run finishes while the page is open; the selected entry stays selected while it is listed.

### Limits

- The explorer reads a whole directory with `std::fs::read_dir` on the interface thread and asks
  for the metadata of every entry, following links; ymp adds one `readlink` per entry. A very large
  directory or a slow network mount holds the interface until it has been read, and there is no
  timeout or cancellation. A background loader that builds the explorer on another thread and
  discards a result the reader has moved away from would remove this, but it is not implemented.
- A directory is not watched between reads.
- Names sort by the text shown for them, not by the bytes of the native name.
- `PageUp` and `PageDown` move twelve rows, the explorer's constant, and `Up` and `Down` wrap
  from one end of the list to the other.
- The unlisted names are hidden in every directory, including directories above the start.
- Special files are identified on Unix only; elsewhere they are reported as special without a kind.

## Syntax highlighting

### Candidates

| Option | Version | Compatibility | Findings |
| --- | --- | --- | --- |
| [tui-syntax-highlight](https://crates.io/crates/tui-syntax-highlight/0.2.0) ([source](https://github.com/aschey/tui-syntax-highlight)) over syntect and two-face | 0.2.0 | `ratatui-core` 0.1, the core of Ratatui 0.30.2; Rust 1.88; MIT OR Apache-2.0 | An adapter, not an editor or viewer widget. `Highlighter::highlight_line(&str, &mut HighlightLines, line, Style, &SyntaxSet)` highlights one line into a `Line<'static>`, so the caller keeps its own per-line length and time bounds. It takes any syntect `Theme`, so the semantic role theme applies unchanged; `line_numbers(false)` leaves numbering to the page. Its default regex engine is Oniguruma; with default features off and `regex-fancy` on it uses the same pure-Rust engine as the bundled grammars. It appends the `\n` the grammars expect and strips only `\n` again, and its converter sets a background from the theme settings. |
| [syntect](https://crates.io/crates/syntect/5.3.0) ([source](https://github.com/trishume/syntect)) directly, with [two-face](https://crates.io/crates/two-face/0.5.2+bat-0.26.1) ([source](https://codeberg.org/CosmicHarper/two-face)) | 5.3.0 and 0.5.2+bat-0.26.1 | Independent of Ratatui; MIT, and MIT OR Apache-2.0 | The engine every option in this table except tree-sitter uses. The earlier draft drove `ParseState` and `HighlightIterator` itself and converted styles by hand; that is the same work the adapter already maintains. two-face supplies the grammars bat collects; its `syntect-fancy` bundle leaves out ARM Assembly, JavaScript (Babel), LiveScript, PowerShell, Sass and Salt State SLS, whose definitions need Oniguruma. |
| [syntect-tui](https://crates.io/crates/syntect-tui/3.0.6) | 3.0.6 | Ratatui 0.29 | Style conversion only. It would add a second Ratatui. |
| [tree-sitter-highlight](https://crates.io/crates/tree-sitter-highlight/0.27.0) ([source](https://github.com/tree-sitter/tree-sitter)) | 0.27.0 | Rust 1.90, above the pinned 1.89 toolchain (0.26.13 requires 1.84) | Parses with a grammar per language, each a crate compiled from C, and reports byte ranges with capture names that ymp would have to map to styles and lines itself. Coverage grows one grammar crate at a time. [lumis](https://crates.io/crates/lumis/0.13.1) 0.13.1, which packages many such grammars, requires Rust 1.91, and [arborium](https://crates.io/crates/arborium/2.18.2) 2.18.2 is oriented to HTML and ANSI output. |

### Decision

tui-syntax-highlight 0.2.0 as the adapter, over syntect 5.3.0 with the two-face 0.5.2 grammars.
Choosing the adapter did not mean giving up syntect: the adapter is a thin, maintained layer over
the same engine, and its per-line entry point keeps every bound ymp needs. An independent probe run
by the integration owner against Ratatui 0.30.2 passed Unicode text, repeated spaces, tabs,
multi-line comment state, semantic colours, and Rust, Python, TOML and TypeScript sources, in
166 ms including loading the syntaxes; that probe is recorded outside this branch.

### Integration

`ymp-tui/src/highlight.rs`:

- **Grammars** load once, on first use, from `two_face::syntax::extra_newlines()`. A file's
  language is found by its whole name, its extension, its extension in lower case, then its first
  line; a fenced block's language by the first word of its info string. Plain Text counts as no
  language.
- **Colours** come from a syntect `Theme` built for the active ymp theme, with no TextMate colour
  scheme involved:

  | Scopes | Role |
  | --- | --- |
  | comments | muted, italic |
  | strings and escapes | good |
  | numbers and constants | warn |
  | keywords and storage | accent |
  | types, classes, namespaces, attribute names | info |
  | functions | text |
  | tags | accent |
  | headings | accent, bold |
  | inserted, deleted and changed markup | good, bad and warn |
  | invalid | bad |

  A role colour whose contrast with the background it is painted on is below 3:1 is replaced by
  the theme's body or text colour, whichever reads better; a test checks every role in all 18
  themes on the panel, raised and screen backgrounds.
- **Lines** go to `highlight_line` one at a time without their `\n` or `\r\n` ending, so a carriage
  return never reaches the adapter. The background the adapter sets is cleared, so the surface
  keeps its own. After a line longer than 4 KiB, or once 250 ms is spent (never before the first
  line), the rest is plain text and the reason is stated. An error from the grammar stops
  highlighting the same way.
- **Text** is never changed by highlighting. File previews expand tabs to four-column stops and
  write control characters in caret notation and bidirectional formatting characters as
  `<U+XXXX>`, in the warning colour, after highlighting. A preview shows at most 5,000 lines and
  1,000 characters of each line and states every cut.
- **Fenced code** that names a known language is highlighted in the popup that shows one message.
  The transcript does not highlight, because it lays out every entry again as it changes.
- **Notices**: the Help page links the acknowledgements of the bundled syntax definitions for the
  exact two-face version, from `two_face::acknowledgement::url()`.

## Verification

- `ymp-workspace` preview tests: the byte limit and a limit inside a multi-byte character, empty,
  binary, UTF-16, Latin-1, byte-order-marked and control-byte content, links to files, directories,
  nothing, a loop and a device, a FIFO opened without waiting, a socket, an unreadable file, and a
  tree that is unchanged afterwards.
- `ymp-tui` unit tests: ordering and unlisted names, entering and leaving with native paths, a
  failed move that keeps the list, the filter and its selection, an empty list at the filesystem
  root, a link to a directory, name escaping including names that are not UTF-8, tab stops, control
  escapes, the per-line character limit, language lookup, role colours, the long-line and budget
  stops, and legibility of every role in every theme.
- `ymp-tui` screen tests drive the application through its key handler and read the drawn
  terminal: descent and return to the same row, literal source with repeated spaces and a tab, role
  colours on screen and a theme change, large, binary and empty files, an unreadable file and
  directory and a FIFO, a directory removed after listing, names with a newline, an escape
  sequence and a backslash that each open their own file, a link described and followed, the
  filter and `Esc`, browsing above the start without changing the working directory, the session,
  the configuration or any file, `Ctrl+C` twice from a preview, and every key on an empty list.
- `ymp-evals/scripts/check-file-navigation.py` exercises a built binary in tmux, with Unicode and
  with ASCII markers: opening at the start directory, descent, a highlighted literal source preview
  whose keyword and call carry different terminal colours, return to the same row, names that read
  alike, binary, FIFO and large files, a link to a directory, the filter, browsing above the start,
  80 and 60 column terminals, an unchanged project and `Ctrl+C` twice. It runs the application
  without an inherited `NO_COLOR`, which the application honours, and records whether one was
  inherited. Its reports are in `ymp-docs/evidence/ymp-139/terminal-report.json` and
  `terminal-ascii-report.json`.
- `ymp-docs/evidence/ymp-139/before-file-navigation.txt` shows the two screen tests for descent and
  literal source failing on the base commit, and `after-file-navigation.txt` shows them passing on
  this branch. The same directory holds the formatting, lint and workspace test output.

## Remaining work

- Background directory loading with cancellation, if slow or very large directories matter.
- The table checker's old `/files` assertions (column titles and numeric sort) are adapted by the
  integration owner.
- Agent output rendering (Markdown, fenced code, explicit diffs) is YMP-140.
