# YMP-134 interface follow-ups: independent review

**Verdict: accept after two small required fixes, R1 and R2.** Navigation removal, detailed mode, the 0.4.2 double Ctrl+C and the YMP-132 attribution and cache fixes are correct. I found no loss of content or focus.

## Scope

- **Revision.** `/tmp/ymp134-integration` at `b01ffe7`, compared with `bd25327`.
- **Commits.** The four commits have the same `git patch-id --stable` as the author's originals: `56c9d05`, `6fd4401`, `5c3dda8` and `5e14f6e`.
- **Method.** Read-only. I ran no build, no tests and no terminal session, so the geometry below is worked out from the source.

## Required

### R1 (Low, regression from `7bda420`): the margin around a centred surface cuts the rule between the main column and the sidebar

**Cause:**

- `render_modal` clears a one-cell margin inside `bounds` (`frame.rs:326`, `clear_around` at `frame.rs:272-288`).
- `ui.rs:97` passes `rows.body` as `bounds`. That rect includes the vertical rule at column `W - S - 1` (`frame.rs:93-99`, `:124-131`, `:134-146`).
- Surfaces are centred on the whole width: `x = (W - w) / 2` (`frame.rs:321`).
- `5e14f6e` protected only the header and bottom rules.

**Effect:**

- When a surface's right margin column is the rule column, the rule loses a segment as tall as the surface plus two rows, even though the surface does not cover it.
- The widths where this happens are:
  - theme chooser: 119–120 columns;
  - Confirm: 129–130;
  - palette: 133–134 and 141–142;
  - Prompt: 137–138 and 145–146;
  - Inspect: 155–156.
- Where a surface spans the rule, the margin also removes the rule cell directly above and below it.

**Reproducer:**

1. Open a 120x36 terminal with the sidebar shown, which is the default.
2. Press `Ctrl+T`.
3. The chooser occupies columns 31–88 and the rule is column 89. The body rows beside the chooser show a blank instead of `│`.

**Test to add:**

1. At 120x36, open `Ctrl+T`.
2. For every body row (2..=32), assert that the character at column `frame::main_width(120, frame::sidebar_width(120))` (89) is `│`.
3. Optionally, repeat for the palette at 134x36.

**Fix direction:** keep the rule cells out of the cleared margin, or draw the rule again in the margin cells that lie outside the surface.

### R2 (Low, documentation): the README still describes sidebar navigation

- `README.md:57` says the sidebar "carries navigation".
- `README.md:67` says "Every destination in the sidebar is read-only".
- The interface guide and usage guide were updated; the README was not.

## Optional

### O1 (Low; existed before, now one row worse): a short terminal can hide the selected palette or theme row

- The new blank row before the keys reduces the body to `height - 6` rows (`frame.rs:308-309`).
- The palette always builds a 10-command window (`ui.rs:827-838`), and both surfaces pass `scroll: 0` (`ui.rs:796`, `:857`).
- **At 40x12:** the palette shows four commands, and pressing `Down` four times selects a fifth command that is not visible. Before, five were visible.
- **At 80x16:** 8 of the 10 commands are visible.
- **Fix direction:** size the window, or scroll it, from the rows the surface actually shows.

### O2 (cosmetic): detailed mode repeats a short message

- Kinds whose headline already contains the one-line body, such as a shared `chat` post, read `chat · TEXT` and then `TEXT` again (`transcript.rs:222-228`, expanded at `:412-421`).

## Verified

- **Navigation and focus:**
  - `Focus::Sidebar`, `nav`, `sidebar_key` and `NAV` are removed.
  - `Tab` and `Shift+Tab` switch between the composer and the transcript or page (`state.rs:820-828`, `:1030-1041`).
  - The order in which `Esc` closes things is unchanged.
  - The sidebar lists SESSION, TOKENS, TEAM and TASKS and gives way in the same order as before (`sidebar.rs:28-37`).
- **Page access:**
  - Every page the sidebar used to list has a command (`state.rs:1932-1944`, plus the `/limits`, `/memory` and `/team` handlers).
  - The palette runs commands.
  - `Enter` on the Help page runs the selected command (`state.rs:1315-1319`).
- **Detailed mode:**
  - Every stored message keeps its full body (`transcript.rs:228`) and is drawn expanded (`ui.rs:372`). This covers plans, bids, reviews, learning notes, chat posts and whole execution reports.
  - `plan_accepted`, `memory` and `notice` stay one row. Their messages are written by `ymp`, not by agents (`engine.rs:2027-2031`, `:2248-2252`, `:2585-2600`).
  - Stream previews stay bounded (`transcript.rs:451-461`).
  - In detailed mode `Space` does nothing and focus does not move (`state.rs:1240-1251`), and its hint is removed (`ui.rs:272-275`).
  - The default mode is unchanged.
- **Floating surfaces:**
  - Every surface wraps its text to `width - 4` (`frame.rs:110-121`; `state.rs:1271`, `:1494`; `ui.rs:661`, `:741`, `:804`, `:895`, `:953`).
  - Palette and Prompt cursors are placed from the padded rect that `render_modal` returns (`frame.rs:366-382`; `ui.rs:861-864`, `:946-949`).
  - The Prompt field rows always fit, because at most `height - 8` field rows are built against a body of `height - 6`.
  - Inspect scrolling still reaches the last line (`state.rs:1643-1649`).
  - The Confirm question is short.
  - Header and bottom rules stay intact for centred surfaces. The completion list clears only inside the main column and ends above the bottom rule.
- **Record alignment:** `views.rs` `field` keeps values inside the width and puts a long label on its own line. No value is lost.
- **0.4.2 and YMP-132:**
  - `exit.rs`, `provenance.rs` and `label.rs` are unchanged, and so is the attribution lookup in `transcript.rs:173`.
  - The every-region Ctrl+C test drops only the removed sidebar region.
  - No YMP-131 or YMP-132 test was removed.

## Limits

- R1 is worked out from layout arithmetic and was not observed in a terminal.
- I did not compare the surfaces visually with earlier ymp.
- I did not re-run or re-check the author's logs.
