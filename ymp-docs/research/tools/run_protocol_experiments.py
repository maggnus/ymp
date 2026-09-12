#!/usr/bin/env python3
"""Exercise the unchanged ymp binary against an offline protocol fixture."""
import argparse
import collections
import json
import signal
import sqlite3
import subprocess
import sys
import tempfile
import time
from pathlib import Path

FIXTURE = Path(__file__).with_name('fault_provider.py').resolve()


def run(binary, settings, members=2, memory=False, adaptive=False, cancel=False):
    with tempfile.TemporaryDirectory(prefix='yr-', dir='/tmp') as temporary:
        root = Path(temporary)
        home, cwd, control = (root / d for d in ('state', 'work', 'control'))
        for directory in (home, cwd, control):
            directory.mkdir()
        (control / 'settings.json').write_text(json.dumps(settings))
        ids = [f'a{i}' for i in range(members)]
        config = f'version=1\nteam={json.dumps(ids)}\n[limits]\nparallel=3\nturns=200\nturn_timeout_secs=1\nattempts=2\n'
        config += f'[[providers]]\nid="fixture"\nkind="codex"\ncommand={json.dumps(sys.executable)}\nargs={json.dumps([str(FIXTURE),str(control)])}\nenabled=true\n'
        for id in ids:
            config += f'[[agents]]\nid="{id}"\nname="{id}"\nprovider="fixture"\ninstructions="fixture_agent={id}"\nenabled=true\n'
        (home / 'config.toml').write_text(config)
        args = [str(binary), '--home', str(home), '-C', str(cwd), 'run', 'Create the requested deterministic fixture artifacts.']
        if not memory:
            args.append('--no-memory')
        if not adaptive:
            args.append('--no-adaptive')
        started = time.monotonic()
        process = subprocess.Popen(args, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
        if cancel:
            until = time.monotonic() + 8
            while time.monotonic() < until and process.poll() is None:
                if (control / 'fixture.sqlite').exists():
                    try:
                        with sqlite3.connect(control / 'fixture.sqlite') as db:
                            reached = db.execute("SELECT COUNT(*) FROM calls WHERE fault='timeout'").fetchone()[0]
                        if reached:
                            process.send_signal(signal.SIGINT)
                            break
                    except sqlite3.OperationalError:
                        pass
                time.sleep(.01)
        stdout, stderr = process.communicate(timeout=20)
        elapsed = time.monotonic() - started
        with sqlite3.connect(home / 'state.sqlite') as db:
            session = json.loads(db.execute('SELECT data FROM sessions ORDER BY rowid DESC LIMIT 1').fetchone()[0])
            events = [(kind,json.loads(raw)) for kind,raw in db.execute('SELECT kind,data FROM events ORDER BY seq')]
            tasks = [json.loads(row[0]) for row in db.execute('SELECT data FROM tasks')]
            observations = db.execute('SELECT success,COUNT(*) FROM observations GROUP BY success').fetchall()
            usage = [json.loads(r[0]) for r in db.execute('SELECT snapshot FROM token_usage WHERE snapshot IS NOT NULL')]
        with sqlite3.connect(control / 'fixture.sqlite') as db:
            faults = db.execute("SELECT phase,fault,retried FROM calls WHERE fault!='none'").fetchall()
        phases = collections.Counter(raw['purpose'] for kind,raw in events if kind == 'turn_started')
        effects = (cwd / 'effects.log').read_text().splitlines() if (cwd / 'effects.log').exists() else []
        count = settings.get('tasks', 1)
        artifacts_ok = all((cwd / f'artifact-{i}.txt').exists() and (cwd / f'artifact-{i}.txt').read_text() == 'verified\n' for i in range(1,count+1))
        result = dict(settings=settings, members=members, memory=memory, adaptive=adaptive, status=session['status'], exit_code=process.returncode,
                      phases=dict(phases), turns=sum(phases.values()), synthetic_reported_tokens=sum(s['counts']['input']+s['counts']['output'] for s in usage),
                      seconds=round(elapsed,4), observations={str(k):v for k,v in observations}, task_attempts=[t['attempts'] for t in tasks],
                      task_states=[t['state'] for t in tasks], faults=faults, side_effects=len(effects), artifacts_ok=artifacts_ok,
                      external_acceptance=artifacts_ok and len(effects)==count,
                      failure_messages=[raw['error'] for kind,raw in events if kind=='turn_failed'],
                      turn_failures=sum(kind=='turn_failed' for kind,_ in events))
        if process.returncode and not session:
            raise AssertionError(stdout+stderr)
        return result


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    binary = args.binary.resolve()
    results=[]
    for members in (2,3,5):
        for tasks in (1,3,8):
            result=run(binary,dict(tasks=tasks),members)
            assert result['status']=='completed' and result['external_acceptance'],result
            assert result['turns']==members+3+tasks*(members+2),result
            results.append(result)
    for difficulty in ('simple','standard','complex'):
        results.append(run(binary,dict(difficulty=difficulty),3))
    for memory,adaptive in ((False,False),(True,False),(False,True),(True,True)):
        results.append(run(binary,{},2,memory,adaptive))
    cases=[
        ('plan_one_exit','plan','none',False,False),
        ('bid_one_exit','bid','none',False,False),
        ('transient_refusal','review_plan','none',False,False),
        ('native_retry_notice','review_plan','none',False,False),
        ('transient_refusal','review_plan','bounded_read',False,False),
        ('transient_refusal','review_plan','bounded_read',True,False),
        ('execute_exit_before','execute','none',False,False),
        ('execute_exit_after','execute','none',False,False),
        ('execute_exit_after','execute','bounded_read',False,False),
        ('execute_exit_after','execute','blind',False,False),
        ('malformed_review','review','none',False,False),
        ('timeout','review','none',False,False),
        ('timeout','review_plan','bounded_read',False,True),
        ('content_bad',None,'bounded_read',False,False),
        ('exit','learn','none',False,False),
        ('exit','synthesis','none',False,False),
    ]
    for fault,phase,policy,persistent,cancel in cases:
        settings=dict(fault=fault,fault_phase=phase,retry_policy=policy,persistent_fault=persistent)
        result=run(binary,settings,2,memory=fault=='exit' and phase=='learn',cancel=cancel)
        results.append(result)
    args.output.write_text(json.dumps(dict(binary_version=subprocess.check_output([str(binary),'--version'],text=True).strip(),
                                          scope='Deterministic offline fixture; synthetic usage and outcomes cannot establish model quality or real latency savings.',
                                          runs=results),indent=2)+'\n')
    print(json.dumps({'runs':len(results),'statuses':dict(collections.Counter(r['status'] for r in results)),'output':str(args.output)}))
