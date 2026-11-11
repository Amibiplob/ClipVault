# Linux packaging — ClipVault 2.0.0

Build on Linux Mint 22.x / Ubuntu 24.04 x86-64 (glibc 2.39). AppImage still requires host glibc and graphics drivers; older systems need a separately verified build baseline. Package formats do not establish desktop compatibility. See [compatibility](../COMPATIBILITY.md) and the [release verification record](../V2.0.0-VERIFICATION.md).

## Prerequisites and local commands

On a disposable build host with network access:

```sh
sudo apt install build-essential pkg-config binutils dpkg-dev python3 curl \
  desktop-file-utils appstream libsqlite3-0 libgdk-pixbuf-2.0-0 \
  libgdk-pixbuf2.0-bin webp-pixbuf-loader libx11-6 libx11-xcb1 \
  libxcursor1 libxi6 libxrandr2 libxinerama1 libxrender1 libxfixes3 \
  libxext6 libxkbcommon0 libxkbcommon-x11-0 libxcb1 libwayland-client0 \
  libwayland-cursor0 libwayland-egl1 libgl1 libegl1 fontconfig patchelf rpm
cargo fetch --locked
bash scripts/install-packaging-tools.sh
cargo test --locked --offline
cargo clippy --locked --offline --all-targets -- -D warnings
cargo fmt -- --check
python3 scripts/packaging-support.py notices
python3 scripts/extract-cached-runtime.py
LDAI_RUNTIME_FILE="$PWD/target/packaging-tools/runtime-x86_64" bash scripts/package-linux.sh all
bash scripts/test-linux-packages.sh
bash scripts/package-rpm.sh
```

`package-linux.sh all` builds Debian and AppImage; individual modes are `deb`, `appimage`, `stage`. `package-rpm.sh` uses the same source assets and binary through `packaging/linux/clipvault.spec`; it fails clearly if rpmbuild is unavailable. RPM does not bundle the AppImage's Ubuntu runtime libraries. RPM dependencies and installation require Fedora/openSUSE-family acceptance. No local RPM is claimed unless an actual file was produced.

Expected generated filenames (existence is recorded in verification, not implied here):

- `dist/clipvault_2.0.0-1_amd64.deb`
- `dist/ClipVault-2.0.0-x86_64.AppImage`
- RPM tool output: `dist/clipvault-2.0.0-1*.x86_64.rpm` (distribution suffix depends on rpmbuild).

The scripts derive versions from Cargo.toml; metadata, changelog and website config are checked separately. SHA256SUMS includes existing historical .deb/AppImage archives too; verify intended filenames and version rather than treating every dist file as current. `scripts/test-linux-packages.sh` checks archives/extracted AppDir, dynamic libraries, help/diagnostics, SQLite and image loaders; `--gui` additionally needs a usable desktop. These are not package-manager installation or FUSE acceptance.

The existing source SVG is installed as scalable icon with matching PNG sizes and native window icon; desktop `Icon=clipvault` and `StartupWMClass=clipvault` match the application. Source/bundled dependency notices are retained. Uninstall scripts update caches and do not remove user data. Login startup and desktop shortcuts use the stable installed/AppImage executable and preserve `--data-dir`.

## Release automation and publication

`.github/workflows/packages.yml` builds candidate artifacts manually on Ubuntu 24.04, with Debian/AppImage and RPM recipes, automated tests and archive smoke checks. It is prepared, not executed here. It does not upload a GitHub Release or publish a repository. Installed upgrade, removal, desktop keyboard/focus and image-paste tests are required before advertising tested platforms.

For system update-manager delivery, use the [signed APT preparation guide](APT-REPOSITORY.md). Signing/hosting credentials, publication and clean-system verification remain maintainer steps. There is no live repository or PPA claim.

GitHub Pages deployment uses `.github/workflows/pages.yml` with `docs/` as artifact root. Enable GitHub Pages with the GitHub Actions source in repository settings, then run/observe deployment. No remote settings or hosting were changed by this task. The website discovers actual released assets through GitHub; if the API fails, it links to Releases.
