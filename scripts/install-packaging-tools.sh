#!/usr/bin/env bash
set -euo pipefail
cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.."
tools="$PWD/target/packaging-tools"
mkdir -p "$tools/bin"
if [[ ! -x $tools/bin/cargo-deb ]] || ! "$tools/bin/cargo-deb" --version | grep -q '3.8.0'; then
    cargo install cargo-deb --version 3.8.0 --locked --root "$tools"
fi
python3 - <<'PY'
import hashlib, json, pathlib, subprocess
lock = json.loads(pathlib.Path('packaging/tools.lock.json').read_text())
for name in ['linuxdeploy', 'linuxdeploy-plugin-appimage']:
    config = lock[name]
    target = pathlib.Path(f'target/packaging-tools/bin/{name}-x86_64.AppImage')
    if target.is_file() and hashlib.sha256(target.read_bytes()).hexdigest() == config['sha256']:
        continue
    part = target.with_suffix('.part')
    subprocess.run(['curl', '--fail', '--location', '--retry', '3', '--proto', '=https',
                    '--connect-timeout', '20', config['url'], '--output', str(part)], check=True)
    if hashlib.sha256(part.read_bytes()).hexdigest() != config['sha256']:
        part.unlink(missing_ok=True)
        raise SystemExit(f'{name}: upstream bytes changed. Review and update packaging/tools.lock.json; refusing unpinned executable.')
    part.replace(target)
    target.chmod(0o755)
PY
printf 'Verified pinned packaging tools in %s/bin\n' "$tools"
