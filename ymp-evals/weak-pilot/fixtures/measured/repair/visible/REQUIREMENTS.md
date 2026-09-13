# Repair availability windows

Repair `windows.py` using Python 3.9+ and its standard library. Do not change
`public_test.py` or these requirements. Deliver the repaired `windows.py`.

`available_windows(start, end, busy, min_length=1)` returns all maximal free
half-open intervals within `[start, end)`, sorted by start, as a list of tuples.
Remove the union of busy intervals, clip to the requested range, and omit free
intervals shorter than `min_length`. A free interval exactly `min_length` long
must remain. Touching busy intervals leave no free time between them. Empty busy
intervals occupy no time. Nested, overlapping, duplicate, unordered and entirely
out-of-range busy intervals are valid. Negative and arbitrarily large integer
endpoints are valid. Every busy pair has `left <= right`.

All endpoints and `min_length` are integers (not booleans). A nonpositive
`min_length` raises `ValueError`, even for an empty requested range. When
`start >= end` and the minimum length is valid, return `[]`.

Preserve the input `busy` list and each pair, keep repeated calls independent,
and preserve the public function names, signatures and return types.
`total_duration(windows)` must still sum `right - left` for every supplied pair,
including overlapping pairs; it must not merge, reorder or mutate them.

You may inspect files, edit your solution, run Python and create your own tests
in this directory. No network, packages, browser, other workspaces, native
subagents or model calls are permitted. Run `python3 public_test.py` for visible
examples; passing these examples alone does not establish correctness.
