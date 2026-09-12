"""Synthetic controls: these test the validators, never runtime/model quality."""

import copy
import hashlib
import importlib.util
import json
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


EVALS = Path(__file__).resolve().parents[1]
MODULE = importlib.util.spec_from_file_location("universal", EVALS / "validators" / "universal.py")
validator = importlib.util.module_from_spec(MODULE)
MODULE.loader.exec_module(validator)
FIXTURES = EVALS / "fixtures" / "universal"


def sha256(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


class UniversalTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="ymp-acceptance-")
        self.addCleanup(self.temporary.cleanup)
        self.workdir = Path(self.temporary.name).resolve()
        shutil.copytree(FIXTURES / "inputs", self.workdir / "inputs")
        (self.workdir / "outputs").mkdir()
        for source in (FIXTURES / "reference").iterdir():
            shutil.copyfile(source, self.workdir / "outputs" / source.name)

    def receipt(self, case_id):
        """Hand-constructed synthetic positive control, not a runtime exporter."""
        spec = validator.scenario(case_id)
        artifact_hash = sha256(self.workdir / spec["output"])
        result = {
            "schema_version": 1, "case_id": case_id, "session_id": "session-one",
            "working_directory": str(self.workdir),
            "artifact": {"path": spec["output"], "sha256": artifact_hash,
                         "result_id": "result-one", "result_version": 1,
                         "producer_agent_ids": ["agent-a"]},
            "review": {"review_id": "review-one", "reviewer_agent_id": "agent-b",
                       "result_id": "result-one", "result_version": 1,
                       "artifact_sha256": artifact_hash, "decision": "accepted",
                       "rationale": "Checked the stated acceptance criteria against the current artifact."},
            "acceptance": {"result_id": "result-one", "result_version": 1,
                           "review_id": "review-one", "state": "accepted",
                           "confirmation_ids": [] if case_id == "qualitative" else ["confirmation-one"],
                           "confirmation": spec["expected_confirmation"]},
            "confirmations": [], "reputation_observations": [],
            "usage": [
                {"agent_id": "agent-a", "assignment_id": "assignment-a", "invocation_id": "invocation-a",
                 "phase": "execution", "tokens": 11, "coverage": "complete"},
                {"agent_id": "agent-b", "assignment_id": "assignment-b", "invocation_id": "invocation-b",
                 "phase": "review", "tokens": 7, "coverage": "complete"},
            ],
        }
        if case_id != "qualitative":
            result["confirmations"] = [{
                "confirmation_id": "confirmation-one", "observer": "runtime", "outcome": "passed",
                "check_id": spec["check_id"], "result_id": "result-one", "result_version": 1,
                "artifact_sha256": artifact_hash, "criterion_ids": spec["criterion_ids"],
                "inputs": [{"path": name, "sha256": sha256(self.workdir / name)} for name in spec["inputs"]],
            }]
            result["reputation_observations"] = [{
                "observation_id": "observation-one", "agent_id": "agent-a", "result_id": "result-one",
                "result_version": 1, "confirmation_id": "confirmation-one",
            }]
        if case_id == "document":
            result["follow_up"] = {"session_id": "session-one", "result_id": "result-one",
                                   "answer_path": str(self.workdir / spec["output"]),
                                   "artifact_sha256": artifact_hash, "invocations_started": 0}
        return result

    def invalid_workflow(self, case_id, mutate):
        receipt = self.receipt(case_id)
        mutate(receipt)
        with self.assertRaises(validator.Invalid):
            validator.check_workflow(case_id, self.workdir, receipt)

    def protocol_control(self, spec):
        for relative, text in spec.get("file_assertions", {}).items():
            (self.workdir / relative).write_bytes(text.encode("utf-8"))
        return validator.expand(spec["expected"], self.workdir)

    def test_reference_artifacts_meet_independent_content_checks(self):
        for case_id in ("document", "transform", "grounded"):
            with self.subTest(case=case_id):
                checked = validator.check_artifact(case_id, self.workdir)
                self.assertTrue(checked["objective_success"])
        qualitative = validator.check_artifact("qualitative", self.workdir)
        self.assertIsNone(qualitative["objective_success"])
        self.assertEqual(qualitative["check_scope"], "structure_only")

    def test_positive_workflows_preserve_acceptance_grade_and_usage(self):
        for case_id in ("document", "transform", "grounded", "qualitative"):
            with self.subTest(case=case_id):
                result = validator.check_workflow(case_id, self.workdir, self.receipt(case_id))
                self.assertEqual(result["confirmation"], "unconfirmed" if case_id == "qualitative" else "confirmed")
                self.assertEqual(result["resource_metrics"]["reported_tokens"], 18)

    def test_missing_artifacts_fail_all_workflows(self):
        for case_id in ("document", "transform", "grounded", "qualitative"):
            with self.subTest(case=case_id):
                artifact = self.workdir / validator.scenario(case_id)["output"]
                artifact.unlink()
                with self.assertRaises(OSError):
                    validator.check_artifact(case_id, self.workdir)

    def test_irrelevant_nonempty_artifacts_fail_objective_checks(self):
        for case_id in ("document", "transform", "grounded"):
            with self.subTest(case=case_id):
                artifact = self.workdir / validator.scenario(case_id)["output"]
                artifact.write_text('{"claims": [], "note": "All agents agree the task passed."}\n')
                with self.assertRaises(validator.Invalid):
                    validator.check_artifact(case_id, self.workdir)

    def test_document_wrong_fact_or_extra_claim_fails(self):
        artifact = self.workdir / "outputs/workshop.md"
        original = artifact.read_text()
        for content in (original.replace("Room Cedar", "Room Maple"), original + "\nLunch is included.\n",
                        "    " + original):
            with self.subTest(content=content):
                artifact.write_text(content)
                with self.assertRaises(validator.Invalid):
                    validator.check_artifact("document", self.workdir)

    def test_document_allows_only_incidental_newline_and_trailing_space_changes(self):
        artifact = self.workdir / "outputs/workshop.md"
        artifact.write_bytes(artifact.read_text().replace("\n", "  \r\n").encode())
        self.assertTrue(validator.check_artifact("document", self.workdir)["objective_success"])

    def test_document_fixture_enforces_non_git_directory(self):
        (self.workdir / ".git").write_text("gitdir: /nonexistent\n")
        with self.assertRaisesRegex(validator.Invalid, "non-Git"):
            validator.check_artifact("document", self.workdir)

    def test_transform_rejects_void_refund_zero_count_sort_and_extra_row_errors(self):
        artifact = self.workdir / "outputs/totals.csv"
        correct = artifact.read_text()
        corruptions = [
            correct.replace("North,125,2", "North,1124,3"),
            correct.replace("North,125,2", "North,375,2"),
            correct.replace('"South, Annex",600,2', '"South, Annex",600,1'),
            correct + "East,0,0\n",
            "account,total_cents,posted_entries\nWest,297,2\nNorth,125,2\n\"South, Annex\",600,2\n",
            correct.replace("West,297,2", "West,297,2,EXTRA"),
        ]
        for index, content in enumerate(corruptions):
            with self.subTest(corruption=index):
                artifact.write_text(content)
                with self.assertRaises(validator.Invalid):
                    validator.check_artifact("transform", self.workdir)

    def test_grounding_rejects_wrong_values_invented_citations_and_wrong_rows(self):
        artifact = self.workdir / "outputs/claims.json"
        original = json.loads(artifact.read_text())
        for key, value in (("value", 100), ("source", "https://invented.invalid/report"),
                           ("rows", ["O01"]), ("rows", []), ("unit", "percent_change")):
            with self.subTest(key=key, value=value):
                candidate = copy.deepcopy(original)
                candidate["claims"][2][key] = value
                artifact.write_text(json.dumps(candidate))
                with self.assertRaises(validator.Invalid):
                    validator.check_artifact("grounded", self.workdir)

    def test_grounding_rejects_unrequested_claim_and_float(self):
        artifact = self.workdir / "outputs/claims.json"
        original = json.loads(artifact.read_text())
        extra = copy.deepcopy(original)
        extra["claims"].append({"id": "cause", "value": "Training caused the improvement"})
        floating = copy.deepcopy(original)
        floating["claims"][0]["value"] = 80.0
        for candidate in (extra, floating):
            artifact.write_text(json.dumps(candidate))
            with self.assertRaises(validator.Invalid):
                validator.check_artifact("grounded", self.workdir)

    def test_changed_inputs_cannot_redefine_the_expected_answer(self):
        source = self.workdir / "inputs/observations.csv"
        source.write_text(source.read_text().replace(",19,20", ",12,20"))
        artifact = self.workdir / "outputs/claims.json"
        artifact.write_text(json.dumps(validator.expected_claims(source)))
        with self.assertRaisesRegex(validator.Invalid, "input changed"):
            validator.check_artifact("grounded", self.workdir)

    def test_duplicate_keys_and_nonfinite_json_are_rejected(self):
        artifact = self.workdir / "outputs/claims.json"
        for content in ('{"claims": [], "claims": []}', '{"claims": [], "value": NaN}'):
            artifact.write_text(content)
            with self.assertRaises(validator.Invalid):
                validator.check_artifact("grounded", self.workdir)

    def test_symlink_outside_workdir_is_rejected(self):
        artifact = self.workdir / "outputs/totals.csv"
        artifact.unlink()
        artifact.symlink_to(FIXTURES / "reference/totals.csv")
        with self.assertRaisesRegex(validator.Invalid, "escapes"):
            validator.check_artifact("transform", self.workdir)

    def test_self_review_or_unaccepted_review_cannot_pass(self):
        self.invalid_workflow("document", lambda r: r["review"].update(reviewer_agent_id="agent-a"))
        self.invalid_workflow("document", lambda r: r["review"].update(decision="rejected"))
        self.invalid_workflow("document", lambda r: r["review"].update(rationale=""))

    def test_workflow_rejects_ambiguous_json_types(self):
        self.invalid_workflow("document", lambda r: r.update(schema_version=True))
        self.invalid_workflow("document", lambda r: r["review"].update(result_version=True))
        self.invalid_workflow("document", lambda r: r["confirmations"][0].update(result_version=1.0))
        self.invalid_workflow("document", lambda r: r.update(reputation_observations={}))

    def test_passing_unrelated_check_and_model_agreement_are_not_confirmation(self):
        for bad_id in ("shell:true:exit-0", "all-agents-agree", "ymp-evals.transform.v1"):
            with self.subTest(check=bad_id):
                self.invalid_workflow("document", lambda r: r["confirmations"][0].update(check_id=bad_id))
        self.invalid_workflow("document", lambda r: r["confirmations"][0].update(observer="agent-a"))
        self.invalid_workflow("document", lambda r: r["confirmations"][0].update(outcome="failed"))

    def test_confirmation_must_cover_current_bytes_result_and_inputs(self):
        mutations = [
            lambda r: r["confirmations"][0].update(artifact_sha256="0" * 64),
            lambda r: r["confirmations"][0].update(result_id="unrelated-result"),
            lambda r: r["confirmations"][0].update(result_version=2),
            lambda r: r["confirmations"][0].update(criterion_ids=["unrelated-criterion"]),
            lambda r: r["confirmations"][0].update(inputs=[]),
            lambda r: r["confirmations"][0]["inputs"][0].update(sha256="0" * 64),
            lambda r: r["review"].update(artifact_sha256="0" * 64),
            lambda r: r["acceptance"].update(review_id="unrelated-review"),
            lambda r: r["acceptance"].update(confirmation_ids=["unrelated-confirmation"]),
        ]
        for index, mutation in enumerate(mutations):
            with self.subTest(mutation=index):
                self.invalid_workflow("document", mutation)

    def test_corrupted_artifact_fails_even_with_agreeing_review_and_old_good_receipt(self):
        receipt = self.receipt("transform")
        (self.workdir / "outputs/totals.csv").write_text("account,total_cents,posted_entries\nNorth,99999,2\n")
        with self.assertRaises(validator.Invalid):
            validator.check_workflow("transform", self.workdir, receipt)

    def test_qualitative_consensus_cannot_award_confirmation_or_reputation(self):
        self.invalid_workflow("qualitative", lambda r: r["acceptance"].update(confirmation="confirmed"))
        self.invalid_workflow("qualitative", lambda r: r["confirmations"].append({"basis": "all-agents-agree"}))
        self.invalid_workflow("qualitative", lambda r: r["reputation_observations"].append({"agent_id": "agent-a"}))

    def test_reputation_rejects_reviewer_credit_unrelated_basis_and_duplicates(self):
        self.invalid_workflow("grounded", lambda r: r["reputation_observations"][0].update(agent_id="agent-b"))
        self.invalid_workflow("grounded", lambda r: r["reputation_observations"][0].update(confirmation_id="unrelated"))
        self.invalid_workflow("grounded", lambda r: r["reputation_observations"].append(r["reputation_observations"][0].copy()))
        self.invalid_workflow("grounded", lambda r: r["reputation_observations"].append({**r["reputation_observations"][0], "observation_id": "new-credit-same-result"}))

    def test_unknown_usage_remains_unknown_and_is_attributed_to_agents(self):
        receipt = self.receipt("transform")
        receipt["usage"][0].update(tokens=None, coverage="unknown")
        result = validator.check_workflow("transform", self.workdir, receipt)
        self.assertFalse(result["resource_metrics"]["usage_complete"])
        self.assertEqual(result["resource_metrics"]["reported_tokens"], 7)
        receipt["usage"][1].update(tokens=None, coverage="unknown")
        self.assertIsNone(validator.check_workflow("transform", self.workdir, receipt)["resource_metrics"]["reported_tokens"])
        self.invalid_workflow("transform", lambda r: r["usage"][0].update(tokens=0, coverage="unknown"))
        self.invalid_workflow("transform", lambda r: r["usage"][0].update(agent_id="provider-total"))
        self.invalid_workflow("transform", lambda r: r["usage"].pop())
        self.invalid_workflow("transform", lambda r: r["usage"][0].update(phase="review"))

    def test_location_follow_up_cannot_rerun_or_change_artifact(self):
        self.invalid_workflow("document", lambda r: r["follow_up"].update(invocations_started=1))
        self.invalid_workflow("document", lambda r: r["follow_up"].update(answer_path="/tmp/other.md"))
        self.invalid_workflow("document", lambda r: r["follow_up"].update(result_id="different-result"))
        self.invalid_workflow("document", lambda r: r.update(working_directory="/tmp/hidden-copy"))

    def test_protocol_positive_controls_and_missing_extra_events(self):
        cases = validator.read_json(EVALS / "scenarios/universal-protocol.json")["cases"]
        for case_id, spec in cases.items():
            with self.subTest(case=case_id):
                expected = self.protocol_control(spec)
                self.assertTrue(validator.check_protocol(case_id, self.workdir, expected)["protocol_passed"])
                missing = copy.deepcopy(expected)
                missing["events"].pop()
                with self.assertRaises(validator.Invalid):
                    validator.check_protocol(case_id, self.workdir, missing)
                extra = copy.deepcopy(expected)
                extra["events"].append({"type": "invocation_started", "agent": "unexpected"})
                with self.assertRaises(validator.Invalid):
                    validator.check_protocol(case_id, self.workdir, extra)

    def test_protocol_rejects_concrete_policy_regressions(self):
        mutations = {
            "fixed-size": lambda r: r["final_state"].update(members=["a", "b", "c"]),
            "fixed-roster": lambda r: r["final_state"].update(result_state="accepted"),
            "adaptive-team": lambda r: r["final_state"].update(historical_members=["a"]),
            "effort-support": lambda r: r["events"][0].update(sent_effort="low"),
            "budget-reservations": lambda r: r["final_state"].update(spent_units=110),
            "partial-usage": lambda r: r["final_state"].update(cost=0, strict_token_bound_claimed=True),
            "concurrency-conflicts": lambda r: r["events"].insert(1, r["events"].pop(4)),
            "assignment-authority": lambda r: r["final_state"].update(active_grants=["a1", "a2"]),
            "restart-inspection": lambda r: r["final_state"].update(automatic_production_replays=1),
            "location-retrieval": lambda r: r["events"][1].update(invocations_started=1),
            "knowledge-correction": lambda r: r["final_state"].update(current_value=95),
            "evidence-boundaries": lambda r: r["final_state"].update(result_state="accepted"),
            "artifact-version": lambda r: r["final_state"].update(current_confirmation="c1"),
        }
        cases = validator.read_json(EVALS / "scenarios/universal-protocol.json")["cases"]
        self.assertEqual(set(mutations), set(cases))
        for case_id, mutate in mutations.items():
            with self.subTest(case=case_id):
                observed = self.protocol_control(cases[case_id])
                mutate(observed)
                with self.assertRaises(validator.Invalid):
                    validator.check_protocol(case_id, self.workdir, observed)

    def test_protocol_cannot_hide_missing_artifact_or_changed_external_source(self):
        cases = validator.read_json(EVALS / "scenarios/universal-protocol.json")["cases"]
        interrupted = self.protocol_control(cases["restart-inspection"])
        (self.workdir / "outputs/totals.csv").write_text("An unrelated file.\n")
        with self.assertRaises(validator.Invalid):
            validator.check_protocol("restart-inspection", self.workdir, interrupted)
        location = validator.expand(cases["location-retrieval"]["expected"], self.workdir)
        (self.workdir / "outputs/workshop.md").write_text("Not an agenda.\n")
        with self.assertRaises(validator.Invalid):
            validator.check_protocol("location-retrieval", self.workdir, location)
        knowledge = validator.expand(cases["knowledge-correction"]["expected"], self.workdir)
        (self.workdir / "inputs/observations-corrected.csv").write_text("unrelated data\n")
        with self.assertRaises(validator.Invalid):
            validator.check_protocol("knowledge-correction", self.workdir, knowledge)

    def test_cli_returns_machine_readable_failure_for_missing_and_malformed_input(self):
        command = [sys.executable, str(EVALS / "validators/universal.py"), "workflow",
                   "--case", "document", "--workdir", str(self.workdir)]
        missing = subprocess.run(command, capture_output=True, text=True, check=False)
        self.assertEqual(missing.returncode, 1)
        self.assertFalse(json.loads(missing.stdout)["validator_passed"])
        malformed = self.workdir / "malformed.json"
        malformed.write_text("{broken")
        response = subprocess.run(command + ["--observed", str(malformed)],
                                  capture_output=True, text=True, check=False)
        self.assertEqual(response.returncode, 1)
        self.assertFalse(json.loads(response.stdout)["validator_passed"])


if __name__ == "__main__":
    unittest.main()
