# Repair interval normalization (preparation only)

Repair `windows.py` using Python 3.9+ and its standard library. Deliver that file;
do not change `public_test.py` or these requirements.

`normalize_windows(windows)` returns the union of nonempty integer half-open
intervals as sorted, disjoint tuples in a new list. Merge touching intervals.
Inputs may be unordered, negative, duplicated, nested or overlapping; endpoints
are arbitrary integers (not booleans) and each pair has `left <= right`.
Ignore empty pairs. Do not mutate the input list or its pairs. Calls must be
independent. Empty input returns `[]`.

Preserve `total_duration(windows)`: sum every `right - left` without merging,
sorting or mutating. Preserve names, signatures and return types.

You may inspect files, edit your solution, run Python and create your own tests
in this directory. No network, packages, browser, other workspaces, native
subagents or model calls are permitted. Run `python3 public_test.py` for visible
examples. This preparation variant is excluded from measured outcomes.
