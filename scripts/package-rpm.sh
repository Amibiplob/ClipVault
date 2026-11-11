#!/usr/bin/env bash
set -euo pipefail
cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.."
command -v rpmbuild >/dev/null || { echo 'rpmbuild is required (rpm-build on Fedora, rpm on Ubuntu). No RPM has been built.' >&2; exit 1; }
version=$(python3 -c 'import tomllib; print(tomllib.load(open("Cargo.toml","rb"))["package"]["version"])')
cargo build --release --locked --offline
stage="$PWD/target/rpm-stage"
python3 - "$stage" <<'PY'
import pathlib, shutil, sys, tomllib
root=pathlib.Path.cwd();stage=pathlib.Path(sys.argv[1]);shutil.rmtree(stage,ignore_errors=True)
for source,destination,mode in tomllib.load(open('Cargo.toml','rb'))['package']['metadata']['deb']['assets']:
    output=stage/destination;output.parent.mkdir(parents=True,exist_ok=True)
    shutil.copyfile(root/source,output);output.chmod(int(mode,8))
PY
rpmbuild -bb packaging/linux/clipvault.spec --define "_topdir $PWD/target/rpmbuild" --define "clipvault_version $version" --define "clipvault_stage $stage" --define '_build_id_links none'
mkdir -p dist
find target/rpmbuild/RPMS -name "clipvault-${version}-1*.rpm" -exec cp -- {} dist/ \;
find dist -maxdepth 1 -name "clipvault-${version}-1*.rpm" -exec rpm -K -- {} \;
