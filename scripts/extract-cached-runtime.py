#!/usr/bin/env python3
"""Reuse the unmodified type-2 runtime already embedded in trusted linuxdeploy.

No network access; --runtime-file is appimagetool's supported offline input.
"""
import hashlib
import json
from pathlib import Path
import subprocess
root=Path(__file__).resolve().parents[1]
source=root/'target/packaging-tools/bin/linuxdeploy-x86_64.AppImage'
offset=int(subprocess.check_output([str(source),'--appimage-offset'],text=True).strip())
with source.open('rb') as f:
    runtime=f.read(offset)
assert len(runtime)==offset and len(runtime)>64
assert runtime[:4]==b'\x7fELF' and runtime[8:11]==b'AI\x02'
assert runtime[4]==2 and int.from_bytes(runtime[18:20],'little')==62
output=root/'target/packaging-tools/runtime-x86_64'
output.write_bytes(runtime)
output.chmod(0o755)
version=subprocess.check_output([str(source),'--appimage-version'],stderr=subprocess.STDOUT,text=True).strip()
notice=subprocess.check_output([str(source),'--appimage-help'],stderr=subprocess.STDOUT,text=True)
provenance={'source':str(source.relative_to(root)),'source_sha256':hashlib.sha256(source.read_bytes()).hexdigest(),
            'runtime_bytes':offset,'runtime_sha256':hashlib.sha256(runtime).hexdigest(),'version':version,
            'notice':'Original runtime licensing information and upstream links follow. Runtime prefix is copied unmodified.\n'+notice}
output.with_suffix('.json').write_text(json.dumps(provenance,indent=2)+'\n')
print(version)
print(f'Unmodified cached runtime: {output}')
print('Use: LDAI_RUNTIME_FILE="$PWD/target/packaging-tools/runtime-x86_64" bash scripts/package-linux.sh all')
