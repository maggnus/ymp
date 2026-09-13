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
hooks={}
def result():
    send({'type':'result','subtype':'success','session_id':'fixture-session','result':'done','is_error':False,'duration_ms':1,'duration_api_ms':1,'num_turns':1,'total_cost_usd':0,'usage':{'input_tokens':1,'output_tokens':1},'modelUsage':{},'permission_denials':[],'uuid':'fixture-result','stop_reason':'end_turn'})
for line in sys.stdin:
    q=json.loads(line)
    with log.open('a') as f: f.write(json.dumps(q)+'\n')
    if q['type']=='control_request':
        if q['request'].get('subtype')=='initialize': hooks=q['request'].get('hooks',{})
        send({'type':'control_response','response':{'subtype':'success','request_id':q['request_id'],'response':{'models':models,'commands':[],'agents':[],'output_style':'default','available_output_styles':['default'],'account':{}}}})
    elif q['type']=='control_response' and q['response'].get('request_id')=='fixture-hook':
        result()
    elif q['type']=='user':
        model=arg('--model','claude-opus-5'); model='claude-opus-5' if model=='opus' else model
        effort=arg('--effort','xhigh')
        prompt=q['message']['content']
        variant=prompt if isinstance(prompt,str) else ''
        init={'type':'system','subtype':'init','session_id':'fixture-session','model':model,'effort':effort,'permissionMode':'default','claude_code_version':'fixture-version','tools':[],'mcp_servers':[],'uuid':'fixture-init','cwd':str(Path.cwd()),'apiKeySource':'fixture','slash_commands':[],'output_style':'default','skills':[],'plugins':[]}
        if variant.startswith('hook-'): init.pop('effort')
        send(init)
        if variant.startswith('hook-'):
            child=variant.startswith('hook-child:')
            event='Stop' if variant.startswith('hook-stop:') else 'PreToolUse'
            registrations=hooks.get(event,[])
            callback=registrations[0]['hookCallbackIds'][0] if registrations else None
            if child:
                send({'type':'assistant','session_id':'fixture-session','parent_tool_use_id':'native-child-tool','uuid':'child-message','message':{'id':'child-message','type':'message','role':'assistant','model':'other-child-model','content':[],'usage':{'input_tokens':1,'output_tokens':1}}})
            if callback:
                data={'hook_event_name':event,'session_id':'fixture-session','transcript_path':'fixture','cwd':str(Path.cwd()),'effort':{'level':variant.split(':',1)[1]}}
                if event=='PreToolUse': data.update(tool_name='Read',tool_input={'file_path':'fixture'},tool_use_id='fixture-tool')
                else: data['stop_hook_active']=False
                if child: data['agent_id']='native-child'
                send({'type':'control_request','request_id':'fixture-hook','request':{'subtype':'hook_callback','callback_id':callback,'input':data,'tool_use_id':'fixture-tool'}})
            else: result()
        else: result()
