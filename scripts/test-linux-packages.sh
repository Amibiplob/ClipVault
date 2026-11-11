#!/usr/bin/env bash
set -euo pipefail
cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.."
root=$PWD
gui=false
stage=false
for option in "$@"; do
    case "$option" in --gui) gui=true ;; --stage) stage=true ;; *) echo 'Usage: scripts/test-linux-packages.sh [--gui] [--stage]' >&2; exit 2 ;; esac
done
version=$(python3 -c 'import tomllib; print(tomllib.load(open("Cargo.toml", "rb"))["package"]["version"])')
deb="$root/dist/clipvault_${version}-1_amd64.deb"
image="$root/dist/ClipVault-${version}-x86_64.AppImage"
if ! $stage; then
    missing=false
    for file in "$deb" "$image"; do
        if [[ ! -s $file ]]; then echo "UNTESTED: package does not exist: $file" >&2; missing=true; fi
    done
    ! $missing || exit 1
fi
work=$(mktemp -d "${TMPDIR:-/tmp}/clipvault-package-check.XXXXXXXX")
trap 'rm -rf -- "$work"' EXIT

check_tree() {
    local tree=$1 launcher=$2 label=$3
    test -x "$tree/usr/bin/clipvault"
    for file in usr/share/applications/io.clipvault.desktop.desktop usr/share/icons/hicolor/scalable/apps/clipvault.svg usr/share/doc/clipvault/LICENSE usr/share/doc/clipvault/THIRD-PARTY-NOTICES.txt usr/share/doc/clipvault/README.md usr/share/doc/clipvault/PACKAGING.md usr/share/doc/clipvault/CHANGELOG-v2.0.0.txt usr/share/doc/clipvault/V2.0.0-VERIFICATION.md usr/share/metainfo/io.clipvault.desktop.metainfo.xml; do
        test -s "$tree/$file"
    done
    desktop-file-validate "$tree/usr/share/applications/io.clipvault.desktop.desktop"
    cmp "$root/CHANGELOG-v2.0.0.txt" "$tree/usr/share/doc/clipvault/CHANGELOG-v2.0.0.txt"
    cmp "$root/packaging/linux/io.clipvault.desktop.metainfo.xml" "$tree/usr/share/metainfo/io.clipvault.desktop.metainfo.xml"
    cmp "$root/assets/clipvault.svg" "$tree/usr/share/icons/hicolor/scalable/apps/clipvault.svg"
    rg -q '^Icon=clipvault$' "$tree/usr/share/applications/io.clipvault.desktop.desktop"
    rg -q '^StartupWMClass=clipvault$' "$tree/usr/share/applications/io.clipvault.desktop.desktop"
    for size in 16 24 32 48 64 128 256 512; do
        cmp "$root/assets/icons/${size}x${size}/clipvault.png" "$tree/usr/share/icons/hicolor/${size}x${size}/apps/clipvault.png"
    done
    readelf -h "$tree/usr/bin/clipvault" | rg -q 'Machine:.*X86-64'
    if ldd "$tree/usr/bin/clipvault" | rg 'not found'; then return 1; fi
    "$launcher" --help | rg -q 'ClipVault'
    "$launcher" --diagnostics | rg -q 'Clipboard: native'
    printf 'PASS: %s contents, desktop entry, x86-64 executable, linkage, help and diagnostics\n' "$label"
    if $gui; then
        timeout 60 "$launcher" --data-dir "$work/$label-history" --screenshot "$work/$label.png"
        python3 - "$work/$label.png" <<'PY'
import pathlib, struct, sys
b = pathlib.Path(sys.argv[1]).read_bytes()
assert b[:8] == b'\x89PNG\r\n\x1a\n'
width, height = struct.unpack('>II', b[16:24])
assert width >= 440 and height >= 420
print(f'PASS: native package screenshot {width}x{height}')
PY
    fi
}

check_appdir() {
    local tree=$1
    export LD_LIBRARY_PATH="$tree/usr/lib:$tree/usr/lib/x86_64-linux-gnu${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
    export GDK_PIXBUF_MODULEDIR="$tree/usr/lib/gdk-pixbuf-2.0/2.10.0/loaders"
    export GDK_PIXBUF_MODULE_FILE="$work/loaders.cache"
    "$tree/usr/bin/gdk-pixbuf-query-loaders" "$GDK_PIXBUF_MODULEDIR"/*.so > "$GDK_PIXBUF_MODULE_FILE"
    while read -r library; do
        [[ -z $library || $library == \#* ]] || test -s "$tree/usr/lib/$library"
    done < "$root/packaging/linux/runtime-libraries.txt"
    python3 "$root/scripts/check-packaged-runtime.py"
    while IFS= read -r -d '' library; do
        if ldd "$library" | rg 'not found'; then return 1; fi
    done < <(find "$tree/usr/lib" -type f -name '*.so*' -print0)
    check_tree "$tree" "$tree/AppRun" AppDir
}

if $stage; then
    cp -a -- "$root/target/packaging/ClipVault.AppDir" "$work/relocated AppDir"
    check_appdir "$work/relocated AppDir"
    echo 'PASS: staged AppDir checks only; no .deb installation, AppImage runtime or portability was tested.'
    exit 0
fi
test "$(dpkg-deb -f "$deb" Package)" = clipvault
test "$(dpkg-deb -f "$deb" Version)" = "$version-1"
test "$(dpkg-deb -f "$deb" Architecture)" = amd64
dependencies=$(dpkg-deb -f "$deb" Depends)
for package in libsqlite3-0 libgdk-pixbuf-2.0-0 libx11-6 libxkbcommon0 libwayland-client0 libgl1 libegl1; do
    [[ $dependencies == *"$package"* ]]
done
dpkg-deb -x "$deb" "$work/deb"
python3 - "$deb" "$work/deb" <<'PY'
import pathlib, subprocess, sys, tomllib, xml.etree.ElementTree as ET

deb, tree = sys.argv[1], pathlib.Path(sys.argv[2])
package = tomllib.loads(pathlib.Path('Cargo.toml').read_text())['package']
expected = package['metadata']['deb']['extended-description']
description = subprocess.check_output(['dpkg-deb', '-f', deb, 'Description'], text=True)
assert ' '.join(description.split()) == package['description'] + ' ' + expected, description
assert (tree / 'usr/share/doc/clipvault/README.md').read_bytes() == pathlib.Path('README.md').read_bytes()
metainfo = tree / 'usr/share/metainfo/io.clipvault.desktop.metainfo.xml'
assert metainfo.read_bytes() == pathlib.Path('packaging/linux/io.clipvault.desktop.metainfo.xml').read_bytes()
component = ET.parse(metainfo).getroot()
assert component.findtext('name') == 'ClipVault'
assert component.findtext('pkgname') == 'clipvault'
assert component.findtext("launchable[@type='desktop-id']") == 'io.clipvault.desktop.desktop'
assert component.findtext('description/p') == expected
url = component.findtext("screenshots/screenshot[@type='default']/image[@type='source']")
assert url == 'https://raw.githubusercontent.com/Amibiplob/ClipVault/main/docs/history.png'
subprocess.run(['appstreamcli', 'validate', '--no-net', '--strict', str(metainfo)], check=True)
print('PASS: Debian plain-text description, separate README, packaged AppStream metadata and screenshot URL configuration (network not tested)')
PY
check_tree "$work/deb" "$work/deb/usr/bin/clipvault" deb
python3 "$root/scripts/check-packaged-runtime.py"
mkdir "$work/image"
(cd "$work/image" && "$image" --appimage-extract > "$work/extraction.log")
test -s "$work/image/squashfs-root/usr/share/doc/clipvault/SYSTEM-LIBRARY-SOURCES.txt"
check_appdir "$work/image/squashfs-root"
echo 'PASS: extracted .deb and AppImage smoke checks. apt installation, FUSE and other desktop systems remain untested.'
