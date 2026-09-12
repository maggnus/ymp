import json, sys
kind, log, variant = sys.argv[1:4]
model = 'model-a'
effort = 'high'
def send(v):
    print(json.dumps(v), flush=True)
def options():
    if variant == 'no-controls': return []
    return [{'id':'thought_level','type':'select','currentValue':effort,'options':[{'value':v} for v in (['high','max'] if model == 'model-a' else ['none','on'])]}]
for line in sys.stdin:
    q=json.loads(line)
    with open(log,'a') as f: f.write(json.dumps(q)+'\n')
    method=q['method']; p=q.get('params',{})
    if 'id' not in q: continue
    result={}
    if method=='initialize': result={'agentCapabilities':{'loadSession':True},'agentInfo':{'version':'fixture-1'}}
    elif method=='model/list': result={'data':[{'id':'picker-'+m,'model':m,'isDefault':m=='model-a','defaultReasoningEffort':'high','supportedReasoningEfforts':[{'reasoningEffort':v} for v in ['high','max','future-level']]} for m in ['model-a','model-b']]}
    elif method in ['thread/start','thread/resume']:
        model=p.get('model','model-a'); effort=p.get('config',{}).get('model_reasoning_effort','high')
        result={'thread':{'id':'session'},'model':'model-a' if variant=='model-drift' else model,'reasoningEffort':effort}
    elif method=='turn/start':
        result={'turn':{'id':'turn'}}
    elif method in ['session/new','session/load']:
        result={'sessionId':'session','modes':{'currentModeId':'bypass_permissions','availableModes':[{'id':id,'name':id} for id in (['bypass_permissions'] if variant=='unsafe-modes' else ['default','bypass_permissions'])]},'models':{'currentModelId':model,'availableModels':[{'modelId':m} for m in ['model-a','model-b']]},'configOptions':options()}
    elif method=='session/set_model':
        model=p['modelId']; effort='on' if model=='model-b' else 'high'
        if variant != 'no-refresh': send({'method':'session/update','params':{'sessionId':'session','update':{'sessionUpdate':'config_option_update','configOptions':options()}}})
    elif method=='session/set_config_option':
        effort='on' if variant=='clamp' else p['value']
        result={'configOptions':options()}
    elif method=='session/prompt': result={'stopReason':'end_turn'}
    if method=='session/prompt':
        send({'method':'session/update','params':{'sessionId':'session','update':{'sessionUpdate':'agent_message_chunk','content':{'text':'done'}}}})
    send({'jsonrpc':'2.0','id':q['id'],'result':result})
    if method=='turn/start':
        send({'method':'item/completed','params':{'threadId':'session','turnId':'turn','item':{'type':'agentMessage','text':'done'}}})
        send({'method':'turn/completed','params':{'threadId':'session','turn':{'id':'turn','status':'completed'}}})
