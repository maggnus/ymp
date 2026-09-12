"""Offline Codex stdio fixture; all messages and token counts are synthetic."""
import json
import sys
from pathlib import Path

settings = json.loads(Path("scenario.json").read_text())


def send(message):
    print(json.dumps(message), flush=True)


def reply(request, result):
    send({"jsonrpc": "2.0", "id": request["id"], "result": result})


for line in sys.stdin:
    request = json.loads(line)
    with Path("requests.jsonl").open("a") as log:
        log.write(json.dumps(request) + "\n")
    method = request.get("method")
    if method == "initialize":
        reply(request, {})
    elif method == "model/list":
        reply(request, {"data": [{"model": settings.get("thread_response", {}).get("model", "requested-model"), "isDefault": True, "supportedReasoningEfforts": [{"reasoningEffort": "high"}], "defaultReasoningEffort": "high"}]})
    elif method in ("thread/start", "thread/resume"):
        reply(request, {"thread": {"id": "thread-fixture"}, **settings.get("thread_response", {})})
    elif method == "turn/start":
        with Path("effects.log").open("a") as effects:
            effects.write("turn-started\n")
        if not settings.get("before_response"):
            reply(request, {"turn": {"id": "turn-fixture"}})
        for message in settings["notifications"]:
            send(message)
        if settings.get("before_response"):
            reply(request, {"turn": {"id": "turn-fixture"}})
        if settings.get("exit_after_notifications", True):
            break
    elif method != "initialized":
        raise AssertionError(f"Unexpected application request: {method}")
