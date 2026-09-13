"""Independent terminal probe of the owner's file-browsing outcome; mock only."""
import argparse
import errno
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import time
import uuid

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('binary', type=Path)
parser.add_argument('output', type=Path)
parser.add_argument('--baseline', action='store_true')
args = parser.parse_args()
root = args.output.resolve()
root.mkdir(parents=True, exist_ok=False)
project, home = root / 'project', root / 'metadata'
project.mkdir()
home.mkdir()
(home / 'config.toml').write_text('''version = 1
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
binary = args.binary.resolve()
demo = subprocess.run([str(binary), '--home', str(home), '-C', str(project), 'demo'], capture_output=True, text=True, check=True, timeout=30)
session = re.search(r'^Session: (\S+)', demo.stdout, re.M).group(1)
folder = project / 'nav-contract'
folder.mkdir()
source = folder / 'literal.rs'
body = 'fn main() {\n    println!("NAV_CONTRACT_139: a  b");\n}\n'
source.write_text(body)
native_name_variant = 'non-UTF-8 filename bytes'
created_names = []
try:
    for byte, marker in [(0xFE, 'PATH_FE_139'), (0xFF, 'PATH_FF_139')]:
        native_path = os.fsencode(folder) + b'/same-' + bytes([byte]) + b'.rs'
        with open(native_path, 'wb') as file:
            file.write(('// ' + marker + '\n').encode())
        created_names.append(native_path)
except OSError as error:
    if error.errno != errno.EILSEQ:
        raise
    # APFS rejects invalid UTF-8 names. Whitespace/control names still exercise
    # distinct native paths whose lossy one-line captions can collide.
    for native_path in created_names:
        os.unlink(native_path)
    native_name_variant = 'newline versus space; filesystem rejects non-UTF-8 names'
    for name, marker in [('same-a\nb.rs', 'PATH_FE_139'), ('same-a b.rs', 'PATH_FF_139')]:
        (folder / name).write_text('// ' + marker + '\n')
outside = root / 'outside.rs'
outside.write_text('// EXPLICIT_LINK_CONTENT_139\n')
(folder / 'link-out.rs').symlink_to(outside)
before = {os.fsencode(p).hex(): hashlib.sha256(p.read_bytes()).hexdigest()
          for p in folder.iterdir() if p.is_file() and not p.is_symlink()}
socket = 'ymp139-parent-' + uuid.uuid4().hex[:10]
report = {'binary': str(binary), 'baseline': args.baseline, 'native_inference': False,
          'native_name_variant': native_name_variant}

def tmux(*values, check=True):
    return subprocess.run(['tmux', '-L', socket, *values], capture_output=True, text=True, check=check, timeout=5)

def screen(name=None):
    text = tmux('capture-pane', '-p', '-t', 'probe').stdout
    if name:
        (root / (name + '.txt')).write_text(text)
    return text

def keys(*values):
    for value in values:
        tmux('send-keys', '-t', 'probe', value)
        time.sleep(0.15)

def type_text(text):
    tmux('send-keys', '-t', 'probe', '-l', text)
    time.sleep(0.15)

def wait_for(needle, timeout=5):
    end = time.monotonic() + timeout
    while time.monotonic() < end:
        if needle in screen():
            return True
        time.sleep(0.1)
    return False

try:
    tmux('new-session', '-d', '-s', 'probe', '-x', '120', '-y', '32', str(binary), '--home', str(home), '-C', str(project), 'resume', session)
    assert wait_for('SESSION'), 'mock session did not open'
    type_text('/files')
    keys('Enter')
    assert wait_for('nav-contract'), 'Files listing did not open'
    type_text('/nav-contract')
    keys('Enter', 'Enter')
    descended = wait_for('literal.rs', 2)
    screen('directory-entry')
    report['directory_descent'] = descended
    if args.baseline:
        assert not descended, 'baseline already supports directory descent'
        report['expected_failure'] = 'Enter only inspects directory metadata; its source child is unreachable.'
    else:
        assert descended, 'Enter did not descend to the selected directory'
        type_text('/literal.rs')
        keys('Enter', 'Enter')
        assert wait_for('NAV_CONTRACT_139'), 'Enter did not show source contents'
        text = screen('source-preview')
        assert 'a  b' in text, 'literal internal whitespace changed'
        (root / 'source-preview.ansi').write_text(tmux('capture-pane', '-p', '-e', '-t', 'probe').stdout)
        report['literal_source_preview'] = True
        keys('Escape', 'Escape')  # close the preview, then clear the retained filter
        type_text('/same-')
        # The native widget retains ../ above matching files; the filter selects
        # a matching entry, while Home deliberately selects the parent row.
        keys('Enter', 'Enter')
        assert wait_for('PATH_'), 'first native filename did not open'
        first = screen('native-name-first')
        keys('Escape', 'Down', 'Enter')
        assert wait_for('PATH_'), 'second native filename did not open'
        second = screen('native-name-second')
        markers = [re.search(r'PATH_(?:FE|FF)_139', text).group(0) for text in [first, second]]
        assert set(markers) == {'PATH_FE_139', 'PATH_FF_139'}, ('distinct native filenames opened the same file', markers)
        report['native_path_identity'] = True
        keys('Escape', 'Escape')
        type_text('/link-out')
        keys('Enter', 'Enter')
        text = screen('outside-link')
        assert wait_for('EXPLICIT_LINK_CONTENT_139'), 'explicitly selected linked file did not preview'
        screen('outside-link')
        report['explicit_link_preview'] = True
    after = {os.fsencode(p).hex(): hashlib.sha256(p.read_bytes()).hexdigest()
             for p in folder.iterdir() if p.is_file() and not p.is_symlink()}
    assert after == before
    report['workspace_unchanged'] = True
    report['passed'] = True
except Exception as error:
    report['passed'] = False
    report['error'] = str(error)
    try:
        screen('failure')
    except Exception:
        pass
    raise
finally:
    tmux('kill-server', check=False)
    (root / 'report.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps(report, indent=2))
