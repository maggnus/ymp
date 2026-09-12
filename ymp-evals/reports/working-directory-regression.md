# Working-directory and conversation regressions

## Observed failure

The original HTML run stopped before execution because a GLM review contained explanatory prose followed by valid JSON. The old parser required the entire response to be JSON. A subsequent location question started a new session with an empty isolated copy and lost the original failure context.

## Corrected behavior

- Parse one unambiguous final JSON object after commentary; still reject multiple decisions, malformed outer objects, missing required fields, and contradictory trailing text.
- Work directly in the user's selected directory. Store file hashes, change records, history, and task state under `~/.ymp2`; create no hidden source copies or Git repositories.
- Permit one writing task at a time and hold a per-project writer lock.
- Route idle follow-up questions through the current conversation, including the previous outcome and actual file paths. A question does not create tasks or repeat execution. Explicit implementation follow-ups link to their parent conversation.

## Validation

Backend regression tests pass for direct file placement, metadata-only storage, exclusive writer ownership, commentary-prefixed JSON, negative parser cases, and follow-up context without new execution. Existing cancellation and independent-review negative controls also pass.

The requested HTML file was created at `/Users/maggnus/Downloads/_ymp2/index.html`. Independent inspection confirmed an HTML5 doctype, language and encoding metadata, title, body, heading, and paragraph. Its contents are in the requested working directory. The recovery was stopped at the user's request after the result had been verified; its session retains the paused status rather than claiming the entire post-processing sequence completed.

Historical isolated copies from the first version are retained for recovery. They are not used as working directories for new runs.
