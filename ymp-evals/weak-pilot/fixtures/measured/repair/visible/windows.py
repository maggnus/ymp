"""Integer scheduling helpers; endpoints use half-open intervals."""


def total_duration(windows):
    return sum(right - left for left, right in windows)


def available_windows(start, end, busy, min_length=1):
    if min_length <= 0:
        raise ValueError("min_length must be positive")
    if start >= end:
        return []
    result = []
    cursor = start
    for left, right in sorted(busy):
        if left > cursor and left - cursor >= min_length:
            result.append((cursor, left))
        cursor = right
    if end - cursor >= min_length:
        result.append((cursor, end))
    return result
