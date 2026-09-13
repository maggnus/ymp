#!/usr/bin/env python3
"""Synthetic transport for the complete consumer, never a native/model launcher.

Answers are deliberately supplied by this test backend, not experiment subjects.
No artifact here establishes model quality. Only fixture-* model IDs are served.
"""

import argparse
import json
from pathlib import Path
import shutil
import sys
import uuid
import time


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--scenario", required=True, type=Path)
    parser.add_argument("--purpose", default="catalog_scan")
    args, _ = parser.parse_known_args()
    scenario = json.loads(args.scenario.read_text())
    assert scenario["execution_kind"] == "protocol-fixture"
    thread = str(uuid.uuid4())
    cwd = None

    def send(value):
        print(json.dumps(value), flush=True)

    for line in sys.stdin:
        request = json.loads(line)
        method, params = request["method"], request.get("params", {})
        # Omit MCP configuration: its capability is never a fixture log field.
        with Path(scenario["request_log"]).open("a") as log:
            log.write(json.dumps({"method": method, "purpose": args.purpose,
                "model": params.get("model"), "effort": params.get("effort"),
                "cwd": params.get("cwd"), "threadId": params.get("threadId"),
                "input": params.get("input"), "permissions": params.get("permissions"),
                "legacy_sandbox_present": "sandbox" in params}) + "\n")
        if "id" not in request:
            continue
        result = {}
        if method == "config/read":
            result = {"config": {"mcp_servers": {"synthetic_unused": {"enabled": True}}}}
        elif method == "model/list":
            result = {"data": [{"model": model, "isDefault": model == "fixture-weak",
                                "supportedReasoningEfforts": [{"reasoningEffort": "low"}]}
                               for model in ["fixture-weak", "fixture-strong"]]}
        elif method in ("thread/start", "thread/resume"):
            assert params["model"] in ("fixture-weak", "fixture-strong")
            assert params["config"]["model_reasoning_effort"] == "low"
            assert "sandbox" not in params and params["permissions"]
            for control in ("multi_agent", "multi_agent_v2", "memories"):
                assert params["config"]["features"][control] is False
            cwd = Path(params["cwd"])
            thread = params.get("threadId", thread)
            result = {"thread": {"id": thread}, "model": params["model"], "reasoningEffort": "low",
                      "permissions": params["permissions"]}
        elif method == "turn/start":
            assert params["effort"] == "low" and cwd is not None
            result = {"turn": {"id": str(uuid.uuid4())}}
        send({"jsonrpc": "2.0", "id": request["id"], "result": result})
        if method != "turn/start":
            continue
        text = respond(scenario, args.purpose, cwd, params["input"][0]["text"])
        turn = result["turn"]["id"]
        counts = {"inputTokens": 80, "outputTokens": 20, "cachedInputTokens": 30,
                  "reasoningOutputTokens": 10}
        if scenario.get("fault") != "unknown-usage":
            for _ in range(2):
                send({"method": "thread/tokenUsage/updated", "params": {
                    "threadId": thread, "turnId": turn, "tokenUsage": {"total": counts, "last": counts}}})
        if scenario.get("fault") == "pending":
            time.sleep(60)
        send({"method": "item/completed", "params": {"threadId": thread, "turnId": turn,
            "item": {"type": "agentMessage", "text": text}}})
        send({"method": "turn/completed", "params": {"threadId": thread,
              "turn": {"id": turn, "status": "failed" if scenario.get("fault") == "provider-error" else "completed",
                       "error": {"message": "synthetic provider failure"} if scenario.get("fault") == "provider-error" else None}}})
        break


def respond(scenario, purpose, cwd, prompt):
    private = Path(scenario["fixture_root"]) / scenario["variant"] / scenario["task"] / "private"
    outputs = ["windows.py"] if scenario["task"] == "repair" else ["totals.csv", "exceptions.csv"]
    tasks = [("Deliver artifact", outputs)]
    if scenario["cooperation_tasks"] == 2:
        # A second actual assignment preserves another public property; no renamed producer.
        tasks = [("Deliver artifact", outputs), ("Inspect public contract", ["public-inspection.txt"])]
    if purpose == "plan":
        return json.dumps({"summary": "Synthetic transport plan, not a model measurement", "tasks": [
            {"title": title, "description": "Produce " + ", ".join(names) + " from visible requirements",
             "competence": "implementation", "difficulty": "simple", "access": "write",
             "dependencies": [], "checks": []} for title, names in tasks]})
    if purpose == "review_plan":
        return json.dumps({"approved": True, "reason": "Synthetic public task split"})
    if purpose in ("execute", "pilot_candidate"):
        if purpose == "execute":
            current = prompt.rsplit("Your current assignment (execute):\n", 1)[-1]
            selected = [names for title, names in tasks if "\n" + title + "\n" in current]
            assert len(selected) == 1
            names = selected[0]
        else:
            assert "Your current assignment" not in prompt
            names = outputs
        for name in names:
            if name == "public-inspection.txt":
                (cwd / name).write_text("Read REQUIREMENTS.md; synthetic inspection only.\n")
            else:
                shutil.copyfile(private / name, cwd / name)
        return "Synthetic provider supplied the assigned files for independent inspection."
    if purpose in ("review", "final_review"):
        return json.dumps({"approved": all((cwd / name).is_file() for name in outputs),
                           "reason": "Synthetic review of supplied artifact presence; no hidden scoring"})
    if purpose == "synthesis":
        return "Synthetic workflow finished; no native measurements or quality conclusions."
    raise ValueError("Unsupported synthetic purpose: " + purpose)


if __name__ == "__main__":
    main()
