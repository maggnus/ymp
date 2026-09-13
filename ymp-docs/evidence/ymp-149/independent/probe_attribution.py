"""YMP-149 independent probe: preference wording versus captured-session attribution.

Usage: probe_attribution.py BINARY OUTPUT_DIR [--ascii]

Mock providers only. A provider whose executable does not exist is configured but never run.
Cases:
  D   `doctor` without --probe labels health `available` / `unavailable`, never `installed`.
  N   Without a session: sidebar `preferred · enabled` lists only enabled preferred agents;
      /team heading `PREFERRED AGENTS, ENABLED`; /agents PREFERRED equals Config.team membership,
      including a disabled preferred profile.
  S   After `ymp demo`, `resume`: sidebar `this session` and /team `MEMBERS OF THIS SESSION` list
      the captured members. `t` on /agents removes a captured member's preference in config.toml;
      the captured sidebar and /team membership still list it.
"""
import json
import pathlib
import re
import subprocess
import sys
import time
import tomllib
import uuid

sys.dont_write_bytecode = True
binary = pathlib.Path(sys.argv[1]).resolve()
root = pathlib.Path(sys.argv[2]).resolve()
ascii_mode = "--ascii" in sys.argv[3:]
root.mkdir(parents=True, exist_ok=False)
home, project = root / "metadata", root / "project"
home.mkdir()
project.mkdir()
(home / "config.toml").write_text('''version = 1
team = ["atlas", "boreal"]
[[providers]]
id = "demo"
kind = "mock"
command = "internal"
[[providers]]
id = "absent"
kind = "acp"
command = "ymp149-verify-no-such-executable"
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
[[agents]]
id = "dorado"
name = "Dorado"
provider = "demo"
''')
report = {"binary": str(binary), "ascii": ascii_mode, "native_inference": False,
          "cases": [], "failures": []}
socket = "ymp149-verify-" + uuid.uuid4().hex[:8]
SIDEBAR = 34


def check(case, condition, detail=""):
    report["cases"].append({"case": case, "passed": bool(condition)})
    if not condition:
        report["failures"].append({"case": case, "detail": str(detail)[-2500:]})


def cli(*values):
    return subprocess.run([str(binary), "--home", str(home), "-C", str(project), *values],
                          capture_output=True, text=True, timeout=90)


def tmux(*values, check=True):
    return subprocess.run(["tmux", "-L", socket, *values], capture_output=True, text=True,
                          check=check, timeout=5)


def screen():
    return tmux("capture-pane", "-p", "-t", "check").stdout


def capture(name):
    text = screen()
    (root / (name + ".txt")).write_text(text)
    return text


def wait_for(predicate, label):
    deadline = time.monotonic() + 8
    while time.monotonic() < deadline:
        text = screen()
        if predicate(text):
            return text
        time.sleep(0.1)
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


def sidebar(text):
    """The sidebar column: what follows the last vertical rule on each line (Unicode or ASCII)."""
    rows = []
    for line in text.splitlines():
        cut = max(line.rfind("│"), line.rfind("|"))
        rows.append(line[cut + 1:] if cut >= 0 else "")
    return "\n".join(rows)


def team_section(text):
    """Sidebar lines from the TEAM title to the next section title."""
    lines = sidebar(text).splitlines()
    start = next((i for i, line in enumerate(lines) if line.strip().startswith("TEAM")), None)
    if start is None:
        return ""
    section = [lines[start]]
    for line in lines[start + 1:]:
        if re.match(r"^\s*(SESSION|TOKENS|TASKS)\b", line):
            break
        section.append(line)
    return "\n".join(section)


def flag(text, needle):
    lines = text.splitlines()
    header = next((i for i, line in enumerate(lines) if "PREFERRED" in line and "AGENT" in line), None)
    if header is None:
        return None
    column = lines[header].index("PREFERRED")
    for line in lines[header + 1:]:
        if needle in line[:column]:
            return line[column:column + len("PREFERRED")].strip()
    return None


def team_config():
    return tomllib.loads((home / "config.toml").read_text())["team"]


def launch(*values):
    env = ["env", *(["LC_ALL=C"] if ascii_mode else [])]
    tmux("new-session", "-d", "-s", "check", "-x", "140", "-y", "40", *env,
         str(binary), "--home", str(home), "-C", str(project), *values)


def leave():
    keys("Escape", "Escape", "Escape", "C-c", "C-c")
    time.sleep(0.5)
    tmux("kill-server", check=False)


try:
    doctor = cli("doctor")
    (root / "doctor.txt").write_text(doctor.stdout + "\n--- stderr\n" + doctor.stderr)
    health = [line for line in doctor.stdout.splitlines() if re.match(r"^(demo|absent)\s", line)]
    check("D1 doctor exit status recorded", True, doctor.returncode)
    report["doctor_exit"] = doctor.returncode
    check("D2 mock provider available", any(re.match(r"^demo\s+available\b", l) for l in health), health)
    check("D3 missing executable unavailable", any(re.match(r"^absent\s+unavailable\b", l) for l in health), health)
    check("D4 no installed claim", not any(re.search(r"\binstalled\b", l) for l in health), health)

    launch()
    wait_for(lambda s: "Describe a task" in s, "startup")
    text = capture("n-chat")
    team = team_section(text)
    report["no_session_sidebar_team"] = team
    check("N1 sidebar scope preferred · enabled", "preferred · enabled" in team, team)
    check("N2 sidebar lists enabled preferred atlas", "Atlas" in team or "atlas" in team, team)
    check("N3 sidebar omits disabled preferred boreal", "Boreal" not in team and "boreal" not in team, team)
    check("N4 sidebar omits non-preferred dorado", "Dorado" not in team and "dorado" not in team, team)
    command("/team")
    text = wait_for(lambda s: "PREFERRED AGENTS, ENABLED" in s, "no-session-team")
    capture("n-team")
    check("N5 team heading and subtitle", "preferred and enabled" in text, text)
    check("N6 team hint names the preference", "toggle preferred" in text, text)
    check("N7 no next-run membership heading", "next run" not in text.lower().replace("turns next run", ""), text)
    command("/agents")
    text = wait_for(lambda s: "PREFERRED" in s, "no-session-agents")
    capture("n-agents")
    # Cygnus's provider executable is missing, so no native identity names it: its row is labelled
    # `unknown model`, which must not claim the agent is unavailable.
    flags = {name: flag(text, name) for name in ("Atlas", "Boreal", "unknown model", "Dorado")}
    report["no_session_flags"] = flags
    check("N8 PREFERRED equals Config.team", flags == {"Atlas": "true", "Boreal": "true",
                                                      "unknown model": "false", "Dorado": "false"}, flags)
    check("N10 missing identity row does not claim unavailability",
          not re.search(r"unknown model\s*·\s*unavailable", text), text)
    check("N9 no READING or provenance in rows", "READING" not in text and "read from" not in text, text)
    leave()

    demo = cli("demo")
    (root / "demo.txt").write_text(demo.stdout + "\n--- stderr\n" + demo.stderr)
    found = re.search(r"^Session: (\S+)", demo.stdout, re.M)
    check("S0 demo created a session", demo.returncode == 0 and found, demo.stdout[-800:] + demo.stderr[-800:])
    session = found.group(1)
    report["team_after_demo"] = team_config()

    launch("resume", session)
    text = wait_for(lambda s: "this session" in s, "resumed")
    capture("s-chat")
    team = team_section(text)
    report["captured_sidebar_team"] = team
    captured = [name for name in ("Atlas", "Boreal", "Cygnus", "Dorado") if name in team]
    report["captured_names"] = captured
    check("S1 captured sidebar scope", "this session" in team and "preferred" not in team, team)
    command("/team")
    text = wait_for(lambda s: "MEMBERS OF THIS SESSION" in s, "captured-team")
    capture("s-team")
    check("S2 captured team heading", "PREFERRED AGENTS" not in text, text)
    target = "Atlas" if "Atlas" in captured else (captured[0] if captured else "Atlas")
    report["toggled"] = target
    command("/agents")
    wait_for(lambda s: "PREFERRED" in s, "captured-agents")
    type_text("/" + target)
    keys("Enter")
    before = team_config()
    keys("t")
    text = wait_for(lambda s: target.lower() not in team_config() or True, "toggle")
    time.sleep(0.4)
    text = capture("s-agents-toggled")
    after = team_config()
    report["team_before_toggle"], report["team_after_toggle"] = before, after
    expected = "false" if target.lower() in before else "true"
    check("S3 t edits Config.team", (target.lower() in before) != (target.lower() in after), [before, after])
    check("S4 PREFERRED cell follows the edit", flag(text, target) == expected, flag(text, target))
    team = team_section(text)
    check("S5 captured sidebar unchanged after the edit",
          "this session" in team and all(name in team for name in captured), team)
    command("/team")
    text = wait_for(lambda s: "MEMBERS OF THIS SESSION" in s, "captured-team-after")
    capture("s-team-after")
    check("S6 captured member still listed on /team", target in text, text)
    leave()
    report["passed"] = not report["failures"]
except Exception as error:
    report["passed"] = False
    report["error"] = repr(error)
finally:
    tmux("kill-server", check=False)
    text = json.dumps(report, indent=2).replace(str(root), "<output>")
    (root / "report.json").write_text(text + "\n")
    print(text)
