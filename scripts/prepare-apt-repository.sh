#!/usr/bin/env bash
# Generate a signed repository artifact. Does not upload or publish anything.
set -euo pipefail
[[ $# == 3 ]] || { echo 'Usage: prepare-apt-repository.sh INPUT.deb OUTPUT_DIR SIGNING_FINGERPRINT' >&2; exit 2; }
input=$(realpath -- "$1")
output=$(realpath -m -- "$2")
signing_fingerprint=$3
[[ -f $input ]] || { echo 'Debian package missing' >&2; exit 1; }
for tool in apt-ftparchive dpkg-deb gpg; do command -v "$tool" >/dev/null; done
[[ $(dpkg-deb -f "$input" Package) == clipvault && $(dpkg-deb -f "$input" Architecture) == amd64 ]] || { echo 'Expected ClipVault amd64 package' >&2; exit 1; }
[[ ! -e $output ]] || { echo 'Use a new output folder to preserve previous repository artifacts.' >&2; exit 1; }
mkdir -p "$output/pool/main/c/clipvault" "$output/dists/stable/main/binary-amd64"
cp -- "$input" "$output/pool/main/c/clipvault/"
cd -- "$output"
apt-ftparchive packages pool > dists/stable/main/binary-amd64/Packages
gzip -n -k dists/stable/main/binary-amd64/Packages
apt-ftparchive -o APT::FTPArchive::Release::Origin=ClipVault -o APT::FTPArchive::Release::Label=ClipVault -o APT::FTPArchive::Release::Suite=stable -o APT::FTPArchive::Release::Codename=stable -o APT::FTPArchive::Release::Architectures=amd64 -o APT::FTPArchive::Release::Components=main release dists/stable > dists/stable/Release
gpg --batch --yes --local-user "$signing_fingerprint" --clearsign --output dists/stable/InRelease dists/stable/Release
gpg --batch --yes --local-user "$signing_fingerprint" --armor --detach-sign --output dists/stable/Release.gpg dists/stable/Release
gpg --batch --export "$signing_fingerprint" > clipvault-archive-keyring.gpg
gpg --verify dists/stable/InRelease
echo "Signed artifact prepared at $output. Upload and clean-system apt verification are separate required steps."
