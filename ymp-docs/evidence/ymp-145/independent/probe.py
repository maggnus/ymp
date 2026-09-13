"""Independent YMP-145 probe: chosen-row SGR attributes and popup rects in a real terminal (mock provider).

Usage: probe.py BINARY OUTPUT_DIR {candidate|control}
  candidate: every assertion below must hold.
  control:   observer sensitivity on a build with the known theme-chooser gap; the dark and light
             theme-chooser rows must be reported as NOT fully covered.

Assertions (colour runs start with NO_COLOR removed from the environment):
  C1 exactly one row inside the popup starts with the selection marker;
  C2 the parsed background of that row's first content cell is a real colour (not default);
  C3 every content cell between the padding cells has that same parsed background, except, for the
     theme chooser only, blank cells within the last 12 columns (the colour chips);
  C4 both padding cells have a parsed background different from the selection background;
  S1 (small terminals) the popup rect is identical on every captured step and C1/C3/C4 hold on each.
Monochrome run (NO_COLOR=1):
  M1 exactly one marker row; M2 no cell anywhere in the capture has a parsed background;
  M3 every non-blank cell of the chosen row after the marker is bold;
  M4 at least one other popup row has a non-blank cell that is not bold (bold distinguishes it).
"""
import json, os, pathlib, re, subprocess, sys, time, uuid

sys.dont_write_bytecode = True
binary = pathlib.Path(sys.argv[1]).resolve()
root = pathlib.Path(sys.argv[2]).resolve()
mode = sys.argv[3]
assert mode in ("candidate", "control")
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
report = {"binary": str(binary), "mode": mode, "native_inference": False, "cases": [], "failures": []}
socket = "ymp145-probe-" + uuid.uuid4().hex[:8]


def sgr_cells(ansi):
    """Terminal text and parsed (fg, bg, bold) per character."""
    text, attrs = [], []
    fg = bg = None
    bold = False
    for part in re.split(r"(\x1b\[[0-9;:]*m)", ansi):
        if part.startswith("\x1b["):
            values = [int(v) if v else 0 for v in re.split(r"[;:]", part[2:-1])]
            i = 0
            while i < len(values):
                v = values[i]
                if v == 0:
                    fg = bg = None; bold = False
                elif v == 1:
                    bold = True
                elif v == 22:
                    bold = False
                elif v == 39:
                    fg = None
                elif v == 49:
                    bg = None
                elif v in (38, 48) and i + 1 < len(values):
                    n = 4 if values[i + 1] == 2 else 2
                    colour = tuple(values[i + 1:i + 1 + n])
                    if v == 38:
                        fg = colour
                    else:
                        bg = colour
                    i += n
                elif 30 <= v <= 37 or 90 <= v <= 97:
                    fg = (v,)
                elif 40 <= v <= 47 or 100 <= v <= 107:
                    bg = (v,)
                i += 1
        else:
            text.extend(part)
            attrs.extend([(fg, bg, bold)] * len(part))
    return "".join(text), attrs


def tmux(*v, check=True):
    return subprocess.run(["tmux", "-L", socket, *v], capture_output=True, text=True, check=check, timeout=5)
def plain():
    return tmux("capture-pane", "-p", "-t", "s").stdout
def ansi():
    return tmux("capture-pane", "-p", "-e", "-t", "s").stdout
def keys(*v, pause=.15):
    for x in v:
        tmux("send-keys", "-t", "s", x); time.sleep(pause)
def wait(needle, label):
    end = time.monotonic() + 8
    while time.monotonic() < end:
        s = plain()
        if needle in s:
            return s
        time.sleep(.1)
    (root / f"failure-{label}.txt").write_text(plain())
    raise AssertionError(f"did not reach {label}")


def popup(rows, heading):
    top = next(i for i, r in enumerate(rows) if heading in r and ("┌" in r or "+" in r))
    r = rows[top]
    left, right = r.find("┌"), r.rfind("┐")
    if left < 0:
        left, right = r.find("+"), r.rfind("+")
    bottom = next(i for i in range(top + 1, len(rows)) if len(rows[i]) > left and rows[i][left] in "└+")
    return top, left, right, bottom


def observe(label, heading, marker="›", chips=False):
    a = ansi(); (root / f"{label}.ansi").write_text(a)
    text, attrs = sgr_cells(a)
    rows, offs, start = text.split("\n"), [], 0
    for r in rows:
        offs.append(start); start += len(r) + 1
    top, left, right, bottom = popup(rows, heading)
    # A command list row is the marker then a command name; the palette's search line also starts
    # with the marker ("› Type to search commands") and is not a list row.
    prefix = marker + " /" if heading == "Commands" else marker + " "
    hits = [y for y in range(top + 1, bottom) if rows[y][left + 2:].startswith(prefix)]
    res = {"label": label, "rect": [left, top, right - left + 1, bottom - top + 1], "marker_rows": len(hits)}
    if len(hits) != 1:
        return res, None
    y = hits[0]; base = offs[y]
    cells = [(x, rows[y][x], attrs[base + x]) for x in range(left + 2, right - 1)]
    selection_bg = cells[0][2][1]
    uncovered = [x for x, ch, at in cells
                 if at[1] != selection_bg and not (chips and ch == " " and x + 12 >= right - 1)]
    padding = [attrs[base + left + 1][1], attrs[base + right - 1][1]]
    res.update({"row": rows[y][left + 2:right - 1], "cells": len(cells), "selection_bg": str(selection_bg),
                "uncovered": len(uncovered), "padding_bg": [str(p) for p in padding],
                "C2": selection_bg is not None, "C3": not uncovered,
                "C4": all(p != selection_bg for p in padding)})
    return res, (rows, attrs, offs, top, left, right, bottom, y)


def record(res, passed):
    res["passed"] = bool(passed)
    report["cases"].append(res)
    if not passed:
        report["failures"].append(res["label"])


def colour_case(label, heading, chips=False):
    res, ctx = observe(label, heading, chips=chips)
    record(res, ctx is not None and res["C2"] and res["C3"] and res["C4"])
    return res


def start(width, height, colour):
    env = ["env", "-u", "NO_COLOR"] if colour else ["env", "NO_COLOR=1"]
    tmux("new-session", "-d", "-s", "s", "-x", str(width), "-y", str(height), *env,
         "PYTHONDONTWRITEBYTECODE=1", str(binary), "--home", str(home), "-C", str(project))
    time.sleep(1.5)


def theme_rows():
    """Dark (default) theme chooser row, then the first light theme's row; Enter keeps it."""
    keys("C-t"); wait("Colour theme", "themes")
    colour_case("dark-theme-chooser-row", "Colour theme", chips=True)
    light = None
    for step in range(20):
        res, _ = observe(f"theme-step-{step:02}", "Colour theme", chips=True)
        if " light" in res.get("row", ""):
            light = True
            colour_case("light-theme-chooser-row", "Colour theme", chips=True)
            break
        keys("Down")
    assert light, "no light theme reached"
    keys("Enter"); time.sleep(.3)


try:
    start(100, 32, colour=True)
    wait("Describe", "welcome")
    if mode == "control":
        theme_rows()
        # Sensitivity: the observer must report the known gap on this build.
        control = [c for c in report["cases"] if c["label"].endswith("theme-chooser-row")]
        report["control_detects_gap"] = len(control) == 2 and all(c.get("uncovered", 0) > 0 for c in control)
        report["passed"] = report["control_detects_gap"]
    else:
        keys("C-p"); wait("Commands", "palette")
        colour_case("dark-palette-first", "Commands")
        keys("Down", "Down", "Down")
        colour_case("dark-palette-moved", "Commands")
        keys("Escape")
        theme_rows()
        keys("C-p"); wait("Commands", "light-palette")
        colour_case("light-palette-first", "Commands")
        keys("Down", "Down")
        colour_case("light-palette-moved", "Commands")
        keys("Escape")
        tmux("send-keys", "-t", "s", "-l", "/"); time.sleep(.4)
        colour_case("light-completion-first", "Commands")
        keys("Down", "Down")
        colour_case("light-completion-moved", "Commands")
        keys("C-u", "Escape")
        for w, h in ((60, 9), (60, 8), (44, 10)):
            tmux("resize-window", "-t", "s", "-x", str(w), "-y", str(h)); time.sleep(.4)
            keys("Escape", "C-p"); wait("Commands", f"palette-{w}x{h}")
            rects, bad = [], []
            sequence = ["Down"] * 45 + ["Up"] * 5
            for step, key in enumerate(sequence):
                res, ctx = observe(f"small-{w}x{h}-{step:02}", "Commands")
                rects.append(tuple(res["rect"]))
                if ctx is None or not (res["C2"] and res["C3"] and res["C4"]):
                    bad.append(step)
                keys(key, pause=.1)
            record({"label": f"small-palette-{w}x{h}", "distinct_rects": sorted(set(rects)),
                    "steps": len(sequence), "bad_steps": bad}, len(set(rects)) == 1 and not bad)
            keys("Escape")
        tmux("kill-server", check=False)
        start(100, 32, colour=False)
        wait("Describe", "mono-welcome")
        keys("C-p"); wait("Commands", "mono-palette")
        res, ctx = observe("mono-palette", "Commands")
        passed = False
        if ctx:
            rows, attrs, offs, top, left, right, bottom, y = ctx
            res["M2"] = all(at[1] is None for at in attrs)
            label_cells = [attrs[offs[y] + x] for x in range(left + 4, right - 1) if rows[y][x] != " "]
            res["M3"] = bool(label_cells) and all(at[2] for at in label_cells)
            others = [attrs[offs[r] + x] for r in range(top + 1, bottom) if r != y
                      for x in range(left + 2, min(right - 1, len(rows[r]))) if rows[r][x] != " "]
            res["M4"] = any(not at[2] for at in others)
            passed = res["M2"] and res["M3"] and res["M4"]
        record(res, passed)
        report["passed"] = not report["failures"]
except Exception as error:
    report["passed"] = False; report["error"] = repr(error)
finally:
    tmux("kill-server", check=False)
    (root / "report.json").write_text(json.dumps(report, indent=1, ensure_ascii=False) + "\n")
    keep = ("label", "passed", "rect", "marker_rows", "uncovered", "cells", "row", "C2", "C3", "C4",
            "distinct_rects", "bad_steps", "M2", "M3", "M4")
    print(json.dumps({"mode": mode, "passed": report.get("passed"), "error": report.get("error"),
                      "control_detects_gap": report.get("control_detects_gap"), "failures": report["failures"],
                      "cases": [{k: c[k] for k in keep if k in c} for c in report["cases"]]},
                     indent=1, ensure_ascii=False))
