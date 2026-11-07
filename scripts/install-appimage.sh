#!/usr/bin/env bash
# Explicit user installation, run locally after building the AppImage.
set -euo pipefail
cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.."
version=$(python3 -c 'import tomllib;print(tomllib.load(open("Cargo.toml","rb"))["package"]["version"])')
image="$PWD/dist/ClipVault-$version-x86_64.AppImage"
test -s "$image"
data=${XDG_DATA_HOME:-$HOME/.local/share}
launcher="$data/applications/io.clipvault.desktop.desktop"
if [[ -f $launcher ]] && ! rg -q '^Name=ClipVault$' "$launcher"; then
    echo "Refusing to replace an unrelated launcher: $launcher" >&2; exit 1
fi
# Keep a stable executable path for desktop and login startup.
install -Dm755 "$image" "$HOME/.local/bin/clipvault.AppImage"
install -Dm644 assets/clipvault.svg "$data/icons/hicolor/scalable/apps/clipvault.svg"
for size in 16 24 32 48 64 128 256 512; do
    install -Dm644 "assets/icons/${size}x${size}/clipvault.png" "$data/icons/hicolor/${size}x${size}/apps/clipvault.png"
done
mkdir -p "$data/applications"
python3 - "$HOME/.local/bin/clipvault.AppImage" "$launcher" <<'PY'
from pathlib import Path
import sys
exe=sys.argv[1].replace('\\','\\\\').replace('"','\\"').replace('`','\\`').replace('$','\\$').replace('%','%%')
text=Path('packaging/linux/io.clipvault.desktop.desktop').read_text().replace('Exec=clipvault',f'Exec="{exe}"')
Path(sys.argv[2]).write_text(text)
PY
if command -v gtk-update-icon-cache >/dev/null; then gtk-update-icon-cache -q -t -f "$data/icons/hicolor" || true; fi
if command -v update-desktop-database >/dev/null; then update-desktop-database "$data/applications"; fi
python3 scripts/audit-icons.py
