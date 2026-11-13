#!/usr/bin/env bash
set -euo pipefail
cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.."
version=$(python3 -c 'import tomllib; print(tomllib.load(open("Cargo.toml","rb"))["package"]["version"])')
mode=${1:-container}
case $mode in
    container|native) ;;
    *) echo 'Usage: scripts/package-rpm.sh [container [--online]|native]' >&2; exit 2 ;;
esac
mkdir -p dist
# Keep previous attempts, including failures, intact.
log=$(mktemp "$PWD/dist/rpm-$version-$mode-$(date -u +%Y%m%dT%H%M%SZ)-XXXXXX.log")
build_rpm() {
echo "ClipVault $version RPM build ($mode)"
echo "Build log: $log"
case $mode in
    container)
        engine=${CONTAINER_ENGINE:-podman}
        if ! engine=$(command -v "$engine"); then
            echo 'RPM blocked: container engine was not found in the build script PATH. No RPM was built.' >&2
            echo 'On Linux Mint, run locally: sudo apt update && sudo apt install podman' >&2
            echo 'Then run podman --version and podman info in the same environment as this script.' >&2
            exit 1
        fi
        echo "Container CLI: $engine (working directory: $PWD)"
        version_status=0
        info_status=0
        echo "+ $engine --version"
        "$engine" --version || version_status=$?
        echo "CLI version exit status: $version_status"
        echo "+ $engine info"
        "$engine" info || info_status=$?
        echo "Container info exit status: $info_status"
        if (( version_status != 0 || info_status != 0 )); then
            echo 'RPM blocked: the installed container CLI is unusable in this build environment. No RPM was built.' >&2
            echo 'Retry where podman info succeeds with writable runtime/storage and permitted user namespaces.' >&2
            if (( info_status != 0 )); then exit "$info_status"; fi
            exit "$version_status"
        fi
        "$engine" build --platform linux/amd64 --build-arg "FEDORA_IMAGE=${FEDORA_IMAGE:-fedora:44}" \
            -f packaging/fedora/Containerfile -t clipvault-fedora-builder packaging/fedora
        python3 scripts/prepare-source.py "${@:2}"
        "$engine" run --rm --network=none --platform linux/amd64 \
            -v "$PWD:/build:Z" clipvault-fedora-builder bash scripts/package-rpm.sh native
        ;;
    native)
        source /etc/os-release
        [[ $ID == fedora && $VERSION_ID == 44 && $(uname -m) == x86_64 ]] || { echo 'Native RPM must be built on Fedora 44 x86-64; use container mode.' >&2; exit 1; }
        command -v rpmbuild >/dev/null || { echo 'RPM blocked: rpmbuild was not found in PATH.' >&2; exit 1; }
        archive="$PWD/target/source-release/clipvault-$version.tar.xz"
        test -s "$archive" || { echo "RPM blocked: prepared source archive is missing: $archive" >&2; exit 1; }
        top="$PWD/target/rpmbuild"
        mkdir -p "$top"/{BUILD,BUILDROOT,RPMS,SOURCES,SPECS,SRPMS} dist
        cp -- "$archive" "$top/SOURCES/"
        rpmbuild -ba packaging/linux/clipvault.spec --define "_topdir $top" \
            --define "clipvault_version $version" --define '_build_id_links none'
        package="$top/RPMS/x86_64/clipvault-$version-1.x86_64.rpm"
        test -s "$package"
        rpm -K "$package"
        if [[ -f /.dockerenv || -f /run/.containerenv ]]; then
            bash scripts/test-rpm.sh "$package"
        else
            echo 'RPM built; installation/launch tests require a disposable Fedora container.'
        fi
        cp -- "$package" dist/
        (cd dist && sha256sum "$(basename "$package")" > RPM-SHA256SUMS)
        python3 scripts/update-metadata.py
        ;;
esac
echo "RPM output: $PWD/dist/clipvault-$version-1.x86_64.rpm"
}

# Wait for tee to finish so even a fast preflight failure leaves a complete log.
# Keep errexit in the builder and preserve its status through the logging pipe.
set +e
(set -e; build_rpm "$@") 2>&1 | tee -- "$log"
statuses=("${PIPESTATUS[@]}")
set -e
if (( statuses[0] != 0 )); then
    echo "RPM build failed (exit ${statuses[0]}). Build log: $log" | tee -a -- "$log" >&2
    exit "${statuses[0]}"
fi
exit "${statuses[1]}"
