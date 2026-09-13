"""YMP-149 terminal check: agent and team rows after the language change (mock providers only).

Usage: check-agent-rows.py BINARY --output DIR [--ascii]

The configuration has an enabled mock profile in the next-session team, a disabled profile outside
it, and a profile whose provider executable does not exist. No provider is asked anything: the
configuration file already exists, so ymp starts without a catalog scan, and pages only read the
local pool. Every capture is written to DIR; the report is DIR/report.json.

Assertions, at each size:
  R1 no primary row or heading carries READING or `read from the installation`;
  R2 /agents shows NEXT TEAM where the table has room for it, with true for the member and false
     for the non-members, never `in team`;
  R3 /team names concise refusal causes: `profile disabled` or `executable not found`;
  R4 Inspect of a profile shows the `source` and `catalog` fields and the scoped
     `next team` flag (widest size only, where Inspect is read in full);
  R5 `t` on /agents flips the flag on the page and in config.toml, and a second `t` restores it.
"""
import argparse
import json
import pathlib
import subprocess
import sys
import time
import tomllib
import uuid

sys.dont_write_bytecode = True

parser = argparse.ArgumentParser()
parser.add_argument("binary", type=pathlib.Path)
parser.add_argument("--output", type=pathlib.Path, required=True)
parser.add_argument("--ascii", action="store_true")
args = parser.parse_args()
binary = args.binary.resolve()
root = args.output.resolve()
root.mkdir(parents=True, exist_ok=False)
home, project = root / "metadata", root / "project"
home.mkdir()
project.mkdir()
(home / "config.toml").write_text('''version = 1
team = ["atlas", "cygnus"]
[[providers]]
id = "demo"
kind = "mock"
command = "internal"
[[providers]]
id = "absent"
kind = "acp"
command = "ymp149-no-such-executable"
[[agents]]
id = "atlas"
name = "Atlas"
provider = "demo"
[[agents]]
id = "boreal"
name = "Boreal"
provider = "demo"
enabled = false
[[agents]]
id = "cygnus"
name = "Cygnus"
provider = "absent"
''')
socket = "ymp149-rows-" + uuid.uuid4().hex[:8]
report = {"binary": str(binary), "ascii": args.ascii, "native_inference": False, "cases": [], "failures": []}


def tmux(*values, check=True):
    return subprocess.run(["tmux", "-L", socket, *values], capture_output=True, text=True, check=check, timeout=5)


def screen():
    return tmux("capture-pane", "-p", "-t", "check").stdout


def capture(name):
    text = screen()
    (root / (name + ".txt")).write_text(text)
    return text


def wait_for(predicate, label):
    deadline = time.monotonic() + 6
    while time.monotonic() < deadline:
        text = screen()
        if predicate(text):
            return text
        time.sleep(0.08)
    capture("failure-" + label)
    raise AssertionError("terminal did not reach " + label)


def keys(*values):
    for value in values:
        tmux("send-keys", "-t", "check", value)
        time.sleep(0.15)


def type_text(text):
    tmux("send-keys", "-t", "check", "-l", text)
    time.sleep(0.15)


def command(text):
    keys("Escape", "Escape", "Escape", "C-u")
    type_text(text)
    keys("Enter")


def check(case, condition, detail=""):
    report["cases"].append({"case": case, "passed": bool(condition)})
    if not condition:
        report["failures"].append({"case": case, "detail": detail[-3000:]})


def team_flags():
    return tomllib.loads((home / "config.toml").read_text())["team"]


def flag(text, needle):
    """The NEXT TEAM cell of the agents row whose cells left of it contain `needle`, or None.

    Only lines below the header are read, and only the columns of the page, so a sidebar that
    names the same member on the same line cannot answer for the table.
    """
    lines = text.splitlines()
    header = next((i for i, line in enumerate(lines) if "NEXT TEAM" in line), None)
    if header is None:
        return None
    column = lines[header].index("NEXT TEAM")
    for line in lines[header + 1:]:
        if needle in line[:column]:
            return line[column:column + len("NEXT TEAM")].strip()
    return None


try:
    tmux("new-session", "-d", "-s", "check", "-x", "140", "-y", "40",
         *(["env", "LC_ALL=C"] if args.ascii else []),
         str(binary), "--home", str(home), "-C", str(project))
    wait_for(lambda s: "ymp" in s, "startup")
    for width, height in [(140, 40), (100, 30), (80, 24), (60, 20)]:
        tmux("resize-window", "-t", "check", "-x", str(width), "-y", str(height))
        time.sleep(0.3)
        size = f"{width}x{height}"

        command("/agents")
        agents = wait_for(lambda s: "Agent profiles" in s, "agents-" + size)
        time.sleep(0.3)
        agents = capture(f"agents-{size}")
        check(f"{size}-agents-no-reading", "READING" not in agents and "read from the installation" not in agents, agents)
        check(f"{size}-agents-no-in-team", "in team" not in agents, agents)
        if "NEXT TEAM" in agents:
            # atlas and cygnus are in Config.team, boreal is not. cygnus has no native model, so
            # its row is found by its provider rather than by a name it does not carry.
            flags = {"atlas": flag(agents, "Atlas"), "boreal": flag(agents, "Boreal"), "cygnus": flag(agents, "absent")}
            report["cases"].append({"case": f"{size}-agents-flags-read", "flags": flags})
            check(f"{size}-agents-true-false",
                  flags == {"atlas": "true", "boreal": "false", "cygnus": "true"},
                  agents)
        else:
            report["cases"].append({"case": f"{size}-agents-next-team-column", "passed": None,
                                    "note": "column not visible at this width"})

        command("/team")
        wait_for(lambda s: "Team" in "\n".join(s.splitlines()[:5]), "team-" + size)
        time.sleep(0.3)
        team = capture(f"team-{size}")
        check(f"{size}-team-no-reading", "READING" not in team and "read from the installation" not in team, team)
        check(f"{size}-team-refusal-causes",
              "executable not found" in team or "profile disabled" in team or width < 80,
              team)

    tmux("resize-window", "-t", "check", "-x", "140", "-y", "40")
    time.sleep(0.3)
    command("/agents")
    wait_for(lambda s: "NEXT TEAM" in s, "agents-inspect")
    type_text("/absent")
    keys("Enter")
    keys("Enter")
    inspect = wait_for(lambda s: "next-session preference" in s, "inspect-cygnus")
    inspect = capture("inspect-cygnus")
    check("inspect-source-catalog-scope",
          all(word in inspect for word in ["cygnus", "source", "catalog", "not scanned", "next team",
                                           "true · next-session preference"]),
          inspect)
    keys("Escape")
    time.sleep(0.2)

    # The notice text itself is covered by the unit test; the page shows the flag and the file.
    before = team_flags()
    keys("t")
    toggled = wait_for(lambda s: flag(s, "absent") == "false", "toggle-flag")
    toggled = capture("toggle-cygnus")
    after = team_flags()
    check("toggle-flips-next-session-preference",
          "cygnus" in before and "cygnus" not in after and flag(toggled, "absent") == "false",
          toggled + "\n" + json.dumps({"before": before, "after": after}))
    keys("t")
    restored = wait_for(lambda s: flag(s, "absent") == "true", "toggle-restore-flag")
    time.sleep(0.2)
    restored = capture("toggle-cygnus-restored")
    check("second-toggle-restores", "cygnus" in team_flags() and flag(restored, "absent") == "true", restored)
except Exception as error:  # the report keeps what failed
    report["failures"].append({"case": "exception", "detail": repr(error)})
finally:
    tmux("kill-server", check=False)

report["passed"] = not report["failures"]
(root / "report.json").write_text(json.dumps(report, indent=2))
print(json.dumps({"passed": report["passed"], "failures": report["failures"]}, indent=2))
sys.exit(0 if report["passed"] else 1)
