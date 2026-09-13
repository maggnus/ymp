# YMP-140 independent acceptance

Author: Claude Code `claude-opus-5`, requested and effective `xhigh`, continuing the owner's
existing assignment. Author commit: `8d3da6a0f4743fb7fa52bd31b55b38802fcd618d`.
Integration: `0829dc6`; release source: `f29bb7a21b01e29250f1ab27f63eccfcfa8f9e61`.
The parent reviewed the code independently and owns the terminal checker and release process.

## Acceptance mapping

| Requirement | Observation and evidence |
| --- | --- |
| Established chat approaches | Pinned Gemini CLI, OpenCode and Aider sources and the parent presentation contract are recorded in `reference-sources.json` and the architecture document. |
| Inline syntax and explicit diff roles | `release-045/output-color/report.json` passes both inline Detailed conversation and actual selected-message Inspect in Ember/Latte. Baseline `baseline-message/report.json` has Inspect colours but lacks inline colours, so the checker distinguishes the requested change. |
| Prose remains prose | The terminal checker asserts list text has the prose style and code styles do not leak to the following paragraph. Source `diff::scan` requires file headers and complete hunk counts for unlabelled patches; unit tests reject incomplete or list-like text. |
| Literal text and origin | Terminal checks hash all stored messages before/after display. Rust tests cover trailing spaces, tabs, Unicode, visible control escapes, narrow wrapping and exact invocation attribution. The change performs no storage writes, filename reads or provider calls in message formatting. |
| Streaming and scroll intent | Rust tests feed incomplete and completed fences, a long trimmed stream and updates while the reader has scrolled upward. The preview stays bounded and preserves its open fence; completed messages use the shared highlighter. |
| Bounded styling and cache | Source review covers 64 KiB/message admission, cooperative block/line time checks, retained results, frame deferral and eviction. Unit tests cover oversized lines/messages and frame/cache limits. Limits are documented precisely below. |
| Light/dark and monochrome | Independent physical terminal checks pass two colour themes and two monochrome configurations; patch markers and message bytes are preserved. |
| Required integration checks | The final source passes fmt, strict workspace Clippy and 554 Rust tests (0 failed, 2 ignored), plus 12+12 file-navigation, 19 table/theme and five deliberate-exit cases. Exact results are in `release-045/verification.json`. |

Paths beginning `release-045/` are relative to the sibling evidence directory. The installed
command has a separate terminal verification recorded there; source hashes identify the exact
binary inputs. No real model requests were made by the independent checks.

## Review decisions and limits

Accept fences indented with list items, and treat a completely structured patch in an unlabelled
fence as a patch. These are intentional supported-subset decisions, not full CommonMark support.
Syntax colours apply to completed messages; streaming code is literal plain code while declared
patches have line-role colours. This satisfies the streaming contract without inventing endings.

Time budgets are cooperative: a started block can exceed the 50 ms frame allowance and a started
line is not cancelled. Cold grammar initialization is separate. The 8 MiB cache limit counts
source bytes, not styled output allocation or total memory. Evicted blocks can be highlighted
again. Parent documentation corrections record these boundaries without claiming hard deadlines
or instantaneous first display. No new renderer implementation was authored by the parent.

No blocking source-review finding remains. Linux/Windows runtime checks and native-provider
inference were not run. Existing synchronous directory enumeration remains an explicit limitation
from YMP-139. Styling a patch never creates actual repository before/after evidence.
