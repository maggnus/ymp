"""Measure popup geometry while scrolling, using a mock session in a real terminal."""
import argparse
import hashlib
import json
import pathlib
import re
import sqlite3
import subprocess
import time
import uuid


def rectangle(screen):
    rows = screen.splitlines()
    top = next(i for i, row in enumerate(rows) if "read only" in row)
    row = rows[top]
    left, right = row.find("┌"), row.rfind("┐")
    if left < 0:
        left, right = row.find("+"), row.rfind("+")
    assert 0 <= left < right, "popup border not observable"
    bottom = next(i for i in range(top + 1, len(rows))
                  if len(rows[i]) > left and rows[i][left] in ("└", "+"))
    return [left, top, right - left + 1, bottom - top + 1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary", type=pathlib.Path)
    parser.add_argument("--output", type=pathlib.Path, required=True)
    parser.add_argument("--baseline", action="store_true")
    parser.add_argument("--ascii", action="store_true")
    args = parser.parse_args()
    binary, root = args.binary.resolve(), args.output.resolve()
    root.mkdir(parents=True, exist_ok=False)
    home, project = root / "metadata", root / "project"
    home.mkdir(); project.mkdir()
    (home / "config.toml").write_text('''version = 1
team = ["reader"]
[[providers]]
id = "demo"
kind = "mock"
command = "internal"
[[agents]]
id = "reader"
name = "Reader"
provider = "demo"
''')
    demo = subprocess.run([str(binary), "--home", str(home), "-C", str(project), "demo"],
                          capture_output=True, text=True, timeout=30, check=True)
    session = re.search(r"^Session: (\S+)", demo.stdout, re.M).group(1)
    report_text = "\n".join(f"REPORT_ROW_{i:03}" for i in range(80))
    with sqlite3.connect(home / "state.sqlite") as db:
        db.execute("INSERT INTO messages(session_id,author,kind,text,created_at) VALUES(?,?,?,?,?)",
                   (session, "reader", "execute", report_text, "2026-09-13T00:00:00Z"))
    (project / "a-short.rs").write_text("fn short_marker() {}\n")
    (project / "z-long.rs").write_text("\n".join(f"// FILE_ROW_{i:03}" for i in range(100)))
    before = {p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in project.iterdir() if p.is_file()}
    socket = "ymp-popup-" + uuid.uuid4().hex[:10]
    report = {"binary": str(binary), "baseline": args.baseline, "ascii": args.ascii,
              "native_inference": False, "cases": []}

    def tmux(*values, check=True):
        return subprocess.run(["tmux", "-L", socket, *values], capture_output=True,
                              text=True, check=check, timeout=5)
    def screen():
        return tmux("capture-pane", "-p", "-t", "check").stdout
    def wait(needle):
        deadline = time.monotonic() + 6
        while time.monotonic() < deadline:
            value = screen()
            if needle in value:
                return value
            time.sleep(.08)
        raise AssertionError("terminal did not show " + needle)
    def keys(*values):
        for value in values:
            tmux("send-keys", "-t", "check", value); time.sleep(.12)
    def capture(name):
        value = screen(); (root / (name + ".txt")).write_text(value); return value
    def measure(name, last_marker=None):
        first = capture(name + "-home"); initial = rectangle(first)
        keys("End"); end = capture(name + "-end"); final = rectangle(end)
        if last_marker:
            assert last_marker in end, "last line cannot be reached"
        keys("Up"); up = rectangle(capture(name + "-up"))
        keys("Down", "Down"); down = rectangle(capture(name + "-down"))
        stable = initial == final == up == down
        report["cases"].append({"case": name, "initial": initial, "end": final,
                                "up": up, "down": down, "stable": stable})
        if not args.baseline:
            assert stable, f"{name} resized or moved during scrolling"
    def select(name):
        marker = ">" if args.ascii else "›"
        for _ in range(12):
            if any(row.lstrip().startswith(marker + " " + name) for row in screen().splitlines()):
                return
            keys("Down")
        raise AssertionError("file not selected: " + name)

    try:
        tmux("new-session", "-d", "-s", "check", "-x", "100", "-y", "32",
             "env", "NO_COLOR=1", *(["LC_ALL=C"] if args.ascii else []),
             str(binary), "--home", str(home), "-C", str(project), "resume", session)
        wait("Describe a task")
        keys("Tab", "End", "Up", "Up", "Enter"); wait("read only")
        measure("long-inspect", "REPORT_ROW_079")
        keys("Home")
        tmux("resize-window", "-t", "check", "-x", "60", "-y", "18"); time.sleep(.2)
        measure("resized-inspect", "REPORT_ROW_079")
        keys("Escape", "Escape", "Escape", "C-u")
        tmux("resize-window", "-t", "check", "-x", "100", "-y", "32"); time.sleep(.2)
        tmux("send-keys", "-t", "check", "-l", "/files"); keys("Enter"); wait("Files[")
        select("z-long.rs"); keys("Enter"); wait("read only")
        measure("long-file-preview", "FILE_ROW_099")
        keys("Escape"); select("a-short.rs"); keys("Enter"); wait("short_marker")
        measure("short-file-preview", "short_marker")
        keys("Escape", "d"); wait("read only"); measure("file-inspect")
        assert before == {p.name: hashlib.sha256(p.read_bytes()).hexdigest()
                          for p in project.iterdir() if p.is_file()}, "viewing changed project files"
        if args.baseline:
            assert any(not case["stable"] for case in report["cases"]), "baseline did not reproduce shrinking"
        report["project_unchanged"] = True
        report["passed"] = True
    except Exception as error:
        report["passed"] = False; report["error"] = str(error); raise
    finally:
        tmux("kill-server", check=False)
        (root / "report.json").write_text(json.dumps(report, indent=2) + "\n")
        print(json.dumps(report, indent=2))


if __name__ == "__main__":
    main()
