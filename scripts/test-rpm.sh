#!/usr/bin/env bash
# This installs the package only inside a disposable Fedora container.
set -euo pipefail
source /etc/os-release
[[ $ID == fedora && -f /.dockerenv || $ID == fedora && -f /run/.containerenv ]] || {
    echo 'RPM installation test requires a disposable Fedora container.' >&2; exit 1;
}
package=$(realpath "${1:?RPM path required}")
test "$(rpm -qp --qf '%{NAME} %{VERSION}-%{RELEASE} %{ARCH}' "$package")" = 'clipvault 2.1.0-1 x86_64'
rpm -K "$package"
rpm -qp --requires "$package"
rpm -qpl "$package" | tee /tmp/clipvault-rpm-contents.txt
! rpm -qpl "$package" | grep -E '^/(home|root)/'
rpm -qp --scripts "$package"
# Dependencies were provisioned in the image; never bypass the dependency check.
rpm -Uvh "$package"
test "$(clipvault --version)" = 'ClipVault 2.1.0'
clipvault --help
clipvault --diagnostics
test -s /usr/share/doc/clipvault/THIRD-PARTY-NOTICES.txt
test -s /usr/share/icons/hicolor/512x512/apps/clipvault.png
work=$(mktemp -d)
trap 'rm -rf -- "$work"' EXIT
LIBGL_ALWAYS_SOFTWARE=1 timeout 90 xvfb-run -a clipvault --data-dir "$work/history" --screenshot "$work/window.png"
test -s "$work/window.png"
rpm -e clipvault
test -d "$work/history"
echo 'PASS: RPM metadata/dependencies/install, isolated screenshot launch, uninstall leaves history. Real desktop integration and installed upgrade remain unverified.'
