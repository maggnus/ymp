#!/usr/bin/env python3
"""Offline native CLI transport for the real Claude SDK, never a model call."""
import json, sys, os
from pathlib import Path

def send(v): print(json.dumps(v), flush=True)
def arg(name, default):
    return sys.argv[sys.argv.index(name)+1] if name in sys.argv else default
log=Path.cwd()/'native.jsonl'
with log.open('a') as f: f.write(json.dumps({'argv':sys.argv[1:]})+'\n')
models=[{'value':'opus[1m]','resolvedModel':'claude-opus-5[1m]','displayName':'Fixture','description':'offline','supportsEffort':True,'supportedEffortLevels':['low','xhigh','max']}, {'value':'small','displayName':'Small','description':'offline','supportsEffort':False}]
for line in sys.stdin:
    q=json.loads(line)
    with log.open('a') as f: f.write(json.dumps(q)+'\n')
    if q['type']=='control_request':
        send({'type':'control_response','response':{'subtype':'success','request_id':q['request_id'],'response':{'models':models,'commands':[],'agents':[],'output_style':'default','available_output_styles':['default'],'account':{}}}})
    elif q['type']=='user':
        model=arg('--model','claude-opus-5'); model='claude-opus-5' if model=='opus' else model
        effort=arg('--effort','xhigh')
        send({'type':'system','subtype':'init','session_id':'fixture-session','model':model,'effort':effort,'permissionMode':'default','claude_code_version':'fixture-version','tools':[],'mcp_servers':[],'uuid':'fixture-init','cwd':str(Path.cwd()),'apiKeySource':'fixture','slash_commands':[],'output_style':'default','skills':[],'plugins':[]})
        send({'type':'result','subtype':'success','session_id':'fixture-session','result':'done','is_error':False,'duration_ms':1,'duration_api_ms':1,'num_turns':1,'total_cost_usd':0,'usage':{'input_tokens':1,'output_tokens':1},'modelUsage':{},'permission_denials':[],'uuid':'fixture-result','stop_reason':'end_turn'})
