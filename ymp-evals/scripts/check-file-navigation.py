"""Exercise the files page of a built ymp in a real terminal, using only a scratch project.

Requires tmux. Captures screens and a JSON report. The metadata directory is created for the run,
no session is resumed, no agent is started and no provider is asked anything.
"""

import argparse
import hashlib
import json
import os
import pathlib
import re
import subprocess
import tempfile
import time
import uuid


def snapshot(root):
    """Every path beneath root with what it is, without following links."""
    seen = {}
    for directory, directories, files in os.walk(root, followlinks=False):
        for name in directories + files:
            path = pathlib.Path(directory, name)
            key = str(path.relative_to(root))
            if path.is_symlink():
                seen[key] = "link " + os.readlink(path)
            elif path.is_fifo():
                seen[key] = "fifo"
            elif path.is_dir():
                seen[key] = "directory"
            else:
                seen[key] = hashlib.sha256(path.read_bytes()).hexdigest()
    return seen


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary", type=pathlib.Path)
    parser.add_argument("--output", type=pathlib.Path)
    parser.add_argument("--ascii", action="store_true", help="Use ASCII semantic markers via LC_ALL=C")
    args = parser.parse_args()
    binary = args.binary.resolve()
    root = args.output or pathlib.Path(tempfile.mkdtemp(prefix="ymp139-terminal-"))
    root.mkdir(parents=True, exist_ok=True)
    home, project = root / "metadata", root / "project"
    home.mkdir()
    project.mkdir()
    (home / "config.toml").write_text('''version = 1
team = ["atlas"]
[[providers]]
id = "demo"
kind = "mock"
command = "internal"
[[agents]]
id = "atlas"
name = "Atlas"
provider = "demo"
''')
    (project / "src" / "nested").mkdir(parents=True)
    (project / "src" / "main.rs").write_text('fn main() {\n    println!("a  b");\n}\n')
    (project / "docs").mkdir()
    (project / "README.md").write_text("# Scratch project\n")
    (project / "same-a\nb.rs").write_text("NEWLINE-PAYLOAD\n")
    (project / "same-a b.rs").write_text("SPACE-PAYLOAD\n")
    (project / "binary.bin").write_bytes(b"ELF\x7f\x00\x01\x02")
    (project / "large.txt").write_text("line of text\n" * 40000)
    os.mkfifo(project / "pipe")
    os.symlink("src", project / "to-src")
    before = snapshot(project)

    socket = "ymp139-" + uuid.uuid4().hex[:12]
    # A NO_COLOR inherited from the calling environment is honoured by the application and would
    # leave no colour to observe, so the terminal runs without it and says that it did.
    report = {"binary": str(binary), "ascii": args.ascii, "native_inference": False,
              "inherited_no_color": "NO_COLOR" in os.environ, "cases": []}
    selection = ">" if args.ascii else "›"

    def tmux(*values, check=True):
        return subprocess.run(["tmux", "-L", socket, *values],
                              capture_output=True, text=True, check=check, timeout=5)

    def screen():
        return tmux("capture-pane", "-p", "-t", "check").stdout

    def capture(name):
        text = screen()
        (root / (name + ".txt")).write_text(text)
        return text

    def wait_for(predicate, label, seconds=5):
        deadline = time.monotonic() + seconds
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

    def selected(text, name):
        # A row starts with the marker. Matching anywhere would take `to-src -> src/` for `src/`
        # when the ASCII marker is `>`.
        return any(row.lstrip().startswith(f"{selection} {name}") for row in text.splitlines())

    def select(name, label):
        for _ in range(80):
            if selected(screen(), name):
                return
            keys("Down")
        capture("failure-select-" + label)
        raise AssertionError("never selected " + label)

    def passed(case):
        report["cases"].append({"case": case, "passed": True})

    def foreground(row, token):
        """The foreground a styled capture sets where `token` starts, or None for the default."""
        plain, colours, current = "", [], None
        for escape, char in re.findall(r"(\x1b\[[0-9;]*m)|(.)", row):
            if escape:
                for code in re.findall(r"\x1b\[([0-9;]*)m", escape):
                    if code in ("", "0", "39"):
                        current = None
                    elif code.startswith("38;"):
                        current = code
                    else:
                        found = re.search(r"(?:^|;)(38;[25];[0-9;]+)", code)
                        current = found.group(1) if found else current
            else:
                plain += char
                colours.append(current)
        return colours[plain.index(token)]

    def styled_row(capture, token):
        return next(row for row in capture.splitlines() if token in re.sub(r"\x1b\[[0-9;]*m", "", row))

    try:
        tmux("new-session", "-d", "-s", "check", "-x", "140", "-y", "45",
             "env", "-u", "NO_COLOR", "COLORTERM=truecolor", *(["LC_ALL=C"] if args.ascii else []),
             str(binary), "--home", str(home), "-C", str(project))
        wait_for(lambda s: "Describe a task" in s, "startup")

        command("/files")
        text = wait_for(lambda s: "Files[" in s and "README.md" in s, "files-open")
        capture("files-open")
        assert selected(text, "../"), "the list does not start on the parent row"
        assert str(project) in "\n".join(text.splitlines()[:5]), "the header does not name the directory"
        passed("opens-at-the-start-directory")

        select("src/", "src")
        keys("Enter")
        wait_for(lambda s: "nested/" in s and "main.rs" in s and "README.md" not in s, "descend")
        capture("files-descended")
        passed("enter-lists-the-directory")

        select("main.rs", "main")
        keys("Enter")
        text = wait_for(lambda s: 'println!("a  b");' in s and "Rust · 3 lines" in s, "source-preview")
        capture("files-source-preview")
        coloured = tmux("capture-pane", "-e", "-p", "-t", "check").stdout
        (root / "files-source-preview-styled.txt").write_text(coloured)
        keyword = foreground(styled_row(coloured, "fn main"), "fn main")
        call = foreground(styled_row(coloured, "println!"), "println!")
        assert keyword is not None, "the keyword has no colour of its own"
        assert keyword != call, "the keyword and the call are drawn in one colour"
        report["colours"] = {"keyword": keyword, "call": call}
        passed("source-preview-literal-and-highlighted")

        keys("Escape", "BSpace")
        wait_for(lambda s: selected(s, "src/") and "README.md" in s, "ascend")
        capture("files-ascended")
        passed("backspace-returns-to-the-row")

        for name, payload, other, label in [
            ("same-a\\nb.rs", "NEWLINE-PAYLOAD", "SPACE-PAYLOAD", "newline-name"),
            ("same-a b.rs", "SPACE-PAYLOAD", "NEWLINE-PAYLOAD", "space-name"),
        ]:
            select(name, label)
            keys("Enter")
            wait_for(lambda s: payload in s and other not in s, label)
            capture("files-" + label)
            keys("Escape")
        passed("names-that-read-alike-open-their-own-files")

        for name, sentence, label in [
            ("binary.bin", "Not shown: this looks like binary content", "binary"),
            ("pipe", "A FIFO is not opened", "fifo"),
            ("large.txt", "Only the first 256.0 kB of", "large"),
        ]:
            select(name, label)
            keys("Enter")
            wait_for(lambda s: sentence in s, label)
            capture("files-" + label)
            keys("Escape")
        passed("binary-fifo-and-large-files-are-described")

        select("to-src -> src/", "link")
        keys("Enter")
        wait_for(lambda s: "nested/" in s and "README.md" not in s, "link-descend")
        keys("BSpace")
        wait_for(lambda s: selected(s, "src/"), "link-ascend")
        passed("a-link-to-a-directory-is-listed-where-it-leads")

        type_text("/READ")
        wait_for(lambda s: "</READ>[1]" in s and "README.md" in s and "binary.bin" not in s, "filter")
        capture("files-filter")
        keys("Enter", "Escape")
        wait_for(lambda s: "binary.bin" in s and "README.md" in s, "filter-clear")
        passed("filter-and-clear")

        keys("BSpace")
        wait_for(lambda s: selected(s, project.name + "/"), "above-start")
        capture("files-above-start")
        keys("Enter")
        wait_for(lambda s: "README.md" in s, "back-to-start")
        passed("browsing-above-the-start-and-back")

        for width, height in [(80, 24), (60, 24)]:
            tmux("resize-window", "-t", "check", "-x", str(width), "-y", str(height))
            time.sleep(0.3)
            text = capture(f"files-{width}x{height}")
            assert "Files" in "\n".join(text.splitlines()[:5]), (width, text)
            assert "panic" not in text.lower(), (width, text)
        passed("narrow-terminals")

        assert snapshot(project) == before, "browsing changed the project"
        passed("project-unchanged")

        keys("C-c")
        wait_for(lambda s: "again to exit" in s, "first-ctrl-c")
        keys("C-c")
        deadline = time.monotonic() + 10
        while time.monotonic() < deadline and tmux("has-session", "-t", "check", check=False).returncode == 0:
            time.sleep(0.1)
        assert tmux("has-session", "-t", "check", check=False).returncode != 0, "a second Ctrl+C did not leave"
        passed("ctrl-c-twice-leaves")
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
