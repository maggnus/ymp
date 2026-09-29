#!/usr/bin/env python3
"""Synthetic stream-json process protocol. No model, network or authentication access."""
import json, os, sys, time
from pathlib import Path
VERSION='0.0.0-fixture'
if '--version' in sys.argv:
    print(VERSION+' (Claude Code)')
    sys.exit(0)
path=Path(__file__)
sys.excepthook=lambda kind,value,trace: path.with_suffix('.error').write_text(str(kind)+': '+str(value)+' at '+str(trace.tb_lineno))
mode_path=path.with_suffix('.mode')
MODE=mode_path.read_text().strip() if mode_path.exists() else 'normal'
MODEL='fixture-haiku'
def log(value):
    with path.with_suffix('.log').open('a') as out: out.write(value+'\n')
def send(value): print(json.dumps(value),flush=True)
def receive():
    line=sys.stdin.readline()
    if not line:sys.exit(0)
    return json.loads(line)
def option(name):
    return sys.argv[sys.argv.index(name)+1] if name in sys.argv else None
assert not [k for k in os.environ if k!='CLAUDE_CONFIG_DIR' and (k.startswith('CLAUDE') or k.startswith('ANTHROPIC'))]
for flag in ['--print','--verbose','--safe-mode','--strict-mcp-config','--disable-slash-commands','--no-session-persistence']:
    assert flag in sys.argv, flag
assert option('--input-format')=='stream-json' and option('--output-format')=='stream-json'
assert option('--tools')=='' and option('--setting-sources')=='' and option('--permission-mode')=='dontAsk'
assert option('--effort') is None and int(option('--max-turns'))>=1
assert json.loads(option('--mcp-config'))=={'mcpServers':{'ymp':{'type':'sdk','name':'ymp'}}}
allowed=[name for name in (option('--allowed-tools') or '').split(',') if name]
session='fixture-session';serial=0;offered=[];request_count=0
def host(method,params=None,notification=False):
    """One request to the in-channel host tool server; returns its JSON-RPC reply."""
    global serial
    serial+=1
    message={'jsonrpc':'2.0','method':method}
    if params is not None:message['params']=params
    if not notification:message['id']=serial
    send({'type':'control_request','request_id':f'fixture-{serial}','request':{'subtype':'mcp_message','server_name':'ymp','message':message}})
    while True:
        answer=receive()
        if answer['type']=='control_request' and answer['request']['subtype']=='interrupt':
            send({'type':'control_response','response':{'subtype':'success','request_id':answer['request_id'],'response':{}}})
            continue
        assert answer['type']=='control_response' and answer['response']['request_id']==f'fixture-{serial}', answer
        return answer['response']['response']['mcp_response']
def tool(identifier,name,arguments):
    send({'type':'assistant','session_id':session,'parent_tool_use_id':None,'message':{'id':f'message-{identifier}','model':MODEL,'role':'assistant','content':[{'type':'tool_use','id':identifier,'name':f'mcp__ymp__{name}','input':arguments}],'usage':{'input_tokens':1,'cache_read_input_tokens':1,'cache_creation_input_tokens':0,'output_tokens':1}}})
    reply=host('tools/call',{'name':name,'arguments':arguments,'_meta':{'claudecode/toolUseId':identifier}})['result']
    send({'type':'user','session_id':session,'parent_tool_use_id':None,'message':{'role':'user','content':[{'type':'tool_result','tool_use_id':identifier,'content':reply['content'],'is_error':reply['isError']}]}})
    return reply
def result(calls):
    # Streamed output counts are placeholders; only this final report is complete.
    # The file scenario fences its whole answer, as light models sometimes do.
    usage={'input_tokens':calls,'cache_read_input_tokens':calls,'cache_creation_input_tokens':1,'output_tokens':1,'output_tokens_details':{'thinking_tokens':1}}
    own={'inputTokens':calls,'cacheReadInputTokens':calls,'cacheCreationInputTokens':1,'outputTokens':1,'thinkingTokens':1,'costUSD':0.5}
    models={MODEL:own}
    if MODE=='partial-usage':models['fixture-other']=dict(own)
    send({'type':'result','subtype':'success','is_error':False,'terminal_reason':'completed','num_turns':calls,'session_id':session,'result':'```json\n{"ok":true}\n```' if calls>1 else '{"ok":true}','total_cost_usd':0.5,'usage':usage,'modelUsage':models,'permission_denials':[]})
while True:
    message=receive()
    if message['type']=='control_request':
        request=message['request']
        if request['subtype']=='initialize':
            assert request['sdkMcpServers']==['ymp'] and request['hooks'] is None
            if MODE=='pause':
                path.with_suffix('.setup').write_text('waiting')
                while not path.with_suffix('.release').exists():time.sleep(.002)
            assert host('initialize',{'protocolVersion':'2025-11-25','capabilities':{},'clientInfo':{'name':'fixture','version':VERSION}})['result']['serverInfo']['name']=='ymp'
            # The account block is native identity; an adapter must not retain it.
            send({'type':'control_response','response':{'subtype':'success','request_id':message['request_id'],'response':{'commands':[],'agents':[],'pid':os.getpid(),'account':{'email':'fixture@example.invalid','apiProvider':'firstParty'},'models':[
                {'value':'default','resolvedModel':'fixture-large','displayName':'Default','supportsEffort':True,'supportedEffortLevels':['low','high']},
                {'value':'large','resolvedModel':'fixture-large','displayName':'Large','supportsEffort':True,'supportedEffortLevels':['low','high']},
                {'value':'haiku','resolvedModel':MODEL,'displayName':'Light'}]}}})
            host('notifications/initialized',notification=True)
            offered=[entry['name'] for entry in host('tools/list')['result']['tools']]
            assert sorted(f'mcp__ymp__{name}' for name in offered)==sorted(allowed), (offered,allowed)
        elif request['subtype']=='mcp_status':
            # Registration settles asynchronously in the native process.
            settled=request_count>0;request_count+=1
            servers=[{'name':'ymp','status':'connected','scope':'dynamic','source':'sdk','tools':[{'name':name,'annotations':{}} for name in offered]}] if settled else []
            if MODE=='ambient':servers.append({'name':'ambient','status':'connected','source':'user','tools':[]})
            send({'type':'control_response','response':{'subtype':'success','request_id':message['request_id'],'response':{'mcpServers':servers}}})
        else:
            send({'type':'control_response','response':{'subtype':'success','request_id':message['request_id'],'response':{}}})
    elif message['type']=='user':
        assert option('--model')==MODEL and message['parent_tool_use_id'] is None
        log('inference')
        send({'type':'system','subtype':'init','session_id':session,'cwd':os.path.realpath(os.getcwd()),'tools':allowed+(['Bash'] if MODE=='surface' else []),'mcp_servers':[{'name':'ymp','status':'connected','source':'sdk'}],'model':MODEL,'permissionMode':'dontAsk','slash_commands':[],'skills':[],'plugins':[{'name':'fixture-builtin','path':'builtin','source':'fixture-builtin@builtin'}]+([{'name':'ambient','path':'/ambient','source':'ambient@user'}] if MODE=='plugin' else []),'agents':['general-purpose'],'apiKeySource':'none','claude_code_version':VERSION})
        calls=0
        if 'files' in message['message']['content']:
            assert tool('use-1','write',{'path':'file','text':'native fixture'})['isError'] is False
            assert tool('use-2','read',{'path':'file','limit':64})['content'][0]['text']=='native fixture'
            assert tool('use-3','read',{'path':'../escape','limit':64})['isError'] is True
            calls=3
        send({'type':'rate_limit_event','session_id':session,'rate_limit_info':{'status':'allowed'}})
        answer={'id':'message-final','model':'fixture-other' if MODE=='model' else MODEL,'role':'assistant','content':[{'type':'text','text':'{"ok":true}'}],'usage':{'input_tokens':1,'cache_read_input_tokens':1,'cache_creation_input_tokens':1,'output_tokens':1}}
        if MODE=='foreign':answer['content']=[{'type':'tool_use','id':'use-foreign','name':'Bash','input':{'command':'true'}}]
        send({'type':'assistant','session_id':session,'parent_tool_use_id':None,'message':answer})
        if MODE=='disconnect':sys.exit(0)
        result(calls+1)
