"""Offline stdio fixture; no Codex binary, network, model, or native home access."""
import json
import sys
import uuid
from pathlib import Path

log = Path(sys.argv[1])
drift = sys.argv[2] == "drift"
thread = str(uuid.uuid4())


def send(value):
    print(json.dumps(value), flush=True)


for line in sys.stdin:
    request = json.loads(line)
    with log.open("a") as output:
        output.write(json.dumps(request) + "\n")
    method = request["method"]
    params = request.get("params", {})
    if "id" not in request:
        continue
    result = {}
    if method == "model/list":
        result = {"data": [{"model": "protocol-weak", "isDefault": True,
                            "supportedReasoningEfforts": [{"reasoningEffort": "low"}]}]}
    elif method == "thread/start":
        result = {"thread": {"id": thread}, "model": params["model"],
                  "reasoningEffort": "high" if drift else params["config"]["model_reasoning_effort"]}
    elif method == "turn/start":
        result = {"turn": {"id": "protocol-turn"}}
    send({"jsonrpc": "2.0", "id": request["id"], "result": result})
    if method == "turn/start":
        usage = {"inputTokens": 8, "outputTokens": 2, "cachedInputTokens": 3,
                 "cacheWriteInputTokens": 1, "reasoningOutputTokens": 1}
        for _ in range(2):
            send({"method": "thread/tokenUsage/updated", "params": {
                "threadId": thread, "turnId": "protocol-turn", "tokenUsage": {"total": usage, "last": usage}}})
        send({"method": "item/completed", "params": {"threadId": thread, "turnId": "protocol-turn",
              "item": {"type": "agentMessage", "text": "protocol-marker"}}})
        send({"method": "turn/completed", "params": {"threadId": thread,
              "turn": {"id": "protocol-turn", "status": "completed"}}})
        break
