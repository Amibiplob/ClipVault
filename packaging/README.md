# Linux packaging — ClipVault 2.1.0

Run from the repository root (`app/` here). Debian/AppImage builds target Ubuntu
24.04 / Mint 22.x x86-64, glibc 2.39. AppImage still needs compatible host glibc
and graphics drivers. Fedora RPMs compile separately against Fedora; they never
convert a Debian package or reuse its bundled libraries. See
[current verification](../V2.1.0-VERIFICATION.md) and
[desktop compatibility](../COMPATIBILITY.md) for measured limits.

## Debian and AppImage

On a disposable Noble build host, install `build-essential pkg-config binutils
 dpkg-dev python3 curl desktop-file-utils appstream patchelf` and the runtime
packages listed in Cargo.toml (`package.metadata.deb.depends`), plus
`libgdk-pixbuf2.0-bin webp-pixbuf-loader fontconfig`. Use Rust >=1.88.

```sh
cargo fetch --locked
bash scripts/install-packaging-tools.sh # network; only when tools are missing
cargo test --locked --offline
cargo clippy --locked --offline --all-targets -- -D warnings
cargo fmt -- --check
python3 scripts/packaging-support.py notices
python3 scripts/extract-cached-runtime.py
LDAI_RUNTIME_FILE="$PWD/target/packaging-tools/runtime-x86_64" bash scripts/package-linux.sh all
bash scripts/test-linux-packages.sh
```

The existing workflows build real release executables and create:

- `dist/clipvault_2.1.0-1_amd64.deb`
- `dist/ClipVault-2.1.0-x86_64.AppImage`

`package-linux.sh` accepts `all`, `deb`, `appimage`, `stage`. Smoke checks inspect
metadata, version, x86-64 ELF, runtime libraries, loaders, icons and notices.
`--gui` additionally attempts isolated screenshot launches on a usable desktop.
Extraction tests do not prove apt installation or FUSE. SHA256SUMS also includes
historical packages; use the exact intended filename.

Install with `sudo apt install ./clipvault_2.1.0-1_amd64.deb`. For AppImage, run
`chmod +x ClipVault-2.1.0-x86_64.AppImage` then
`./ClipVault-2.1.0-x86_64.AppImage`. If FUSE is unavailable, use
`./ClipVault-2.1.0-x86_64.AppImage --appimage-extract-and-run`.
Keep a stable executable location before configuring shortcuts/startup;
`bash scripts/install-appimage.sh` can install a user menu launcher.

## Fedora RPM

The existing spec now compiles vendored source, runs tests and installs the shared
Cargo manifest asset list with `scripts/install-release-assets.py`. It includes
all logo sizes, launcher, AppStream metadata, notices and explicit dlopen library
dependencies; ELF scanning adds linked-library requirements from the actual
Fedora binary. Release is exactly `1`, without a distribution suffix.

On a host with Podman or Docker and network access:

```sh
bash scripts/package-rpm.sh container --online
```

This provisions `packaging/fedora/Containerfile` (Fedora 44), prepares the complete
vendor archive, then builds/tests with container networking disabled. Set
`CONTAINER_ENGINE=podman` or `docker` to select an engine. Pin the base image to a
reviewed digest with `FEDORA_IMAGE=fedora@sha256:...` and record toolchain/system-package versions for reproducibility;
mutable image tags alone do not guarantee identical package bytes.

The native mode requires Fedora x86-64 and a prepared source archive:
`python3 scripts/prepare-source.py --online`, then
`bash scripts/package-rpm.sh native`. Its installation test deliberately requires
a disposable container. `scripts/test-rpm.sh` checks metadata, file list,
dependencies, installation without `--nodeps`, version, isolated Xvfb/software
screenshot launch and uninstall retaining fixture history. This does not prove
real desktop capture/paste/shortcuts or an installed v2.0 upgrade.

Expected output only after success: `dist/clipvault-2.1.0-1.x86_64.rpm`.
Install a verified Fedora package with
`sudo dnf install ./clipvault-2.1.0-1.x86_64.rpm`.
No RPM was produced here: rpmbuild, Podman/Docker and registry DNS are unavailable.
Fedora installation/launch tests remain unrun. Other RPM distributions, including
openSUSE, require separate dependency specs, builds and tests.

## Source PPA / APT publication

The new `debian/` recipe targets Noble/Mint 22.x amd64 with Ubuntu's versioned
Rust/Cargo 1.89 and offline Cargo vendoring. See
[Launchpad source packaging and account setup](LAUNCHPAD-PPA.md) for reproducible
preparation, clean builds, signing and dput. Complete source preparation is
blocked here by uncached cross-target crates and missing tooling/network.
No accepted source upload or working PPA installation command is claimed.

The independent [signed binary APT workflow](APT-REPOSITORY.md) remains available
and is distinct from Launchpad source builds. Manual candidate CI jobs have not
run here and do not publish a release. The website only links directly to assets
confirmed by published GitHub Releases, retaining light/dark/system themes.

## Upgrade, removal and troubleshooting

Stop ClipVault and back up the whole data directory, including SQLite WAL and
payload files, before upgrading. Install the newer package over the older one and
keep the same data location or `--data-dir`. v2.1.0 changes no schema, setting name
or shortcut format; existing migrations remain. AppImages should keep the stable
path used by startup/shortcuts. Normal uninstall and Debian purge never delete
personal clipboard history.

- Missing libraries: use the package built for that distribution and let apt/dnf
  resolve dependencies. Do not bypass checks or put Ubuntu libraries in an RPM.
- Older glibc: build and verify separately on that baseline.
- No RPM tools: use the Fedora container workflow on a capable host.
- Incomplete source cache: prepare online once, retain all vendor licenses and
  checksums, then verify in a clean offline builder.
- Paste/shortcut problems: inspect Settings → Shortcuts and `clipvault --diagnostics`.
  Use manual Ctrl+V when the desktop cannot support safe automatic paste.
- Storage errors: stop the app and back up the complete data directory; do not
  delete history/settings as an upgrade repair.
