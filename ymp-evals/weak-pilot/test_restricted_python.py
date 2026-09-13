"""No-inference controls for both imports of submitted pilot Python code."""

import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch


ROOT = Path(__file__).resolve().parent


def load(name):
    spec = importlib.util.spec_from_file_location(name, ROOT / (name + ".py"))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


observer = load("observer")
selector = load("public_select")
restricted = load("restricted_python")


class RestrictedPythonTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="ymp201-c3-tests-")
        self.addCleanup(self.temporary.cleanup)
        self.directory = Path(self.temporary.name).resolve()

    def candidate(self, name, prefix=""):
        target = self.directory / name
        observer.stage("repair", "measured", target)
        correct = observer.fixture("repair", "measured") / "private/windows.py"
        # Prefix has no future imports; the checked-in reference has none either.
        (target / "windows.py").write_text(prefix + correct.read_text())
        return target

    def probe_source(self):
        paths = {}
        for label in ("private", "peer", "native-state", "credentials"):
            parent = self.directory / label
            parent.mkdir()
            target = parent / "artificial-canary.txt"
            target.write_text("ARTIFICIAL-PRIVATE-CANARY-" + label)
            paths[label] = str(target)
        # No test reads a real private/native-state/credential file.
        return (
            "import os as _os, sys as _sys\n"
            f"for _label, _path in {paths!r}.items():\n"
            "    try:\n"
            "        with open(_path) as _stream: _leak = _stream.read()\n"
            "    except PermissionError:\n"
            "        print('DENIED:' + _label, file=_sys.stderr)\n"
            "    else:\n"
            "        raise RuntimeError('CANARY-READ:' + _leak)\n"
            "    try:\n"
            "        _os.listdir(_os.path.dirname(_path))\n"
            "    except PermissionError:\n"
            "        print('DENIED-PARENT:' + _label, file=_sys.stderr)\n"
            "    else:\n"
            "        raise RuntimeError('CANARY-PARENT-ENUMERATED:' + _label)\n"
            "    try:\n"
            "        _os.listdir(_os.path.dirname(_os.path.dirname(_path)))\n"
            "    except PermissionError:\n"
            "        print('DENIED-COMMON-PARENT:' + _label, file=_sys.stderr)\n"
            "    else:\n"
            "        raise RuntimeError('CANARY-COMMON-PARENT-ENUMERATED:' + _label)\n"
            "    _link = _os.path.join(_os.getcwd(), 'link-' + _label)\n"
            "    _os.symlink(_path, _link)\n"
            "    try:\n"
            "        with open(_link) as _stream: _leak = _stream.read()\n"
            "    except PermissionError:\n"
            "        print('DENIED-SYMLINK:' + _label, file=_sys.stderr)\n"
            "    else:\n"
            "        raise RuntimeError('CANARY-SYMLINK-READ:' + _leak)\n"
            "assert 'YMP201_ARTIFICIAL_SECRET' not in _os.environ\n"
        )

    def test_public_selection_denies_private_peer_state_and_parent_reads(self):
        canary = self.candidate("canary", self.probe_source())
        correct = self.candidate("correct")
        with patch.dict(os.environ, {"YMP201_ARTIFICIAL_SECRET": "synthetic-never-copy"}):
            result = selector.select("repair", [canary, correct], correct / "public_test.py")
        self.assertTrue(result["measurement_valid"])
        self.assertEqual(result["selected_participant_ordinal"], 1)
        self.assertTrue(all(row["public_passed"] for row in result["observations"]))
        self.assertNotIn("ARTIFICIAL-PRIVATE-CANARY", json.dumps(result))
        for label in ("private", "peer", "native-state", "credentials"):
            self.assertIn("DENIED:" + label, result["observations"][0]["stderr"])
            self.assertIn("DENIED-PARENT:" + label, result["observations"][0]["stderr"])
            self.assertIn("DENIED-COMMON-PARENT:" + label, result["observations"][0]["stderr"])
            self.assertIn("DENIED-SYMLINK:" + label, result["observations"][0]["stderr"])

    def test_scoring_denies_the_same_reads_and_keeps_expected_outside_probe(self):
        candidate = self.candidate("canary", self.probe_source())
        original_run = observer.restricted_python.run
        captured = []

        def observe_run(script, files, **options):
            response = original_run(script, files, **options)
            captured.append((script, files, options, response))
            return response

        with patch.object(observer.restricted_python, "run", side_effect=observe_run):
            with patch.dict(os.environ, {"YMP201_ARTIFICIAL_SECRET": "synthetic-never-copy"}):
                checks = observer.check_artifacts("repair", "measured", candidate)
        self.assertTrue(all(check["passed"] for check in checks))
        self.assertEqual(len(captured), 1)
        script, files, options, response = captured[0]
        self.assertEqual(set(files), {"windows.py"})
        self.assertEqual(script, observer.repair_probe.PROBE_SOURCE.encode())
        self.assertTrue(all(set(call) == {"function", "args"}
                            for call in json.loads(options["input_text"])))
        self.assertNotIn("ARTIFICIAL-PRIVATE-CANARY", response.stdout + response.stderr)
        for label in ("private", "peer", "native-state", "credentials"):
            self.assertIn("DENIED:" + label, response.stderr)
            self.assertIn("DENIED-PARENT:" + label, response.stderr)
            self.assertIn("DENIED-COMMON-PARENT:" + label, response.stderr)
            self.assertIn("DENIED-SYMLINK:" + label, response.stderr)

    def test_unrestricted_before_control_demonstrates_the_canary_can_be_read(self):
        candidate = self.candidate("canary", self.probe_source())
        # Only this deliberately unrestricted negative control imports the
        # synthetic candidate directly. It is not a consumer execution route.
        completed = subprocess.run([sys.executable, "-B", "public_test.py"], cwd=candidate,
                                   text=True, capture_output=True, timeout=5)
        self.assertNotEqual(completed.returncode, 0)
        self.assertIn("CANARY-READ:ARTIFICIAL-PRIVATE-CANARY-private", completed.stderr)

    def test_correct_public_and_private_checks_pass_for_both_variants(self):
        for variant in ("preparation", "measured"):
            reference = observer.fixture("repair", variant) / "private/windows.py"
            visible = observer.fixture("repair", variant) / "visible/public_test.py"
            response = restricted.run(visible.read_bytes(), {"windows.py": reference.read_bytes()})
            self.assertEqual(response.returncode, 0, response.stderr)
            self.assertTrue(all(row["passed"] for row in observer.repair_checks(variant, reference)))

    def test_direct_probe_cli_also_denies_canary_reads(self):
        candidate = self.candidate("canary", self.probe_source())
        cases = observer.read_json(observer.fixture("repair", "measured") / "private/cases.json")
        response = subprocess.run([sys.executable, "-B", str(ROOT / "repair_probe.py"),
                                   str(candidate / "windows.py")], input=json.dumps(cases),
                                  capture_output=True, text=True, timeout=5)
        self.assertEqual(response.returncode, 0, response.stderr)
        self.assertEqual(len(json.loads(response.stdout)), len(cases))
        self.assertIn("DENIED:private", response.stderr)
        self.assertNotIn("ARTIFICIAL-PRIVATE-CANARY", response.stdout + response.stderr)

    def test_candidate_cannot_fork_or_access_network(self):
        response = restricted.run(b"""import os, socket
try:
    os.fork()
except PermissionError:
    print('fork-denied')
else:
    raise RuntimeError('fork was permitted')
try:
    with socket.socket() as network:
        network.bind(('127.0.0.1', 0))
except PermissionError:
    print('network-denied')
else:
    raise RuntimeError('network was permitted')
""", {})
        self.assertEqual(response.returncode, 0, response.stderr)
        self.assertEqual(response.stdout, "fork-denied\nnetwork-denied\n")

    def test_timeout_terminates_process_and_is_a_valid_negative_outcome(self):
        with self.assertRaises(restricted.RestrictedExecutionError) as caught:
            restricted.run(b"while True: pass\n", {}, timeout=0.25)
        self.assertEqual(caught.exception.kind, "timeout")
        self.assertTrue(caught.exception.process_terminated)
        candidate = self.candidate("forever", "while True: pass\n")
        correct = self.candidate("correct")
        original_run = selector.restricted_python.run

        def short_run(script, files, **options):
            return original_run(script, files, timeout=0.25)

        with patch.object(selector.restricted_python, "run", side_effect=short_run):
            result = selector.select("repair", [candidate, correct], correct / "public_test.py")
        self.assertTrue(result["measurement_valid"])
        self.assertEqual(result["observations"][0]["execution_status"], "timeout")
        self.assertTrue(result["observations"][0]["process_terminated"])
        self.assertTrue(result["observations"][1]["public_passed"])
        original_probe = observer.restricted_python.run

        def short_probe(script, files, **options):
            options["timeout"] = 0.25
            return original_probe(script, files, **options)

        with patch.object(observer.restricted_python, "run", side_effect=short_probe):
            checks = observer.check_artifacts("repair", "measured", candidate)
        self.assertFalse(checks[0]["passed"])
        self.assertEqual(checks[0]["candidate_error"], "timeout")
        self.assertTrue(checks[0]["process_terminated"])

    def test_failed_public_assertion_is_a_valid_negative_check(self):
        first = self.candidate("first")
        second = self.candidate("second")
        (first / "windows.py").write_text("raise AssertionError('incorrect candidate')\n")
        result = selector.select("repair", [first, second], second / "public_test.py")
        self.assertTrue(result["measurement_valid"])
        self.assertEqual(result["observations"][0]["execution_status"], "completed")
        self.assertFalse(result["observations"][0]["public_passed"])
        self.assertEqual(result["selected_participant_ordinal"], 2)

    def test_unsupported_platform_and_sandbox_refusal_do_not_execute_candidate(self):
        with patch.object(restricted.sys, "platform", "linux"):
            with self.assertRaises(restricted.RestrictedExecutionError) as caught:
                restricted.run(b"raise RuntimeError('must not execute')", {})
        self.assertEqual(caught.exception.kind, "unsupported_platform")
        with patch.object(restricted, "SANDBOX", Path("/usr/bin/false")):
            with self.assertRaises(restricted.RestrictedExecutionError) as caught:
                restricted.run(b"raise RuntimeError('must not execute')", {})
        self.assertEqual(caught.exception.kind, "sandbox_refused")

    def test_selector_reports_boundary_failure_separately_from_public_failure(self):
        first = self.candidate("first")
        second = self.candidate("second")
        with patch.object(selector.restricted_python, "SANDBOX", Path("/usr/bin/false")):
            result = selector.select("repair", [first, second], second / "public_test.py")
        self.assertFalse(result["measurement_valid"])
        self.assertTrue(all(row["execution_status"] == "sandbox_refused" for row in result["observations"]))
        self.assertTrue(all("exit_code" not in row for row in result["observations"]))


if __name__ == "__main__":
    unittest.main()
