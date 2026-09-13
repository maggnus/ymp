"""YMP-201 preparation only: validator discrimination, no model measurements."""

import importlib.util
import json
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1] / "weak-pilot"


def module(name):
    spec = importlib.util.spec_from_file_location(name, ROOT / (name + ".py"))
    loaded = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(loaded)
    return loaded


observer = module("observer")
selector = module("public_select")


class WeakPilotTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="ymp-pilot-tests-")
        self.addCleanup(self.temporary.cleanup)
        self.directory = Path(self.temporary.name).resolve()

    def candidate(self, task, variant, name="work"):
        work = self.directory / name
        observer.stage(task, variant, work)
        for output in observer.outputs(task):
            shutil.copyfile(observer.fixture(task, variant) / "private" / output, work / output)
        return work

    def test_all_four_references_pass_and_are_not_measured_results(self):
        for task in observer.TASKS:
            for variant in observer.VARIANTS:
                with self.subTest(task=task, variant=variant):
                    work = self.candidate(task, variant, task + variant)
                    sealed = self.directory / (task + variant + "-sealed")
                    observer.seal(task, variant, work, sealed, "0" * 32)
                    result = observer.score(sealed)
                    self.assertTrue(result["objective_success"])
                    self.assertIsNone(result["runtime_acceptance"])
                    self.assertIsNone(result["model_quality"])

    def test_every_materialized_wrong_control_fails(self):
        for task in observer.TASKS:
            for variant in observer.VARIANTS:
                private = observer.fixture(task, variant) / "private"
                controls = list((private / "controls").iterdir())
                if task == "repair":
                    controls.append(observer.fixture(task, variant) / "visible/windows.py")
                for index, control in enumerate(controls):
                    with self.subTest(task=task, variant=variant, control=control.name):
                        work = self.candidate(task, variant, f"{task}-{variant}-{index}")
                        for output in observer.outputs(task):
                            shutil.copyfile(control if control.is_file() else control / output, work / output)
                        checks = observer.check_artifacts(task, variant, work)
                        self.assertTrue(any(not item["passed"] for item in checks), control.name)

    def test_plausible_wrong_outputs_pass_public_checks_but_fail_private(self):
        for task, control in [("reconcile", "float-rounding"), ("repair", "strict-minimum.py")]:
            work = self.candidate(task, "measured", task)
            source = observer.fixture(task, "measured") / "private/controls" / control
            for output in observer.outputs(task):
                shutil.copyfile(source if source.is_file() else source / output, work / output)
            public = subprocess.run([sys.executable, "-B", "public_test.py"], cwd=work, capture_output=True)
            self.assertEqual(public.returncode, 0, public.stderr)
            self.assertFalse(all(item["passed"] for item in observer.check_artifacts(task, "measured", work)))

    def test_selection_has_no_hidden_oracle_even_when_later_answer_is_correct(self):
        wrong = self.candidate("repair", "measured", "first")
        correct = self.candidate("repair", "measured", "second")
        shutil.copyfile(observer.fixture("repair", "measured") / "private/controls/strict-minimum.py", wrong / "windows.py")
        selected = selector.select("repair", [wrong, correct], correct / "public_test.py")
        self.assertEqual(selected["selected_participant_ordinal"], 1)
        self.assertEqual(selected["model_invocations"], 0)
        self.assertTrue(all(item["public_passed"] for item in selected["observations"]))
        self.assertFalse(all(item["passed"] for item in observer.check_artifacts("repair", "measured", wrong)))

    def test_export_contains_only_visible_files_and_variants_are_distinct(self):
        for task in observer.TASKS:
            for variant in observer.VARIANTS:
                destination = self.directory / (task + variant)
                observer.stage(task, variant, destination)
                self.assertEqual(sorted(path.name for path in destination.iterdir()), observer.files(task, variant))
                self.assertFalse((destination / "private").exists())
            name = "ledger.csv" if task == "reconcile" else "windows.py"
            self.assertNotEqual((observer.fixture(task, "preparation") / "visible" / name).read_bytes(),
                                (observer.fixture(task, "measured") / "visible" / name).read_bytes())

    def test_input_tampering_and_stale_artifact_are_rejected(self):
        work = self.candidate("reconcile", "measured")
        sealed = self.directory / "sealed"
        observer.seal("reconcile", "measured", work, sealed, "a" * 32)
        (sealed / "totals.csv").write_text("wrong\n")
        with self.assertRaisesRegex(ValueError, "stale"):
            observer.score(sealed)
        (work / "ledger.csv").write_text("changed\n")
        with self.assertRaisesRegex(ValueError, "input"):
            observer.seal("reconcile", "measured", work, self.directory / "second", "b" * 32)

    def test_symlink_escape_and_destination_reuse_are_rejected(self):
        work = self.candidate("repair", "measured")
        with self.assertRaisesRegex(ValueError, "exist"):
            observer.stage("repair", "measured", work)
        (work / "windows.py").unlink()
        (work / "windows.py").symlink_to(observer.fixture("repair", "measured") / "private/windows.py")
        with self.assertRaisesRegex(ValueError, "escapes"):
            observer.seal("repair", "measured", work, self.directory / "sealed", "a" * 32)

    def test_matrix_is_reproducible_randomized_complete_and_unmeasured(self):
        first = observer.matrix(2010914)
        self.assertEqual(first, observer.matrix(2010914))
        self.assertNotEqual(first["attempts"], observer.matrix(2010915)["attempts"])
        self.assertEqual(len(first["attempts"]), 12)
        for task in observer.TASKS:
            attempts = [row for row in first["attempts"] if row["task"] == task]
            self.assertEqual({row["condition"] for row in attempts}, set(observer.CONDITIONS))
            self.assertEqual({row["order_within_task"] for row in attempts}, set(range(1, 7)))
            for row in attempts:
                for field in ["actual_model", "actual_effort", "actual_agent_ids", "usage", "elapsed_seconds", "external_score"]:
                    self.assertIsNone(row[field])
        for group in ["cooperation-2", "cooperation-3"]:
            spec = first["condition_protocol"][group]
            self.assertEqual(spec["producer_limit"] + spec["reserved_independent_reviewer"], spec["participants"])

    def test_score_cli_reports_nonzero_for_validly_sealed_wrong_artifact(self):
        work = self.candidate("reconcile", "measured")
        shutil.copyfile(observer.fixture("reconcile", "measured") / "private/controls/float-rounding/totals.csv", work / "totals.csv")
        sealed = self.directory / "sealed"
        observer.seal("reconcile", "measured", work, sealed, "a" * 32)
        result = subprocess.run([sys.executable, "-B", str(ROOT / "observer.py"), "score", "--submission", str(sealed)],
                                capture_output=True, text=True)
        self.assertEqual(result.returncode, 1, result.stderr)
        self.assertFalse(json.loads(result.stdout)["objective_success"])

    def test_manifest_and_result_templates_leave_actual_observations_null(self):
        result = observer.read_json(ROOT / "result.template.json")
        invocation = observer.read_json(ROOT / "invocation.template.json")
        self.assertTrue(all(value is None for value in invocation.values()))
        self.assertIsNone(result["invocations"])
        self.assertIsNone(result["execution_kind"])
        self.assertTrue(all(value is None for value in result["resources"].values()))


if __name__ == "__main__":
    unittest.main()
