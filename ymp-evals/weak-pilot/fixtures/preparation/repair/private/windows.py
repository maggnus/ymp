"""Synthetic positive control; never distribute to experimental participants."""


def total_duration(windows):
    return sum(right - left for left, right in windows)


def normalize_windows(windows):
    result = []
    for left, right in sorted(windows):
        if left == right:
            continue
        if result and left <= result[-1][1]:
            result[-1] = (result[-1][0], max(result[-1][1], right))
        else:
            result.append((left, right))
    return result
