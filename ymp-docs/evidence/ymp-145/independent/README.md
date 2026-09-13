# Independent acceptance

The parent accepted the product source at `35786ec` after independent Claude Code
review and direct inspection of the shared drawing/caller changes. The two factual
README corrections were committed as `ee02248`; product and checker source bytes
did not change. Both commits are integrated in main. Parent comparison of Rust,
bridges, evaluation scripts, Cargo manifests/lockfile and Cargo configuration
against the accepted author revision has no differences.

The author ran formatting, strict Clippy and the workspace suite on these product
bytes: 579 passed, two ignored. Independent checks additionally exercised nine
targeted tests, the Git full-row check, nine popup geometry cases, dark/light
palette/theme/completion selection and small-terminal navigation. Existing 0.4.6
is the negative control: the same observer finds 22 unfilled cells out of 54 in
both dark and light theme selection, while the candidate fills the expected row.

Raw probe reports are deliberately retained, including `passed: false` from
expectations subsequently diagnosed as pre-existing behavior. In NO_COLOR, palette
selection uses its marker, with no SGR or bold; the old and new builds agree.
At 60x8, search and separator consume the body, so selection is hidden while
geometry stays fixed. The same defect exists in 0.4.6 and is recorded as YMP-152.
The 60x9 and 44x10 cases retain visible selection and fixed geometry. Neither
finding is a new regression or evidence for the reported shrinking.

The renewed shrinking report did not reproduce in author or independent checks.
Its originating path/version remains unknown; this acceptance claims full-row
selection correction and the documented geometry observations, not a newly
reproduced or corrected shrinking defect.

Limits: the final 12 theme-chip columns are excluded for blank cells by both
observers, so the source inspection also supports that boundary. Wide glyphs were
verified in Ratatui buffers rather than independently in the terminal. The unused
Some-selected/positive-scroll combination has no new explicit test. Linux runtime
and a new installed release are not claimed by this source acceptance.

The manifest binds retained independent files by exact hashes. The first probe
attempt with an observer error is excluded; `probe-candidate-2` is the corrected
observer run. Review-generated Python bytecode in the author tree was removed by
exact path, and that tree was clean before integration. No real provider inference
or comparative product experiment was used.
