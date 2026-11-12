#!/usr/bin/env bash
set -euo pipefail
cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.."
# No signing/uploading here. Review, clean-build and sign in separate steps.
series=${UBUNTU_SERIES:-noble}
[[ $series == noble ]] || { echo 'Only noble (Ubuntu 24.04 / Mint 22.x), amd64, is prepared.' >&2; exit 2; }
version=$(python3 -c 'import tomllib; print(tomllib.load(open("Cargo.toml","rb"))["package"]["version"])')
python3 scripts/prepare-source.py "$@"
work="$PWD/target/ppa-source"
mkdir -p "$work"
archive="$PWD/target/source-release/clipvault-$version.tar.xz"
cp -- "$archive" "$work/clipvault_$version.orig.tar.xz"
rm -rf -- "$work/clipvault-$version"
tar -xJf "$archive" -C "$work"
cp -a debian "$work/clipvault-$version/debian"
chmod 755 "$work/clipvault-$version/debian/rules" "$work/clipvault-$version/debian/clipvault.postinst" "$work/clipvault-$version/debian/clipvault.postrm"
(cd "$work/clipvault-$version" && dpkg-buildpackage -S -sa -us -uc -d -nc)
echo "Unsigned source outputs: $work/clipvault_${version}-1~ppa1~noble1_source.changes"
echo 'Not published: run lintian and a clean sbuild, then review identity, sign and upload as documented.'
