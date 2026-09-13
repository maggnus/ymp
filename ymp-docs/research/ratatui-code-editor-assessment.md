# ratatui-code-editor replacement assessment

Status: research only, requested on 2026-09-13. YMP-142 records completion; YMP-141 (popup
geometry) is paused at the owner's direction. Viewing and editing with explicit saving is the
intended future capability. No editor integration or replacement is authorized in this stage.

Latest owner direction: keep `/files` read-only with the current highlighter for now. Editing
is deferred; live Git inspection is a separate feature recorded in
[separate-git-inspection.md](separate-git-inspection.md). The editor findings below remain research.

## Finding

`ratatui-code-editor` can supply the buffer and widget for an embedded file editor, including
its own Tree-sitter highlighting. It is not a replacement for every component delivered in
YMP-139/140. The strongest overlap is the highlighted **file-content view**; directory navigation,
file I/O policy, Markdown/message presentation and reported-patch semantics remain separate.
The existing work is partly reusable even if its syntax backend changes later.

## Verified candidate

Published version: **0.0.6**, released 2026-07-07, MIT. Upstream revision:
`40ff181514914a8602d2e5d2df647cca1f0ef621`. The published package's `.cargo_vcs_info.json` names
that revision, and all published `src/*.rs` bytes match the checked-out upstream source.
The standalone parent probe compiles and runs on actual **Rust 1.89.0**, Ratatui **0.30.2** and
Crossterm **0.29.0**. This is measured compatibility; the crate itself does not declare a Rust
minimum. Cargo selected `tree-sitter-language 0.1.7`; 0.1.8 requires Rust 1.90. Keep a
compatible resolved lockfile when evaluating this crate on Rust 1.89. Its workflow tests stable
Rust on Ubuntu, not a declared cross-platform matrix.

The library depends on `ratatui-core ~0.1.0`, Tree-sitter `~0.26`, Ropey and sixteen language
crates. Crossterm is optional; grammars, diff support and `arboard` are not optional features.
Thus using only its highlighting API still includes its broader dependency set. The version and
source facts are recorded in the [research evidence](evidence/code-editor-142/verification.json).

## Replacement map

| Current responsibility | Candidate coverage | Consequence |
| --- | --- | --- |
| `ratatui-explorer` directory navigation, selection, filter and native paths | No filesystem browser. `Editor::new` takes language and text. | Retain the explorer or replace it separately. |
| Read-only file-content preview, cursor movement, selection and future editing | Editor widget, Ropey buffer, undo/redo, selection, folding and highlighting. | Could replace the content view for editable files. Input routing and read-only behavior remain host responsibilities. |
| Bounded file reads and handling of binary, unsupported-encoding, missing and special files | The widget receives text; it does not implement the application file loader. | Retain or evolve `ymp-workspace::preview`; a truncated preview must not become an editable full document. |
| File saving and unsaved-buffer lifecycle | `get_content` returns text. The example opens/truncates a file directly when saving. | Explicit save, dirty state, close/discard behavior, conflict detection, permissions and coordination with agent writes require a separate host contract. |
| `tui-syntax-highlight` + syntect + two-face for file syntax | Tree-sitter parsing and token-range highlighting are built in. | Avoid applying both syntax engines to the same editor buffer. This is real overlap. |
| Syntax engine for fenced code in messages | Public `Code::highlight_interval` could provide token ranges. | Replacing syntect is possible as a separate backend migration, not by dropping in the editor widget. Preserve language detection/fallback, limits, caching and semantic styles. |
| Markdown prose, fenced blocks, literal wrapping, incomplete streamed messages and originating labels | The `markdown` grammar highlights Markdown source. It still displays heading markers and fences. No chat/timeline renderer. | Retain `text.rs`, transcript/state integration and their contracts; a different backend can sit beneath them. |
| Reported unified/Git patches in messages | Built-in diff compares actual original and current buffers. The `diff` language has no grammar in this version. | It does not replace the current parser/styles for supplied patches. Never infer original file contents from ordinary agent prose. |
| Actual before/after file comparison | `set_original_code` plus current content supports added/deleted rows and focused context. | Useful for a future editor review view when both real versions exist. It does not create missing historical contents. |
| Eighteen semantic themes, monochrome behavior and global shortcuts | Token/diff theme keys and optional native input handler. Some fallback styles are hard-coded. | Application adapters and verification remain necessary. |
| Popup geometry bug | The editor renders into a rectangle supplied by ymp. | The shared modal sizing defect remains a host issue; swapping the content widget does not fix other popups. |

## Reproduced integration issues

1. **Light-theme fallback.** With black `text`/`default` theme entries and a white parent
   background, unclassified text is rendered white on white. `render.rs` hard-codes
   `Color::White` for default text; line numbers, selection and fold separators also use fixed
   `DarkGray`. Token colour configuration alone does not solve the fallback.
2. **Wide-character horizontal focus.** In a 30-column area, moving to the end of twelve CJK
   characters leaves `get_visible_cursor` as `None` while `offset_x` stays zero. This reproduces
   upstream issue #15; PR #16 is open, not a released fix. Do not generalize this to every
   Unicode operation: the probe's Unicode buffer and undo/redo checks pass.
3. **Newline policy.** Loading and round-tripping CRLF is lossless, but Enter inserts LF.
   The probe changes `first\r\nsecond\r\n` to `first\n\r\nsecond\r\n`. The host must define
   and test newline handling for edited documents before saving.
4. **Input routing.** Passing both press and release events inserts the character twice.
   The host must filter event kinds. Unhandled control combinations can fall through to
   ordinary character insertion; Home/End/PageUp/PageDown/Delete are not all provided by the
   default handler. Global ymp shortcuts must be intercepted deliberately.
5. **Literal tabs and controls.** The buffer retains them, but the default view draws a tab
   as a single space. This differs from ymp's four-column tab stops and visible control escapes.
   Buffer fidelity must not be mistaken for faithful visual presentation.
6. **Work and memory limits.** Visible-range token queries are cached, but initial construction
   parses the full input synchronously. The interval cache is a `HashMap` without a size/eviction
   limit. A roughly 240 KiB Rust fixture took about 230 ms to construct and 102–105 ms for its
   first draw in the development probe. Those numbers are observations, not a fair release-mode
   benchmark or proof of superiority/inferiority to syntect.

Detailed inputs and outputs are in [probe.rs](evidence/code-editor-142/probe.rs) and
[probe-observations.json](evidence/code-editor-142/probe-observations.json). The probe changes only
in-memory buffers; it reads no clipboard and writes no user file.

## Options and recommendation

**Use it only for editable file content.** Retain the explorer, bounded file I/O and the existing
chat renderer/highlighter. Let the editor use its own Tree-sitter syntax for its buffer. This
limits the migration and preserves already-verified chat behavior, but ships two syntax stacks.
The reproduced theme, navigation, newline and input issues need explicit resolution first.

**Unify the syntax backend later.** Use the candidate's public Code API, or a separately chosen
Tree-sitter integration, below the existing chat renderer. Only after equivalent grammar coverage,
text fidelity, resource limits and performance are confirmed could syntect, two-face and
`tui-syntax-highlight` be removed. Markdown, stream handling, patch classification and file I/O
would remain. The candidate supports sixteen language families with a Markdown-inline parser;
current two-face coverage is broader, so missing-language fallback must be evaluated explicitly.

The present recommendation is to retain the installed solution while evaluating the candidate
as a file-editor component. Do not replace every layer or run two highlighters on one buffer.
The earlier work supplied both useful functionality and executable contracts that can validate
any later replacement; the file-content renderer and syntax adapter are the parts most likely
to become redundant if that replacement is accepted.

The parent and Claude independently reviewed the source and agreed on these boundaries;
[joint-decision.json](evidence/code-editor-142/joint-decision.json) records the final recommendation.
The Code API accepts an explicitly chosen language and a generic style mapping through
`highlight_interval`; hex-theme construction and extension mapping are convenience APIs. Custom
queries do not add parsers beyond the fixed language set.

YMP-142 research is complete. Full application/editor integration, file saving, a release-mode
comparison and Linux/Windows runtime behavior remain unverified. YMP-141 is paused with an
unrun 185-line test draft retained separately. The installed application remains 0.4.5.

## Primary sources

- [Pinned editor source and APIs](https://github.com/vipmax/ratatui-code-editor/tree/40ff181514914a8602d2e5d2df647cca1f0ef621/src)
- [Published crate](https://crates.io/crates/ratatui-code-editor/0.0.6)
- [Dependency manifest](https://github.com/vipmax/ratatui-code-editor/blob/40ff181514914a8602d2e5d2df647cca1f0ef621/Cargo.toml)
- [Upstream usage and feature description](https://github.com/vipmax/ratatui-code-editor/blob/40ff181514914a8602d2e5d2df647cca1f0ef621/README.md)
- [Wide-glyph cursor issue](https://github.com/vipmax/ratatui-code-editor/issues/15) and [proposed correction](https://github.com/vipmax/ratatui-code-editor/pull/16)
- [Request for optional grammars](https://github.com/vipmax/ratatui-code-editor/issues/14)
