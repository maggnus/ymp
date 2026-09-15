"""Deterministic offline Codex App Server fixture for ymp-runtime tests."""

import json
import os
import signal
import subprocess
import sys
import time
from pathlib import Path


root = Path(os.environ["YMP_CODEX_FIXTURE"])
scenario = json.loads((root / "scenario.json").read_text())
(root / "argv.json").write_text(json.dumps(sys.argv[1:]))
(root / "process.pid").write_text(str(os.getpid()))


def send(message):
    print(json.dumps(message), flush=True)


def reply(request, result):
    send({"jsonrpc": "2.0", "id": request["id"], "result": result})


def notification(method, params):
    send({"jsonrpc": "2.0", "method": method, "params": params})


def log_request(request):
    with (root / "requests.jsonl").open("a") as output:
        output.write(json.dumps(request) + "\n")


def model_page(cursor):
    pages = scenario.get(
        "model_pages",
        [
            [
                {
                    "id": "picker-model-a",
                    "model": "model-a",
                    "isDefault": True,
                    "supportedReasoningEfforts": [
                        {"reasoningEffort": "low"},
                        {"reasoningEffort": "high"},
                    ],
                    "defaultReasoningEffort": "high",
                }
            ]
        ],
    )
    page = 0 if cursor is None else int(cursor.removeprefix("page-"))
    result = {"data": pages[page]}
    if page + 1 < len(pages):
        result["nextCursor"] = f"page-{page + 1}"
    return result


def emit_notifications():
    for message in scenario.get("notifications", []):
        send(message)
    if scenario.get("malformed_after_turn"):
        print("not-json", flush=True)
    if scenario.get("oversized_after_turn"):
        sys.stdout.write("x" * (16 * 1024 * 1024 + 1) + "\n")
        sys.stdout.flush()


def start_descendant():
    child = subprocess.Popen(
        [
            sys.executable,
            "-c",
            "import signal,time; signal.signal(signal.SIGTERM, signal.SIG_IGN); time.sleep(60)",
        ]
    )
    (root / "descendant.pid").write_text(str(child.pid))


def record_term(_signum, _frame):
    marker = os.environ.get("YMP_TEST_PROVIDER_EXITED")
    if marker:
        Path(marker).write_text("exited")
    raise SystemExit(0)


if scenario.get("ignore_term"):
    signal.signal(signal.SIGTERM, signal.SIG_IGN)
elif os.environ.get("YMP_TEST_PROVIDER_EXITED"):
    signal.signal(signal.SIGTERM, record_term)
if scenario.get("stderr_bytes"):
    sys.stderr.write("x" * int(scenario["stderr_bytes"]))
    sys.stderr.flush()


for line in sys.stdin:
    request = json.loads(line)
    log_request(request)
    method = request.get("method")
    if method == "initialize":
        if scenario.get("hang_initialize"):
            while True:
                time.sleep(1)
        if scenario.get("initialize_error"):
            send(
                {
                    "jsonrpc": "2.0",
                    "id": request["id"],
                    "error": scenario["initialize_error"],
                }
            )
            continue
        reply(
            request,
            {
                "userAgent": scenario.get("user_agent", "codex-fixture 1.0"),
                "codexHome": str(root),
                "platformFamily": "unix",
                "platformOs": sys.platform,
            },
        )
    elif method == "initialized":
        continue
    elif method == "model/list":
        cursor = request.get("params", {}).get("cursor")
        if scenario.get("repeat_cursor"):
            reply(request, {"data": model_page(None)["data"], "nextCursor": "same"})
        else:
            reply(request, model_page(cursor))
    elif method in ("thread/start", "thread/resume"):
        params = request.get("params", {})
        response = {
            "thread": {"id": scenario.get("session", "thread-fixture")},
            "model": params.get("model", "model-a"),
            "reasoningEffort": params.get("config", {}).get(
                "model_reasoning_effort", "high"
            ),
        }
        response.update(scenario.get("thread_response", {}))
        reply(request, response)
    elif method == "turn/start":
        with (root / "effects.log").open("a") as output:
            output.write("turn-started\n")
        if scenario.get("server_request"):
            send(
                {
                    "jsonrpc": "2.0",
                    "id": 900,
                    "method": "item/commandExecution/requestApproval",
                    "params": {},
                }
            )
            response = json.loads(sys.stdin.readline())
            log_request(response)
            (root / "server-response.json").write_text(json.dumps(response))
        if scenario.get("before_turn_response"):
            emit_notifications()
        reply(request, {"turn": {"id": scenario.get("turn", "turn-fixture")}})
        if not scenario.get("before_turn_response"):
            emit_notifications()
        (root / "notifications.done").write_text("done\n")
        if scenario.get("delay_exit"):
            time.sleep(float(scenario["delay_exit"]))
        if scenario.get("spawn_descendant"):
            start_descendant()
        if scenario.get("close_stdout_then_hang"):
            sys.stdout.close()
            while True:
                time.sleep(1)
        if scenario.get("hang"):
            while True:
                time.sleep(1)
        break
    else:
        raise AssertionError(f"unexpected request method: {method}")


raise SystemExit(scenario.get("exit_code", 0))
