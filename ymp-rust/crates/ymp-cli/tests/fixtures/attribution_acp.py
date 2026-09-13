#!/usr/bin/env python3
"""Scripted ACP installation for headless heading checks; no network or installed model calls.

Every turn first posts one team message through the ymp coordination socket and then answers
its assignment. The installation describes its model with a caption and reports its thought
level, which is what a heading must name instead of the caption or the actor identifier.
"""
import json
import socket
import sys
import time

mcp = None


def send(value):
    print(json.dumps(value), flush=True)


def post(text):
    endpoint = mcp["args"][mcp["args"].index("--socket") + 1]
    token = next(entry["value"] for entry in mcp["env"] if entry["name"] == "YMP_MCP_TOKEN")
    with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as connection:
        connection.connect(endpoint)
        request = {
            "token": token,
            "request_id": "fixture-chat",
            "name": "team_post",
            "arguments": {"text": text},
        }
        connection.sendall(json.dumps(request).encode() + b"\n")
        reply = json.loads(connection.makefile("rb").readline())
        if not reply.get("ok"):
            raise SystemExit("team_post was refused: " + json.dumps(reply))


for line in sys.stdin:
    request = json.loads(line)
    if "id" not in request:
        continue
    method = request["method"]
    params = request.get("params", {})
    result = {}
    if method == "initialize":
        result = {"protocolVersion": 1, "agentCapabilities": {"loadSession": True}}
    elif method in ("session/new", "session/load"):
        mcp = params["mcpServers"][0]
        result = {
            "sessionId": "fixture-session",
            "models": {
                "currentModelId": "native-model-z",
                "availableModels": [
                    {"modelId": "native-model-z", "name": "Recommended descriptive caption"}
                ],
            },
            "modes": {
                "currentModeId": "default",
                "availableModes": [
                    {"id": "default", "name": "Default permissions"},
                    {"id": "bypass_permissions", "name": "Fixture write"},
                ],
            },
            "configOptions": [
                {
                    "id": "thought_level",
                    "type": "select",
                    "currentValue": "max",
                    "options": [{"value": "max", "name": "Maximum"}, {"value": "low", "name": "Low"}],
                }
            ],
        }
    elif method == "session/prompt":
        prompt = params["prompt"][0]["text"]
        time.sleep(0.1)
        post("Fixture shared finding")
        if "current assignment (plan)" in prompt:
            text = json.dumps(
                {
                    "summary": "Heading fixture",
                    "tasks": [
                        {
                            "title": "Report a fixture fact",
                            "description": "Return a short fixture response",
                            "competence": "implementation",
                            "difficulty": "simple",
                            "dependencies": [],
                            "checks": [],
                        }
                    ],
                }
            )
        elif any(
            "current assignment (%s)" % purpose in prompt
            for purpose in ("review_plan", "review", "final_review")
        ):
            text = json.dumps({"approved": True, "reason": "Fixture response is present"})
        else:
            text = "Fixture response complete"
        send(
            {
                "method": "session/update",
                "params": {
                    "sessionId": "fixture-session",
                    "update": {"sessionUpdate": "agent_message_chunk", "content": {"text": text}},
                },
            }
        )
        result = {"stopReason": "end_turn", "usage": {"inputTokens": 1, "outputTokens": 1, "thoughtTokens": 0}}
    send({"jsonrpc": "2.0", "id": request["id"], "result": result})
