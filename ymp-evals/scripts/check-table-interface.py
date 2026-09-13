"""Exercise a built ymp in a real terminal using only an isolated mock session.

Requires tmux. Captures screens and a JSON report; never opens the user's metadata.
"""

import argparse
import hashlib
import json
import pathlib
import re
import sqlite3
import subprocess
import tempfile
import time
import uuid


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary", type=pathlib.Path)
    parser.add_argument("--output", type=pathlib.Path)
    args = parser.parse_args()
    binary = args.binary.resolve()
    root = args.output or pathlib.Path(tempfile.mkdtemp(prefix="ymp133-terminal-"))
    root.mkdir(parents=True, exist_ok=True)
    home, project = root / "metadata", root / "project"
    home.mkdir()
    project.mkdir()
    (home / "config.toml").write_text('''version = 1
team = ["atlas", "boreal", "cygnus"]
[[providers]]
id = "demo"
kind = "mock"
command = "internal"
[[agents]]
id = "atlas"
name = "Atlas"
provider = "demo"
[[agents]]
id = "boreal"
name = "Boreal"
provider = "demo"
[[agents]]
id = "cygnus"
name = "Cygnus"
provider = "demo"
''')
    demo = subprocess.run(
        [str(binary), "--home", str(home), "-C", str(project), "demo"],
        capture_output=True, text=True, timeout=30,
    )
    (root / "demo.log").write_text(demo.stdout + demo.stderr)
    assert demo.returncode == 0, "mock demo failed"
    session = re.search(r"^Session: (\S+)", demo.stdout, re.M).group(1)
    with sqlite3.connect(home / "state.sqlite") as db:
        db.execute(
            "INSERT INTO messages(session_id,author,kind,text,created_at) VALUES(?,?,?,?,?)",
            (session, "atlas", "execute", "\n\n".join(
                f"Terminal detail paragraph {i}." for i in range(1, 13)),
             "2026-09-13T00:00:00Z"),
        )
    files = {"a-large.txt": 1024, "m-small.txt": 3, "z-medium.txt": 80}
    for name, size in files.items():
        (project / name).write_bytes(b"x" * size)
    before = {p.name: hashlib.sha256(p.read_bytes()).hexdigest()
              for p in project.iterdir() if p.is_file()}
    socket = "ymp133-" + uuid.uuid4().hex[:12]
    report = {"binary": str(binary), "native_inference": False, "cases": []}

    def tmux(*args, check=True):
        return subprocess.run(["tmux", "-L", socket, *args],
                              capture_output=True, text=True, check=check, timeout=5)

    def screen():
        return tmux("capture-pane", "-p", "-t", "check").stdout

    def capture(name):
        text = screen()
        (root / (name + ".txt")).write_text(text)
        return text

    def wait_for(predicate, label):
        deadline = time.monotonic() + 5
        while time.monotonic() < deadline:
            text = screen()
            if predicate(text):
                return text
            time.sleep(0.08)
        capture("failure-" + label)
        raise AssertionError("terminal did not reach " + label)

    def keys(*values):
        tmux("send-keys", "-t", "check", *values)
        time.sleep(0.12)

    def type_text(text):
        tmux("send-keys", "-t", "check", "-l", text)
        time.sleep(0.12)

    def command(text):
        keys("Escape", "Escape", "Escape", "C-u")
        type_text(text)
        keys("Enter")

    def passed(case):
        report["cases"].append({"case": case, "passed": True})

    try:
        tmux("new-session", "-d", "-s", "check", "-x", "140", "-y", "45",
             str(binary), "--home", str(home), "-C", str(project), "resume", session)
        wait_for(lambda s: "ymp" in s and "SESSION" in s, "startup")
        text = capture("wide-chat")
        assert "NAVIGATE" not in text, "sidebar still contains NAVIGATE"
        passed("sidebar-without-navigation")
        assert "Terminal detail paragraph 12." not in text, "default report was not collapsed"
        keys("C-l")
        wait_for(lambda s: "Terminal detail paragraph 12." in s, "detailed-expansion")
        capture("detailed-chat")
        keys("C-l")
        wait_for(lambda s: "Terminal detail paragraph 12." not in s, "default-collapse")
        passed("detailed-expands-long-agent-report")

        command("/files")
        wait_for(lambda s: all(name in s for name in files), "files")
        text = capture("files-default")
        assert all(title in text for title in ("NAME", "SIZE")), "missing table headers"
        passed("files-table")

        # A substring matches names but does not accidentally match the size or file kind.
        type_text("/m-small")
        wait_for(lambda s: "m-small.txt" in s and "a-large.txt" not in s
                 and "z-medium.txt" not in s, "filter")
        capture("files-filter")
        keys("Enter", "Escape")
        wait_for(lambda s: all(name in s for name in files), "filter-clear")
        passed("filter-and-clear")

        type_text("/!m-small")
        wait_for(lambda s: "a-large.txt" in s and "z-medium.txt" in s
                 and "m-small.txt" not in s, "inverse-filter")
        capture("files-inverse-filter")
        keys("Enter", "Escape")
        wait_for(lambda s: all(name in s for name in files), "inverse-clear")
        passed("inverse-filter")

        type_text("S")
        text = capture("files-size-ascending")
        assert text.index("m-small.txt") < text.index("z-medium.txt") < text.index("a-large.txt"), text
        type_text("S")
        text = capture("files-size-descending")
        assert text.index("a-large.txt") < text.index("z-medium.txt") < text.index("m-small.txt"), text
        passed("numeric-sort-both-directions")

        type_text("/no-file-has-this-name")
        wait_for(lambda s: "Nothing" in s and all(name not in s for name in files), "empty-filter")
        capture("files-no-match")
        keys("Enter", "Escape")
        wait_for(lambda s: all(name in s for name in files), "empty-clear")
        passed("empty-filter-recovery")

        for width, height in [(140, 45), (80, 24), (60, 24)]:
            tmux("resize-window", "-t", "check", "-x", str(width), "-y", str(height))
            time.sleep(0.2)
            for page in ["/help", "/tasks", "/usage", "/team", "/files"]:
                command(page)
                text = capture(f"{width}x{height}-{page[1:]}")
                assert "ymp" in text, (page, width, "application screen missing")
                assert "NAVIGATE" not in text, (page, width, "navigation returned")
                assert "panic" not in text.lower(), (page, width, text)
            passed(f"pages-at-{width}x{height}")

        tmux("resize-window", "-t", "check", "-x", "120", "-y", "36")
        time.sleep(0.2)
        keys("C-t")
        text = capture("theme-sidebar-divider")
        assert all(len(row) > 89 and row[89] == "│"
                   for row in text.splitlines()[2:33]), "theme margin erased the sidebar divider"
        keys("Escape")
        passed("modal-margin-preserves-sidebar-divider")

        tmux("resize-window", "-t", "check", "-x", "40", "-y", "12")
        time.sleep(0.2)
        keys("C-p", "Down", "Down", "Down", "Down")
        text = capture("short-palette-selection")
        assert re.search(r"›\s+/tasks\b", text), "selected command is outside the visible palette"
        keys("Escape", "C-t", "Down", "Down", "Down", "Down")
        text = capture("short-theme-selection")
        assert re.search(r"›\s+Terminal\b", text), "selected theme is outside the visible chooser"
        keys("Escape")
        passed("short-surface-selection-stays-visible")

        tmux("resize-window", "-t", "check", "-x", "60", "-y", "24")
        time.sleep(0.2)
        keys("C-p")
        text = capture("narrow-palette")
        assert "Commands" in text and "Type to search commands" in text, text
        keys("Escape", "C-t")
        text = capture("narrow-themes")
        assert "Ember" in text and "Slate" in text, text
        keys("Escape", "Escape", "Escape")
        command("/agents")
        type_text("m")
        text = capture("narrow-model-prompt")
        assert "Model for" in text and "cancel" in text, text
        keys("Escape")
        command("/memory")
        keys("End")
        type_text("f")
        text = capture("narrow-memory-confirm")
        assert "Confirm" in text and "Retire memory entry" in text, text
        keys("Escape")
        passed("narrow-choice-prompt-and-confirm-surfaces")
        after = {p.name: hashlib.sha256(p.read_bytes()).hexdigest()
                 for p in project.iterdir() if p.is_file()}
        assert before == after, "browsing changed workspace files"
        passed("workspace-unchanged")
        command("/quit")
        report["passed"] = True
    except Exception as error:
        report["passed"] = False
        report["error"] = str(error)
        try:
            capture("failure")
        except Exception:
            pass
        raise
    finally:
        tmux("kill-server", check=False)
        (root / "report.json").write_text(json.dumps(report, indent=2) + "\n")
        print(json.dumps({**report, "evidence": str(root)}, indent=2))


if __name__ == "__main__":
    main()
