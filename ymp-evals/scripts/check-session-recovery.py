"""Recover a stopped session from the team page of a real terminal, with offline native fixtures.

A saved plan's review fails on an ACP fixture agent that could write. Through /team the window adds
an independent Codex-protocol fixture agent and removes the failed one, asks for a new read-only
review of the saved plan, continues with current files after one confirmation, and waits for the
ordinary run of the same session to finish. The recorded trace is then checked: one plan, one plan
review, one current-files authorization, accepted tasks, the historical failure still recorded and
the configuration file unchanged by the owner actions. No real provider, credential or model is
used; the providers are the runtime's protocol fixture script.
"""
import argparse
import hashlib
import json
import pathlib
import re
import subprocess
import time
import uuid


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary", type=pathlib.Path)
    parser.add_argument("--output", type=pathlib.Path, required=True)
    parser.add_argument("--ascii", action="store_true")
    parser.add_argument("--width", type=int, default=100)
    parser.add_argument("--height", type=int, default=32)
    args = parser.parse_args()
    binary, root = args.binary.resolve(), args.output.resolve()
    repository = pathlib.Path(__file__).resolve().parents[2]
    fixture = repository / "ymp-rust/crates/ymp-runtime/tests/fixtures/fresh_plan_review.py"
    root.mkdir(parents=True, exist_ok=False)
    home, project, log = root / "metadata", root / "project", root / "protocol.jsonl"
    home.mkdir()
    project.mkdir()
    providers = "".join(
        f'''
[[providers]]
id = "{actor}-native"
kind = "{kind}"
command = "python3"
args = [{json.dumps(str(fixture))}, "{kind}", "{actor}", {json.dumps(str(log))}, "native-v1"]
'''
        for actor, kind in (("author", "codex"), ("failed", "acp"), ("fresh", "codex"))
    )
    # The replacement agent names the model its provider's catalog reports, so the owner API has
    # verified native metadata for it after the scan below.
    agents = "".join(
        f'''
[[agents]]
id = "{actor}"
name = "{actor.title()}"
provider = "{actor}-native"
{'model = "fixture-model"' if actor == "fresh" else ""}
'''
        for actor in ("author", "failed", "fresh")
    )
    (home / "config.toml").write_text(f'''version = 1
team = ["author", "failed"]

[team_constraints]
fixed_roster = ["author", "failed"]

[limits]
parallel = 1
turns = 40
turn_timeout_secs = 5
attempts = 3

[limits.resources]
startup_invocations = 10
{providers}{agents}''')
    marker = ">" if args.ascii else "›"
    report = {"binary": str(binary), "ascii": args.ascii, "size": [args.width, args.height],
              "native_inference": False, "steps": [], "checks": {}}

    def ymp(*words, timeout=120):
        return subprocess.run([str(binary), "--home", str(home), "-C", str(project), *words],
                              capture_output=True, text=True, timeout=timeout)

    scan = ymp("catalog", "--refresh", "--provider", "fresh-native", "--timeout-secs", "5")
    assert scan.returncode == 0, scan.stderr
    first = ymp("run", "--no-memory", "Inspect the directory")
    (root / "first-run.txt").write_text(first.stdout + first.stderr)
    session = re.search(r"^Session: (\S+)", first.stdout, re.M).group(1)
    report["session"] = session
    config_before = hashlib.sha256((home / "config.toml").read_bytes()).hexdigest()
    plans_before = sum(1 for line in log.read_text().splitlines()
                       if json.loads(line).get("purpose") == "plan")
    requests_before = len(log.read_text().splitlines())
    socket = "ymp-recovery-" + uuid.uuid4().hex[:10]

    def tmux(*values, check=True):
        return subprocess.run(["tmux", "-L", socket, *values], capture_output=True,
                              text=True, check=check, timeout=5)

    def screen():
        return tmux("capture-pane", "-p", "-t", "check").stdout

    def wait(needle, seconds=10, found=None):
        found = found or (lambda value: needle in value)
        deadline = time.monotonic() + seconds
        while time.monotonic() < deadline:
            value = screen()
            if found(value):
                return value
            time.sleep(.1)
        (root / "timeout.txt").write_text(screen())
        raise AssertionError("terminal did not show " + needle)

    def recorded(kind):
        trace = ymp("trace", session)
        return trace.returncode == 0 and any(
            decision["kind"] == kind for decision in json.loads(trace.stdout)["decisions"])

    def keys(*values):
        for value in values:
            tmux("send-keys", "-t", "check", value)
            time.sleep(.15)

    def capture(name):
        value = screen()
        (root / (name + ".txt")).write_text(value)
        report["steps"].append(name)
        return value

    def command(text):
        # A page shows the composer too but takes typing as its filter. Escape closes a dialog,
        # clears a filter, leaves the page and focuses the composer; with nothing left it is inert.
        keys("Escape", "Escape", "Escape", "Escape")
        wait("Describe a task")
        tmux("send-keys", "-t", "check", "-l", text)
        keys("Enter")

    def selected(label):
        return any(marker in row and label in row[row.index(marker):]
                   for row in screen().splitlines())

    def select(label, limit=60):
        for _ in range(limit):
            if selected(label):
                return
            keys("Down")
        raise AssertionError("could not select " + label)

    try:
        tmux("new-session", "-d", "-s", "check", "-x", str(args.width), "-y", str(args.height),
             "env", "NO_COLOR=1", *(["LC_ALL=C"] if args.ascii else []),
             str(binary), "--home", str(home), "-C", str(project), "resume", session)
        wait("Describe a task")
        command("/team")
        wait("CURRENT TEAM")
        # A short terminal scrolls to the stopped stage; its label may be shortened there.
        select("plan")
        capture("team-page")

        command("/team add fresh")
        wait("Add fresh")
        capture("confirm-add")
        keys("y")
        wait("Added fresh")
        command("/team remove failed")
        wait("Remove failed")
        keys("y")
        wait("Removed failed")
        capture("team-changed")

        select("plan")
        keys("Enter")
        wait("Review saved plan again")
        select("Review saved plan again", limit=12)
        capture("stage-actions")
        keys("Enter")
        wait("Choose a reviewer")
        select("fresh", limit=12)
        capture("reviewers")
        keys("Enter")
        wait("plan_review in the trace", seconds=60, found=lambda _: recorded("plan_review"))
        time.sleep(1)
        capture("reviewed")

        select("plan")
        keys("Enter")
        wait("Continue with current files")
        select("Continue with current files", limit=12)
        capture("stage-actions-after-review")
        keys("Enter")
        wait("Continue session", seconds=30)
        capture("confirm-current-files")
        keys("y")
        # The header names the session's status.
        wait("completed session", seconds=120,
             found=lambda value: "completed" in value.splitlines()[0])
        capture("continued")
        keys("C-c", "C-c")
        time.sleep(1)

        trace = json.loads(ymp("trace", session).stdout)
        (root / "trace.json").write_text(json.dumps(trace, indent=2) + "\n")
        kinds = [decision["kind"] for decision in trace["decisions"]]
        requests = [json.loads(line) for line in log.read_text().splitlines()]
        failures = [decision["links"]["failure"] for decision in trace["decisions"]
                    if decision.get("links", {}).get("failure")]
        checks = {
            "session_completed": trace["session"]["status"] == "completed",
            "tasks_accepted": bool(trace["tasks"]) and all(
                str(task["state"]).lower() == "accepted" for task in trace["tasks"]),
            "one_plan": kinds.count("plan_proposed") == 1,
            "one_plan_review": kinds.count("plan_review") == 1,
            "one_current_files_authorization": kinds.count("owner_current_files_authorized") == 1,
            "no_new_planning_call": sum(1 for r in requests if r.get("purpose") == "plan") == plans_before,
            "failed_agent_not_called_again": not any(
                r.get("actor") == "failed" for r in requests[requests_before:]),
            "one_new_review_and_one_execution": (
                sum(1 for r in requests[requests_before:] if r.get("purpose") == "review_plan") == 1
                and sum(1 for r in requests[requests_before:] if r.get("purpose") == "execute") == 1),
            "historical_failure_recorded": any(f.get("agent_id") == "failed" for f in failures),
            "result_written": (project / "current-result.txt").read_text()
            == "New work from current files; historical effects remain unknown.\n",
            "configuration_unchanged": hashlib.sha256((home / "config.toml").read_bytes()).hexdigest()
            == config_before,
        }
        report["checks"] = checks
        assert all(checks.values()), checks
        report["passed"] = True
    except Exception as error:
        report["passed"] = False
        report["error"] = str(error)
        raise
    finally:
        tmux("kill-server", check=False)
        (root / "report.json").write_text(json.dumps(report, indent=2) + "\n")
        print(json.dumps(report, indent=2))


if __name__ == "__main__":
    main()
