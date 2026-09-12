#!/usr/bin/env python3
"""Validate a proposed role policy against declared capability fixtures, offline."""
import hashlib,json
from pathlib import Path

def requested(role,difficulty):
    if role in ('bid','plan','learn'): return 'low'
    if role=='final_review': return 'xhigh'
    if role=='execute': return {'simple':'low','standard':'high','complex':'xhigh'}[difficulty]
    return 'medium'

roles=('plan','review_plan','bid','execute','review','final_review','learn','review_memory','synthesis')
capabilities={'codex_fixture':['low','medium','high','xhigh'],'claude_sdk_fixture':['low','medium','high','xhigh','max'],'glm_5_2_installed':['none','high','max']}
rows=[]
for provider,allowed in capabilities.items():
    for role in roles:
        for difficulty in ('simple','standard','complex'):
            level=requested(role,difficulty)
            supported=level in allowed
            rows.append(dict(provider=provider,role=role,difficulty=difficulty,requested=level,accepted=level if supported else None,result='supported_fixture' if supported else 'unsupported_no_silent_fallback'))
assert any(r['result']=='unsupported_no_silent_fallback' for r in rows)
def version(effort):
    return hashlib.sha256(json.dumps(dict(profile='a',resolved_model='fixture',effort=effort),sort_keys=True).encode()).hexdigest()
assert version('low')!=version('xhigh')
assert requested('review','simple')!='low'
assert requested('final_review','simple')=='xhigh'
result=dict(scope='81 transport-policy cases; Codex and Claude tier sets are declared fixtures, not proof that a live selected model honors them. GLM levels come from the installed package.',
            policy_cases=rows,effective_effort_changes_experience_key=True,
            conclusions=['The requested funnel has unsupported GLM tiers in this installation.','A mock cannot measure quality or token savings from changing reasoning effort.','All requests in a role-mapped native thread must set the effective effort explicitly to avoid inherited-tier contamination.'])
Path(__file__).resolve().parents[1].joinpath('evidence/effort-policy-probe.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps({'cases':len(rows),'unsupported':sum(not r['accepted'] for r in rows)}))
