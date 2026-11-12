#!/usr/bin/env bash
set -euo pipefail
cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.."
version=$(python3 -c 'import tomllib; print(tomllib.load(open("Cargo.toml","rb"))["package"]["version"])')
case ${1:-container} in
    container)
        engine=${CONTAINER_ENGINE:-$(command -v podman || command -v docker || true)}
        [[ -n $engine ]] || { echo 'RPM blocked: install Podman/Docker and provide network access for the Fedora build image. No RPM was built.' >&2; exit 1; }
        "$engine" build --platform linux/amd64 --build-arg "FEDORA_IMAGE=${FEDORA_IMAGE:-fedora:44}" \
            -f packaging/fedora/Containerfile -t clipvault-fedora-builder packaging/fedora
        python3 scripts/prepare-source.py "${@:2}"
        "$engine" run --rm --network=none --platform linux/amd64 \
            -v "$PWD:/build:Z" clipvault-fedora-builder bash scripts/package-rpm.sh native
        ;;
    native)
        source /etc/os-release
        [[ $ID == fedora && $(uname -m) == x86_64 ]] || { echo 'Native RPM must be built on Fedora x86-64; use container mode.' >&2; exit 1; }
        command -v rpmbuild >/dev/null
        archive="$PWD/target/source-release/clipvault-$version.tar.xz"
        test -s "$archive"
        top="$PWD/target/rpmbuild"
        mkdir -p "$top"/{BUILD,BUILDROOT,RPMS,SOURCES,SPECS,SRPMS} dist
        cp -- "$archive" "$top/SOURCES/"
        rpmbuild -ba packaging/linux/clipvault.spec --define "_topdir $top" \
            --define "clipvault_version $version" --define '_build_id_links none'
        package="$top/RPMS/x86_64/clipvault-$version-1.x86_64.rpm"
        test -s "$package"
        cp -- "$package" dist/
        rpm -K "$package"
        if [[ -f /.dockerenv || -f /run/.containerenv ]]; then
            bash scripts/test-rpm.sh "dist/$(basename "$package")"
        else
            echo 'RPM built; installation/launch tests require a disposable Fedora container.'
        fi
        (cd dist && sha256sum "$(basename "$package")" > RPM-SHA256SUMS)
        ;;
    *) echo 'Usage: scripts/package-rpm.sh [container [--online]|native]' >&2; exit 2 ;;
esac
