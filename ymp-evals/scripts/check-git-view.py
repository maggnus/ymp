"""Verify /git in a real terminal with an embedded-library fixture and no tools on app PATH."""
import argparse
import hashlib
import importlib.util
import json
import os
import pathlib
import subprocess
import time
import uuid


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary", type=pathlib.Path)
    parser.add_argument("--fixture", type=pathlib.Path, required=True)
    parser.add_argument("--output", type=pathlib.Path, required=True)
    parser.add_argument("--baseline", action="store_true")
    parser.add_argument("--selection-baseline", action="store_true",
                        help="stop after recording that the chosen branch row is not fully highlighted")
    parser.add_argument("--ascii", action="store_true")
    args = parser.parse_args()
    spec = importlib.util.spec_from_file_location("output_check", pathlib.Path(__file__).with_name("check-agent-output.py"))
    output_check = importlib.util.module_from_spec(spec); spec.loader.exec_module(output_check)
    binary, helper, root = args.binary.resolve(), args.fixture.resolve(), args.output.resolve()
    root.mkdir(parents=True, exist_ok=False)
    home, project, empty_path = root / "metadata", root / "project", root / "no-tools"
    home.mkdir(); empty_path.mkdir()
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
    def fixture(action):
        value = subprocess.run([str(helper), action, str(project)], capture_output=True,
                               text=True, timeout=10, check=True)
        return json.loads(value.stdout)
    fixture("init")
    socket = "ymp-git-" + uuid.uuid4().hex[:10]
    report = {"binary": str(binary), "baseline": args.baseline, "ascii": args.ascii,
              "application_path": str(empty_path), "fixture_uses_git_cli": False,
              "native_inference": False, "cases": []}
    def tmux(*values, check=True):
        return subprocess.run(["tmux", "-L", socket, *values], capture_output=True,
                              text=True, timeout=5, check=check)
    def screen():
        return tmux("capture-pane", "-p", "-t", "check").stdout
    def capture(label):
        value = screen(); (root / (label + ".txt")).write_text(value); return value
    def wait(predicate, label):
        deadline = time.monotonic() + 10
        while time.monotonic() < deadline:
            value = screen()
            if predicate(value):
                return value
            time.sleep(.1)
        capture("failure-" + label)
        raise AssertionError("terminal did not reach " + label)
    def keys(*values):
        for value in values:
            tmux("send-keys", "-t", "check", value); time.sleep(.12)
    def command(value):
        keys("Escape", "Escape", "Escape", "C-u")
        tmux("send-keys", "-t", "check", "-l", value); keys("Enter")
    def passed(name):
        report["cases"].append({"case": name, "passed": True})
    def mode(value):
        return value.upper() + " CHANGES"
    def chosen_row(label):
        """Backgrounds of the chosen chooser row, from its marker to the cell before the padding."""
        ansi = tmux("capture-pane", "-p", "-e", "-t", "check").stdout
        (root / (label + ".ansi")).write_text(ansi)
        plain, colors = output_check.colored_characters(ansi)
        offsets, start = [], 0
        for row in plain.split("\n"):
            offsets.append((start, row)); start += len(row) + 1
        top = next(i for i, (_, row) in enumerate(offsets) if "Switch branch" in row)
        left, right = offsets[top][1].find("┌"), offsets[top][1].rfind("┐")
        marker = ">" if args.ascii else "›"
        start, row = next((start, row) for start, row in offsets[top + 1:]
                          if row[left + 2:].startswith(marker + " "))
        padding = {colors[start + left + 1][1], colors[start + right - 1][1]}
        cells = [colors[start + column][1] for column in range(left + 2, right - 1)]
        return {"row": row[left + 2:right - 1], "cells": len(cells),
                "selection_backgrounds": sorted({str(cell) for cell in cells}),
                "padding_backgrounds": sorted(str(cell) for cell in padding),
                "uncovered": sum(cell != cells[0] for cell in cells),
                "padding_distinct": cells[0] not in padding}
    def switch_main(check_selection=False):
        keys("b")
        wait(lambda s: "feature" in s and "main" in s and "checked out here" in s, "branch-chooser")
        if check_selection and not args.ascii:
            selection = chosen_row("branch-chooser")
            report["branch_chooser_selection"] = selection
            if args.selection_baseline:
                assert selection["uncovered"] > 0, "baseline did not reproduce the selection gap"
                return True
            assert selection["uncovered"] == 0 and selection["padding_distinct"], "chosen row is not highlighted across its content"
            passed("chosen-branch-row-highlighted-across-content")
        keys("Home", "Down", "Enter")
        wait(lambda s: "Check out main" in s, "branch-confirmation")
        keys("y")
    try:
        color_env = ["env", "NO_COLOR=1"] if args.ascii else ["env", "-u", "NO_COLOR"]
        tmux("new-session", "-d", "-s", "check", "-x", "120", "-y", "36",
             *color_env, "PATH=" + str(empty_path), "COLORTERM=truecolor",
             *(["LC_ALL=C"] if args.ascii else []), str(binary),
             "--home", str(home), "-C", str(project))
        wait(lambda s: "Describe a task" in s, "startup")
        command("/git")
        if args.baseline:
            wait(lambda s: "Unknown command /git" in s, "missing-git-command")
            capture("before-git-command"); passed("expected-missing-git-command"); report["passed"] = True
            return
        wait(lambda s: mode("uncommitted") in s and "new.rs" in s, "uncommitted")
        capture("uncommitted"); passed("uncommitted-without-git-or-shell-on-path")
        keys("Home", "Enter")
        wait(lambda s: "read only" in s and "+fn WORKING_143" in s, "working-diff")
        value = capture("working-diff")
        assert "-fn COMMITTED_143" in value
        ansi = tmux("capture-pane", "-p", "-e", "-t", "check").stdout
        (root / "working-diff.ansi").write_text(ansi)
        if not args.ascii:
            plain, colors = output_check.colored_characters(ansi)
            assert colors[plain.index("+fn WORKING_143")] != colors[plain.index("-fn COMMITTED_143")], "diff roles lack observable colours"
        passed("actual-working-diff-with-shared-colours-or-markers")
        keys("Escape", "m")
        wait(lambda s: mode("committed") in s and "new.rs" not in s, "committed")
        capture("committed"); keys("Home", "Enter")
        wait(lambda s: "read only" in s and "+fn COMMITTED_143" in s, "committed-diff")
        value = capture("committed-diff"); assert "-fn BASE_143" in value and "WORKING_143" not in value
        passed("committed-compares-real-base-and-head")
        keys("Escape", "m"); wait(lambda s: mode("uncommitted") in s, "return-working")
        keys("End"); fixture("live")
        wait(lambda s: "a-live.rs" in s, "live-update")
        keys("Enter"); wait(lambda s: "read only" in s and "UNTRACKED_143" in s, "selection-retained")
        capture("selected-new-file-after-update"); passed("live-refresh-retains-selected-file")
        keys("Escape")
        if switch_main(check_selection=True):
            passed("expected-selection-gap"); report["passed"] = True
            return
        wait(lambda s: "uncommitted changes" in s.lower(), "dirty-switch-refused")
        assert fixture("head")["branch"] == "feature"
        capture("dirty-switch-refused"); passed("dirty-branch-switch-does-not-discard")
        fixture("clean")
        wait(lambda s: mode("committed") in s and "new.rs" not in s, "automatic-clean-mode")
        passed("clean-state-restores-automatic-comparison")
        switch_main()
        wait(lambda s: "No changes to display" in s, "clean-main-empty")
        assert fixture("head")["branch"] == "main"
        capture("clean-main-empty"); passed("explicit-clean-checkout-and-empty-state")
        keys("w"); wait(lambda s: "linked-worktree" in s, "worktree-chooser")
        keys("Home", "Enter")
        wait(lambda s: "linked.rs" in s, "other-worktree")
        capture("other-worktree"); passed("existing-worktree-selection")
        command("/files")
        value = wait(lambda s: "Files[" in s and "code.rs" in s, "files-original-directory")
        assert "linked.rs" not in value
        capture("files-original-directory"); passed("git-selection-keeps-files-and-execution-directory")
        assert not any(empty_path.iterdir())
        report["passed"] = True
    except Exception as error:
        report["passed"] = False; report["error"] = str(error); raise
    finally:
        tmux("kill-server", check=False)
        (root / "report.json").write_text(json.dumps(report, indent=2) + "\n")
        print(json.dumps(report, indent=2))


if __name__ == "__main__":
    main()
