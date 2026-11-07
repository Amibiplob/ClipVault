# Linux distribution: ClipVault 0.1.0

The packaging target is **Linux Mint 22.x, x86-64 (amd64)**, based on Ubuntu
24.04, glibc 2.39. Build on Mint 22.x or Ubuntu 24.04 x86-64; the script rejects
newer build bases, cross-compilation and redirected Cargo target directories.
Use the default Rust x86-64 CPU target, without `-C target-cpu=native`.
Ubuntu 24.04 is a compatible build base but has not been runtime-tested here.
Mint 21.x / Ubuntu 22.04, LMDE, 32-bit x86, ARM and other distributions are not
supported by these release scripts. Newer distributions require separate tests.

Mint Cinnamon, MATE and Xfce X11 sessions are the intended desktop targets.
Wayland capture still requires ext-data-control-v1 or wlr-data-control-v1 and a
compositor shortcut; packaging does not add missing compositor protocols.
Only the current host's X11 session can be checked here. GPU drivers, OpenGL/EGL,
glibc and a desktop session must be supplied by the computer running ClipVault.

## Build the packages

All commands below run from `app/`. No command installs ClipVault or uploads a
release. Generated artifacts stay in ignored `dist/` and `target/` directories.
Install these build prerequisites on the build computer:

```sh
sudo apt install build-essential pkg-config dpkg-dev python3 curl desktop-file-utils \
  libsqlite3-0 libgdk-pixbuf-2.0-0 libgdk-pixbuf2.0-bin webp-pixbuf-loader \
  libx11-6 libx11-xcb1 libxcursor1 libxi6 libxrandr2 libxinerama1 libxrender1 \
  libxfixes3 libxext6 libxkbcommon0 libxkbcommon-x11-0 libxcb1 libxcb-render0 \
  libxcb-shape0 libxcb-xfixes0 libwayland-client0 libwayland-cursor0 \
  libwayland-egl1 libgl1 libegl1 xkb-data
# Install stable Rust with rustup if cargo/rustc are missing.
cargo fetch --locked
scripts/install-packaging-tools.sh
python3 scripts/packaging-support.py notices
scripts/package-linux.sh all
scripts/test-linux-packages.sh
# Optional: run real native screenshot checks in an available desktop session.
scripts/test-linux-packages.sh --gui
```

`install-packaging-tools.sh` installs cargo-deb 3.8.0 under
`target/packaging-tools/` and downloads official linuxdeploy and its AppImage
output plugin. These downloads require internet access. The continuous tool
releases can change; their SHA256SUMS are recorded alongside the downloads.
Archive the tools and hashes with build records to repeat an exact build.
There is no automatic upload, signing, system installation or release creation.
`package-linux.sh` then builds with `--release --locked --offline`, verifies
notices, and uses `cargo deb` and `linuxdeploy --output appimage`. The official
linuxdeploy AppImage also includes an AppImage output plugin. You can provide
an existing linuxdeploy via the absolute `LINUXDEPLOY` environment variable.
`APPIMAGE_EXTRACT_AND_RUN=1` permits build tools to work without FUSE.

For version 0.1.0, successful builds produce exactly:

```text
app/dist/clipvault_0.1.0-1_amd64.deb
app/dist/ClipVault-0.1.0-x86_64.AppImage
app/dist/SHA256SUMS
```

These are output names, not proof the files have been built. See
[VALIDATION.md](VALIDATION.md) for the current run's actual results.
Use `scripts/package-linux.sh deb` or `appimage` for one format, or `stage` to
build the executable and stage an AppDir without packaging tools. Staging alone
does not create a distribution package or apply linuxdeploy's ELF path adjustments.

## Contents and dependencies

Both formats include the release executable, desktop launcher, original
ClipVault SVG icon, full MIT LICENSE, README, this guide, and
`THIRD-PARTY-NOTICES.txt`. The Interface.ttf font is Noto Sans Regular, verified
byte-for-byte against the Mint fonts-noto-core copy; its original attribution
and OFL terms are in `assets/Interface.ttf.LICENSE`. egui embeds additional
Hack, Ubuntu, Noto Emoji and emoji icon fonts, whose original notices and font
metadata are included. The notice generator reads the locked active Linux
Cargo graph, includes build dependencies conservatively, preserves license and
NOTICE files, and stops for missing texts. Version-specific upstream license
copies in `packaging/licenses/` fill omissions from published crate archives.
The installed Rust toolchain's full standard-library notices are also included;
these cover linked runtime components outside Cargo.lock. Rust's
`share/doc/rust/COPYRIGHT-library.html` must be available to regenerate them.
Regenerate and review notices when changing dependencies or Rust toolchains; the package script
fails if the checked-in notices are stale.

The Debian manifest combines cargo-deb's `$auto` ELF dependency discovery with
explicit dependencies for SQLite, GdkPixbuf, X11, XCB, Wayland, xkbcommon,
OpenGL and EGL. Merely using `$auto` would miss `libloading`/dlopen dependencies.
GIF support is provided by the GdkPixbuf package; WebP's loader is recommended
for the .deb. Missing optional thumbnail support does not discard original
clipboard bytes. Install with `sudo apt install ./dist/clipvault_0.1.0-1_amd64.deb`
so apt resolves dependencies; launch `clipvault` or the desktop menu entry.

The AppDir explicitly stages the libraries in `linux/runtime-libraries.txt`,
GdkPixbuf's query tool and GIF/WebP loaders, plus their non-host ELF dependency
closure. linuxdeploy deploys these libraries and adjusts their ELF search paths;
an additional check rejects unexpected host library fallbacks. OpenGL/EGL vendor
dispatch libraries, GPU drivers, libgcc and glibc remain on the host. `AppRun` sets the
bundled library search path and creates a temporary GdkPixbuf loader cache at
the actual mounted location; build paths are never baked into that cache.
System library copyright texts, all referenced common license texts and exact
Debian binary/source package versions are collected **after** bundling. Unknown
library provenance stops packaging. The source manifest identifies the Ubuntu
source repositories and `apt-get source SOURCE=VERSION` command for the matching
library sources. When distributing LGPL components, also make their matching
source available alongside the release as required by their included licenses;
the packaging scripts do not fetch or upload source archives. Extracting an
AppImage allows users to replace its shared libraries and run `AppRun`.

Run `chmod +x dist/ClipVault-0.1.0-x86_64.AppImage` and launch that file.
If FUSE is unavailable, run it with `--appimage-extract`, then
`./squashfs-root/AppRun`. Keep the AppImage at a stable location before enabling
login startup. Startup entries reference the original AppImage rather than
its temporary mount. Extracted AppDirs must also stay at a stable location.

## Release validation

`test-linux-packages.sh` extracts both real packages to a temporary directory,
checks executable/launcher/icon/notices, validates .deb architecture/version and
runtime dependency declarations, runs help/diagnostics, checks library linkage,
and exercises bundled SQLite with an in-memory database plus GIF/WebP thumbnail
decoding. `--gui` captures both launches using separate temporary data dirs;
it does not touch the user's clipboard history. Normal AppImage FUSE mounting,
sudo/apt installation/removal, desktop-menu integration, autostart after reboot,
external copy/select/paste, other Mint desktops/versions, Wayland and hardware
GPU combinations still need real-system validation. Craft Anything's
`rebuild_check` checks the source build and a native window, not installation
or portable-package compatibility.

Tool documentation: [cargo-deb](https://github.com/kornelski/cargo-deb),
[linuxdeploy user guide](https://docs.appimage.org/packaging-guide/from-source/linuxdeploy-user-guide.html),
[AppImage output plugin](https://github.com/linuxdeploy/linuxdeploy-plugin-appimage).
