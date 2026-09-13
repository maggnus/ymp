"""Offline native protocol fixture; no provider authentication or network calls."""
import json
import os
import sys
import time
from pathlib import Path

kind, actor, log_path, version = sys.argv[1:5]
session = actor + "-" + str(os.getpid())
sandbox = None


def send(value):
    print(json.dumps(value), flush=True)


for line in sys.stdin:
    request = json.loads(line)
    method = request.get("method")
    params = request.get("params", {})
    # Never persist MCP capability/configuration payloads.
    observed = {"actor": actor, "method": method, "version": version}
    for key in ("threadId", "sessionId", "sandbox", "approvalPolicy"):
        if key in params:
            observed[key] = params[key]
    if method in ("turn/start", "session/prompt"):
        blocks = params.get("input", params.get("prompt", []))
        prompt = "\n".join(block.get("text", "") for block in blocks)
        current = prompt.rsplit("Your current assignment (", 1)[-1]
        purpose = current.split("):", 1)[0]
        observed["purpose"] = purpose
    with Path(log_path).open("a") as log:
        log.write(json.dumps(observed) + "\n")
    if "id" not in request:
        continue
    result = {}
    if method == "initialize":
        result = {"agentInfo": {"version": version}, "agentCapabilities": {"loadSession": True}}
    elif method == "model/list":
        result = {"data": [{"model": "fixture-model", "isDefault": True}]}
    elif method in ("thread/start", "thread/resume"):
        sandbox = params["sandbox"]
        result = {"thread": {"id": session}}
    elif method in ("session/new", "session/load"):
        result = {"sessionId": session, "models": {"currentModelId": "fixture-model", "availableModels": [{"modelId": "fixture-model"}]}}
    elif method in ("turn/start", "session/prompt"):
        if kind == "acp":
            send({"jsonrpc": "2.0", "id": request["id"], "error": {"code": -32000, "message": "Connection error."}})
            continue
        control_path = Path(log_path).parent / "control.json"
        control = json.loads(control_path.read_text()) if control_path.exists() else {}
        if purpose == "review_plan" and control.get("gate_review"):
            control_path.with_name("review-started").write_text("ready")
            while not control_path.with_name("review-release").exists():
                time.sleep(0.01)
        if purpose == "plan":
            text = json.dumps({"summary": "Exact native proposal", "tasks": [{"title": "Inspect independently", "description": "Produce a finding from the current workspace", "access": "write", "competence": "analysis", "difficulty": "simple", "dependencies": [], "checks": []}]})
        elif purpose == "conversation":
            text = json.dumps({"action": "answer", "answer": "The saved review remains pending."})
        elif purpose in ("review_plan", "review", "final_review"):
            text = json.dumps({"approved": control.get("approved", True), "reason": "Independent review preserves this objection when rejected; old external effects remain unknown."})
            if control.get("malformed"):
                text = "incomplete {"
        elif purpose == "execute":
            assert sandbox == "danger-full-access"
            Path("current-result.txt").write_text("New work from current files; historical effects remain unknown.\n")
            if control.get("fail_execute"):
                send({"jsonrpc": "2.0", "id": request["id"], "result": {"turn": {"id": "turn"}}})
                send({"method": "error", "params": {"threadId": session, "turnId": "turn", "willRetry": False, "error": {"message": "New unknown execution failure"}}})
                send({"method": "turn/completed", "params": {"threadId": session, "turn": {"id": "turn", "status": "failed"}}})
                continue
            text = "Created current-result.txt from the current workspace."
        elif purpose == "synthesis":
            text = "New work completed; historical effects remain unknown."
        else:
            raise AssertionError("Unexpected purpose: " + purpose)
        if purpose != "execute":
            assert sandbox == "read-only"
        send({"jsonrpc": "2.0", "id": request["id"], "result": {"turn": {"id": "turn"}}})
        send({"method": "thread/tokenUsage/updated", "params": {"threadId": session, "tokenUsage": {"total": {"inputTokens": 21, "outputTokens": 3, "cachedInputTokens": 7, "reasoningOutputTokens": 1}, "last": {"inputTokens": 21, "outputTokens": 3, "cachedInputTokens": 7, "reasoningOutputTokens": 1}}}})
        send({"method": "item/completed", "params": {"threadId": session, "turnId": "turn", "item": {"type": "agentMessage", "text": text}}})
        send({"method": "turn/completed", "params": {"threadId": session, "turn": {"id": "turn", "status": "completed"}}})
        continue
    send({"jsonrpc": "2.0", "id": request["id"], "result": result})
