#!/usr/bin/env python3
"""Check reuse guards with declared router decisions; no semantic model tested."""
import copy,hashlib,json,tempfile
from pathlib import Path


def decide(record,request,current):
    if request['project']!=record['project']: return 'task','different_project'
    if request.get('redo'): return 'task','explicit_redo'
    if request['intent']!='same_contract': return 'task','changed_or_unknown_intent'
    if record['status']!='completed' or not record['checks_passed']: return 'task','unverified_outcome'
    if not record.get('verified_manifest'): return 'task','no_verification_anchor'
    if not record.get('dependencies_covered'): return 'task','uncovered_dependencies'
    if record['verified_manifest']!=current: return 'task','changed_files'
    return 'answer','attributed_prior_outcome'


record=dict(project='p1',session='s1',status='completed',checks_passed=True,verified_manifest={'index.html':'verified_hash'},dependencies_covered=True)
request=dict(project='p1',intent='same_contract',redo=False)
cases=[]
for name,changes,request_changes,current in [
    ('exact_repeat',{}, {}, {'index.html':'verified_hash'}),
    ('translated_repeat_declared_same_intent',{}, {}, {'index.html':'verified_hash'}),
    ('different_project',{}, {'project':'p2'}, {'index.html':'verified_hash'}),
    ('explicit_redo',{}, {'redo':True}, {'index.html':'verified_hash'}),
    ('new_dark_theme',{}, {'intent':'changed_contract'}, {'index.html':'verified_hash'}),
    ('ambiguous_request',{}, {'intent':'unknown'}, {'index.html':'verified_hash'}),
    ('edited_artifact',{}, {}, {'index.html':'new_hash'}),
    ('deleted_artifact',{}, {}, {}),
    ('only_initial_snapshot',{'verified_manifest':None},{},{'index.html':'verified_hash'}),
    ('unfinished_session',{'status':'running'},{},{'index.html':'verified_hash'}),
    ('failed_check',{'checks_passed':False},{},{'index.html':'verified_hash'}),
    ('external_or_ignored_dependency',{'dependencies_covered':False},{},{'index.html':'verified_hash'}),
]:
    answer,reason=decide({**record,**changes},{**request,**request_changes},current)
    expected='answer' if name in ('exact_repeat','translated_repeat_declared_same_intent') else 'task'
    assert answer==expected,(name,answer)
    cases.append(dict(case=name,decision=answer,reason=reason))
# The primary H8 risk remains: fingerprints cannot catch an incorrect same-intent label.
wrong_route,_=decide(record,request,{'index.html':'verified_hash'})
assert wrong_route=='answer'
with tempfile.TemporaryDirectory(prefix='yv-',dir='/tmp') as directory:
    base=Path(directory);root=base/'work';root.mkdir()
    (base/'target').write_text('first')
    (root/'artifact').symlink_to('../target')
    link_before=hashlib.sha256(b'symlink:'+str((root/'artifact').readlink()).encode()).hexdigest()
    (base/'target').write_text('changed')
    link_after=hashlib.sha256(b'symlink:'+str((root/'artifact').readlink()).encode()).hexdigest()
    assert link_before==link_after
    symlink=dict(link_identity_unchanged=True,target_content_changed=True,note='Target must be covered separately; link-string equality alone is insufficient.')
result=dict(scope='Offline decision guards conditional on declared router labels; semantic and translation accuracy are unmeasured.',cases=cases,
            negative_control=dict(case='router_falsely_labels_dark_theme_as_same',fact_only_gate_returns=wrong_route,actually_correct=False),
            symlink_dependency=symlink,
            break_even='Reuse is economical only if p_valid * (C_full - C_revalidate) > C_gate, before adding the expected loss from incorrect reuse.')
Path(__file__).resolve().parents[1].joinpath('evidence/outcome-reuse-probe.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps({'cases':len(cases),'wrong_intent_negative_control_rejected_as_claim':True,'output':'outcome-reuse-probe.json'}))
