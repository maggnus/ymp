import json, os, subprocess, hashlib, time, uuid
from pathlib import Path
ROOT=Path('/tmp/ymp201-consumer-rereview').resolve(); REPO=Path('/Users/maggnus/.paseo/worktrees/1ms2ynax/ymp201-weak-pilot'); BIN=Path('/tmp/ymp146-recovery-review/target/debug/ymp-weak-pilot')
ENV=dict(os.environ,PYTHONDONTWRITEBYTECODE='1')
cases=[('control','weak-solo',None),('wrong-answer','weak-solo','wrong-answer'),('missing-deliverable','weak-solo','missing-deliverable'),('engine-negative','cooperation-2','negative-final-review'),('known-budget','independent-2',None),('known-deadline','weak-solo','deadline-after-completed'),('unknown-usage','weak-solo','unknown-usage'),('provider-error','weak-solo','provider-error'),('broken-envelope','weak-solo','broken-envelope')]
results=[]
for name,condition,fault in cases:
 root=ROOT/'cases'/name;root.mkdir(parents=True,exist_ok=True)
 command=['python3','-B',str(REPO/'ymp-evals/weak-pilot/consumer_manifest.py'),'--runner',str(BIN),'--output',str(root/'controller'),'--workspaces',str(root/'workspaces'),'--phase','protocol-e2e']
 made=subprocess.run(command,cwd=REPO,env=ENV,capture_output=True,text=True,check=True)
 spec=json.loads(made.stdout)
 spec['attempts']=[{'id':'first','condition':condition,'task':'reconcile','variant':'preparation','blind_id':uuid.uuid4().hex},{'id':'second','condition':'weak-solo','task':'repair','variant':'preparation','blind_id':uuid.uuid4().hex}]
 if fault:spec['attempts'][0]['fixture_fault']=fault
 if name=='known-budget':spec['config']['limits']['resources'].update(observed_tokens=150,invocation_tokens=100,review_reserve_tokens=100)
 if name=='known-deadline':
  spec['group_seconds']=4
  spec['config']['limits']['turn_timeout_secs']=4
 # This guard prevents the independent proof from ever crossing to a real provider.
 assert spec['execution_kind']=='protocol-fixture' and spec['codex']==str(REPO/'ymp-evals/weak-pilot/codex_protocol_v2.py') and spec['weak_model'].startswith('fixture-')
 manifest=root/'manifest.json';manifest.write_text(json.dumps(spec,indent=2)+'\n')
 cmd=[str(BIN),'scripted','--manifest',str(manifest)]
 start=time.monotonic()
 with (root/'stdout.log').open('w') as stdout,(root/'stderr.log').open('w') as stderr:
  proc=subprocess.Popen(cmd,cwd=REPO,env=ENV,stdout=stdout,stderr=stderr)
  try:code=proc.wait(timeout=45)
  except subprocess.TimeoutExpired:proc.kill();proc.wait();raise
 report=json.loads((root/'controller/run.json').read_text())
 first=report['outcomes'][0]; runtime=first.get('runtime') or {}; trace_path=root/'controller/first'/('trace.json' if condition.startswith('cooperation') else 'group/trace.json')
 trace=json.loads(trace_path.read_text()) if trace_path.exists() else None
 live=[]
 for path in (root/'controller').rglob('fixture-pids.jsonl'):
  for line in path.read_text().splitlines():
   pid=json.loads(line)['pid']
   try:os.kill(pid,0);live.append(pid)
   except ProcessLookupError:pass
 row={'case':name,'command':cmd,'exit':code,'elapsed':round(time.monotonic()-start,3),'complete':report['complete'],'calibration_allows_pilot':report['calibration_allows_pilot'],'first_status':first.get('status'),'interpretable':first.get('interpretable'),'objective_success':first.get('objective_success'),'second_status':report['outcomes'][1].get('status'),'error':first.get('error'),'errors':first.get('errors'),'runtime_status':runtime.get('status'),'accounting_closed':runtime.get('accounting_closed'),'protocol_deviations':runtime.get('protocol_deviations'),'measurement_valid':first.get('measurement_valid'),'task_outcome':first.get('task_outcome'),'task_issues':first.get('task_issues'),'invalid_reasons':first.get('invalid_reasons'),'usage':runtime.get('usage'),'active_fixture_pids':live,'pid_journal_available':any((root/'controller').rglob('fixture-pids.jsonl')),'manifest_sha256':hashlib.sha256(manifest.read_bytes()).hexdigest()}
 if trace:row['invocations']=[{'state':i['state'],'usage':i.get('usage'),'ended_at':i.get('ended_at')} for i in trace['invocations']]
 results.append(row);(ROOT/'outcomes.json').write_text(json.dumps(results,indent=2)+'\n')
 print(json.dumps({k:row[k] for k in ('case','exit','complete','calibration_allows_pilot','interpretable','objective_success','second_status','active_fixture_pids')}),flush=True)
