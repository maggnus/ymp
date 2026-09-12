#!/usr/bin/env python3
"""Build a temporary probe against unchanged local crates, offline."""
import json,subprocess,tempfile
from pathlib import Path
root=Path(__file__).resolve().parents[3]
with tempfile.TemporaryDirectory(prefix='ymp-context-probe-',dir='/tmp') as directory:
    crate=Path(directory)
    (crate/'src').mkdir()
    deps=''
    for name in ('ymp-core','ymp-storage','ymp-workspace'):
        deps+=f'{name} = {{ path = {json.dumps(str(root/"ymp-rust/crates"/name))} }}\n'
    (crate/'Cargo.toml').write_text('[package]\nname="ymp-context-probe"\nversion="0.0.0"\nedition="2021"\n[dependencies]\n'+deps+'serde_json="1"\ntempfile="3"\nwalkdir="2"\n')
    (crate/'src/main.rs').write_bytes(Path(__file__).with_name('context_probe.rs').read_bytes())
    result=subprocess.run(['cargo','run','--offline','--release','--quiet','--manifest-path',str(crate/'Cargo.toml'),'--target-dir',str(root/'target'),'--',str(root)],capture_output=True,text=True)
    if result.returncode:
        raise SystemExit(result.stderr)
    evidence=json.loads(result.stdout)
    evidence['build_profile']='release'
    (root/'ymp-docs/research/evidence/context-probe.json').write_text(json.dumps(evidence,indent=2)+'\n')
    print(json.dumps(evidence))
