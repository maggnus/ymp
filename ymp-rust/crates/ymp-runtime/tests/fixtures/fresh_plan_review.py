"""Offline native protocol fixture; no provider authentication or network calls."""
import json
import os
import sys
from pathlib import Path

kind, actor, log_path, version = sys.argv[1:5]
session = actor + "-" + str(os.getpid())


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
        result = {"thread": {"id": session}}
    elif method in ("session/new", "session/load"):
        result = {"sessionId": session}
    elif method in ("turn/start", "session/prompt"):
        if kind == "acp":
            send({"jsonrpc": "2.0", "id": request["id"], "error": {"code": -32000, "message": "Connection error."}})
            continue
        if purpose == "plan":
            text = json.dumps({"summary": "Exact native proposal", "tasks": [{"title": "Inspect independently", "description": "Inspect the current workspace", "access": "read_only", "competence": "analysis", "difficulty": "simple", "dependencies": [], "checks": []}]})
        elif purpose == "conversation":
            text = json.dumps({"action": "answer", "answer": "The saved review remains pending."})
        elif purpose in ("review_plan", "review", "final_review"):
            text = json.dumps({"approved": True, "reason": "The immutable proposal was independently reviewed; old external effects remain unknown."})
        else:
            raise AssertionError("The plan-only fixture must never run production: " + purpose)
        send({"jsonrpc": "2.0", "id": request["id"], "result": {"turn": {"id": "turn"}}})
        send({"method": "item/completed", "params": {"threadId": session, "turnId": "turn", "item": {"type": "agentMessage", "text": text}}})
        send({"method": "turn/completed", "params": {"threadId": session, "turn": {"id": "turn", "status": "completed"}}})
        continue
    send({"jsonrpc": "2.0", "id": request["id"], "result": result})
