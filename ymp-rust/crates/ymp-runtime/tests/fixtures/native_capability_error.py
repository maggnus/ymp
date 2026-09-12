"""Offline protocol fixture. Only the runtime-issued test capability is read."""
import json
import os
import socket
import sys

capability = os.environ["YMP_MCP_TOKEN"]
with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as report:
    report.connect(sys.argv[1])
    report.sendall((capability + "\n").encode())
for line in sys.stdin:
    request = json.loads(line)
    if "id" in request:
        print(json.dumps({
            "jsonrpc": "2.0",
            "id": request["id"],
            "error": {
                "code": -32000,
                "message": "MCP setup failed: YMP_MCP_TOKEN=" + capability,
                "data": {"cause": "Transport rejected assignment capability " + capability},
            },
        }), flush=True)
