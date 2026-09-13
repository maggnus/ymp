"""Visible examples, deliberately incomplete."""

from windows import available_windows, total_duration

assert available_windows(0, 10, [(2, 4), (6, 8)]) == [(0, 2), (4, 6), (8, 10)]
assert available_windows(0, 10, [(2, 8), (3, 4)]) == [(0, 2), (8, 10)]
assert total_duration([(0, 3), (1, 5)]) == 7
print("public examples passed")
