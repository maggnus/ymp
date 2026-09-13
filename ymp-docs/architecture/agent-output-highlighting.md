# Agent output: prose, code and explicit diffs

Implementation contract for YMP-140, building on the YMP-139 highlighter. The task
register records delivery status; this document does not claim implementation.

## Reference systems

| System | Relevant approach | Source |
| --- | --- | --- |
| Gemini CLI | Markdown fences are rendered as language-tagged code blocks. File diffs have a separate renderer with addition, deletion, context and hunk roles. Tool display data distinguishes strings from `FileDiff` records carrying actual diff and content fields. | [MarkdownDisplay](https://github.com/google-gemini/gemini-cli/blob/9c1b0a610534d6f8120964cf2672c07807d8fc90/packages/cli/src/ui/utils/MarkdownDisplay.tsx), [DiffRenderer](https://github.com/google-gemini/gemini-cli/blob/9c1b0a610534d6f8120964cf2672c07807d8fc90/packages/cli/src/ui/components/messages/DiffRenderer.tsx), [tool display types](https://github.com/google-gemini/gemini-cli/blob/9c1b0a610534d6f8120964cf2672c07807d8fc90/packages/core/src/tools/tools.ts) |
| OpenCode | Streamed code blocks retain raw text, language and completion state; failed highlighting falls back to readable text. Themes define syntax roles separately from diff additions, removals, context and hunk headers. | [streaming Markdown component](https://github.com/anomalyco/opencode/blob/95daf90670b7c039c436c85537da5fbfe2205b41/packages/session-ui/src/components/markdown.tsx), [theme reference](https://opencode.ai/docs/themes/) |
| Aider | Assistant replies use Markdown with a code theme, with a plain-text alternative. Showing repository differences is a distinct action after edit handling. | [assistant output](https://github.com/Aider-AI/aider/blob/5dc9490bb35f9729ef2c95d00a19ccd30c26339c/aider/io.py), [edit flow](https://github.com/Aider-AI/aider/blob/5dc9490bb35f9729ef2c95d00a19ccd30c26339c/aider/coders/base_coder.py), [output options](https://aider.chat/docs/config/options.html) |

Adopt the separation of content types and the readable fallback. Do not copy these
products' application runtimes, approval flows or repository-edit mechanisms.

## Presentation contract

- Prose keeps the existing Markdown presentation. Inline code stays literal.
- Fenced code uses its declared language and the shared syntax highlighter. Unknown
  languages remain readable plain code. Preserve internal and trailing whitespace,
  Unicode, blank lines and the meaning of tabs; any display expansion is explicit
  and leaves stored text unchanged.
- Explicit `diff` or `patch` blocks receive addition, deletion, context, filename and
  hunk-header styles. Preserve `+`, `-`, space prefixes and patch metadata, including
  the no-newline marker. Use one column in the terminal; do not add a side-by-side
  editor. A raw message may receive diff styling only when it has an unambiguous
  unified/Git patch structure. A prose list or a line beginning with `+` or `-` alone
  is not enough.
- A displayed patch is content reported by its originating message. Styling never
  means that it was applied, independently accepted or recorded as a real file
  change. Do not compute a fictional before/after pair from ordinary assistant text.
- Keep message origin labels and the full raw body available through the existing
  Detailed/Inspect behavior. Rendering must not rewrite stored messages, run a
  command, read referenced filenames or invoke a provider.
- During streaming, show an unfinished code block without inventing its missing
  ending. Recompute only affected display data and preserve selection/scroll intent.
  Length or time limits disable expensive styling with a truthful fallback rather
  than silently discarding the underlying report. Bound work per message as well as
  per line so many small blocks cannot multiply the cost without limit.
- Apply the existing semantic themes. Diff roles must remain distinguishable by
  their textual markers when color is unavailable. Avoid a second independent theme
  chooser or a different global terminal event loop.

## Existing data boundary

Current `Message` records carry text, author, kind and session identity; `UiEvent`
exposes message and text-delta events. Recorded workspace changes contain paths and
hashes rather than complete earlier file contents. Therefore this increment can
style supplied code and patches, but cannot promise an actual repository diff for
every tool operation. A richer structured edit display requires actual captured
before/after evidence and belongs in a separate runtime contract if needed later.

## Acceptance evidence

Use mocked messages and terminal scenarios covering mixed prose/code, Rust and
another language, a unified diff, a prose list that must not become a diff, an
unfinished streamed fence, long/multiple blocks and control characters. Check
literal whitespace, unchanged stored message bytes and originating labels. Exercise
light/dark themes, narrow windows and the existing Detailed/Inspect path. Run the
required workspace checks on final integration and distinguish those results from
the already completed standalone highlighter probe.
