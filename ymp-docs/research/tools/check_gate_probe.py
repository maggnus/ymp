#!/usr/bin/env python3
"""Demonstrate prefix-gate limits using harmless markers in a temporary directory."""
import json,os,subprocess,tempfile
from pathlib import Path

results=[]
with tempfile.TemporaryDirectory(prefix='yg-',dir='/tmp') as temporary:
    root=Path(temporary)
    bin=root/'bin';bin.mkdir()
    stub=bin/'cargo';stub.write_text('#!/bin/sh\nexit 0\n');stub.chmod(0o755)
    env={**os.environ,'PATH':str(bin)+os.pathsep+os.environ['PATH']}
    for name,command in [('plain','cargo test'),('composition',"cargo test; printf injected > marker"),('substitution','cargo test "$(printf injected > marker)"')]:
        marker=root/'marker'
        marker.unlink(missing_ok=True)
        allowed=command.startswith('cargo test')
        result=subprocess.run(['/bin/sh','-c',command],cwd=root,env=env,capture_output=True,check=True)
        results.append(dict(case=name,prefix_allowed=allowed,unexpected_marker=marker.exists()))
    stub.write_text('#!/bin/sh\nprintf repository_script > marker\n');stub.chmod(0o755)
    (root/'marker').unlink(missing_ok=True)
    subprocess.run([str(stub),'test'],cwd=root,env=env,check=True)
    results.append(dict(case='permitted_program_behavior_without_shell',prefix_allowed=True,unexpected_marker=(root/'marker').exists()))
assert [r['unexpected_marker'] for r in results]==[False,True,True,True]
Path(__file__).resolve().parents[1].joinpath('evidence/check-gate-probe.json').write_text(json.dumps(dict(scope='Controlled stubs and harmless marker files; no real build tools or project code executed.',cases=results),indent=2)+'\n')
print(json.dumps(results))
