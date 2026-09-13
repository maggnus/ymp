#!/usr/bin/env python3
"""Freeze an executable consumer proposal, without authorizing model calls."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import random
import shutil
import subprocess
import sys
import tempfile
import uuid
from datetime import datetime, timezone

from native_controls_probe import CONTROLS
import restricted_python

ROOT = Path(__file__).resolve().parents[2]
CONDITIONS = ["strong-solo", "weak-solo", "independent-2", "independent-3", "cooperation-2", "cooperation-3"]


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def generate(runner, output, workspaces, phase, seed=2010914, prerequisite=None):
    scripted = phase == "protocol-e2e"
    evidence = ROOT / "ymp-docs/evidence/ymp-201/rework-round1/native-envelope-final.json"
    codex = ROOT / "ymp-evals/weak-pilot/codex_protocol_v2.py" if scripted else Path(shutil.which("codex")).resolve()
    models = {"weak": "fixture-weak" if scripted else "gpt-5.6-luna", "strong": "fixture-strong" if scripted else "gpt-6-astra"}
    calibration = phase == "preparation-calibration"
    calls, seconds, turn, tokens, reservation = (8, 90, 90, 10000, 1000) if scripted else (
        (12, 480, 480, 80000, 6000) if calibration else (16, 900, 900, 160000, 10000))
    resources = {"unknown_usage": "stop", "startup_invocations": 2, "required_review_invocations": 1,
                 "max_context_chars": 128000, "startup_context_chars": 32000, "max_output_chars": 64000,
                 "native_max_turns": 16, "observed_tokens": tokens, "invocation_tokens": reservation,
                 "review_reserve_tokens": reservation}
    profiles, execution = [], {}
    for role, size in [("weak", 3), ("strong", 1)]:
        for ordinal in range(1, size + 1):
            identifier = f"{role}-{ordinal}"
            profiles.append({"id": identifier, "name": models[role], "provider": "codex",
                             "model": models[role], "instructions": "", "enabled": True})
            execution[identifier] = {"fixed": {"model": models[role], "effort": "low"}}
    catalog_models = []
    if scripted:
        offerings = [{"model": value, "id": value, "supportedReasoningEfforts": [{"reasoningEffort": "low"}]} for value in models.values()]
    else:
        native_report = json.loads((ROOT / "ymp-docs/evidence/ymp-201/native-controls-v2/probe.json").read_text())
        offerings = native_report["runs"][0]["models_selected"]
    for offering in offerings:
        catalog_models.append({"id": offering["model"], "picker_id": offering.get("id"), "controls": [
            {"id": "effort", "values": {"kind": "choices", "options": [row["reasoningEffort"] for row in offering["supportedReasoningEfforts"]]},
             "default": offering.get("defaultReasoningEffort")} ]})
    captured_at = datetime.now(timezone.utc) if scripted else datetime.fromtimestamp(
        (ROOT / "ymp-docs/evidence/ymp-201/native-controls-v2/probe.json").stat().st_mtime, timezone.utc)
    catalog = {"source": {"kind": "native_metadata", "method": "protocol-fixture model/list" if scripted else "model/list (probe.json; timestamp is capture-file mtime)",
                          "observed_at": captured_at.isoformat()},
               "models_complete": False, "models": catalog_models}
    config = {"version": 1, "limits": {"parallel": 2, "turns": calls, "turn_timeout_secs": turn, "attempts": 1, "resources": resources},
              "providers": [{"id": "codex", "kind": "codex", "command": str(codex), "args": [], "env_refs": {}, "enabled": True}],
              "agents": profiles, "team": ["weak-1", "weak-2"], "execution": execution, "capabilities": {"codex": catalog}}
    attempts = []
    generator = random.Random(seed)
    for task in ["reconcile", "repair"]:
        conditions = CONDITIONS.copy()
        if calibration:
            conditions = ["strong-solo", "weak-solo"] + (["cooperation-2", "cooperation-3"] if task == "repair" else [])
        generator.shuffle(conditions)
        for condition in conditions:
            identifier = task + "-" + condition
            attempts.append({"id": identifier, "condition": condition, "task": task,
                             "variant": "preparation" if calibration else "measured",
                             "blind_id": uuid.uuid4().hex})
    frozen = {}
    for area in ["ymp-evals/weak-pilot", "ymp-evals/validators", "ymp-rust/crates/ymp-eval-driver/src/bin"]:
        for path in sorted((ROOT / area).rglob("*")):
            if path.is_file() and "__pycache__" not in path.parts:
                frozen[str(path.relative_to(ROOT))] = sha(path)
    return {"schema_version": 2, "execution_kind": "protocol-fixture" if scripted else "native", "phase": phase,
            "source_revision": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
            "repository": str(ROOT), "output": str(Path(output).resolve()), "workspace_root": str(Path(workspaces).resolve()),
            "runner_sha256": sha(runner), "python": str(Path(sys.executable).resolve()), "python_sha256": sha(sys.executable),
            "candidate_execution": restricted_python.identity(), "codex": str(codex), "codex_sha256": sha(codex),
            "native_home": str(Path(os.environ.get("CODEX_HOME", Path.home() / ".codex")).resolve()),
            "protected_roots": sorted({str(Path.home().resolve()), str(Path("/tmp").resolve()), str(Path(tempfile.gettempdir()).resolve())}),
            "controls": CONTROLS, "control_evidence": str(evidence), "control_evidence_sha256": sha(evidence) if evidence.exists() else "",
            "config": config, "catalog": catalog, "weak_model": models["weak"], "strong_model": models["strong"],
            "max_invocations": calls, "group_seconds": seconds, "seed": seed,
            "time_rule": "remaining_condition_deadline", "independent_schedule": "ordinal_serial_shared_deadline",
            "approval_scope": "one_phase_once",
            "approval_ledger": str((Path(output).resolve().parent / "test-approval-ledger") if scripted else (Path.home() / ".local/state/ymp201/phase-approvals").resolve()),
            "task_prompt": "Read REQUIREMENTS.md in the selected directory and deliver the requested files. Use only the permitted tools and visible files in this directory.",
            "frozen_files": frozen, "prerequisite_manifest_sha256": prerequisite, "attempts": attempts}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--runner", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path, help="future private controller output")
    parser.add_argument("--workspaces", required=True, type=Path, help="separate future solving root")
    parser.add_argument("--phase", choices=["protocol-e2e", "preparation-calibration", "measured-pilot"], required=True)
    parser.add_argument("--seed", type=int, default=2010914)
    parser.add_argument("--prerequisite-manifest", type=Path)
    args = parser.parse_args()
    if args.phase == "measured-pilot" and not args.prerequisite_manifest:
        parser.error("A pilot proposal must reference its preparation calibration manifest")
    print(json.dumps(generate(args.runner, args.output, args.workspaces, args.phase, args.seed,
                              sha(args.prerequisite_manifest) if args.prerequisite_manifest else None), indent=2))


if __name__ == "__main__":
    main()
