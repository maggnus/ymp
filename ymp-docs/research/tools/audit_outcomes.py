#!/usr/bin/env python3
"""Audit repeated outcomes and usage shapes without calling a provider."""
import argparse
import collections
import datetime as dt
import hashlib
import json
import sqlite3
from pathlib import Path


def shape(raw):
    if not isinstance(raw,dict): return 'absent'
    if 'total' in raw and 'last' in raw: return 'codex_thread_total_and_last'
    if 'modelUsage' in raw: return 'claude_query_usage_and_model_map'
    if 'input_tokens' in raw: return 'claude_legacy_main_usage'
    if 'inputTokens' in raw: return 'acp_flat_last_request'
    return 'unknown'


def input_bounds(counts, read_weight, write_weight):
    value=counts.get('input')
    if value is None: return None
    read,write=counts.get('cache_read'),counts.get('cache_write')
    # Input includes both cache subsets. Unknown subsets produce bounds, not zeros.
    known_read,known_write=read or 0,write or 0
    remaining=value-known_read-known_write
    if remaining < 0: raise ValueError('Cache subsets exceed input')
    base=known_read*read_weight+known_write*write_weight
    possible=[1.0]
    if read is None: possible.append(read_weight)
    if write is None: possible.append(write_weight)
    return [base+remaining*min(possible),base+remaining*max(possible)]


def audit(path):
    db=sqlite3.connect(':memory:');db.row_factory=sqlite3.Row
    with sqlite3.connect(path.resolve().as_uri()+'?mode=ro',uri=True) as original: original.backup(db)
    sessions=[json.loads(r['data']) for r in db.execute('SELECT data FROM sessions ORDER BY rowid')]
    project_labels={r['id']:f'P{i+1:02}' for i,r in enumerate(db.execute('SELECT id FROM projects ORDER BY rowid'))}
    outputs=[]
    manifests={}
    for index,session in enumerate(sessions):
        if index not in (2,3,4): continue
        key=session['id'];team={a['id']:a for a in session['team']}
        starts={};finishes={};checks=[]
        for r in db.execute('SELECT kind,data,created_at FROM events WHERE session_id=? ORDER BY seq',[key]):
            value=json.loads(r['data'])
            if r['kind']=='turn_started': starts[value['turn']]={**value,'time':r['created_at']}
            if r['kind']=='turn_completed': finishes[value['turn']]={**value,'time':r['created_at']}
            if r['kind']=='check': checks.append(value)
        rows=[]
        agents=collections.defaultdict(lambda:dict(invocations=0,input=0,output=0,cache_read=0,cache_write=0,partial=0,missing_cache_read=0,missing_cache_write=0))
        for r in db.execute('SELECT agent,turn,status,snapshot FROM token_usage WHERE session_id=? ORDER BY turn',[key]):
            snapshot=json.loads(r['snapshot']) if r['snapshot'] else None
            raw=finishes.get(r['turn'],{}).get('usage')
            data=dict(turn=r['turn'],agent=r['agent'],provider=team.get(r['agent'],{}).get('provider'),phase=starts.get(r['turn'],{}).get('purpose'),
                      native_shape=shape(raw),naive_flat_input=raw.get('inputTokens') if isinstance(raw,dict) else None,
                      counts=snapshot.get('counts') if snapshot else None,partial=snapshot.get('partial') if snapshot else True,
                      finalized=snapshot.get('finalized') if snapshot else False)
            if snapshot:
                total=agents[r['agent']];total['invocations']+=1
                for field in ('input','output','cache_read','cache_write'):
                    total[field]+=snapshot['counts'].get(field) or 0
                for field in ('cache_read','cache_write'):
                    total['missing_'+field]+=snapshot['counts'].get(field) is None
                total['partial']+=bool(snapshot['partial'] or not snapshot['finalized'])
                data['illustrative_weighted_input_bounds']=input_bounds(snapshot['counts'],.1,1.25)
            rows.append(data)
        metadata=path.parent/'projects'/session['project_id']/'sessions'/key/'workspace'
        workspace=json.loads((metadata/'workspace.json').read_text()) if (metadata/'workspace.json').exists() else None
        if workspace: manifests[f'S{index+1:02}']=workspace.get('initial',{})
        changes=json.loads((metadata/'changes.json').read_text()) if (metadata/'changes.json').exists() else None
        start_times=[dt.datetime.fromisoformat(v['time']) for v in starts.values()]
        end_times=[dt.datetime.fromisoformat(v['time']) for v in finishes.values()]
        native_field_missing=sum(r['naive_flat_input'] is None for r in rows)
        native_explicit_zero=sum(r['naive_flat_input']==0 for r in rows)
        bounds=[sum(r['illustrative_weighted_input_bounds'][i] for r in rows if r.get('illustrative_weighted_input_bounds')) for i in (0,1)]
        outputs.append(dict(label=f'S{index+1:02}',project=project_labels[session['project_id']],status=session['status'],recorded_turns=session['turns_used'],started=len(starts),completed=len(finishes),
                            elapsed_first_start_to_last_completion_seconds=(max(end_times)-min(start_times)).total_seconds(),
                            last_completed_at=max(end_times).isoformat(),reported_input=sum(a['input'] for a in agents.values()),reported_output=sum(a['output'] for a in agents.values()),
                            reported_cache_read=sum(a['cache_read'] for a in agents.values()),reported_cache_write=sum(a['cache_write'] for a in agents.values()),
                            partial_invocations=sum(a['partial'] for a in agents.values()),illustrative_weighted_input_bounds=bounds,
                            naive_flat_missing=native_field_missing,naive_flat_explicit_zero=native_explicit_zero,agents=dict(agents),
                            check_events=len(checks),failed_checks=sum(not c['success'] for c in checks),
                            saved_workspace_fields=sorted(workspace) if workspace else [],initial_fingerprint_files=len(workspace.get('initial',{})) if workspace else None,
                            end_change_count=len(changes) if changes is not None else None,explicit_post_verification_fingerprint=False,rows=rows))
    sensitivity=[]
    for read_weight in (0,.1,.5,1):
        for write_weight in (1,1.25,2):
            values=[]
            for outcome in outputs:
                bounds=[sum(input_bounds(r['counts'],read_weight,write_weight)[i] for r in outcome['rows'] if r['counts']) for i in (0,1)]
                values.append(dict(label=outcome['label'],bounds=bounds))
            ordered=sorted(values,key=lambda v:v['bounds'][0])
            unambiguous=all(a['bounds'][1]<b['bounds'][0] for a,b in zip(ordered,ordered[1:]))
            sensitivity.append(dict(read_weight=read_weight,write_weight=write_weight,values=values,order=[v['label'] for v in ordered],order_unambiguous=unambiguous))
    return dict(snapshot_at=dt.datetime.now(dt.timezone.utc).isoformat(),max_event_seq=db.execute('SELECT MAX(seq) FROM events').fetchone()[0],
                source='Read-only consistent SQLite snapshot; repeated requests S03–S05 identified by the request appendix.',outcomes=outputs,sensitivity=sensitivity,
                cross_run=dict(all_three_same_registered_project=len({o['project'] for o in outputs})==1,
                               s03_s05_initial_manifests_equal=manifests.get('S03')==manifests.get('S05'),
                               raw_input_order=[o['label'] for o in sorted(outputs,key=lambda o:o['reported_input'])]),
                formula='I_noncache + w_read * R + w_write * W; I_noncache = I - R - W. Input only; output is separately reported.',
                assumptions=['Weights 0.1 and 1.25 are illustrative hypotheses, not verified model prices or an invoice.',
                             'Missing cache fields generate interval bounds; partial invocation totals remain lower-bound observations.',
                             'No inference of an absent native input field as zero. Native raw payloads retain provider-specific semantics.',
                             'An initial workspace fingerprint is not automatically a post-verification snapshot. An empty recorded end change list needs separate interpretation.',
                             'No private prompts, file contents, credentials or native session handles are exported.'])


if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--database',type=Path,required=True)
    parser.add_argument('--output',type=Path,required=True)
    args=parser.parse_args()
    data=audit(args.database)
    args.output.write_text(json.dumps(data,indent=2)+'\n')
    for item in data['outcomes']:
        print(json.dumps({k:v for k,v in item.items() if k!='rows'}))
