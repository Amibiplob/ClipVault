# Current feature update: 1.2.0

See [V1.2.0-VERIFICATION.md](../V1.2.0-VERIFICATION.md) for current evidence and remaining desktop checks. Earlier verification below is historical.

---

# Current feature update: 1.1.0

See [FEATURE-VERIFICATION.md](../FEATURE-VERIFICATION.md) for current feature tests and package results. The 1.0.0 evidence below is historical.

---

# Current 1.0.0 validation

See [VERIFICATION.md](../VERIFICATION.md) for the current release results,
actual package paths and required manual checks. The section below preserves
the earlier packaging investigation; its missing-tool status is historical,
and does not describe the current tool cache or 1.0.0 artifacts.

## Historical 0.1.0 investigation

# Packaging validation — 2026-11-07

**No finished .deb or .AppImage exists in this workspace.** Distribution is
blocked by unavailable packaging tools and network access. No upload occurred.
The source executable and a staged AppDir were built and checked; these are
not substitutes for the two requested distribution packages.

## Environment and blocked commands

- Host: Linux Mint 22.3 (Ubuntu noble), x86_64; rustc 1.99.1, cargo 1.99.1.
- `cargo-deb`, `linuxdeploy` and `linuxdeploy-plugin-appimage` were not installed
  or found in the checked local tool/cache locations.
- An offline cargo-deb installation attempt failed because its source was not
  cached. An HTTPS request for official linuxdeploy failed with
  `curl: (6) Could not resolve host: github.com`. Network is restricted and this
  session does not permit requesting elevated access.
- `scripts/package-linux.sh deb` and `scripts/package-linux.sh appimage` each
  failed at their missing-tool preflight, before producing a package.
- `scripts/test-linux-packages.sh` failed explicitly for both missing package
  files. Neither package test is reported as passing.

## Completed checks

- `cargo build --release --locked --offline`: passed after the AppImage startup
  fix; `target/release/clipvault` is the actual 7.5 MB x86-64 executable.
- `cargo build --locked --offline`: passed.
- `cargo test --locked --offline`: **9 passed, 0 failed**.
- `cargo fmt -- --check`: passed after formatting the startup change.
- `cargo clippy --locked --offline -- -D warnings`: passed.
- Shell syntax and Python compilation checks: passed.
- Notice generation and `notices --check`: passed for **158 active Linux Rust
  dependencies**, build dependencies included, plus bundled font attributions
  and the installed Rust standard library's complete notices.
- `scripts/package-linux.sh stage`: passed; generated
  `target/packaging/ClipVault.AppDir` with executable, launcher, icon, notices,
  explicitly loaded libraries, thumbnail loaders and non-host dependencies.
- `scripts/test-linux-packages.sh --stage`: passed, including copying the
  AppDir to a different path containing spaces, library linkage, desktop-entry
  validation, executable architecture, help, diagnostics, an actual SQLite
  create/insert/read cycle and GdkPixbuf GIF/WebP thumbnail decoding. These
  checks use the bundled libraries and a cache generated at the relocated path.
- The non-host ELF library closure check passed. Distribution library notice
  and source metadata collection passed for **39 system library packages**,
  including full copyright files and referenced common license texts.
- Craft Anything `rebuild_check`: **build passed; app-screenshot passed** at
  **740×780**, `checks/round-0/screenshot.png` (outside app/). Image inspected.
  This check captures the normal source build, not either distribution package.

## Untested or incomplete

- .deb archive creation/content inspection, apt dependency resolution,
  installation/removal, installed desktop launcher and installed executable.
- linuxdeploy execution, ELF patching, AppImage output-plugin execution,
  actual embedded AppImage runtime and its notices, FUSE mounting and extracted
  AppImage behavior. The staged launcher is not an AppImage runtime test.
- A direct staged `--gui` check failed with
  `ClipVault: Operation not permitted (os error 1)` under the shell sandbox.
  Only the source GUI capture through `rebuild_check` passed.
- Login startup after reboot, inherited AppImage launcher environments,
  external clipboard copy/select/paste, Wayland and other Mint releases,
  MATE/Xfce and other GPU/processor systems.
- Retrieval of matching system-library source archives for publication beside
  a future AppImage. The generated source manifest records package names and
  versions, but is not a source archive or proof of its distribution.

## Next build with internet and packaging tools

Follow [README.md](README.md), then run the real package checks on a Mint
desktop before uploading. Update this report with actual artifact hashes,
archive checks, runtime tests and any remaining limitations. Package creation
and a successful source screenshot do not demonstrate installation or portable
runtime compatibility.

The exact **expected output paths, not finished files**, are:

```text
/home/biplob/Documents/Craft Anything/build-clipvault-a-modern/app/dist/clipvault_0.1.0-1_amd64.deb
/home/biplob/Documents/Craft Anything/build-clipvault-a-modern/app/dist/ClipVault-0.1.0-x86_64.AppImage
```
