# YMP-146 second-round bounded corrections

Parent contract: `acceptance-round2.md`, read from the parent workspace without
changes. Baseline: `1dff2cbcb558639a74f46fe009372a71652cc4e7`. R4 is independently
accepted and its implementation is preserved.

## Completed malformed response

The independent probe was copied with verified hashes. Only output/temp path
literals are redirected; assertions and dependency versions are unchanged.
`malformed-before.json` records exit **101** on 1dff2cb: Failed+Unknown passes;
Completed+MalformedResponse is wrongly rejected before inspection. Originals and
exact copies remain immutable in `reviewer-originals`.

Storage now accepts Completed only with the exact recorded runtime
MalformedResponse, matching assignment/invocation and obligation binding, terminal
records, and no valid accepted/negative verdict for that invocation. All scope,
independence and current-state checks remain. No failure history is cleared.

`malformed-after.json` records two zero exits: the unchanged external probe
(two passing controls) and eight sustainable strict-inspection scenarios, including
the new completed-malformed positive and ordinary accepted/negative verdict
rejections. `malformed-source.json` binds the checked implementation and tests.

## Remaining second-round work

Implement the separately authorized fresh, independent, actually read-only saved
plan review. It must not create effect_resolution or historical local scope, reuse
native context, run production, or bypass holds/budgets/unknown dependencies.
The parent has been asked to confirm the concrete downstream dependency boundary:
current tasks do not model dependencies on unknown effects of older invocations.
The final required check chain is reserved for the complete second-round source.
