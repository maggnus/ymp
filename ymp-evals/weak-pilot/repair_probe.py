"""Run submitted Python in a separate process; expected answers stay outside it.

This is process separation, not a security sandbox. Only reviewed synthetic
controls are executed during preparation. Never put this file in solving context.
"""

import copy
import importlib.util
import json
import sys


def main():
    spec = importlib.util.spec_from_file_location("submitted_windows", sys.argv[1])
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    observations = []
    for call in json.load(sys.stdin):
        args = call["args"]
        before = copy.deepcopy(args)
        try:
            value = getattr(module, call["function"])(*args)
            list_of_tuples = (type(value) is list and all(
                type(pair) is tuple and len(pair) == 2 and
                all(type(endpoint) is int for endpoint in pair) for pair in value))
            # Call twice on fresh equivalent data to detect retained state.
            again = getattr(module, call["function"])(*copy.deepcopy(before))
            observations.append({"value": value, "error": None,
                                 "unchanged": args == before, "repeatable": again == value,
                                 "return_type": "intervals" if list_of_tuples else type(value).__name__})
        except Exception as error:
            observations.append({"value": None, "error": type(error).__name__,
                                 "unchanged": args == before, "repeatable": None,
                                 "return_type": None})
    print(json.dumps(observations, allow_nan=False))


if __name__ == "__main__":
    main()
