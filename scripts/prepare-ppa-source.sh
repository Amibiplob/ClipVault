#!/usr/bin/env bash
set -euo pipefail
cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.."
# Unsigned source preparation only. Never access a keyring or upload anything.
online=()
orig=''
while (($#)); do
    case "$1" in
        --online) online=(--online); shift ;;
        --orig)
            [[ $# -ge 2 && -f $2 ]] || { echo '--orig requires an existing trusted orig.tar.xz' >&2; exit 2; }
            orig=$(realpath -- "$2"); shift 2 ;;
        *) echo 'Usage: prepare-ppa-source.sh [--online | --orig /path/to/clipvault_VERSION.orig.tar.xz]' >&2; exit 2 ;;
    esac
done
[[ -z $orig || ${#online[@]} == 0 ]] || { echo '--online and --orig are mutually exclusive' >&2; exit 2; }
series=${UBUNTU_SERIES:-noble}
[[ $series == noble ]] || { echo 'Only noble (Ubuntu 24.04 / Mint 22.x), amd64, is prepared.' >&2; exit 2; }
for tool in python3 cargo rustc dpkg-parsechangelog dpkg-buildpackage; do
    command -v "$tool" >/dev/null || { echo "Missing $tool; see packaging/LAUNCHPAD-PPA.md" >&2; exit 1; }
done
version=$(python3 -c 'import tomllib; print(tomllib.load(open("Cargo.toml","rb"))["package"]["version"])')
deb_version=$(dpkg-parsechangelog -l debian/changelog -S Version)
[[ $(dpkg-parsechangelog -l debian/changelog -S Source) == clipvault &&
   $(dpkg-parsechangelog -l debian/changelog -S Distribution) == noble &&
   $deb_version == "$version"-* && $deb_version != *:* ]] || {
    echo 'Changelog must describe clipvault, the current upstream version, and noble (without an epoch).' >&2; exit 2;
}
mkdir -p target/ppa-source
work=$(mktemp -d "$PWD/target/ppa-source/${deb_version}-XXXXXXXX")
echo "Source work directory: $work"
# Preserve the RPM source archive, signed files, and earlier attempts.
if [[ -z $orig ]]; then
    python3 scripts/prepare-source.py "${online[@]}" --linux-ppa --output-dir "$work/upstream"
    orig="$work/upstream/clipvault-$version.tar.xz"
fi
cp -- "$orig" "$work/clipvault_$version.orig.tar.xz"
tar -xJf "$work/clipvault_$version.orig.tar.xz" -C "$work"
source="$work/clipvault-$version"
test -d "$source/vendor"
test -s "$source/.cargo/config.toml"
[[ $(python3 -c 'import sys,tomllib; print(tomllib.load(open(sys.argv[1],"rb"))["package"]["version"])' "$source/Cargo.toml") == "$version" ]]
cmp Cargo.lock "$source/Cargo.lock"
mkdir "$work/empty-cargo-home"
(cd "$source" && CARGO_HOME="$work/empty-cargo-home" cargo metadata --locked --offline --format-version 1 > "$work/cargo-metadata.json")
cp -a debian "$source/debian"
if [[ -n ${DEBFULLNAME:-} || -n ${DEBEMAIL:-} ]]; then
    : "${DEBFULLNAME:?Set both DEBFULLNAME and DEBEMAIL}"
    : "${DEBEMAIL:?Set both DEBFULLNAME and DEBEMAIL}"
    python3 - "$source" <<'PY'
import os, pathlib, re, sys
root = pathlib.Path(sys.argv[1]) / 'debian'
name, email = os.environ['DEBFULLNAME'], os.environ['DEBEMAIL']
if any(c in name + email for c in '\r\n<>') or not re.fullmatch(r'[^\s@]+@[^\s@]+\.[^\s@]+', email) or email.endswith('.invalid'):
    raise SystemExit('Provide your real public signing identity and Launchpad-verified email')
identity = f'{name} <{email}>'
p = root / 'control'
p.write_text(re.sub(r'^Maintainer: .*$', lambda m: 'Maintainer: ' + identity, p.read_text(), count=1, flags=re.M))
p = root / 'changelog'
p.write_text(re.sub(r'^ -- .*?  ', lambda m: ' -- ' + identity + '  ', p.read_text(), count=1, flags=re.M))
PY
else
    echo 'WARNING: placeholder maintainer; rebuild with DEBFULLNAME and DEBEMAIL before signing/uploading.' >&2
fi
chmod 755 "$source/debian/rules" "$source/debian/clipvault.postinst" "$source/debian/clipvault.postrm"
(cd "$source" && dpkg-buildpackage -S -sa -us -uc -d -nc)
(cd "$work" && sha256sum ./*.orig.tar.xz ./*.debian.tar.xz ./*.dsc ./*_source.changes ./*_source.buildinfo > SHA256SUMS.unsigned)
printf '%s\n' "$work" > target/ppa-source/latest-path.txt
echo "Unsigned source outputs: $work/clipvault_${deb_version}_source.changes"
echo 'Not signed or published. Run lintian and a clean Noble build, then sign/upload in your regular Linux Mint Terminal.'
