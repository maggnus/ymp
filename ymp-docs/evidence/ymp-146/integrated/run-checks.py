#!/usr/bin/env python3
"""Run one authorized integration sequence without modifying tracked sources."""
import datetime, hashlib, json, os, platform, re, shutil, subprocess, sys
from pathlib import Path
ROOT = Path('/Users/maggnus/Code/ymp2')
OUT = Path('/tmp/ymp146-integrated-checks')
TARGET = Path('/tmp/ymp146-recovery-review/target')
BASE = 'c28f50192aa136b4d0fdf05e9457a90ea510543f'
ENV = dict(os.environ, CARGO_TARGET_DIR=str(TARGET), CARGO_NET_OFFLINE='true', CARGO_TERM_COLOR='never')
def now():
    return datetime.datetime.now(datetime.timezone.utc).isoformat()
def git(*args):
    return subprocess.check_output(['git', *args], cwd=ROOT, text=True)
def digest(path):
    h = hashlib.sha256()
    with Path(path).open('rb') as stream:
        for block in iter(lambda: stream.read(1024*1024), b''):
            h.update(block)
    return h.hexdigest()
def write(name, data):
    (OUT/name).write_text(json.dumps(data, indent=2, ensure_ascii=False)+'\n')
def snapshot():
    tracked = git('ls-files', '-z').split('\0')
    paths = [p for p in tracked if p and (p in ('Cargo.toml','Cargo.lock','rust-toolchain','rust-toolchain.toml') or p.startswith(('ymp-rust/','ymp-bridges/','ymp-evals/','.cargo/','scripts/'))) and not p.endswith(('.md','.log')) and '/reports/' not in p]
    return {'head':git('rev-parse','HEAD').strip(), 'porcelain':git('status','--porcelain'), 'execution_input_sha256':{p:digest(ROOT/p) for p in paths}, 'dirty_request_sha256':digest(ROOT/'ymp-docs/requests/02-intent-alignment-and-next-steps.md')}
state = {'integration_base':BASE, 'platform':platform.platform(), 'machine':platform.machine(), 'target':str(TARGET), 'started_at':now(), 'commands':[], 'status':'running', 'environment_overrides':{k:ENV[k] for k in ('CARGO_TARGET_DIR','CARGO_NET_OFFLINE','CARGO_TERM_COLOR')}}
assert not (OUT/'manifest.json').exists(), 'Existing integration result must not be overwritten'
before = snapshot()
write('before.json', before)
state['start_head'] = before['head']
state['accepted_backend_diff'] = git('diff','--name-only','743543f','HEAD','--','ymp-rust/crates/ymp-core','ymp-rust/crates/ymp-runtime','ymp-rust/crates/ymp-storage','ymp-rust/crates/ymp-providers','Cargo.toml','Cargo.lock').splitlines()
state['accepted_ui_diff'] = git('diff','--name-only','0d3f9f4','HEAD','--','ymp-rust/crates/ymp-tui','ymp-rust/crates/ymp-cli','ymp-rust/crates/ymp-eval-driver','ymp-evals','ymp-bridges').splitlines()
assert not state['accepted_backend_diff'] and not state['accepted_ui_diff'], 'Integrated components differ from accepted versions'
state['toolchain'] = {}
for name, args in [('cargo',['cargo','--version']),('rustc',['rustc','--version','--verbose'])]:
    state['toolchain'][name] = {'version':subprocess.check_output(args,cwd=ROOT,env=ENV,text=True).strip()}
for name in ('cargo','rustc','rustfmt','clippy-driver'):
    resolved = subprocess.check_output(['rustup','which',name],cwd=ROOT,env=ENV,text=True).strip()
    state['toolchain'].setdefault(name,{})
    state['toolchain'][name].update({'path':resolved,'sha256':digest(resolved)})
for name in ('python3','sh'):
    resolved = shutil.which(name)
    if resolved:
        state['toolchain'][name] = {'path':resolved,'sha256':digest(resolved)}
write('manifest.json',state)
checks = [('fmt',['cargo','fmt','--all','--check']),('clippy',['cargo','clippy','--workspace','--all-targets','--','-D','warnings']),('tests',['cargo','test','--workspace'])]
failed = False
for name, command in checks:
    current = snapshot()
    assert current['execution_input_sha256'] == before['execution_input_sha256'], 'Executable sources changed during integration checks'
    entry = {'name':name,'command':command,'cwd':str(ROOT),'started_at':now(),'log':name+'.log','exit_code':None}
    state['commands'].append(entry)
    write('manifest.json',state)
    print('START '+name+' '+entry['started_at'],flush=True)
    with (OUT/entry['log']).open('wb') as log:
        result = subprocess.run(command,cwd=ROOT,env=ENV,stdout=log,stderr=subprocess.STDOUT)
    entry.update(exit_code=result.returncode,finished_at=now(),log_sha256=digest(OUT/entry['log']))
    print('END '+name+' exit='+str(result.returncode),flush=True)
    write('manifest.json',state)
    if result.returncode != 0:
        failed = True
        break
executables = {}
results = []
if (OUT/'tests.log').exists():
    data = (OUT/'tests.log').read_text(errors='replace')
    for match in re.finditer(r'Running .+? \(([^)]+)\)',data):
        p = Path(match.group(1))
        if not p.is_absolute(): p = ROOT/p
        if p.is_file(): executables[str(p)] = {'sha256':digest(p),'bytes':p.stat().st_size}
    for match in re.finditer(r'test result: ([^.]*)\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; (\d+) filtered out',data):
        results.append(dict(status=match.group(1),**dict(zip(('passed','failed','ignored','measured','filtered_out'),map(int,match.groups()[1:])))))
for name in ('ymp','ymp-eval-driver'):
    p = TARGET/'debug'/name
    if p.is_file(): executables[str(p)] = {'sha256':digest(p),'bytes':p.stat().st_size}
write('executables.json',executables)
state['test_result_groups'] = results
state['test_totals'] = {name:sum(r[name] for r in results) for name in ('passed','failed','ignored','measured','filtered_out')}
after = snapshot()
write('after.json',after)
state['finish_head'] = after['head']
state['execution_inputs_unchanged'] = after['execution_input_sha256'] == before['execution_input_sha256']
state['dirty_request_unchanged'] = after['dirty_request_sha256'] == before['dirty_request_sha256']
state['execution_input_count'] = len(before['execution_input_sha256'])
state['post_base_changed_paths'] = git('diff','--name-only',BASE,'HEAD').splitlines()
state['status'] = 'failed' if failed or not state['execution_inputs_unchanged'] else 'passed'
state['finished_at'] = now()
state['executable_count'] = len(executables)
state['runner_sha256'] = digest(OUT/'run-checks.py')
state['limits'] = ['macOS only; no Linux verification','No real provider inference or original session action','No installed release update','Existing 49-case acceptance package was not separately repeated','Document-only commits may differ between start and finish']
write('manifest.json',state)
print(json.dumps({'status':state['status'],'totals':state['test_totals'],'source_files':state['execution_input_count'],'source_unchanged':state['execution_inputs_unchanged'],'executables':len(executables),'finish_head':after['head']},ensure_ascii=False),flush=True)
sys.exit(0 if state['status']=='passed' else 1)
