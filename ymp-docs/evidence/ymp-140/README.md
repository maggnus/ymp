# YMP-140 evidence: agent output highlighting

The contract is [agent-output-highlighting.md](../../architecture/agent-output-highlighting.md).
Every scenario uses mocked messages and text deltas; no provider or model was invoked.

## Required workspace checks

Run with `CARGO_BUILD_JOBS=2`, `CARGO_PROFILE_DEV_DEBUG=0`, `CARGO_INCREMENTAL=0` and the target
directory inside the worktree.

| Check | Result | Output |
| --- | --- | --- |
| `cargo fmt --all --check` | exit 0 | [fmt.txt](fmt.txt) |
| `cargo clippy --workspace --all-targets -- -D warnings` | exit 0 | [clippy.txt](clippy.txt) |
| `cargo test --workspace` | exit 0; 554 passed, 0 failed, 2 ignored | [workspace-tests.txt](workspace-tests.txt) |

These results are separate from the standalone highlighter probe completed earlier, which was not
repeated here, and from the terminal checkers `check-agent-output.py` and
`check-table-interface.py`, which were not run in this branch.

## Highlighting tests

[highlighting-tests.txt](highlighting-tests.txt) is the last of three consecutive runs of the 28
tests below; each run passed all 28. Colours are read from Ratatui's `TestBackend`, so `NO_COLOR`
in the environment does not affect them.

| Acceptance item | Tests |
| --- | --- |
| Mixed prose and code; Rust and another language; light and dark themes | `mixed_prose_and_code_take_syntax_colours_inline_in_dark_and_light_themes` (Ember and Catppuccin Latte: Rust and Python role colours, prose body colour, literal inline code, no fence lines drawn) |
| A unified diff | `a_declared_diff_is_styled_by_line_role_and_keeps_its_markers` (Slate and Solarized Light); `diff::tests::every_line_of_a_git_patch_has_its_role`, `a_patch_ends_after_its_last_complete_hunk`, `a_declared_block_is_classified_without_counts` |
| A prose list that must not become a diff | `a_raw_git_patch_is_styled_but_a_list_of_plus_and_minus_lines_is_not`; `diff::tests::text_that_only_looks_like_a_patch_is_not_one` |
| Diff roles without colour, and in every theme | the marker assertions in `a_declared_diff_is_styled_by_line_role_and_keeps_its_markers`; `diff::tests::every_role_reads_in_every_theme` |
| An unfinished streamed fence; selection and scroll intent | `a_streamed_code_block_is_shown_unfinished_without_moving_the_reader`, `a_long_stream_keeps_the_fence_of_the_code_block_it_is_inside`, `a_single_line_stream_cannot_fill_the_transcript` |
| Fence boundaries | `fences_close_only_on_a_matching_fence_and_keep_every_code_character` (longer and tilde fences, text after a fence, list indentation, CRLF, backticks in the info string, a block left open) |
| Long and multiple blocks; bounded work per message, per line and per frame | `highlighting_stops_at_the_message_limit_and_every_block_stays_whole`; `highlight::tests::a_long_line_or_a_spent_budget_leaves_the_rest_plain`, `a_kept_result_is_reused_for_the_same_text_theme_and_styles_only`, `the_cache_forgets_its_oldest_results_to_stay_within_bounds`, `a_result_the_frame_used_goes_last_and_the_frame_hears_when_it_must_go`, `a_frame_with_no_work_left_defers_new_text_but_not_kept_text` |
| Control characters and literal whitespace | `a_code_row_keeps_trailing_spaces_tabs_and_writes_control_characters_out`, `wrapping_keeps_whitespace_that_carries_meaning`, `a_fenced_code_block_is_rendered_character_for_character`, `files::tests::tabs_stop_every_four_columns_and_controls_are_written_out` |
| Unchanged stored message bytes; origin labels; the Inspect path | `rendering_leaves_the_stored_message_and_its_origin_unchanged` |
| Detailed mode; narrow windows | `detailed_mode_and_a_narrow_window_keep_code_whole_and_highlighted` |

The remaining tests in the run cover the highlighter shared with file previews:
`a_language_is_found_by_name_extension_first_line_or_token`,
`highlighting_keeps_the_text_and_colours_it_by_role`,
`every_role_reads_on_panels_and_code_in_every_theme` and
`the_notice_names_the_bundled_version`.

## Not covered here

- No terminal captures before and after the change were recorded for this task.
- No physical terminal colour check was run; the parent's `check-agent-output.py` covers colour and
  monochrome terminals.
