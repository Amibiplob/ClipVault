# Linux distribution: ClipVault 1.2.0

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

## Copy-and-paste commands for Linux Mint

Use these commands on **Linux Mint 22.x, 64-bit Intel/AMD**, with internet
access. Open Terminal with **Ctrl+Alt+T**. Run each block in order in the same
terminal, and stop if any command reports an error. The first build and tool
installation can take several minutes. `sudo` asks for your Linux login password;
the terminal does not display characters while you type it.

### 1. Open the project folder

This is the current project location on your computer. Keep the quotation marks
because the path contains spaces. If you moved the project, use its new `app/`
path instead.

```sh
cd "/home/biplob/Documents/Craft Anything/build-clipvault-a-modern/app"
```

### 2. Install system prerequisites

These commands install build tools and libraries, not ClipVault itself:

```sh
sudo apt update
sudo apt install build-essential binutils file pkg-config dpkg-dev python3 curl \
  ca-certificates desktop-file-utils appstream ripgrep \
  libsqlite3-0 libgdk-pixbuf-2.0-0 libgdk-pixbuf2.0-bin webp-pixbuf-loader \
  libx11-6 libx11-xcb1 libxcursor1 libxi6 libxrandr2 libxinerama1 libxrender1 \
  libxfixes3 libxext6 libxkbcommon0 libxkbcommon-x11-0 libxcb1 libxcb-render0 \
  libxcb-shape0 libxcb-xfixes0 libwayland-client0 libwayland-cursor0 \
  libwayland-egl1 libgl1 libegl1 xkb-data
```

The project already uses Rust and Cargo. Confirm they are available:

```sh
cargo --version
rustc --version
```

If your Rust installation uses rustup with a minimal profile, install its
documentation component too; the notice generator needs the standard library's
copyright file. Skip this command if you do not use rustup and that file is
already supplied by your Rust installation:

```sh
rustup component add rust-docs
```

### 3. Download dependencies and install the missing packaging tools

Run these as your normal user, without `sudo`. The existing installer installs
**cargo-deb 3.8.0**, **linuxdeploy** and **linuxdeploy-plugin-appimage** locally
under `app/target/packaging-tools/bin/`. The packaging script finds them there
automatically; you do not need to change your terminal's `PATH`.

```sh
cargo fetch --locked
bash scripts/install-packaging-tools.sh
```

### 4. Create both packages and check them

Regenerate notices for this computer's Rust toolchain, build both formats, then
check the actual archives. The build script enables extraction mode for its
AppImage tools automatically, so building does not require FUSE.

```sh
python3 scripts/packaging-support.py notices
bash scripts/package-linux.sh all
bash scripts/test-linux-packages.sh
```

Optionally, in a running desktop session, also check that both packaged apps
open and draw their windows:

```sh
bash scripts/test-linux-packages.sh --gui
```

### 5. Find the finished files

After a successful build, version **1.2.0** saves these files inside the
project's **app/dist/** folder:

```text
/home/biplob/Documents/Craft Anything/build-clipvault-a-modern/app/dist/clipvault_1.2.0-1_amd64.deb
/home/biplob/Documents/Craft Anything/build-clipvault-a-modern/app/dist/ClipVault-1.2.0-x86_64.AppImage
/home/biplob/Documents/Craft Anything/build-clipvault-a-modern/app/dist/SHA256SUMS
```

From the same terminal, list the packages and verify their checksums with:

```sh
ls -lh dist/clipvault_1.2.0-1_amd64.deb dist/ClipVault-1.2.0-x86_64.AppImage dist/SHA256SUMS
(cd dist && sha256sum --check SHA256SUMS)
```

If you moved the project, the files are in `dist/` inside its new `app/` folder.
These are expected paths after a successful build; this guide does not mean
the packages already exist. See [VALIDATION.md](VALIDATION.md) for the recorded
build results. The commands above do not install ClipVault or upload a release.
Generated artifacts stay in ignored `dist/` and `target/` directories.

## How the scripts work

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

Use `scripts/package-linux.sh deb` or `appimage` for one format, or `stage` to
build the executable and stage an AppDir without packaging tools. Staging alone
does not create a distribution package or apply linuxdeploy's ELF path adjustments.

## Contents and dependencies

The Debian control description uses explicit plain text from
`package.metadata.deb.extended-description`; the README remains a separate file
at `/usr/share/doc/clipvault/README.md`. The Debian package also installs
`/usr/share/metainfo/io.clipvault.desktop.metainfo.xml`, with ClipVault's display
name, package name, desktop launcher ID, and an HTTPS screenshot URL for
`docs/history.png`. Packaging validates that metadata offline with `appstreamcli`;
an offline validation does not verify the public image URL.

See [Software Manager verification](SOFTWARE-MANAGER-VERIFICATION.md) for the
current checks, network/installation restrictions, and the separate Mint 8.4.0
limitation: its Debian screenshot path does not consume packaged AppStream
screenshots. Installing a standalone `.deb` therefore does not by itself supply
a screenshot to that version of Software Manager.

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
clipboard bytes. Install with `sudo apt install ./dist/clipvault_1.2.0-1_amd64.deb`
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

Run `chmod +x dist/ClipVault-1.2.0-x86_64.AppImage` and launch that file.
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


## Offline AppImage runtime

The installed linuxdeploy tools may be cached while appimagetool's separate
runtime download is unavailable. Reuse the unmodified type-2 runtime already
embedded in the trusted local linuxdeploy AppImage, through appimagetool's
supported `--runtime-file` input:

```sh
python3 scripts/extract-cached-runtime.py
LDAI_RUNTIME_FILE="$PWD/target/packaging-tools/runtime-x86_64" bash scripts/package-linux.sh all
bash scripts/test-linux-packages.sh
```

The helper validates the x86_64 ELF/type-2 header and records source/runtime
hashes, the embedded upstream runtime commit, and its original licensing
information in `target/packaging-tools/runtime-x86_64.json`. The AppImage includes
this manifest as `usr/share/doc/clipvault/APPIMAGE-RUNTIME.json`. This build uses
runtime commit `fedfad5`; original runtime licensing and source links remain
embedded in the runtime as well. No replacement runtime design or network
fetch is involved. Only run this helper with a packaging tool you trust.

## Logo and desktop integration in 1.2.0

Both packages include the unchanged SVG and PNGs at 16, 24, 32, 48, 64, 128,
256 and 512 pixels under `usr/share/icons/hicolor/*/apps/clipvault.*`.
`Icon=clipvault` resolves this theme name. The desktop filename is
`io.clipvault.desktop.desktop`, matching the Wayland app ID
`io.clipvault.desktop`; `StartupWMClass=clipvault` matches winit's X11
executable-derived class. The native window explicitly receives the logo
instead of the framework default. Debian maintainer hooks update icon and
desktop caches when the tools are available. AppImages include the icons but
require `bash scripts/install-appimage.sh` for user menu registration.

Run `python3 scripts/audit-icons.py` to inspect launchers before changing any
old entries. It never removes files. The 0.1.0 installation had only one
related application launcher in the checked directories, with a mismatched
uppercase class and only the scalable icon. An archive-content check does not
prove a desktop's menu/taskbar icon; see [manual verification](../VERIFICATION.md).
