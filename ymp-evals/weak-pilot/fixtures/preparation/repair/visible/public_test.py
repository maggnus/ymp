"""Visible examples, deliberately incomplete."""

from windows import normalize_windows, total_duration

assert normalize_windows([(0, 2), (4, 6)]) == [(0, 2), (4, 6)]
assert normalize_windows([(0, 2), (2, 4)]) == [(0, 4)]
assert total_duration([(0, 3), (1, 5)]) == 7
print("public examples passed")
