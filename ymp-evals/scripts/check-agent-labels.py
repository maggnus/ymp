import argparse,json,pathlib,re,subprocess,tempfile
p=argparse.ArgumentParser();p.add_argument('binary');p.add_argument('--baseline',action='store_true');p.add_argument('--reported-effort',choices=['max','on','none','missing'],default='max');args=p.parse_args()
root=pathlib.Path(tempfile.mkdtemp(prefix='ymp132-cli-'));home=root/'home';project=root/'project';home.mkdir();project.mkdir();stub=root/'provider.py'
stub.write_text('''import json,sys,socket,time
native_effort=__NATIVE_EFFORT__
mcp=None
for line in sys.stdin:
 q=json.loads(line)
 if "id" not in q: continue
 method=q["method"];result={}
 if method in ["session/new","session/load"]: mcp=q["params"]["mcpServers"][0]
 if method=="initialize": result={"protocolVersion":1,"agentCapabilities":{"loadSession":True}}
 elif method in ["session/new","session/load"]: result={"sessionId":"fixture-session","models":{"currentModelId":"native-model-z","availableModels":[{"modelId":"native-model-z","name":"Recommended descriptive caption"}]},"modes":{"currentModeId":"default","availableModes":[{"id":"default","name":"Default permissions"},{"id":"bypass_permissions","name":"Fixture write"}]},"configOptions":[{"id":"thought_level","type":"select","currentValue":"max","options":[{"value":"max","name":"Maximum"},{"value":"low","name":"Low"}]}]}
 elif method=="session/prompt":
  prompt=q["params"]["prompt"][0]["text"]
  time.sleep(0.1)
  endpoint=mcp["args"][mcp["args"].index("--socket")+1]
  token=next(e["value"] for e in mcp["env"] if e["name"]=="YMP_MCP_TOKEN")
  with socket.socket(socket.AF_UNIX,socket.SOCK_STREAM) as connection:
   connection.connect(endpoint)
   payload={"token":token,"request_id":"fixture-chat","name":"team_post","arguments":{"text":"Fixture shared finding"}}
   connection.sendall(json.dumps(payload).encode()+bytes([10]))
   reply=json.loads(connection.makefile("rb").readline())
   assert reply["ok"]
  if "current assignment (plan)" in prompt: text=json.dumps({"summary":"Presentation fixture","tasks":[{"title":"Report a fixture fact","description":"Return a short fixture response","competence":"implementation","difficulty":"simple","dependencies":[],"checks":[]}]})
  elif "current assignment (review_plan)" in prompt or "current assignment (review)" in prompt or "current assignment (final_review)" in prompt: text=json.dumps({"approved":True,"reason":"Fixture response is present"})
  else: text="Fixture response complete"
  print(json.dumps({"method":"session/update","params":{"sessionId":"fixture-session","update":{"sessionUpdate":"agent_message_chunk","content":{"text":text}}}}),flush=True)
  result={"stopReason":"end_turn","usage":{"inputTokens":1,"outputTokens":1,"thoughtTokens":0}}
 if method in ["session/new","session/load"]:
  if native_effort is None: result["configOptions"]=[]
  else:
   option=result["configOptions"][0];option["currentValue"]=native_effort
   if native_effort not in [v["value"] for v in option["options"]]: option["options"].append({"value":native_effort,"name":native_effort})
 print(json.dumps({"jsonrpc":"2.0","id":q["id"],"result":result}),flush=True)
'''.replace('__NATIVE_EFFORT__',repr(None if args.reported_effort=='missing' else args.reported_effort)))
(home/'config.toml').write_text('''version = 1
team = ["transport-one", "transport-two"]
[limits]
turns = 20
parallel = 2
attempts = 1
turn_timeout_secs = 10
[[providers]]
id = "fixture-provider"
kind = "acp"
command = "python3"
args = ['''+json.dumps(str(stub))+''']
[[agents]]
id = "transport-one"
name = "Default (recommended)"
provider = "fixture-provider"
[[agents]]
id = "transport-two"
name = "Recommended descriptive caption"
provider = "fixture-provider"
''')
r=subprocess.run([args.binary,'--home',str(home),'-C',str(project),'run','Report the fixture fact','--no-memory','--no-adaptive'],capture_output=True,text=True,timeout=90)
text=r.stdout+r.stderr;(root/'output.log').write_text(text)
assert r.returncode==0,(r.returncode,text[-2000:])
headers=[h for h in re.findall(r'^\[(.*?)\]$',text,re.M) if any(h.endswith(' · '+kind) for kind in ['plan','review_plan','execute','review','final_review','synthesis','chat'])]
assert headers, text
suffix='' if args.reported_effort in ['missing','on'] else ' '+args.reported_effort
expected='native-model-z'+suffix+' · '
correct=all(h.startswith(expected) and 'transport-' not in h and 'descriptive caption' not in h for h in headers)
assert correct != args.baseline,headers
report={'binary':str(pathlib.Path(args.binary).resolve()),'directory':str(root),'reported_effort':args.reported_effort,'headers':headers,'native_inference':False,'result':'expected old labeling failure' if args.baseline else 'passed'}
(root/'report.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report,indent=2))
