#!/usr/bin/env python3
"""Check the existing scenario against the canned Mock without claiming model quality."""
import json,sqlite3,subprocess,tempfile
from pathlib import Path
repo=Path(__file__).resolve().parents[3]
scenario=json.loads((repo/'ymp-evals/scenarios/greeting.json').read_text())
with tempfile.TemporaryDirectory(prefix='ys-',dir='/tmp') as directory:
    root=Path(directory);home=root/'state';work=root/'work';home.mkdir();work.mkdir()
    config='version=1\nteam=["writer","reviewer"]\n[[providers]]\nid="fixture"\nkind="mock"\ncommand="mock"\nenabled=true\n'
    for id in ('writer','reviewer'):
        config+=f'[[agents]]\nid="{id}"\nname="{id}"\nprovider="fixture"\ninstructions="[mock:usage]"\nenabled=true\n'
    (home/'config.toml').write_text(config)
    process=subprocess.run([str(repo/'target/release/ymp'),'--home',str(home),'-C',str(work),'run',scenario['prompt'],'--no-memory','--no-adaptive'],capture_output=True,text=True)
    with sqlite3.connect(home/'state.sqlite') as db:
        session=json.loads(db.execute('SELECT data FROM sessions').fetchone()[0])
    checks=[]
    for command in scenario['acceptance']:
        result=subprocess.run(['/bin/sh','-c',command],cwd=work,capture_output=True,text=True)
        checks.append(dict(command=command,exit_code=result.returncode))
    outcome=dict(scenario=scenario['id'],application_status=session['status'],application_exit=process.returncode,
                 files=sorted(p.name for p in work.iterdir()),independent_checks=checks,
                 independent_acceptance=all(c['exit_code']==0 for c in checks),
                 interpretation='Mock uses canned greeting.txt behavior; it is not a model that can solve greeting.py. This is an oracle/fixture sanity check, not an estimate of agent quality.')
    assert session['status']=='completed' and not outcome['independent_acceptance'],outcome
(repo/'ymp-docs/research/evidence/scenario-smoke.json').write_text(json.dumps(outcome,indent=2)+'\n')
print(json.dumps(outcome))
