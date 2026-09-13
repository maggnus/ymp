import argparse, fcntl, json, os, pathlib, pty, re, select, shlex, struct, subprocess, tempfile, termios, time
p=argparse.ArgumentParser();p.add_argument('binary');p.add_argument('--baseline',action='store_true');args=p.parse_args()
binary=str(pathlib.Path(args.binary).resolve());root=pathlib.Path(tempfile.mkdtemp(prefix='ymp131-pty-'));home=root/'metadata';project=root/"project ' $() literal";elsewhere=root/'other launch directory'
for d in [home,project,elsewhere]: d.mkdir()
(home/'config.toml').write_text('''version = 1
team = ["one", "two"]
[[providers]]
id = "mock"
kind = "mock"
command = "internal"
[[agents]]
id = "one"
name = "One"
provider = "mock"
[[agents]]
id = "two"
name = "Two"
provider = "mock"
''')
result=subprocess.run([binary,'--home',str(home),'-C',str(project),'demo'],capture_output=True,text=True,timeout=30)
assert result.returncode==0, result.stderr[-1500:]
sid=re.search(r'^Session: (\S+)',result.stdout,re.M).group(1)
def launch(session=True):
 master,slave=pty.openpty();fcntl.ioctl(slave,termios.TIOCSWINSZ,struct.pack('HHHH',50,240,0,0))
 command=[binary,'--home',str(home),'-C',str(elsewhere)]
 if session: command+=['resume',sid]
 proc=subprocess.Popen(command,stdin=slave,stdout=slave,stderr=slave,start_new_session=True);os.close(slave)
 return proc,master,bytearray()
def read(proc,fd,buf,seconds):
 end=time.monotonic()+seconds
 while time.monotonic()<end:
  if select.select([fd],[],[],min(.05,max(0,end-time.monotonic())))[0]:
   try: data=os.read(fd,65536)
   except OSError: break
   if not data: break
   buf.extend(data)
def finish(proc,fd,buf,label,session=True):
 read(proc,fd,buf,12);assert proc.poll()==0,(label,'did not exit normally')
 data=buf.decode(errors='replace');(root/(label+'.log')).write_text(data)
 if session:
  marker='Resume this session with:';assert marker in data,(label,'no resume hint')
  assert 0 <= data.rfind('\x1b[?1049l') < data.index(marker),(label,'hint printed before terminal restoration')
  tail=re.sub(r'\x1b\[[0-?]*[ -/]*[@-~]','',data.split(marker,1)[1]);line=next(x.strip() for x in tail.splitlines() if x.strip())
  words=shlex.split(line);assert sid in words,(label,line)
  stubdir=root/'bin';stubdir.mkdir(exist_ok=True);stub=stubdir/'ymp'
  stub.write_text('#!/usr/bin/env python3\nimport json,sys\nprint(json.dumps(sys.argv[1:]))\n');stub.chmod(0o755)
  env=dict(os.environ);env['PATH']=str(stubdir)+os.pathsep+env.get('PATH','')
  quoted=subprocess.run(['/bin/sh','-c',line],capture_output=True,text=True,env=env,timeout=5)
  assert quoted.returncode==0 and json.loads(quoted.stdout)==words[1:],(label,'command did not survive real shell parsing')
  assert pathlib.Path(words[words.index('-C')+1]).resolve()==project.resolve(),(label,'wrong project',line)
  assert pathlib.Path(words[words.index('--home')+1]).resolve()==home.resolve(),(label,'wrong metadata',line)
 else: assert 'Resume this session with:' not in data
 os.close(fd)
results=[]
for case in (['double'] if args.baseline else ['double','input-reset','expiry','quit','sessionless']):
 proc,fd,buf=launch(case!='sessionless')
 try:
  read(proc,fd,buf,.8);assert proc.poll() is None,(case,'startup failed',buf.decode(errors='replace')[-1500:])
  if case=='quit': os.write(fd,b'/quit\r')
  else:
   os.write(fd,b'\x03');read(proc,fd,buf,.25)
   if args.baseline:
    assert 'Press Ctrl-C again to exit' not in buf.decode(errors='replace'),'old behavior unexpectedly has confirmation hint'
    (root/'baseline.log').write_bytes(buf)
    results.append({'case':case,'expected_old_failure':'first Ctrl+C has no confirmation hint','process_exited':proc.poll() is not None,'alternate_screen_restored':b'\x1b[?1049l' in buf});os.close(fd);break
   assert proc.poll() is None,(case,'first Ctrl+C exited')
   assert 'Press Ctrl-C again to exit' in buf.decode(errors='replace'),(case,'missing hint')
   if case=='input-reset': os.write(fd,b'x');read(proc,fd,buf,.1)
   if case=='expiry': read(proc,fd,buf,2.3)
   if case in ['input-reset','expiry']:
    os.write(fd,b'\x03');read(proc,fd,buf,.15);assert proc.poll() is None,(case,'sequence did not reset')
   os.write(fd,b'\x03')
  finish(proc,fd,buf,case,case!='sessionless');results.append({'case':case,'passed':True})
 finally:
  if proc.poll() is None: proc.kill();proc.wait()
report={'binary':binary,'fixture_directory':str(root),'session':sid,'cases':results,'native_inference':False}
print(json.dumps(report,indent=2));(root/'report.json').write_text(json.dumps(report,indent=2)+'\n')
