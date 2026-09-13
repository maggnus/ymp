# Independent acceptance plan for YMP-139

This plan defines pending checks, not implemented behavior or completed evidence.

1. Re-run `parent-probe.py` against the integrated candidate. The baseline must fail
   directory descent, and the candidate must descend and display the literal source
   sentinel with internal whitespace intact. Compare project checksums before/after.
2. Inspect the library comparison against linked upstream releases, package manifests
   and licenses. Confirm the actual normal dependency graph uses the chosen versions
   without introducing a second Ratatui runtime. Check fallback and theme behavior in
   code, not just the author's report.
3. In a real terminal, descend and ascend, retain selection, and open a source preview.
   Check page/filter/focus interactions and closure of the preview. Inspect captured
   syntax colors on a dark and a light palette with unchanged source text.
4. Exercise binary input, unknown syntax, invalid UTF-8, large files, oversized lines,
   deleted/unreadable entries and special files. Verify explicit limits/fallbacks and
   responsiveness. Confirm source contents are not interpreted as Markdown or terminal
   commands and that directory navigation does not change execution cwd/session data.
   The owner clarified that /files must use a ready-made library widget; the agreed
   component is ratatui-explorer 0.3.0, with normal parent-directory navigation.
5. Inspect and test native path identity, including distinct non-UTF-8 names which can
   have identical lossy captions. Check symlink loops and explicit selection of a link
   to a file outside the initial cwd. Parent traversal and such deliberate file
   selection are allowed; the application's execution cwd must remain unchanged.
   Use only temporary owned fixtures. The prior parent-imposed root restriction is
   superseded by the ready-widget integration decision.
6. Run the required formatting, strict workspace Clippy and workspace tests once on
   final integrated source. Run the existing table/theme and deliberate-exit checks
   where new focus or overlays can affect them, with mock/scripted providers only.
7. Preserve an existing installed executable before replacement; verify the built and
   installed artifact, record exact commits/hashes and limitations, and update task
   evidence. Archive finished assignment workspaces only after commits and useful
   evidence are retained.
