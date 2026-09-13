"""Check agent code/diff display in a real terminal with an isolated mock session."""

import argparse
import hashlib
import json
import pathlib
import re
import sqlite3
import subprocess
import time
import uuid


MESSAGE = '''OUTPUT_BEGIN_140

```rust
fn main() {
    let code_140 = "literal  spaces";
}
```

```diff
diff --git a/demo.rs b/demo.rs
--- a/demo.rs
+++ b/demo.rs
@@ -1 +1 @@
-let REMOVED_140 = 1;
+let ADDED_140 = 2;
```

- PROSE_LIST_140 is an ordinary list item.

OUTPUT_END_140'''


def colored_characters(ansi):
    """Return terminal text and SGR color attributes for each character."""
    text, styles = [], []
    foreground = background = None
    for part in re.split(r"(\x1b\[[0-9;]*m)", ansi):
        if part.startswith("\x1b["):
            values = [int(value) if value else 0 for value in part[2:-1].split(";")]
            index = 0
            while index < len(values):
                value = values[index]
                if value == 0:
                    foreground = background = None
                elif value == 39:
                    foreground = None
                elif value == 49:
                    background = None
                elif value in (38, 48) and index + 2 < len(values):
                    length = 4 if values[index + 1] == 2 else 2
                    color = tuple(values[index + 1:index + 1 + length])
                    if value == 38:
                        foreground = color
                    else:
                        background = color
                    index += length
                elif 30 <= value <= 37 or 90 <= value <= 97:
                    foreground = (value,)
                elif 40 <= value <= 47 or 100 <= value <= 107:
                    background = (value,)
                index += 1
        else:
            text.extend(part)
            styles.extend([(foreground, background)] * len(part))
    return "".join(text), styles


def modal_content(text, colors):
    """Restrict assertions to the visible read-only dialog, excluding chat behind it."""
    rows = text.splitlines(keepends=True)
    top = next(index for index, row in enumerate(rows) if "read only" in row)
    header = rows[top]
    left, right = header.find("┌"), header.rfind("┐")
    if left < 0:
        left = header.rfind("+", 0, header.index("read only"))
        right = header.rfind("+")
    assert 0 <= left < right, "cannot locate the read-only dialog"
    bottom = next(index for index in range(top + 1, len(rows))
                  if len(rows[index]) > left and rows[index][left] in ("└", "+"))
    position, content, styles = 0, [], []
    for index, row in enumerate(rows):
        if top < index < bottom:
            part = row[left + 1:right]
            content.append(part + "\n")
            styles.extend(colors[position + left + 1:position + right])
            styles.append((None, None))
        position += len(row)
    return "".join(content), styles


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary", type=pathlib.Path)
    parser.add_argument("--output", required=True, type=pathlib.Path)
    parser.add_argument("--baseline", action="store_true")
    parser.add_argument("--monochrome", action="store_true")
    args = parser.parse_args()
    binary, root = args.binary.resolve(), args.output.resolve()
    root.mkdir(parents=True, exist_ok=False)
    home, project = root / "metadata", root / "project"
    home.mkdir()
    project.mkdir()
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
    demo = subprocess.run(
        [str(binary), "--home", str(home), "-C", str(project), "demo"],
        capture_output=True, text=True, check=True, timeout=30,
    )
    session = re.search(r"^Session: (\S+)", demo.stdout, re.M).group(1)
    database = home / "state.sqlite"
    with sqlite3.connect(database) as db:
        db.execute(
            "INSERT INTO messages(session_id,author,kind,text,created_at) VALUES(?,?,?,?,?)",
            (session, "reader", "execute", MESSAGE, "2026-09-13T00:00:00Z"),
        )

    def messages_digest():
        with sqlite3.connect(database) as db:
            rows = db.execute("SELECT * FROM messages ORDER BY seq").fetchall()
        return hashlib.sha256(json.dumps(rows, ensure_ascii=False).encode()).hexdigest()

    before = messages_digest()
    report = {"binary": str(binary), "baseline": args.baseline, "monochrome": args.monochrome,
              "native_inference": False, "cases": []}
    try:
        for theme in ["ember", "catppuccin-latte"]:
            with sqlite3.connect(database) as db:
                db.execute("INSERT OR REPLACE INTO kv(key,value) VALUES(?,?)", (
                    "ui.prefs", json.dumps({"theme": theme, "details": True, "sidebar": False}),
                ))
            socket = "ymp-output-" + uuid.uuid4().hex[:10]

            def tmux(*values, check=True):
                return subprocess.run(["tmux", "-L", socket, *values], capture_output=True,
                                      text=True, check=check, timeout=5)

            def screen():
                return tmux("capture-pane", "-p", "-t", "check").stdout

            def keys(*values):
                for value in values:
                    tmux("send-keys", "-t", "check", value)
                    time.sleep(0.15)

            try:
                # The agent execution environment may set NO_COLOR. Color checks
                # need observable SGR output; monochrome behavior is a separate case.
                color_env = ["env", "NO_COLOR=1"] if args.monochrome else ["env", "-u", "NO_COLOR"]
                tmux("new-session", "-d", "-s", "check", "-x", "120", "-y", "50",
                     *color_env, "COLORTERM=truecolor",
                     str(binary), "--home", str(home), "-C", str(project), "resume", session)
                deadline = time.monotonic() + 5
                while time.monotonic() < deadline and "OUTPUT_END_140" not in screen():
                    time.sleep(0.1)
                chat = screen()
                (root / (theme + "-chat.txt")).write_text(chat)
                assert "OUTPUT_BEGIN_140" in chat and "OUTPUT_END_140" in chat, "Detailed chat hid the report"
                assert 'let code_140 = "literal  spaces";' in chat, "chat changed source whitespace"
                chat_ansi = tmux("capture-pane", "-p", "-e", "-t", "check").stdout
                (root / (theme + "-chat.ansi")).write_text(chat_ansi)
                chat_plain, chat_colors = colored_characters(chat_ansi)

                def chat_style(needle):
                    return chat_colors[chat_plain.index(needle)]

                chat_code_colored = chat_style("let code_140") != chat_style("literal  spaces")
                chat_diff_colored = chat_style("-let REMOVED_140") != chat_style("+let ADDED_140")
                assert chat_style("PROSE_LIST_140") == chat_style("OUTPUT_BEGIN_140"), "chat prose list acquired diff styling"
                assert chat_style("OUTPUT_END_140") == chat_style("OUTPUT_BEGIN_140"), "chat code styling leaked into prose"
                # Loading the session appends a non-message notice after our
                # injected report. Select the report before opening Inspect.
                # End enters follow mode; the first Up selects the final notice,
                # and the second selects our actual report.
                keys("Tab", "End", "Up", "Up", "Enter")
                # The first grammar load can outlast the deliberate key interval.
                deadline = time.monotonic() + 5
                while time.monotonic() < deadline and "read only" not in screen():
                    time.sleep(0.08)
                text = screen()
                (root / (theme + ".txt")).write_text(text)
                assert "read only" in text, "Inspect did not open the agent report"
                assert "OUTPUT_BEGIN_140" in text and "OUTPUT_END_140" in text, text
                assert 'let code_140 = "literal  spaces";' in text, "source whitespace changed"
                assert "-let REMOVED_140 = 1;" in text and "+let ADDED_140 = 2;" in text
                assert "PROSE_LIST_140" in text
                ansi = tmux("capture-pane", "-p", "-e", "-t", "check").stdout
                (root / (theme + ".ansi")).write_text(ansi)
                plain, colors = colored_characters(ansi)
                distinct_colors = len(set(colors))
                if args.monochrome:
                    assert all(pair == (None, None) for pair in colors), "NO_COLOR was ignored"
                else:
                    assert distinct_colors > 2, "terminal color capture is not observable"
                inspected, inspected_colors = modal_content(plain, colors)
                assert "OUTPUT_BEGIN_140" in inspected and "OUTPUT_END_140" in inspected, "Inspect selected a different timeline entry"

                def style(needle):
                    return inspected_colors[inspected.index(needle)]

                code_colored = style("let code_140") != style("literal  spaces")
                diff_colored = style("-let REMOVED_140") != style("+let ADDED_140")
                assert style("PROSE_LIST_140") == style("OUTPUT_BEGIN_140"), "Inspect prose list acquired diff styling"
                assert style("OUTPUT_END_140") == style("OUTPUT_BEGIN_140"), "Inspect code styling leaked into prose"
                case = {"theme": theme, "literal_text": True, "code_colored": code_colored,
                        "diff_colored": diff_colored, "prose_visible": True,
                        "chat_code_colored": chat_code_colored, "chat_diff_colored": chat_diff_colored,
                        "inspect_opened": True,
                        "prose_not_diff": True,
                        "distinct_color_pairs": distinct_colors}
                if args.monochrome:
                    assert all(pair == (None, None) for pair in chat_colors), "chat ignored NO_COLOR"
                    case["monochrome_markers_preserved"] = True
                elif args.baseline:
                    assert not (code_colored and diff_colored and chat_code_colored and chat_diff_colored), "baseline already has the full output behavior"
                    case["expected_missing_behavior"] = True
                else:
                    assert code_colored, "Rust tokens are not differentiated"
                    assert diff_colored, "diff additions/removals are not differentiated"
                    assert chat_code_colored, "chat Rust tokens are not differentiated"
                    assert chat_diff_colored, "chat diff additions/removals are not differentiated"
                report["cases"].append(case)
            finally:
                tmux("kill-server", check=False)
        assert messages_digest() == before, "rendering changed stored messages"
        report["stored_messages_unchanged"] = True
        report["passed"] = True
    except Exception as error:
        report["passed"] = False
        report["error"] = str(error)
        raise
    finally:
        (root / "report.json").write_text(json.dumps(report, indent=2) + "\n")
        print(json.dumps(report, indent=2))


if __name__ == "__main__":
    main()
