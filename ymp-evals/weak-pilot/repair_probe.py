"""Run submitted Python within the shared restricted execution boundary.

The trusted observer copies PROBE_SOURCE and candidate bytes into restricted_python;
the process receives only function names/arguments, never expected answers.
The direct CLI uses that same boundary. Never put this file in solving context.
"""

import importlib.util
import json
from pathlib import Path
import sys


PROBE_SOURCE = r'''
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
'''


def main():
    spec = importlib.util.spec_from_file_location("restricted_python", Path(__file__).with_name("restricted_python.py"))
    restricted = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(restricted)
    try:
        # Even a direct probe invocation cannot pass expected answers onward.
        calls = [{"function": call["function"], "args": call["args"]} for call in json.load(sys.stdin)]
        result = restricted.run(PROBE_SOURCE.encode(), {"windows.py": Path(sys.argv[1]).read_bytes()},
                                args=("windows.py",), input_text=json.dumps(calls), timeout=5)
        sys.stdout.write(result.stdout)
        sys.stderr.write(result.stderr)
        return result.returncode
    except restricted.RestrictedExecutionError as error:
        valid = error.kind == "timeout" and error.process_terminated
        print(json.dumps({"candidate_error": error.kind, "measurement_valid": valid,
                          "process_terminated": error.process_terminated}))
        return 1 if valid else 2


if __name__ == "__main__":
    sys.exit(main())
