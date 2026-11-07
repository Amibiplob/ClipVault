#!/usr/bin/env bash
set -euo pipefail
cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.."
export PATH="$PWD/target/packaging-tools/bin:$PATH"
mode=${1:-all}
case "$mode" in all|deb|appimage|stage) ;; *) echo 'Usage: scripts/package-linux.sh [all|deb|appimage|stage]' >&2; exit 2 ;; esac
[[ $(uname -sm) == 'Linux x86_64' ]] || { echo 'Only Linux x86_64 is supported.' >&2; exit 1; }
# Keep the glibc baseline honest; newer build hosts cannot produce this release.
source /etc/os-release
[[ ${UBUNTU_CODENAME:-${VERSION_CODENAME:-}} == noble ]] || { echo 'Build on Linux Mint 22.x or Ubuntu 24.04 (noble).' >&2; exit 1; }
[[ -z ${CARGO_BUILD_TARGET:-} && -z ${CARGO_TARGET_DIR:-} ]] || { echo 'Unset CARGO_BUILD_TARGET and CARGO_TARGET_DIR for native packaging.' >&2; exit 1; }
if [[ $mode == all || $mode == deb ]]; then
    command -v cargo-deb >/dev/null || { echo 'cargo-deb is missing; run scripts/install-packaging-tools.sh with internet access.' >&2; exit 1; }
fi
if [[ $mode == all || $mode == appimage ]]; then
    linuxdeploy=${LINUXDEPLOY:-$(command -v linuxdeploy-x86_64.AppImage || command -v linuxdeploy || true)}
    [[ -x $linuxdeploy ]] || { echo 'linuxdeploy is missing; run scripts/install-packaging-tools.sh with internet access.' >&2; exit 1; }
    appimage_plugin=${LINUXDEPLOY_PLUGIN_APPIMAGE:-$(command -v linuxdeploy-plugin-appimage-x86_64.AppImage || command -v linuxdeploy-plugin-appimage || true)}
    [[ -x $appimage_plugin ]] || { echo 'linuxdeploy AppImage plugin is missing; run scripts/install-packaging-tools.sh.' >&2; exit 1; }
fi
cargo build --release --locked --offline
python3 scripts/packaging-support.py notices --check
desktop-file-validate packaging/linux/io.clipvault.desktop.desktop
version=$(python3 -c 'import tomllib; print(tomllib.load(open("Cargo.toml", "rb"))["package"]["version"])')
mkdir -p dist
if [[ $mode == all || $mode == deb ]]; then
    deb="$PWD/dist/clipvault_${version}-1_amd64.deb"
    cargo deb --no-build --offline --locked --output "$deb"
    printf 'Created %s\n' "$deb"
fi
if [[ $mode == all || $mode == appimage || $mode == stage ]]; then
    python3 scripts/packaging-support.py stage
fi
if [[ $mode == all || $mode == appimage ]]; then
    appdir="$PWD/target/packaging/ClipVault.AppDir"
    # Force deployment of dlopen libraries, including ones linuxdeploy normally excludes.
    libraries=()
    while IFS= read -r -d '' library; do libraries+=(--library "$library"); done < <(
        python3 - <<'PY'
import json, pathlib, sys
sources = json.loads(pathlib.Path('target/packaging/staged-sources.json').read_text())
for relative, source in sources.items():
    if pathlib.PurePosixPath(relative).parent == pathlib.PurePosixPath('usr/lib'):
        sys.stdout.buffer.write(source.encode() + b'\0')
PY
    )
    export APPIMAGE_EXTRACT_AND_RUN=1
    export ARCH=x86_64
    export VERSION="$version"
    "$linuxdeploy" --appdir "$appdir" --deploy-deps-only "$appdir/usr/bin" \
        --deploy-deps-only "$appdir/usr/lib/gdk-pixbuf-2.0/2.10.0/loaders" \
        --desktop-file packaging/linux/io.clipvault.desktop.desktop --icon-file assets/clipvault.svg \
        --custom-apprun packaging/linux/AppRun "${libraries[@]}"
    LD_LIBRARY_PATH="$appdir/usr/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" python3 scripts/packaging-support.py verify-closure
    # Collect notices after linuxdeploy has added the transitive library closure.
    python3 scripts/packaging-support.py system-notices
    if [[ -n ${LDAI_RUNTIME_FILE:-} ]]; then
        test -s "$LDAI_RUNTIME_FILE"
        runtime_manifest="${LDAI_RUNTIME_FILE%.*}.json"
        # Cached runtime helper has no extension in its output filename.
        if [[ -f ${LDAI_RUNTIME_FILE}.json ]]; then runtime_manifest="${LDAI_RUNTIME_FILE}.json"; fi
        [[ ! -f $runtime_manifest ]] || cp -- "$runtime_manifest" "$appdir/usr/share/doc/clipvault/APPIMAGE-RUNTIME.json"
    fi
    export LDAI_OUTPUT="$PWD/dist/ClipVault-${version}-x86_64.AppImage"
    export LDAI_VERSION="$version"
    # Deployment and notice collection already completed above. Pack directly
    # with the same linuxdeploy output plugin, without redeploying the closure.
    "$appimage_plugin" --appdir "$appdir"
    test -s "$LDAI_OUTPUT"
    chmod 755 "$LDAI_OUTPUT"
    printf 'Created %s\n' "$LDAI_OUTPUT"
fi
if [[ $mode != stage ]]; then
    (cd dist && find . -maxdepth 1 -type f \( -name '*.deb' -o -name '*.AppImage' \) -print0 | sort -z | xargs -0 -r sha256sum > SHA256SUMS)
fi
