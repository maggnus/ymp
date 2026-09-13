"""Synthetic positive control; never distribute to experimental participants."""


def total_duration(windows):
    return sum(right - left for left, right in windows)


def available_windows(start, end, busy, min_length=1):
    if min_length <= 0:
        raise ValueError("min_length must be positive")
    cursor = start
    result = []
    for left, right in sorted(busy):
        left, right = max(start, left), min(end, right)
        if left >= right:
            continue
        if left - cursor > min_length:
            result.append((cursor, left))
        cursor = max(cursor, right)
    if end - cursor > min_length:
        result.append((cursor, end))
    return result if start < end else []
