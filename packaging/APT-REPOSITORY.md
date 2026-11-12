# Signed APT repository preparation — not published

No APT repository or PPA was created, uploaded, or verified. The app and website must not show an APT install command until a real signed endpoint passes the checks below.

Prepared automation:

- `scripts/prepare-apt-repository.sh INPUT.deb NEW_OUTPUT_DIR SIGNING_FINGERPRINT` verifies package identity/architecture, builds Packages/Packages.gz and Release, generates InRelease and detached Release.gpg, exports the public key and verifies the signature. It refuses to overwrite an existing artifact directory. It does not upload anything.
- `.github/workflows/apt-artifact.yml` accepts an exact GitHub release `.deb` URL and independently verified SHA256. It checks the source origin and digest, imports a dedicated repository signing key into a temporary GNUPGHOME, produces the signed artifact, removes private key material, and uploads only the public repository artifact. No private key enters Git.

Remaining setup, in order:

1. Finish distribution installation, shortcut and paste acceptance for the candidate. Publish the verified .deb as an approved release with its digest.
2. Create a dedicated APT signing key using the maintainer's chosen custody/rotation procedure. Configure `APT_SIGNING_KEY` (ASCII armored private key usable noninteractively in this workflow) and `APT_SIGNING_FINGERPRINT` as protected Actions secrets. No signing material is available here. Use protected workflow access and a trusted runner. For passphrase/HSM custody, adapt the signing step to that system; do not commit credentials.
3. Enable Actions for this repository. Run the signed-artifact workflow with the exact package URL/digest. This has not been executed here; local `apt-ftparchive` and signing tools/material are prerequisites.
4. Choose a maintained HTTPS origin/path, publish the public artifact preserving `pool/`, `dists/`, and public key, and arrange repeat publication for each approved package. This requires account/hosting credentials and explicit publication authorization unavailable here.
5. From a clean supported Mint/Ubuntu system, download the public key, independently compare the fingerprint, use a dedicated `/usr/share/keyrings/…` key and `signed-by=` source, run apt update, inspect package policy/origin, install the intended version, and test an upgrade from the old package while preserving real data. Test expired/wrong signatures fail closed and verify uninstall preserves the data directory.
6. Only then add the verified real APT instructions, fingerprint, supported suites and update cadence to README/website. Keep endpoint availability/signatures monitored. Never insert a placeholder domain into copyable install commands.

For Launchpad source packaging, use [LAUNCHPAD-PPA.md](LAUNCHPAD-PPA.md). A Launchpad PPA is an alternative requiring a Launchpad account, signed source packages and builder acceptance. The prepared workflow above is for a signed binary APT repository, not a claimed PPA. Ubuntu/Mint software-manager updates require a real configured repository; optional GitHub checks alone do not supply them.
