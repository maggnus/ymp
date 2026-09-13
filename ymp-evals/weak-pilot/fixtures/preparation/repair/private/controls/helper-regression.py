"""Synthetic positive control; never distribute to experimental participants."""


def total_duration(windows):
    return max((right for left, right in windows), default=0) - min((left for left, right in windows), default=0)


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
