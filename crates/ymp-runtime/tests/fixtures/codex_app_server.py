#!/usr/bin/env python3
"""Synthetic 0.156.1 process protocol. No model, network or authentication access."""
import json, os, sys, time
from pathlib import Path
if '--version' in sys.argv:
    if Path(__file__).with_suffix('.mode').exists() and Path(__file__).with_suffix('.mode').read_text()=='orphan-version':
        if os.fork()==0:time.sleep(30);sys.exit(0)
    print('codex-cli 0.156.1')
    sys.exit(0)
path=Path(__file__)
sys.excepthook=lambda kind,value,trace: path.with_suffix('.error').write_text(str(kind)+': '+str(value)+' at '+str(trace.tb_lineno))
mode_path=path.with_suffix('.mode')
def mode(): return mode_path.read_text().strip() if mode_path.exists() else 'normal'
def log(value):
    with path.with_suffix('.log').open('a') as out: out.write(value+'\n')
def send(value): print(json.dumps(value),flush=True)
def reply(i,value):send({'id':i,'result':value})
def notify(method,params):send({'method':method,'params':params})
flags={}
for arg in sys.argv:
    if arg.startswith('features.') and '=' in arg:
        key,value=arg.split('=',1);flags[key[9:]]=value=='true'
thread='native-fixture-thread';turn=0;cwd=os.getcwd();total=0;phase=0

def usage(n):return {'inputTokens':n*5,'cachedInputTokens':n*2,'cacheWriteInputTokens':n,'outputTokens':n,'reasoningOutputTokens':n,'totalTokens':n*6}
def stage_usage(stage):
    previous=usage(turn-1);previous['inputTokens']+=stage;previous['totalTokens']+=stage
    last={'inputTokens':stage,'cachedInputTokens':0,'cacheWriteInputTokens':0,'outputTokens':0,'reasoningOutputTokens':0,'totalTokens':stage}
    notify('thread/tokenUsage/updated',{**identity(),'tokenUsage':{'total':previous,'last':last}})
def identity():return {'threadId':thread,'turnId':f'turn-{turn}'}
def report_usage(n):notify('thread/tokenUsage/updated',{**identity(),'tokenUsage':{'total':usage(n),'last':usage(1)}})
def thread_data():return {'id':thread,'cliVersion':'0.156.1','environments':[],'status':{'type':'idle'},'turns':[{'id':f'turn-{turn}','status':'completed','items':[]} ] if turn else []}
def final():
    intermediate=usage(turn);intermediate['inputTokens']-=1;intermediate['totalTokens']-=1
    last=usage(1);last['inputTokens']-=1;last['totalTokens']-=1
    for _ in range(2):notify('thread/tokenUsage/updated',{**identity(),'tokenUsage':{'total':intermediate,'last':last}})
    # The final cumulative usage arrives only before the explicit state reply.
    params={**identity(),'item':{'id':f'answer-{turn}','type':'agentMessage','phase':'final_answer','text':'{"ok":true}'},'completedAtMs':0}
    if mode()=='foreign':params.pop('threadId')
    notify('item/completed',params)
    if mode()=='disconnect':sys.exit(0)
    notify('turn/completed',{'threadId':thread,'turn':{'id':f'turn-{turn}','status':'completed','items':[]}})
for line in sys.stdin:
    message=json.loads(line);method=message.get('method');i=message.get('id');p=message.get('params',{})
    if method=='initialize':reply(i,{'userAgent':'fixture/0.156.1'})
    elif method=='initialized':pass
    elif method=='config/read':
        if mode()=='pause':
            path.with_suffix('.setup').write_text('waiting')
            while not path.with_suffix('.release').exists():time.sleep(.002)
        enabled='mcp_servers.ambient.enabled=false' not in sys.argv
        reply(i,{'config':{'mcp_servers':{'ambient':{'enabled':enabled}},'notify':[],'web_search':'disabled','approval_policy':'never'}})
    elif method=='experimentalFeature/list':
        rows=[{'name':k,'enabled':v,'defaultEnabled':True,'stage':'stable'} for k,v in flags.items()]
        if mode()=='bad-guard':rows.append({'name':'hooks','enabled':True})
        reply(i,{'data':rows,'nextCursor':None})
    elif method=='configRequirements/read':reply(i,{'requirements':None})
    elif method=='account/read':reply(i,{'account':{'type':'apiKey'},'requiresOpenaiAuth':True})
    elif method=='model/list':reply(i,{'data':[{'id':'picker-fixture','model':'fixture-codex','supportedReasoningEfforts':[{'reasoningEffort':'low'}],'defaultReasoningEffort':'low','isDefault':True}],'nextCursor':None})
    elif method in ['thread/start','thread/resume']:
        assert p['cwd']==cwd and p['runtimeWorkspaceRoots']==[] and p['approvalPolicy']=='never'
        if method=='thread/start':assert p['environments']==[] and len(p['dynamicTools'])==2
        else:assert p['threadId']==thread
        log(method)
        reply(i,{'thread':thread_data(),'model':p['model'],'modelProvider':'openai','cwd':cwd,'approvalPolicy':'never','approvalsReviewer':'user','sandbox':{'type':'readOnly','networkAccess':False},'runtimeWorkspaceRoots':[]})
    elif method=='turn/start':
        assert p['threadId']==thread and p['environments']==[] and p['runtimeWorkspaceRoots']==[] and p['model']=='fixture-codex' and p['effort']=='low'
        turn+=1;log('turn/start');reply(i,{'turn':{'id':f'turn-{turn}','status':'inProgress','items':[]}})
        if 'files' in p['input'][0]['text']:
            phase=1;send({'id':'tool-one','method':'item/tool/call','params':{**identity(),'callId':f'write-{turn}','tool':'ymp_write','arguments':{'path':'file','text':'native fixture'}}})
        else:final()
    elif method=='thread/read':
        # Native state barrier receives a late, equal cumulative report before reply.
        if mode()=='bad-final':
            broken=usage(turn);broken['inputTokens']=-1
            notify('thread/tokenUsage/updated',{**identity(),'tokenUsage':{'total':broken,'last':usage(1)}})
        elif mode()=='stale-usage':
            old=usage(turn);old['inputTokens']-=1;old['totalTokens']-=1
            last=usage(1);last['inputTokens']-=1;last['totalTokens']-=1
            notify('thread/tokenUsage/updated',{**identity(),'tokenUsage':{'total':old,'last':last}})
        else:report_usage(turn)
        reply(i,{'thread':thread_data()})
    elif method=='turn/interrupt':reply(i,{}) # ACK deliberately does not fabricate terminal.
    elif method is None and i=='tool-one':
        assert message['result']['success'], message['result'];phase=2
        if mode()!='missing-early':stage_usage(1)
        send({'id':'tool-new-write','method':'item/tool/call','params':{**identity(),'callId':f'new-write-{turn}','tool':'ymp_write','arguments':{'path':'file','text':'later native fixture'}}})
    elif method is None and i=='tool-new-write':
        assert message['result']['success'];phase=3
        stage_usage(2)
        send({'id':'tool-repeat','method':'item/tool/call','params':{**identity(),'callId':f'write-{turn}','tool':'ymp_write','arguments':{'path':'file','text':'native fixture'}}})
    elif method is None and i=='tool-repeat':
        assert message['result']['success'];phase=3
        send({'id':'tool-denied','method':'item/tool/call','params':{**identity(),'callId':f'escape-{turn}','tool':'ymp_read','arguments':{'path':'../escape','limit':64}}})
    elif method is None and i=='tool-denied':assert not message['result']['success'];final()
    else:raise RuntimeError('Unexpected host method')
