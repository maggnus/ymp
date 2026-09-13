import hashlib,importlib.util,json,os,re,shutil,subprocess,sys
from pathlib import Path
ROOT=Path('/tmp/ymp201-consumer-rereview/c3');ROOT.mkdir()
REPO=Path('/Users/maggnus/.paseo/worktrees/1ms2ynax/ymp201-weak-pilot/ymp-evals/weak-pilot')
def load(name):
 spec=importlib.util.spec_from_file_location(name,REPO/(name+'.py'));mod=importlib.util.module_from_spec(spec);spec.loader.exec_module(mod);return mod
observer=load('observer');selector=load('public_select');restricted=load('restricted_python')
marker=ROOT/'private'/'canary.txt';marker.parent.mkdir();marker.write_text('ONLY-SYNTHETIC-C3-CANARY')
reference=(REPO/'fixtures/measured/repair/private/windows.py').read_text()
old=Path('/tmp/ymp201-consumer-review/selection-canary/candidate1/windows.py').read_text()
canary="import os as _os,sys as _sys\nprint('CHILD_PID='+str(_os.getpid()),file=_sys.stderr,flush=True)\n"+f"_target={str(marker)!r}\n"+"_os.symlink(_target,'private-link')\nfor _route,_path in [('direct',_target),('symlink','private-link')]:\n    try:\n        _data=open(_path).read()\n    except PermissionError:\n        print('DENIED:'+_route,file=_sys.stderr)\n    else:\n        raise RuntimeError('PRIVATE_CANARY_ESCAPED:'+_data)\n"
candidates={}
for name,source in [('correct',reference),('old-route',old),('canary',canary+reference)]:
 work=ROOT/name;observer.stage('repair','measured',work);(work/'windows.py').write_text(source);candidates[name]=work
public=REPO/'fixtures/measured/repair/visible/public_test.py'
old_selection=selector.select('repair',[candidates['old-route'],candidates['correct']],public)
assert old_selection['measurement_valid'] and old_selection['selected_participant_ordinal']==2
assert 'SYNTHETIC-HIDDEN-CANARY-201' not in json.dumps(old_selection)
selection=selector.select('repair',[candidates['canary'],candidates['correct']],public)
assert selection['selected_participant_ordinal']==1 and all(r['public_passed'] for r in selection['observations'])
assert 'DENIED:direct' in selection['observations'][0]['stderr'] and 'DENIED:symlink' in selection['observations'][0]['stderr']
scored={};captured=[];original=observer.restricted_python.run
def observed_run(script,files,**kw):
 result=original(script,files,**kw)
 calls=json.loads(kw['input_text']);assert all(set(call)=={'function','args'} for call in calls)
 captured.append({'files':list(files),'stdin_keys':[sorted(call) for call in calls],'stderr':result.stderr,'exit':result.returncode})
 return result
observer.restricted_python.run=observed_run
for i,name in enumerate(('correct','old-route','canary'),1):
 sealed=ROOT/('sealed-'+name);observer.seal('repair','measured',candidates[name],sealed,format(i,'032x'))
 score=observer.score(sealed);scored[name]=score
 assert score['objective_success']==(name!='old-route')
assert 'DENIED:direct' in captured[-1]['stderr'] and 'DENIED:symlink' in captured[-1]['stderr']
calls=[{'function':'total_duration','args':[[[0,2]]],'expected':'NEVER_PASS_EXPECTED_TO_CHILD'}]
cli=subprocess.run([sys.executable,'-B',str(REPO/'repair_probe.py'),str(candidates['canary']/'windows.py')],input=json.dumps(calls),capture_output=True,text=True,timeout=10)
assert cli.returncode==0 and json.loads(cli.stdout)[0]['value']==2
assert 'DENIED:direct' in cli.stderr and 'DENIED:symlink' in cli.stderr
pids=[]
for text in [selection['observations'][0]['stderr'],captured[-1]['stderr'],cli.stderr]:
 for p in re.findall(r'CHILD_PID=(\d+)',text):
  try:os.kill(int(p),0);alive=True
  except ProcessLookupError:alive=False
  pids.append({'pid':int(p),'alive_after_return':alive});assert not alive
launched=[];Popen=restricted.subprocess.Popen
def record(*args,**kwargs):
 process=Popen(*args,**kwargs);launched.append(process);return process
restricted.subprocess.Popen=record
try:
 restricted.run(b'while True: pass\n',{},timeout=0.4)
 raise AssertionError('Timeout must stop the candidate')
except restricted.RestrictedExecutionError as error:
 assert error.kind=='timeout' and error.process_terminated
for proc in launched:assert proc.poll() is not None
report={'old_selection':old_selection,'paired_selection':selection,'scores':scored,'scorer_child_inputs':captured,'direct_probe':{'exit':cli.returncode,'stdout':cli.stdout,'stderr':cli.stderr},'child_pids':pids,'timeout_processes_collected':[p.returncode for p in launched],'canary_hash':hashlib.sha256(marker.read_bytes()).hexdigest()}
assert 'ONLY-SYNTHETIC-C3-CANARY' not in json.dumps(report)
(ROOT/'result.json').write_text(json.dumps(report,indent=2)+'\n')
print(json.dumps({'old_route_read_refused':True,'selection_canary_and_control_pass':True,'scoring_canary_and_control_pass':True,'symlink_denied':True,'direct_probe_guarded':True,'children_ended':True,'child_stdin_contains_expected':False}))
