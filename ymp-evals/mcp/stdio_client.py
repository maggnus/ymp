"""Actual official MCP client against the built CLI. All providers are deterministic mocks.
Run: python stdio_client.py /absolute/path/to/target/debug/ymp
"""
import asyncio
from contextlib import asynccontextmanager
import json
import os
from pathlib import Path
import sqlite3
import signal
import subprocess
import sys
import tempfile
import time

from mcp import ClientSession, StdioServerParameters
from mcp.client.stdio import stdio_client
from mcp import types

BINARY = str(Path(sys.argv[1]).resolve())
MAX_REPLY = 128 * 1024
SEEN_BYTES = []


def config(home):
    home.mkdir()
    (home / "config.toml").write_text('''version = 1
team = ["one", "two"]
[limits]
parallel = 2
turns = 80
turn_timeout_secs = 10
attempts = 2
[execution.one.fixed]
model = "mock"
effort = "low"
[execution.two.fixed]
model = "mock"
effort = "low"
[[providers]]
id = "scripted"
kind = "mock"
command = "internal"
[[agents]]
id = "one"
name = "One"
provider = "scripted"
model = "mock"
instructions = "[mock:usage]"
[[agents]]
id = "two"
name = "Two"
provider = "scripted"
model = "mock"
instructions = "[mock:usage]"
''')


def env(home):
    # Native configuration and authentication are deliberately absent. Only Mock is configured.
    return {"HOME": str(home), "PATH": "/usr/bin:/bin", "TMPDIR": tempfile.gettempdir()}


def params(home, project, execute=False, client="test"):
    args = ["--home", str(home), "-C", str(project), "mcp", "--client-id", client]
    if execute:
        args.append("--allow-execution")
    return StdioServerParameters(command=BINARY, args=args, env=env(home))


@asynccontextmanager
async def client(home, project, execute=False, identity="test"):
    async with stdio_client(params(home, project, execute, identity)) as streams:
        async with ClientSession(*streams) as session:
            initialized = await session.initialize()
            assert initialized.protocolVersion == "2025-06-18", initialized
            assert initialized.capabilities.tools is not None
            yield session


async def tool(session, name, args, error=False):
    result = await session.call_tool(name, args)
    assert result.isError == error, result
    raw = result.model_dump_json()
    SEEN_BYTES.append(len(raw.encode()))
    assert len(raw.encode()) <= MAX_REPLY, (name, len(raw))
    assert "YMP_MCP_TOKEN" not in raw
    value = json.loads(result.content[0].text)
    assert value == result.structuredContent
    return value


async def inspect(session, kind, **kwargs):
    return await tool(session, "ymp_inspect_v1", {"kind": kind, **kwargs})


def count(home, table="invocations"):
    with sqlite3.connect(home / "state.sqlite") as db:
        return db.execute(f"SELECT COUNT(*) FROM {table}").fetchone()[0]


async def terminal(session, request):
    deadline = time.monotonic() + 20
    while time.monotonic() < deadline:
        status = await tool(session, "ymp_request_v1", {"request_id": request})
        if status["observation"] == "terminal":
            return status
        await asyncio.sleep(0.01)
    raise AssertionError("run did not become terminal")


def start_args(request):
    return {"request_id": request, "action": "start", "prompt": "Create greeting.txt containing Hello from ymp and verify it.", "max_turns": 80, "turn_timeout_secs": 10, "max_seconds": 10}


def seed_large_knowledge(home, project, foreign_project):
    with sqlite3.connect(home / "state.sqlite") as db:
        for n in range(28):
            entry = {"id": f"large-{n:03d}", "project_id": project, "kind": "lesson", "title": "Large lesson", "content": "lesson " + "x" * 30000, "source_session": "fixture", "author": "fixture", "reviewer": None, "status": "active", "created_at": "2026-09-12T00:00:00Z"}
            db.execute("INSERT INTO memory(id,project_id,status,data) VALUES (?,?,?,?)", (entry["id"], project, "active", json.dumps(entry)))
            db.execute("INSERT INTO memory_search(id,title,content) VALUES (?,?,?)", (entry["id"], entry["title"], entry["content"]))
        entry.update(id="foreign-knowledge", project_id=foreign_project, content="foreign-project-secret")
        db.execute("INSERT INTO memory(id,project_id,status,data) VALUES (?,?,?,?)", (entry["id"], foreign_project, "active", json.dumps(entry)))
        db.execute("INSERT INTO memory_search(id,title,content) VALUES (?,?,?)", (entry["id"], entry["title"], entry["content"]))


async def sdk_walk(home, a, b):
    async with client(home, b) as session:
        foreign_project = (await inspect(session, "scope"))["project"]["id"]
    async with client(home, a) as session:
        discovered = await session.list_tools()
        assert len(discovered.tools) == 5
        for item in discovered.tools:
            assert item.name.endswith("_v1") and item.inputSchema["additionalProperties"] is False
        scope = await inspect(session, "scope")
        assert scope["actions"] == ["read"]
        assert len((await inspect(session, "pool"))["items"]) == 2
        assert (await inspect(session, "sessions"))["items"] == []
        assert (await tool(session, "ymp_knowledge_v1", {}))["items"] == []
        denied = await tool(session, "ymp_run_v1", start_args("denied"), error=True)
        assert "execution_denied" in denied["message"]
        assert count(home) == 0 and count(home, "sessions") == 0
        seed_large_knowledge(home, scope["project"]["id"], foreign_project)
        assert (await tool(session, "ymp_knowledge_v1", {"query": "lesson"}))["items"] == []
        cursor = None
        ids = []
        first = None
        while True:
            args = {"query": "lesson", "include_unconfirmed": True, "limit": 25}
            if cursor is not None:
                args["cursor"] = cursor
            page = await tool(session, "ymp_knowledge_v1", args)
            for row in page["items"]:
                assert row["truncated"] is True
                value = row["value"]
                assert len(value["entry"]["content"]) == 1024
                ids.append(value["id"])
                first = first or value
            cursor = page["next_cursor"]
            if cursor is None:
                break
        assert len(ids) == len(set(ids)) == 28
        await tool(session, "ymp_knowledge_v1", {"id": first["id"], "version": first["version"], "include_unconfirmed": True})
        await tool(session, "ymp_knowledge_v1", {"id": first["id"], "version": "stale", "include_unconfirmed": True}, error=True)
        await tool(session, "ymp_knowledge_v1", {"id": "foreign-knowledge", "include_unconfirmed": True}, error=True)
        await tool(session, "ymp_inspect_v1", {"kind": "sessions", "project_id": foreign_project}, error=True)
        await tool(session, "ymp_inspect_v1", {"kind": "pool", "limit": 26}, error=True)
        await tool(session, "ymp_inspect_v1", {"kind": "scope", "limit": 0}, error=True)
        assert count(home) == 0

    async with client(home, a, True) as session:
        args = start_args("complete")
        started = await tool(session, "ymp_run_v1", args)
        session_id = started["operation"]["session_id"]
        repeated = await tool(session, "ymp_run_v1", args)
        assert repeated["operation"]["session_id"] == session_id
        await tool(session, "ymp_run_v1", {**args, "prompt": "different"}, error=True)
        await tool(session, "ymp_run_v1", {**start_args("unsafe"), "max_turns": 81}, error=True)
        done = await terminal(session, "complete")
        assert done["operation"]["status"] == "completed", done
        assert (a / "greeting.txt").read_text() == "Hello from ymp\n"
        assert not (b / "greeting.txt").exists()
        before = count(home)
        for kind in ["session", "tasks", "results", "evidence", "history"]:
            page = await inspect(session, kind, session_id=session_id)
            if kind in ("results", "evidence"):
                assert page["items"], (kind, page)
                assert '"bytes"' not in json.dumps(page)
        assert count(home) == before
        tasks = await inspect(session, "tasks", session_id=session_id)
        assert all(row["value"]["state"] == "accepted" for row in tasks["items"])
        results = await inspect(session, "results", session_id=session_id)
        assert all(row["value"]["confirmation"] == "unconfirmed" for row in results["items"])
        assert count(home, "sessions") == 1
        assert done["usage"]["value"]["total"]["calls"] > 0
        # Notification cancellation of a completed MCP call cannot cancel its durable operation.
        await session.send_notification(types.ClientNotification(types.CancelledNotification(method="notifications/cancelled", params=types.CancelledNotificationParams(requestId="already-completed", reason="ignored"))))
        await session.send_ping()
        stopped = await tool(session, "ymp_run_v1", start_args("cancel"))
        await tool(session, "ymp_cancel_v1", {"request_id": "cancel"})
        cancelled = await terminal(session, "cancel")
        assert cancelled["operation"]["status"] == "paused", cancelled
        resume_id = stopped["operation"]["session_id"]
        resumed = await tool(session, "ymp_run_v1", {"request_id": "resume", "action": "resume", "session_id": resume_id, "max_seconds": 10})
        assert resumed["operation"]["session_id"] == resume_id
        assert (await terminal(session, "resume"))["operation"]["status"] == "completed"

    async with client(home, b, True) as foreign:
        for kind in ["session", "tasks", "results", "evidence", "history"]:
            await tool(foreign, "ymp_inspect_v1", {"kind": kind, "session_id": session_id}, error=True)
        await tool(foreign, "ymp_run_v1", {"request_id": "foreign", "action": "resume", "session_id": session_id, "max_seconds": 10}, error=True)
        await tool(foreign, "ymp_request_v1", {"request_id": "complete"}, error=True)
    async with client(home, a, True, "other-client") as other:
        await tool(other, "ymp_request_v1", {"request_id": "complete"}, error=True)
    before = count(home)
    async with client(home, a, True) as session:
        reopened = await tool(session, "ymp_run_v1", start_args("complete"))
        assert reopened["operation"]["session_id"] == session_id
        assert reopened["observation"] == "terminal"
        assert count(home) == before
    # Exiting the SDK stdio context closes stdin and terminates the server.
    async with client(home, a, True) as session:
        disconnected = await tool(session, "ymp_run_v1", start_args("disconnect"))
        assert disconnected["operation"]["session_id"]
    async with client(home, a, True) as session:
        status = await tool(session, "ymp_request_v1", {"request_id": "disconnect"})
        assert status["observation"] in ("terminal", "interrupted_or_running_elsewhere")
        assert count(home, "sessions") == 3
        await tool(session, "ymp_run_v1", {"request_id": "reopen-resume", "action": "resume", "session_id": status["operation"]["session_id"], "max_seconds": 10})
        assert (await terminal(session, "reopen-resume"))["operation"]["status"] == "completed"
    with sqlite3.connect(home / "state.sqlite") as db:
        invocations = [json.loads(row[0]) for row in db.execute("SELECT data FROM invocations")]
        assert invocations and all(i["requested"]["effort"] == "low" and i["requested"]["model"] == "mock" for i in invocations)
        assert all(i["state"] != "running" for i in invocations)


async def wire_edges(home, project):
    p = params(home, project)
    process = await asyncio.create_subprocess_exec(p.command, *p.args, env=p.env, stdin=asyncio.subprocess.PIPE, stdout=asyncio.subprocess.PIPE, stderr=asyncio.subprocess.PIPE)
    async def exchange(raw):
        process.stdin.write(raw.encode() + b"\n")
        await process.stdin.drain()
        return json.loads(await asyncio.wait_for(process.stdout.readline(), 3))
    assert (await exchange("{"))["error"]["code"] == -32700
    assert (await exchange('{"jsonrpc":"2.0","id":1,"method":"tools/list"}'))["error"]["code"] == -32002
    assert (await exchange('[{"id":2}]'))["error"]["code"] == -32600
    assert (await exchange('{"jsonrpc":"2.0","id":2,"method":"ping"}'))["result"] == {}
    assert (await exchange('{"jsonrpc":"2.0","id":2,"method":"ping"}'))["error"]["code"] == -32600
    assert (await exchange(json.dumps({"jsonrpc":"2.0","id":3,"method":"initialize","params":{"protocolVersion":"2099-01-01","capabilities":{},"clientInfo":{"name":"wire","version":"1"}}})))["result"]["protocolVersion"] == "2025-06-18"
    process.stdin.write(b'{"jsonrpc":"2.0","method":"notifications/initialized"}\n')
    await process.stdin.drain()
    assert (await exchange('{"jsonrpc":"2.0","id":4,"method":"unknown"}'))["error"]["code"] == -32601
    assert (await exchange('{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"internal_team_tool"}}'))["error"]["code"] == -32602
    assert (await exchange('{"jsonrpc":"2.0","id":6,"method":"tools/list","params":[]}'))["error"]["code"] == -32602
    # Split inside a UTF-8 code point and delay the newline: framing must retain bytes.
    unicode_id = "unicode-\U0001f310"
    frame = json.dumps({"jsonrpc":"2.0","id":unicode_id,"method":"ping"}, ensure_ascii=False).encode() + b"\n"
    split = frame.index(unicode_id[-1].encode()) + 1
    process.stdin.write(frame[:split])
    await process.stdin.drain()
    await asyncio.sleep(0.005)
    process.stdin.write(frame[split:])
    await process.stdin.drain()
    assert json.loads(await asyncio.wait_for(process.stdout.readline(), 3))["id"] == unicode_id
    assert (await exchange('x' * 65537))["error"]["code"] == -32600
    process.stdin.close()
    assert await asyncio.wait_for(process.wait(), 10) == 0
    assert not await process.stderr.read()


async def hard_exit_and_bridge(home, project):
    p = params(home, project, True)
    process = await asyncio.create_subprocess_exec(p.command, *p.args, env=p.env, stdin=asyncio.subprocess.PIPE, stdout=asyncio.subprocess.PIPE, stderr=asyncio.subprocess.PIPE)
    async def rpc(message, response=True):
        process.stdin.write(json.dumps(message).encode() + b"\n")
        await process.stdin.drain()
        if response:
            return json.loads(await asyncio.wait_for(process.stdout.readline(), 3))
    await rpc({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"crash","version":"1"}}})
    await rpc({"jsonrpc":"2.0","method":"notifications/initialized"}, False)
    before = count(home)
    response = await rpc({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"ymp_run_v1","arguments":start_args("hard-exit")}})
    session_id = response["result"]["structuredContent"]["operation"]["session_id"]
    deadline = time.monotonic() + 3
    while count(home) == before:
        assert time.monotonic() < deadline
        await asyncio.sleep(0.001)
    process.kill()
    assert await process.wait() < 0
    spent = count(home)
    async with client(home, project, True) as session:
        operation = await tool(session, "ymp_request_v1", {"request_id":"hard-exit"})
        assert operation["observation"] == "interrupted_or_running_elsewhere"
        assert count(home) == spent
        await tool(session, "ymp_run_v1", {"request_id":"recover-hard-exit","action":"resume","session_id":session_id,"max_seconds":10})
        recovered = await terminal(session, "recover-hard-exit")
        assert recovered["operation"]["status"] == "completed", recovered
        assert count(home) > spent
    # Synthetic credential tests bridge dispatch/lifecycle only; no live grant is borrowed.
    bridge = StdioServerParameters(command=BINARY, args=["mcp","--socket",str(home / "absent-test.sock")], env={**env(home),"YMP_MCP_TOKEN":"synthetic-not-a-live-capability"})
    async with stdio_client(bridge) as streams:
        async with ClientSession(*streams) as session:
            assert (await session.initialize()).serverInfo.name == "ymp-team"
            assert "team_post" in [t.name for t in (await session.list_tools()).tools]


async def signal_exit_walk():
    async def scenario(sig, active):
        with tempfile.TemporaryDirectory(prefix="ymp-mcp-signal-") as tmp:
            root = Path(tmp)
            home, project = root / "metadata", root / "project"
            project.mkdir(); config(home)
            p = params(home, project, True)
            process = subprocess.Popen([p.command, *p.args], env=p.env, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
            async def rpc(value, reply=True):
                process.stdin.write(json.dumps(value) + "\n")
                process.stdin.flush()
                if reply:
                    return json.loads(await asyncio.wait_for(asyncio.to_thread(process.stdout.readline), 3))
            try:
                await rpc({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"signal-exit","version":"1"}}})
                await rpc({"jsonrpc":"2.0","method":"notifications/initialized"}, False)
                if active:
                    await rpc({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"ymp_run_v1","arguments":start_args("signal")}})
                    deadline = time.monotonic() + 3
                    while count(home) == 0:
                        assert time.monotonic() < deadline
                        await asyncio.sleep(0.001)
                    await asyncio.sleep(0.01)
                else:
                    await rpc({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"ymp_inspect_v1","arguments":{"kind":"sessions"}}})
                    await asyncio.sleep(0.05)
                assert not process.stdin.closed
                started = time.monotonic()
                process.send_signal(sig)
                try:
                    code = await asyncio.wait_for(asyncio.to_thread(process.wait), 5)
                except asyncio.TimeoutError:
                    code = None
                elapsed = time.monotonic() - started
                # The input writer deliberately remains open through exit or timeout.
                assert not process.stdin.closed
                with sqlite3.connect(home / "state.sqlite") as db:
                    operations = [json.loads(v) for v, in db.execute("SELECT value FROM kv WHERE key LIKE 'public_mcp:v1:%'")]
                    invocations = [json.loads(v) for v, in db.execute("SELECT data FROM invocations")]
                accounting = (len(operations) == 1 and operations[0]["status"] == "paused" and operations[0]["ended_at"] is not None and bool(invocations) and all(i["state"] != "running" and i["ended_at"] is not None for i in invocations)) if active else not operations and not invocations
                result = {"signal":sig.name,"active":active,"stdin_open":True,"exit":code,"elapsed_seconds":round(elapsed,3),"terminal_accounting":accounting,"invocations":len(invocations)}
                print(json.dumps(result), flush=True)
                return result
            finally:
                if process.poll() is None:
                    process.kill()
                    await asyncio.to_thread(process.wait)
                process.stdin.close()
                stderr = process.stderr.read()
                assert not stderr, stderr
    results = await asyncio.gather(*(scenario(sig, active) for sig in (signal.SIGTERM, signal.SIGINT) for active in (False, True)))
    assert all(r["exit"] == 0 and r["elapsed_seconds"] < 5 and r["terminal_accounting"] for r in results), results


async def main():
    if "--signals-only" in sys.argv[2:]:
        await signal_exit_walk()
        return
    await signal_exit_walk()
    with tempfile.TemporaryDirectory(prefix="ymp-mcp-client-") as tmp:
        root = Path(tmp)
        home, a, b = root / "metadata", root / "a", root / "b"
        a.mkdir(); b.mkdir(); config(home)
        await sdk_walk(home, a, b)
        await wire_edges(home, a)
        await hard_exit_and_bridge(home, a)
    print(json.dumps({"client":"official mcp Python SDK 1.28.1","protocol":"2025-06-18","provider":"mock only, fixed mock/low","checks":"discovery, read-only admission, bounds, continuation, versions, no inference reads, scoped execution, conflicts, progress, usage, result/evidence, cancel/resume, foreign refs, disconnect/reopen, SIGKILL recovery, internal bridge lifecycle, malformed frames","largest_reply_bytes":max(SEEN_BYTES),"tool_calls":len(SEEN_BYTES)}, indent=2))


if __name__ == "__main__":
    asyncio.run(main())
