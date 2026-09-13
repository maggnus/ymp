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
import tomllib
import uuid


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary", type=pathlib.Path)
    parser.add_argument("--output", type=pathlib.Path)
    parser.add_argument("--ascii", action="store_true", help="Use ASCII semantic markers via LC_ALL=C")
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
        captured = json.loads(db.execute("SELECT data FROM sessions WHERE id=?", (session,)).fetchone()[0])
        shared_member = captured["team"][0]["name"]
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
    report = {"binary": str(binary), "ascii": args.ascii, "native_inference": False, "cases": []}
    selection = ">" if args.ascii else "›"
    descending = "v" if args.ascii else "↓"
    vertical = "|" if args.ascii else "│"

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
        # Separate deliberate presses: adjacent Escape bytes can encode Alt+Escape.
        for value in values:
            tmux("send-keys", "-t", "check", value)
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

    def sidebar_text(text, width):
        side = 34 if width >= 140 else 30 if width >= 100 else 26 if width >= 72 else 0
        return "\n".join(row[width - side:] for row in text.splitlines()[2:-3]) if side else ""

    try:
        tmux("new-session", "-d", "-s", "check", "-x", "140", "-y", "45",
             *(["env", "LC_ALL=C"] if args.ascii else []),
             str(binary), "--home", str(home), "-C", str(project), "resume", session)
        wait_for(lambda s: "ymp" in s and "SESSION" in s, "startup")
        text = capture("wide-chat")
        assert "NAVIGATE" not in sidebar_text(text, 140), "sidebar still contains NAVIGATE"
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
        assert "Files" in "\n".join(text.splitlines()[:5]), "missing Files page header"
        passed("files-widget")

        type_text("/m-")
        tmux("set-buffer", "-b", "ymp-check", "small")
        tmux("paste-buffer", "-p", "-b", "ymp-check", "-t", "check")
        wait_for(lambda s: "m-small.txt" in s and "a-large.txt" not in s
                 and "z-medium.txt" not in s, "pasted-filter")
        text = capture("files-pasted-filter")
        assert "Describe a task" in text.splitlines()[-2], "filter paste leaked into the composer"
        keys("Enter", "Escape")
        wait_for(lambda s: all(name in s for name in files), "pasted-filter-clear")
        passed("paste-stays-in-filter")

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

        # /files uses the explorer library's directory/name order. Column sorting
        # remains on the application's record tables, not this native widget.
        text = capture("files-native-order")
        assert text.index("a-large.txt") < text.index("m-small.txt") < text.index("z-medium.txt"), text
        passed("file-widget-native-order")

        type_text("/no-file-has-this-name")
        wait_for(lambda s: "Nothing" in s and all(name not in s for name in files), "empty-filter")
        capture("files-no-match")
        keys("Enter", "Escape")
        wait_for(lambda s: all(name in s for name in files), "empty-clear")
        passed("empty-filter-recovery")

        type_text("/m-small")
        keys("Enter")
        type_text("d")
        text = capture("filtered-row-inspect")
        assert "read only" in text and "m-small.txt" in text, "Inspect did not open the filtered row"
        keys("Escape", "Escape")
        wait_for(lambda s: all(name in s for name in files), "inspect-return")
        passed("inspect-filtered-row-and-return")

        pages = {
            "/help": "Help", "/tasks": "Tasks", "/usage": "Token usage",
            "/sessions": "Sessions", "/files": "Files", "/diff": "Changed files",
            "/checks": "Recorded checks", "/assignments": "Assignments",
            "/decisions": "Decisions", "/team": "Team", "/agents": "Agent profiles",
            "/providers": "Providers", "/memory": "Memory", "/reputation": "Reputation",
            "/limits": "Limits",
        }
        for width, height in [(140, 45), (80, 24), (60, 24)]:
            tmux("resize-window", "-t", "check", "-x", str(width), "-y", str(height))
            time.sleep(0.2)
            visible_pages = pages if width != 80 else {
                key: pages[key] for key in ["/help", "/tasks", "/usage", "/team", "/files"]
            }
            for page, title in visible_pages.items():
                command(page)
                text = capture(f"{width}x{height}-{page[1:]}")
                assert "ymp" in text, (page, width, "application screen missing")
                assert title in "\n".join(text.splitlines()[:5]), (page, width, "wrong page", text)
                assert "NAVIGATE" not in sidebar_text(text, width), (page, width, "navigation returned")
                assert "panic" not in text.lower(), (page, width, text)
            passed(f"pages-at-{width}x{height}")

        tmux("resize-window", "-t", "check", "-x", "140", "-y", "45")
        time.sleep(0.2)
        command("/agents")
        type_text("N")  # ENABLED; R is reserved, and READING uses E.
        type_text("/Cygnus")
        keys("Enter", "Escape")

        def enabled_profiles():
            config = tomllib.loads((home / "config.toml").read_text())
            return {agent["id"]: agent.get("enabled", True) for agent in config["agents"]}

        original_enabled = enabled_profiles()
        keys("Space")
        first_toggle = enabled_profiles()
        assert not first_toggle["cygnus"], "the first edit did not target the selected profile"
        keys("Space")
        capture("sorted-profile-edited-twice")
        assert enabled_profiles() == original_enabled, "a rebuild moved the selection and edited a different profile"
        passed("sorted-profile-edit-retains-record")

        command("/team")
        type_text("/" + shared_member)
        keys("Enter")
        keys("End")
        type_text("M")
        type_text("M")
        text = capture("team-pool-sort-twice")
        lines = text.splitlines()
        pool_line = next(i for i, row in enumerate(lines) if "AVAILABLE ON THIS MACHINE" in row)
        selected = next(i for i, row in enumerate(lines[2:-3], 2) if row.lstrip().startswith(selection))
        assert selected > pool_line, "sorting a pool row jumped to a member row with the same ID"
        assert f"MODEL {descending}" in "\n".join(lines[pool_line:]), "pool sort did not cycle to descending"
        passed("team-selection-keeps-table-identity")

        tmux("resize-window", "-t", "check", "-x", "120", "-y", "36")
        time.sleep(0.2)
        keys("C-t")
        text = capture("theme-sidebar-divider")
        assert all(len(row) > 89 and row[89] == vertical
                   for row in text.splitlines()[2:33]), "theme margin erased the sidebar divider"
        keys("Escape")
        passed("modal-margin-preserves-sidebar-divider")

        tmux("resize-window", "-t", "check", "-x", "40", "-y", "12")
        time.sleep(0.2)
        keys("C-p", "Down", "Down", "Down", "Down")
        text = capture("short-palette-selection")
        assert re.search(re.escape(selection) + r"\s+/tasks\b", text), "selected command is outside the visible palette"
        keys("Escape")
        tmux("resize-window", "-t", "check", "-x", "80", "-y", "10")
        time.sleep(0.2)
        keys("C-t", "End")
        text = capture("short-theme-selection")
        assert re.search(re.escape(selection) + r"\s+Midnight Commander\b", text), "selected theme is outside the visible chooser"
        keys("Escape")
        passed("short-surface-selection-stays-visible")

        tmux("resize-window", "-t", "check", "-x", "40", "-y", "12")
        time.sleep(0.2)
        keys("C-t", "Home")
        names = [
            "Ember", "Slate", "Dracula", "One Dark Pro", "Nord", "Catppuccin Mocha",
            "Catppuccin Latte", "Gruvbox Dark", "Gruvbox Light", "Tokyo Night",
            "Solarized Dark", "Solarized Light", "Monokai Pro", "Rosé Pine", "Kanagawa",
            "Everforest", "Cyberpunk", "Midnight Commander",
        ]
        for index, name in enumerate(names):
            text = capture(f"theme-{index:02d}-40x12")
            assert re.search(re.escape(selection) + r"\s+" + re.escape(name) + r"\b", text), (
                name, "selected theme is clipped in a narrow terminal", text,
            )
            if index + 1 < len(names):
                keys("Down")
        keys("Escape")
        passed("all-eighteen-themes-reachable-at-40x12")

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
