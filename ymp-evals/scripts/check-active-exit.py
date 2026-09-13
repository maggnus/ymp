"""Verify double Ctrl+C around a real TUI invocation of a scripted ACP process."""
import fcntl
import json
import os
from pathlib import Path
import pty
import select
import sqlite3
import struct
import subprocess
import sys
import tempfile
import termios
import time

binary = str(Path(sys.argv[1]).resolve())
root = Path(tempfile.mkdtemp(prefix="ymp-active-exit-"))
home, project = root / "home", root / "project"
home.mkdir()
project.mkdir()
stub = root / "provider.py"
stub.write_text('''import json,sys,time
for line in sys.stdin:
 q=json.loads(line)
 if "id" not in q: continue
 method=q["method"]; result={}
 if method=="initialize": result={"protocolVersion":1,"agentCapabilities":{}}
 elif method=="session/new": result={"sessionId":"fixture-session","models":{"currentModelId":"native-exit-fixture","availableModels":[{"modelId":"native-exit-fixture"}]},"modes":{"currentModeId":"default","availableModes":[{"id":"default","name":"Read only"}]}}
 elif method=="session/prompt": time.sleep(120)
 print(json.dumps({"jsonrpc":"2.0","id":q["id"],"result":result}),flush=True)
''')
(home / "config.toml").write_text('''version = 1
team = ["one", "two"]
[limits]
turns = 20
parallel = 2
attempts = 1
turn_timeout_secs = 30
[[providers]]
id = "fixture"
kind = "acp"
command = "python3"
args = [''' + json.dumps(str(stub)) + ''']
[[agents]]
id = "one"
name = "One"
provider = "fixture"
[[agents]]
id = "two"
name = "Two"
provider = "fixture"
''')
master, slave = pty.openpty()
fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 50, 240, 0, 0))
proc = subprocess.Popen([binary, "--home", str(home), "-C", str(project)],
                        stdin=slave, stdout=slave, stderr=slave, start_new_session=True)
os.close(slave)
output = bytearray()


def drain(duration):
    end = time.monotonic() + duration
    while time.monotonic() < end:
        if select.select([master], [], [], min(.05, max(0, end - time.monotonic())))[0]:
            try:
                part = os.read(master, 65536)
            except OSError:
                break
            if not part:
                break
            output.extend(part)


def latest():
    try:
        with sqlite3.connect("file:" + str(home / "state.sqlite") + "?mode=ro", uri=True) as db:
            row = db.execute("SELECT data FROM invocations ORDER BY rowid DESC LIMIT 1").fetchone()
            return json.loads(row[0]) if row else None
    except sqlite3.OperationalError:
        return None


try:
    drain(.8)
    os.write(master, b"Inspect the fixture\r")
    deadline = time.monotonic() + 10
    while time.monotonic() < deadline:
        drain(.1)
        invocation = latest()
        if invocation and invocation.get("reported", {}).get("model") == "native-exit-fixture":
            break
    else:
        raise AssertionError("Fixture never reached a native invocation")
    assert invocation["state"] == "running"
    session = invocation["session_id"]
    os.write(master, b"\x03")
    drain(.3)
    assert proc.poll() is None and latest()["state"] == "running", "First press cancelled work"
    assert b"Press Ctrl-C again to exit" in output
    os.write(master, b"\x03")
    drain(12)
    assert proc.wait(timeout=2) == 0
    trace = json.loads(subprocess.check_output([binary, "--home", str(home), "trace", session], text=True))
    assert trace["session"]["status"] == "paused"
    assert len(trace["invocations"]) == 1 and trace["invocations"][0]["state"] == "cancelled"
    grants = {g for assignment in trace["assignments"] for g in assignment["grant_ids"]}
    revoked = {e["data"]["grant"]["id"] for e in trace["history"]
               if e["kind"] == "provenance" and e["data"].get("change") == "grant_revoked"}
    assert grants and grants <= revoked
    text = output.decode(errors="replace")
    assert "Resume this session with:" in text
    (root / "terminal.log").write_text(text)
    print(json.dumps({"binary": binary, "directory": str(root),
                      "first_press": "invocation remained running", "second_press": "graceful exit",
                      "session_status": "paused", "cancelled_invocations": 1,
                      "revoked_grants": len(grants), "native_inference": False}, indent=2))
finally:
    if proc.poll() is None:
        proc.kill()
        proc.wait()
    os.close(master)
