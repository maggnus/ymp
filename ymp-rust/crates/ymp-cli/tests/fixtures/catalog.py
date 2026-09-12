#!/usr/bin/env python3
"""Scripted native metadata/turn protocol; no network or installed model calls."""
import json
import sys
import time

kind, spec_path, log_path = sys.argv[1:4]
spec = json.load(open(spec_path))
model = "wire-a"
effort = "quiet"

def send(value):
    print(json.dumps(value), flush=True)

for line in sys.stdin:
    request = json.loads(line)
    with open(log_path, "a") as log:
        log.write(json.dumps(request) + "\n")
    method = request.get("method")
    params = request.get("params", {})
    if "id" not in request:
        continue
    if spec.get(kind) == "hang":
        time.sleep(10)
    if spec.get(kind) == "fail":
        send({"id": request["id"], "error": {"code": -1, "message": "SYNTHETIC_SECRET_DIAGNOSTIC"}})
        continue
    names = ["Orchid · native A", "Quartz / native B"]
    models = ["wire-a", "wire-b"]
    if spec.get(kind) == "changed":
        names = ["Renamed by native provider", "New native offering"]
        models = ["wire-a", "wire-c"]
    result = {}
    if method == "initialize":
        result = {"agentInfo": {"version": "catalog-fixture"}, "agentCapabilities": {}}
    elif method == "model/list":
        index = 1 if params.get("cursor") else 0
        result = {"data": [{"id": "picker-" + models[index], "model": models[index], "displayName": names[index], "isDefault": index == 0,
                            "defaultReasoningEffort": "quiet", "supportedReasoningEfforts": [{"reasoningEffort": value} for value in ["quiet", "future-control"]]}],
                  "nextCursor": None if index else "next-page"}
    elif method == "session/new":
        result = {"sessionId": "fixture-session", "models": {"currentModelId": "wire-a", "availableModels": [{"modelId": m, "name": names[i]} for i, m in enumerate(models)]},
                  "configOptions": [{"id": "thought_level", "name": "Native thought mode", "type": "select", "currentValue": "none", "options": [{"value": "none", "name": "Native none"}, {"value": "max", "name": "Native maximum"}]},
                                    {"id": "native_tempo", "name": "Tempo / native", "type": "select", "currentValue": "steady", "options": [{"value": "steady", "name": "Steady choice"}]}]}
    elif method in ["thread/start", "thread/resume"]:
        model = params.get("model", "wire-a")
        effort = params.get("config", {}).get("model_reasoning_effort", "quiet")
        result = {"thread": {"id": "fixture-session"}, "model": model, "reasoningEffort": effort}
    elif method == "turn/start":
        result = {"turn": {"id": "fixture-turn"}}
    else:
        send({"id": request["id"], "error": {"code": -1, "message": "Unexpected protocol method"}})
        continue
    send({"id": request["id"], "result": result})
    if method == "turn/start":
        send({"method": "item/completed", "params": {"threadId": "fixture-session", "turnId": "fixture-turn", "item": {"type": "agentMessage", "text": "done"}}})
        send({"method": "turn/completed", "params": {"threadId": "fixture-session", "turn": {"id": "fixture-turn", "status": "completed"}}})
