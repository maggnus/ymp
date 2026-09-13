"""Integer interval helpers."""


def total_duration(windows):
    return sum(right - left for left, right in windows)


def normalize_windows(windows):
    windows.sort()
    result = []
    for left, right in windows:
        if result and left < result[-1][1]:
            result[-1] = (result[-1][0], right)
        else:
            result.append((left, right))
    return result
