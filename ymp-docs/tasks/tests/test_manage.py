from __future__ import annotations

import hashlib
import json
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


SCRIPT = Path(__file__).resolve().parents[1] / "manage.py"


def record(
    task_id: str,
    *,
    area: str = "test",
    status: str = "planned",
    dependencies: list[str] | None = None,
    evidence: list[str] | None = None,
    owner: str | None = None,
    title: str | None = None,
    note: str = "Initial record.",
    revision: int = 1,
) -> dict[str, object]:
    history = [
        {
            "revision": index,
            "at": f"2026-09-15T00:{index % 60:02d}:00Z",
            "actor": owner or "test",
            "status": status,
            "note": note,
        }
        for index in range(1, revision + 1)
    ]
    return {
        "schema_version": 1,
        "id": task_id,
        "area": area,
        "title": title or f"Task {task_id}",
        "type": "test",
        "priority": int(task_id.split("-")[1]),
        "status": status,
        "depends_on": dependencies or [],
        "owner": owner,
        "goal": f"Exercise {task_id}.",
        "scope": ["Keep the fixture bounded."],
        "acceptance": ["The stated behavior is observed."],
        "evidence": evidence or [],
        "context": ["Synthetic test data."],
        "revision": revision,
        "history": history,
    }


class ManageCliTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory(prefix="ymp-tasks-test-")
        self.root = Path(self.temporary.name) / "tasks"
        (self.root / "records").mkdir(parents=True)

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def run_cli(
        self, *arguments: str, expected: int | None = 0
    ) -> subprocess.CompletedProcess[str]:
        result = subprocess.run(
            [sys.executable, str(SCRIPT), "--root", str(self.root), *arguments],
            text=True,
            capture_output=True,
            check=False,
        )
        if expected is not None:
            self.assertEqual(
                result.returncode,
                expected,
                msg=f"stdout:\n{result.stdout}\nstderr:\n{result.stderr}",
            )
        return result

    def write_record(self, value: dict[str, object]) -> Path:
        directory = self.root / "records"
        directory.mkdir(parents=True, exist_ok=True)
        path = directory / f"{value['id']}.json"
        path.write_text(json.dumps(value, indent=2) + "\n")
        return path

    def write_input(self, name: str, value: object) -> Path:
        path = Path(self.temporary.name) / name
        path.write_text(json.dumps(value, indent=2) + "\n")
        return path

    def snapshot(self) -> dict[str, str]:
        return {
            str(path.relative_to(self.root)): hashlib.sha256(path.read_bytes()).hexdigest()
            for path in self.root.rglob("*.json")
        }

    def test_full_cli_lifecycle_changes_only_one_record(self) -> None:
        baseline = self.write_record(
            record("W1-0001", status="done", evidence=["baseline evidence"])
        )
        baseline_hash = hashlib.sha256(baseline.read_bytes()).hexdigest()
        draft = self.write_input(
            "draft.json",
            {
                "area": "tooling",
                "title": "Create a lifecycle fixture",
                "type": "tooling",
                "priority": 2,
                "status": "planned",
                "depends_on": ["W1-0001"],
                "goal": "Exercise every writer command.",
                "scope": ["Use only the temporary task root."],
                "acceptance": ["The task reaches done with evidence."],
            },
        )
        created = json.loads(self.run_cli("create", "--wave", "W1", "--file", str(draft)).stdout)
        self.assertEqual(created["created"], "W1-0002")

        shown = self.run_cli("show", "W1-0002").stdout
        self.assertIn("history excluded", shown)
        self.assertNotIn("Task created.", shown)
        claimed = json.loads(
            self.run_cli(
                "claim", "W1-0002", "--owner", "worker-a", "--expect-revision", "1"
            ).stdout
        )
        self.assertEqual(claimed["revision"], 2)
        patch = self.write_input("patch.json", {"title": "Completed lifecycle fixture"})
        updated = json.loads(
            self.run_cli(
                "update",
                "W1-0002",
                "--file",
                str(patch),
                "--expect-revision",
                "2",
                "--note",
                "Refined the title.",
            ).stdout
        )
        self.assertEqual(updated["revision"], 3)
        finished = json.loads(
            self.run_cli(
                "status",
                "W1-0002",
                "done",
                "--expect-revision",
                "3",
                "--note",
                "Verified the lifecycle.",
                "--actor",
                "reviewer",
                "--evidence",
                "temporary CLI transcript",
            ).stdout
        )
        self.assertEqual(finished["revision"], 4)
        final = json.loads((self.root / "records/W1-0002.json").read_text())
        self.assertEqual(final["status"], "done")
        self.assertEqual(final["owner"], "worker-a")
        self.assertEqual([entry["revision"] for entry in final["history"]], [1, 2, 3, 4])
        self.assertEqual(final["history"][-1]["actor"], "reviewer")
        self.assertEqual(hashlib.sha256(baseline.read_bytes()).hexdigest(), baseline_hash)
        self.run_cli("check")

    def test_stale_and_invalid_updates_leave_record_unchanged(self) -> None:
        self.write_record(record("W1-0001"))
        patch = self.write_input("patch.json", {"goal": "A changed goal."})
        first = self.run_cli(
            "update",
            "W1-0001",
            "--file",
            str(patch),
            "--expect-revision",
            "1",
            "--note",
            "Applied once.",
        )
        self.assertIn('"revision": 2', first.stdout)
        before = self.snapshot()
        stale = self.run_cli(
            "update",
            "W1-0001",
            "--file",
            str(patch),
            "--expect-revision",
            "1",
            "--note",
            "Must not apply.",
            expected=4,
        )
        self.assertIn("stale revision", stale.stderr)
        immutable = self.write_input("immutable.json", {"area": "other"})
        self.run_cli(
            "update",
            "W1-0001",
            "--file",
            str(immutable),
            "--expect-revision",
            "2",
            "--note",
            "Must not apply.",
            expected=3,
        )
        self.assertEqual(self.snapshot(), before)

    def test_dependencies_cycles_and_started_states_are_validated(self) -> None:
        first_path = self.write_record(record("W1-0001"))
        second_path = self.write_record(record("W1-0002", dependencies=["W1-0001"]))
        self.run_cli("check")

        original_first = first_path.read_bytes()
        original_second = second_path.read_bytes()
        bad = json.loads(second_path.read_text())
        bad["depends_on"] = ["W1-9999"]
        second_path.write_text(json.dumps(bad))
        self.assertIn("unknown dependency", self.run_cli("check", expected=3).stderr)

        second_path.write_bytes(original_second)
        cycle = json.loads(first_path.read_text())
        cycle["depends_on"] = ["W1-0002"]
        first_path.write_text(json.dumps(cycle))
        self.assertIn("dependency cycle", self.run_cli("check", expected=3).stderr)

        first_path.write_bytes(original_first)
        started = json.loads(second_path.read_text())
        started["status"] = "in_progress"
        started["history"][-1]["status"] = "in_progress"
        second_path.write_text(json.dumps(started))
        self.assertIn("unfinished dependencies", self.run_cli("check", expected=3).stderr)

        done_first = record("W1-0001", status="done", evidence=["checked"])
        first_path.write_text(json.dumps(done_first))
        self.run_cli("check")

    def test_premature_claim_completion_and_missing_evidence_are_rejected(self) -> None:
        self.write_record(record("W1-0001"))
        blocked_path = self.write_record(record("W1-0002", dependencies=["W1-0001"]))
        before = self.snapshot()
        claim = self.run_cli(
            "claim", "W1-0002", "--owner", "worker", "--expect-revision", "1", expected=4
        )
        self.assertIn("not ready", claim.stderr)
        done = self.run_cli(
            "status",
            "W1-0002",
            "done",
            "--expect-revision",
            "1",
            "--note",
            "Premature.",
            "--evidence",
            "not sufficient",
            expected=3,
        )
        self.assertIn("unfinished dependencies", done.stderr)
        self.assertEqual(self.snapshot(), before)

        blocked = json.loads(blocked_path.read_text())
        blocked["depends_on"] = []
        blocked_path.write_text(json.dumps(blocked))
        no_evidence = self.run_cli(
            "status",
            "W1-0002",
            "done",
            "--expect-revision",
            "1",
            "--note",
            "No evidence.",
            expected=3,
        )
        self.assertIn("only from in_progress", no_evidence.stderr)
        self.run_cli(
            "claim", "W1-0002", "--owner", "worker", "--expect-revision", "1"
        )
        before_done = blocked_path.read_bytes()
        no_evidence_after_start = self.run_cli(
            "status",
            "W1-0002",
            "done",
            "--expect-revision",
            "2",
            "--note",
            "Still no evidence.",
            expected=3,
        )
        self.assertIn("without evidence", no_evidence_after_start.stderr)
        self.assertEqual(blocked_path.read_bytes(), before_done)

        new_path = self.write_record(record("W1-0003", status="new"))
        before_new = new_path.read_bytes()
        start_new = self.run_cli(
            "status",
            "W1-0003",
            "in_progress",
            "--expect-revision",
            "1",
            "--note",
            "Bypass scheduling.",
            expected=3,
        )
        self.assertIn("use claim", start_new.stderr)
        self.assertEqual(new_path.read_bytes(), before_new)

    def test_rejected_dependency_never_satisfies_readiness(self) -> None:
        self.write_record(record("W1-0001", status="rejected"))
        self.write_record(record("W1-0002", dependencies=["W1-0001"]))
        result = json.loads(self.run_cli("next", "--json").stdout)
        self.assertEqual(result["page"]["total"], 0)
        waiting = json.loads(
            self.run_cli("list", "--readiness", "waiting", "--json").stdout
        )
        self.assertEqual([task["id"] for task in waiting["tasks"]], ["W1-0002"])

    def test_malformed_history_and_revision_are_rejected_then_fixed(self) -> None:
        path = self.write_record(record("W1-0001", revision=2))
        self.run_cli("check")
        value = json.loads(path.read_text())
        value["history"][1]["revision"] = 7
        path.write_text(json.dumps(value))
        self.assertIn("contiguous", self.run_cli("check", expected=3).stderr)
        value["history"][1]["revision"] = 2
        value["revision"] = 3
        path.write_text(json.dumps(value))
        self.assertIn("length must equal revision", self.run_cli("check", expected=3).stderr)
        path.write_text(json.dumps(record("W1-0001", revision=2)))
        self.run_cli("check")

    def test_schema_and_field_types_are_strictly_validated(self) -> None:
        path = self.write_record(record("W1-0001"))
        original = path.read_bytes()
        cases = (
            ("schema_version", True, "schema_version"),
            ("priority", "1", "priority"),
            ("acceptance", "not-an-array", "acceptance"),
            ("type", "UPPER CASE", "safe slug"),
        )
        for field, invalid, message in cases:
            with self.subTest(field=field):
                value = json.loads(original)
                value[field] = invalid
                path.write_text(json.dumps(value))
                result = self.run_cli("check", expected=3)
                self.assertIn(message, result.stderr)
        path.write_bytes(original)
        self.run_cli("check")

    def test_duplicate_ids_path_mismatch_and_unsafe_names_are_rejected(self) -> None:
        first = record("W1-0001", area="one")
        self.write_record(first)
        duplicate = {key: value for key, value in first.items() if key not in {"revision", "history"}}
        duplicate["area"] = "two"
        duplicate_path = self.write_input("duplicate.json", duplicate)
        before = self.snapshot()
        self.assertIn(
            "already exists",
            self.run_cli("create", "--file", str(duplicate_path), expected=4).stderr,
        )
        self.assertEqual(self.snapshot(), before)

        mismatch = record("W1-0002", area="one")
        mismatch_path = self.write_record(mismatch)
        mismatch["id"] = "W1-0003"
        mismatch_path.write_text(json.dumps(mismatch))
        self.assertIn("does not match its stable path", self.run_cli("check", expected=3).stderr)
        mismatch_path.unlink()

        draft = self.write_input(
            "traversal.json",
            {
                "area": "../outside",
                "title": "Unsafe",
                "type": "test",
                "priority": 1,
                "status": "new",
                "depends_on": [],
                "goal": "Must be rejected.",
                "scope": ["No traversal."],
                "acceptance": ["Rejected."],
            },
        )
        self.assertIn(
            "safe slug", self.run_cli("create", "--wave", "W1", "--file", str(draft), expected=3).stderr
        )
        self.assertFalse((self.root / "outside").exists())

    @unittest.skipUnless(hasattr(os, "symlink"), "symlinks are unavailable")
    def test_symlinked_record_directory_is_rejected(self) -> None:
        external = Path(self.temporary.name) / "external"
        external.mkdir()
        (self.root / "records/link").symlink_to(external, target_is_directory=True)
        self.assertIn("symlinked record path", self.run_cli("check", expected=3).stderr)

    def test_two_process_claim_has_exactly_one_winner(self) -> None:
        self.write_record(record("W1-0001"))
        command = [
            sys.executable,
            str(SCRIPT),
            "--root",
            str(self.root),
            "claim",
            "W1-0001",
            "--expect-revision",
            "1",
        ]
        first = subprocess.Popen(
            [*command, "--owner", "worker-a"], text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE
        )
        second = subprocess.Popen(
            [*command, "--owner", "worker-b"], text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE
        )
        first_output = first.communicate(timeout=10)
        second_output = second.communicate(timeout=10)
        codes = [first.returncode, second.returncode]
        self.assertEqual(sorted(codes), [0, 4], msg=str((first_output, second_output)))
        final = json.loads((self.root / "records/W1-0001.json").read_text())
        self.assertIn(final["owner"], {"worker-a", "worker-b"})
        self.assertEqual(final["revision"], 2)
        self.assertEqual(len(final["history"]), 2)
        self.assertTrue((self.root / ".manage.lock").exists())
        repeated = self.run_cli(
            "claim",
            "W1-0001",
            "--owner",
            str(final["owner"]),
            "--expect-revision",
            "2",
            expected=4,
        )
        self.assertIn("cannot be claimed", repeated.stderr)

    def test_large_deep_graph_validates_and_paginates_without_overlap(self) -> None:
        directory = self.root / "records"
        for number in range(1, 1501):
            task_id = f"W1-{number:04d}"
            value = record(
                task_id,
                area="scale",
                dependencies=[f"W1-{number - 1:04d}"] if number > 1 else [],
            )
            value["priority"] = 1
            (directory / f"{task_id}.json").write_text(json.dumps(value))
        checked = self.run_cli("check")
        self.assertIn("1500 tasks", checked.stdout)

        default_page = json.loads(self.run_cli("list", "--json").stdout)
        self.assertEqual(default_page["page"]["returned"], 20)
        all_ids: list[str] = []
        for offset in range(0, 1500, 100):
            result = json.loads(
                self.run_cli(
                    "list", "--limit", "100", "--offset", str(offset), "--json"
                ).stdout
            )
            page_ids = [task["id"] for task in result["tasks"]]
            self.assertEqual(len(page_ids), 100)
            self.assertFalse(set(all_ids) & set(page_ids))
            all_ids.extend(page_ids)
        self.assertEqual(all_ids, [f"W1-{number:04d}" for number in range(1, 1501)])
        self.assertNotIn("history", default_page["tasks"][0])
        self.assertNotIn("goal", default_page["tasks"][0])

        dependencies = json.loads(
            self.run_cli("deps", "W1-1500", "--limit", "100", "--json").stdout
        )
        self.assertEqual(dependencies["page"]["total"], 1499)
        self.assertEqual(dependencies["dependencies"][0]["id"], "W1-1499")
        self.assertEqual(dependencies["dependencies"][-1]["id"], "W1-1400")

    def test_long_text_is_explicitly_bounded(self) -> None:
        long_text = "x" * 100_000
        value = record("W1-0001", title=long_text, note=long_text, revision=25)
        value["goal"] = long_text
        value["scope"] = [long_text]
        self.write_record(value)

        listed = self.run_cli("list").stdout
        self.assertLess(len(listed), 1_000)
        self.assertIn("truncated", listed)

        shown = self.run_cli("show", "W1-0001").stdout
        self.assertLess(len(shown), 13_000)
        self.assertIn("Output truncated", shown)
        shown_json = json.loads(self.run_cli("show", "W1-0001", "--json").stdout)
        self.assertEqual(shown_json["page"]["returned_characters"], 12_000)
        self.assertIsNotNone(shown_json["page"]["next_offset"])

        history = self.run_cli("history", "W1-0001").stdout
        self.assertLess(len(history), 23_000)
        self.assertIn("truncated", history)
        self.assertIn("More history", history)
        rendered = self.run_cli("render").stdout
        self.assertLess(len(rendered), 2_000)
        self.assertIn("truncated", rendered)

    def test_render_includes_the_latest_active_update_from_records(self) -> None:
        self.write_record(record("W1-0001", status="in_progress", owner="codex", note="Snapshot checks passed; root binding remains.", revision=2))
        self.write_record(record("W1-0002", note="Planned detail stays out of current work."))
        rendered = self.run_cli("render").stdout
        self.assertIn("| W1 | 0 | 2 | 1 |", rendered)
        self.assertIn("## Current work", rendered)
        self.assertIn("### W1-0001 — `in_progress`", rendered)
        self.assertIn("Revision 2; updated", rendered)
        self.assertIn("Snapshot checks passed; root binding remains.", rendered)
        self.assertNotIn("Planned detail stays out of current work.", rendered)
        page = self.run_cli("render", "--offset", "1", "--limit", "1").stdout
        self.assertNotIn("## Current work", page)

    def test_read_commands_do_not_modify_the_task_root(self) -> None:
        self.write_record(record("W1-0001"))
        before = {
            str(path.relative_to(self.root)): (path.stat().st_mtime_ns, path.read_bytes())
            for path in self.root.rglob("*")
            if path.is_file()
        }
        for command in (
            ("next",),
            ("list",),
            ("show", "W1-0001"),
            ("deps", "W1-0001"),
            ("history", "W1-0001"),
            ("summary",),
            ("render",),
            ("progress",),
            ("check",),
        ):
            self.run_cli(*command)
        after = {
            str(path.relative_to(self.root)): (path.stat().st_mtime_ns, path.read_bytes())
            for path in self.root.rglob("*")
            if path.is_file()
        }
        self.assertEqual(after, before)
        self.assertFalse((self.root / ".manage.lock").exists())

    def test_filters_defaults_and_summary_json(self) -> None:
        self.write_record(record("W1-0001", area="alpha"))
        self.write_record(record("W1-0002", area="alpha", status="done", evidence=["ok"]))
        self.write_record(record("W1-0003", area="beta", status="new"))
        default = json.loads(self.run_cli("list", "--json").stdout)
        self.assertEqual([item["id"] for item in default["tasks"]], ["W1-0001", "W1-0003"])
        terminal = json.loads(
            self.run_cli("list", "--readiness", "terminal", "--all", "--json").stdout
        )
        self.assertEqual([item["id"] for item in terminal["tasks"]], ["W1-0002"])
        filtered = json.loads(
            self.run_cli("list", "--area", "beta", "--status", "new", "--json").stdout
        )
        self.assertEqual([item["id"] for item in filtered["tasks"]], ["W1-0003"])
        summary = json.loads(self.run_cli("summary", "--json").stdout)
        self.assertEqual(summary["tasks"], 3)
        self.assertEqual(summary["page"]["total"], 4)

    def test_allocation_supports_ids_above_9999(self) -> None:
        self.write_record(record("W1-10000"))
        draft = self.write_input(
            "draft.json",
            {
                "area": "test",
                "title": "Allocated above four digits",
                "type": "test",
                "priority": 1,
                "depends_on": [],
                "goal": "Verify allocation.",
                "scope": ["Allocate the next ID in W1."],
                "acceptance": ["W1-10001 is created."],
            },
        )
        result = json.loads(self.run_cli("create", "--wave", "W1", "--file", str(draft)).stdout)
        self.assertEqual(result["created"], "W1-10001")
        self.assertTrue((self.root / "records/W1-10001.json").exists())

    def test_wave_allocation_is_explicit_local_and_atomic(self) -> None:
        draft = record("W1-0001", status="new")
        for field in ("id", "revision", "history"):
            draft.pop(field)
        path = self.write_input("draft.json", draft)
        missing = self.run_cli("create", "--file", str(path), expected=3)
        self.assertIn("--wave", missing.stderr)
        self.assertEqual(self.snapshot(), {})
        for wave, expected_id in (("W2", "W2-0001"), ("W1", "W1-0001"), ("W2", "W2-0002")):
            created = json.loads(self.run_cli("create", "--wave", wave, "--file", str(path)).stdout)
            self.assertEqual(created["created"], expected_id)
            self.assertEqual(created["path"], f"records/{expected_id}.json")
        before = self.snapshot()
        draft["id"] = "W3-0001"
        path.write_text(json.dumps(draft))
        mismatch = self.run_cli("create", "--wave", "W2", "--file", str(path), expected=3)
        self.assertIn("does not match", mismatch.stderr)
        self.assertEqual(self.snapshot(), before)
        created = json.loads(self.run_cli("create", "--file", str(path)).stdout)
        self.assertEqual(created["created"], "W3-0001")
        self.run_cli("check")

    def test_wave_filters_order_dependencies_and_rendered_paths(self) -> None:
        for task_id in ("W10-0001", "W2-0002", "W1-0001", "W2-0001"):
            value = record(task_id)
            value["priority"] = 1
            self.write_record(value)
        ordered = json.loads(self.run_cli("list", "--json").stdout)
        self.assertEqual(
            [item["id"] for item in ordered["tasks"]],
            ["W1-0001", "W2-0001", "W2-0002", "W10-0001"],
        )
        self.write_record(record("W2-0001", dependencies=["W1-0001"]))
        waiting = json.loads(self.run_cli("list", "--wave", "W2", "--readiness", "waiting", "--json").stdout)
        self.assertEqual([item["id"] for item in waiting["tasks"]], ["W2-0001"])
        ready = json.loads(self.run_cli("next", "--wave", "W2", "--json").stdout)
        self.assertEqual([item["id"] for item in ready["tasks"]], ["W2-0002"])
        deps = json.loads(self.run_cli("deps", "W2-0001", "--json").stdout)
        self.assertEqual(deps["dependencies"][0]["id"], "W1-0001")
        self.write_record(record("W1-0001", status="done", evidence=["checked"]))
        self.assertEqual(
            json.loads(self.run_cli("next", "--wave", "W2", "--json").stdout)["tasks"][0]["id"],
            "W2-0001",
        )
        selected = json.loads(self.run_cli("list", "--wave", "W2", "--wave", "W10", "--json").stdout)
        self.assertEqual(len(selected["tasks"]), 3)
        summary = json.loads(self.run_cli("summary", "--json").stdout)
        self.assertEqual([g["name"] for g in summary["groups"] if g["group"] == "wave"], ["W1", "W2", "W10"])
        rendered = self.run_cli("render").stdout
        self.assertIn("[W2-0001](records/W2-0001.json)", rendered)
        self.assertNotIn("records/test/", rendered)

    def test_noncanonical_ids_and_wave_arguments_are_rejected_without_writes(self) -> None:
        self.write_record(record("W1-0001"))
        before = self.snapshot()
        invalid_ids = (
            "DEV-0001", "W0-0001", "W01-0001", "W1-0000", "W1-001",
            "W1-00001", "w1-0001", "../W1-0001", "W1-0001.json",
        )
        for task_id in invalid_ids:
            with self.subTest(task_id=task_id):
                result = self.run_cli("show", task_id, expected=3)
                self.assertNotIn("Traceback", result.stderr)
                draft = record("W1-0002")
                draft["id"] = task_id
                draft.pop("revision")
                draft.pop("history")
                path = self.write_input("invalid.json", draft)
                self.run_cli("create", "--file", str(path), expected=3)
        for wave in ("W0", "W01", "w1", "1", "../W1"):
            with self.subTest(wave=wave):
                self.run_cli("list", "--wave", wave, expected=2)
        self.assertEqual(self.snapshot(), before)

    def test_flat_store_preserves_placeholder_and_rejects_nested_records(self) -> None:
        placeholder = self.root / "records/.gitkeep"
        placeholder.touch()
        self.run_cli("check")
        self.write_record(record("W1-0001"))
        self.run_cli("check")
        self.assertEqual(placeholder.read_bytes(), b"")
        nested = self.root / "records/test"
        nested.mkdir()
        (nested / "W2-0001.json").write_text(json.dumps(record("W2-0001")))
        self.assertIn("nested record paths", self.run_cli("check", expected=3).stderr)

    def test_concurrent_creators_allocate_distinct_ids_in_one_wave(self) -> None:
        self.write_record(record("W1-10000"))
        draft = record("W2-0001", status="new")
        for field in ("id", "revision", "history"):
            draft.pop(field)
        path = self.write_input("draft.json", draft)
        command = [
            sys.executable, str(SCRIPT), "--root", str(self.root),
            "create", "--wave", "W2", "--file", str(path),
        ]
        processes = [subprocess.Popen(command, text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE) for _ in range(2)]
        outputs = [process.communicate(timeout=10) for process in processes]
        self.assertEqual([p.returncode for p in processes], [0, 0], outputs)
        self.assertEqual({json.loads(out)["created"] for out, _ in outputs}, {"W2-0001", "W2-0002"})
        self.assertEqual(len(self.snapshot()), 3)
        self.run_cli("check")

    def test_patch_that_creates_cycle_is_rejected_atomically(self) -> None:
        self.write_record(record("W1-0001"))
        self.write_record(record("W1-0002", dependencies=["W1-0001"]))
        patch = self.write_input("cycle.json", {"depends_on": ["W1-0002"]})
        before = self.snapshot()
        result = self.run_cli(
            "update",
            "W1-0001",
            "--file",
            str(patch),
            "--expect-revision",
            "1",
            "--note",
            "Attempt a cycle.",
            expected=3,
        )
        self.assertIn("dependency cycle", result.stderr)
        self.assertEqual(self.snapshot(), before)

    def test_limit_caps_and_unknown_tasks_return_readable_errors(self) -> None:
        self.write_record(record("W1-0001"))
        too_large = self.run_cli("list", "--limit", "101", expected=3)
        self.assertIn("between 1 and 100", too_large.stderr)
        unknown = self.run_cli("show", "W1-9999", expected=3)
        self.assertIn("unknown task W1-9999", unknown.stderr)
        self.assertNotIn("Traceback", too_large.stderr + unknown.stderr)

    def test_progress_tables_use_marks_hong_kong_time_and_evidence_commits(self) -> None:
        self.write_record(
            record(
                "W1-0001",
                status="done",
                owner="codex",
                evidence=["Commit adc78d190c023192d158259756fc86f31ca4fd59 in main; checked on 20260915."],
                revision=2,
            )
        )
        self.write_record(record("W1-0002", status="in_progress", owner="codex", dependencies=["W1-0001"]))
        self.write_record(record("W1-0003", dependencies=["W1-0002"], title="Blocked | by two"))
        self.write_record(record("W1-0004"))
        self.write_record(record("W1-0005", status="paused"))
        self.write_record(record("W1-0006", status="rejected"))
        self.write_record(record("W1-0007", status="owner_question"))
        self.write_record(record("W2-0001", status="new", dependencies=["W1-0006"]))
        first = self.run_cli("progress").stdout
        self.assertEqual(first, self.run_cli("progress").stdout)
        self.assertFalse((self.root / "PROGRESS.md").exists())
        self.assertIn("Last record change: 2026-09-15 08:02.", first)
        self.assertIn("| [W1](#wave-w1) | 1 | 1 | 1 | 3 | 1 | 7 | 14% |", first)
        self.assertIn("| [W2](#wave-w2) | 0 | 0 | 1 | 0 | 0 | 1 | 0% |", first)
        self.assertIn("### W1-0002 — [~] in_progress", first)
        self.assertIn(
            "| [x] | [W1-0001](records/W1-0001.json) | Task W1-0001 | test | done | — | codex "
            "| 2026-09-15 08:02 | 2 | commit `adc78d1` |",
            first,
        )
        self.assertIn(
            "| [~] | [W1-0002](records/W1-0002.json) | Task W1-0002 | test | in_progress | W1-0001 ✓ | codex "
            "| 2026-09-15 08:01 | 1 | — |",
            first,
        )
        self.assertIn("| [=] | [W1-0003](records/W1-0003.json) | Blocked \\| by two | test | planned (blocked) | W1-0002 | — |", first)
        self.assertIn("| [ ] | [W1-0004](records/W1-0004.json) | Task W1-0004 | test | planned | — | — |", first)
        self.assertIn("| [=] | [W1-0005](records/W1-0005.json) | Task W1-0005 | test | paused |", first)
        self.assertIn("| [!] | [W1-0006](records/W1-0006.json) | Task W1-0006 | test | rejected |", first)
        self.assertIn("| [=] | [W1-0007](records/W1-0007.json) | Task W1-0007 | test | owner_question |", first)
        self.assertIn("| [ ] | [W2-0001](records/W2-0001.json) | Task W2-0001 | test | new | W1-0006 ✗ |", first)
        self.assertNotIn("`2026091", first)
        written = self.run_cli("progress", "--write").stdout
        self.assertIn("Refreshed PROGRESS.md: 8 tasks", written)
        self.assertEqual((self.root / "PROGRESS.md").read_text(), first)
        self.assertEqual([path.name for path in self.root.iterdir() if path.name.startswith(".PROGRESS")], [])

    def test_progress_distinguishes_commit_references_and_orders_real_instants(self) -> None:
        first = record("W1-0001", evidence=["snapshot " + "a" * 64, "commit abc1234", "https://example.test/repo/commit/def5678"])
        first["history"][-1]["at"] = "2026-09-16T00:00:00+08:00"
        second = record("W1-0002")
        second["history"][-1]["at"] = "2026-09-15T23:00:00Z"
        self.write_record(first)
        self.write_record(second)
        text = self.run_cli("progress").stdout
        self.assertIn("Last record change: 2026-09-16 07:00", text)
        self.assertIn("snapshot " + "a" * 64, text)
        self.assertNotIn("commit `aaaaaaa`", text)
        self.assertIn("commit `abc1234`", text)
        self.assertIn("commit `def5678`", text)
        self.assertIn("`new` is unscheduled", text)

    def test_progress_timestamp_overflow_cannot_mask_a_successful_record_write(self) -> None:
        value = record("W1-0001")
        value["history"][-1]["at"] = "9999-12-31T23:59:59Z"
        self.write_record(value)
        self.write_record(record("W1-0002"))
        result = self.run_cli("claim", "W1-0002", "--owner", "worker", "--expect-revision", "1")
        self.assertEqual(json.loads(result.stdout)["revision"], 2)
        self.assertNotIn("Traceback", result.stderr)
        self.assertIn("outside HKT display range", (self.root / "PROGRESS.md").read_text())

    def test_record_writes_refresh_the_progress_document(self) -> None:
        self.write_record(record("W1-0001"))
        progress = self.root / "PROGRESS.md"
        self.assertFalse(progress.exists())
        claimed = self.run_cli("claim", "W1-0001", "--owner", "worker-a", "--expect-revision", "1")
        self.assertEqual(json.loads(claimed.stdout)["revision"], 2)
        self.assertIn(
            "| [~] | [W1-0001](records/W1-0001.json) | Task W1-0001 | test | in_progress | — | worker-a |",
            progress.read_text(),
        )
        finished = self.run_cli(
            "status", "W1-0001", "done", "--expect-revision", "2", "--note", "Finished.",
            "--evidence", "Commit 0123456789abcdef0123456789abcdef01234567 in main",
        )
        self.assertEqual(json.loads(finished.stdout)["revision"], 3)
        text = progress.read_text()
        self.assertIn("| [x] | [W1-0001](records/W1-0001.json) |", text)
        self.assertIn("`0123456`", text)
        draft = record("W1-0002", dependencies=["W1-0001"])
        for field in ("id", "revision", "history"):
            draft.pop(field)
        created = self.run_cli("create", "--wave", "W1", "--file", str(self.write_input("draft.json", draft)))
        self.assertEqual(json.loads(created.stdout)["created"], "W1-0002")
        self.assertIn("| [ ] | [W1-0002](records/W1-0002.json) | Task W1-0002 | test | planned | W1-0001 ✓ |", progress.read_text())
        patch = self.write_input("patch.json", {"title": "Renamed through update"})
        updated = self.run_cli("update", "W1-0002", "--file", str(patch), "--expect-revision", "1", "--note", "Renamed.")
        self.assertEqual(json.loads(updated.stdout)["revision"], 2)
        text = progress.read_text()
        self.assertIn("| Renamed through update |", text)
        self.assertEqual(text, self.run_cli("progress").stdout)
        self.assertEqual([path.name for path in self.root.iterdir() if path.name.startswith(".PROGRESS")], [])

    def test_symlinked_progress_document_is_not_followed(self) -> None:
        self.write_record(record("W1-0001"))
        target = Path(self.temporary.name) / "elsewhere.md"
        target.write_text("keep\n")
        (self.root / "PROGRESS.md").symlink_to(target)
        result = self.run_cli("claim", "W1-0001", "--owner", "worker-a", "--expect-revision", "1")
        self.assertEqual(json.loads(result.stdout)["revision"], 2)
        self.assertIn("PROGRESS.md was not refreshed", result.stderr)
        self.assertNotIn("Traceback", result.stderr)
        self.assertEqual(target.read_text(), "keep\n")
        self.assertTrue((self.root / "PROGRESS.md").is_symlink())
        refused = self.run_cli("progress", "--write", expected=3)
        self.assertIn("must not be a symlink", refused.stderr)


if __name__ == "__main__":
    unittest.main()
