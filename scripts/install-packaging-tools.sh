#!/usr/bin/env bash
set -euo pipefail
cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.."
tools="$PWD/target/packaging-tools"
mkdir -p "$tools/bin"
cargo install cargo-deb --version 3.8.0 --locked --root "$tools"
for name in linuxdeploy linuxdeploy-plugin-appimage; do
    curl --fail --location --retry 3 --connect-timeout 20 \
        "https://github.com/linuxdeploy/$name/releases/download/continuous/$name-x86_64.AppImage" \
        --output "$tools/bin/$name-x86_64.AppImage.part"
    mv -- "$tools/bin/$name-x86_64.AppImage.part" "$tools/bin/$name-x86_64.AppImage"
    chmod 755 "$tools/bin/$name-x86_64.AppImage"
done
(cd "$tools/bin" && sha256sum cargo-deb *.AppImage > SHA256SUMS)
printf 'Packaging tools installed in %s/bin\n' "$tools"
