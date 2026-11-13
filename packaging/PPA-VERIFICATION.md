# ClipVault 2.1.0 Noble PPA preparation verification

Checked 2026-11-13 in the Craft Anything restricted environment, on Linux Mint
22.3 (Zena), Ubuntu base `noble`, amd64. This records local preparation evidence,
not Launchpad publication. Follow [the owner Terminal guide](LAUNCHPAD-PPA.md).

## Toolchain and Build-Depends

- Installed preparer: `rustc 1.99.1 (58cf8f9c3 2026-10-13)` and
  `cargo 1.99.1 (5f94df478 2026-08-27)`; only the stable rustup toolchain is present.
- The existing PPA rules deliberately select Ubuntu Rust/Cargo 1.89, separately
  from the preparer's toolchain. Neither versioned executable is installed here.
- Local `apt-cache policy rustc-1.89 cargo-1.89` reports candidate
  `1.89.0+dfsg~24.04-0ubuntu0.24.04.2` from Noble updates/security universe;
  the official Ubuntu package pages also list these versioned packages.
- Linux-filtered locked resolution: **256 packages**, maximum declared MSRV
  **1.88**. This is compatible by metadata with the selected 1.89 compiler,
  but compilation using 1.89 remains unverified.
- `dpkg-checkbuilddeps` fails here with these missing requirements:
  `debhelper-compat (= 13)`, `rustc-1.89 (>= 1.89)`, `cargo-1.89 (>= 1.89)`,
  `libwayland-dev`, `libxkbcommon-dev`, `libegl-dev`, `libgl-dev`.
  `dpkg-buildpackage`/`dpkg-source`, Python, pkg-config, desktop-file-utils,
  SQLite and GdkPixbuf are available. Debsign/devscripts, lintian, sbuild and
  dput are unavailable. No host packages were installed.
- A download-only attempt to obtain the four versioned Rust/Cargo/standard-library
  packages failed (exit 100) on DNS resolution. Log:
  `target/ppa-toolchain/download.log`. No compiler was extracted or tested.

## Local checks completed

- `cargo metadata --locked --offline --filter-platform x86_64-unknown-linux-gnu`
  succeeded. Metadata retained in `target/ppa-linux-metadata.json`.
- Complete `cargo vendor --locked --offline --versioned-dirs` now succeeds,
  including the previously missing other-platform crates. The old guide's
  missing-`wasi` blocker no longer describes this checkout.
- The source preparer resolves the **complete** vendored lockfile graph with
  `cargo metadata --locked --offline` and an empty Cargo home before creating
  the tar archive; the PPA preparer repeats this after extraction.
- A fresh `cargo check --locked --offline` from the vendored source with an empty
  Cargo home and separate empty target directory passed in **52.30 seconds**
  using **1.99.1**. This verifies vendor-only compilation checks, including build
  scripts/procedural macros; it does not link/test the final program or use 1.89.
  Log: `target/ppa-source/2.1.0-1~ppa1~noble1-H0YBbaJx/offline-check.log`.
- `cargo build --release --locked --offline`: passed with the installed **1.99.1**
  compiler. This is a local native build using the local cache, not a Noble
  Rust 1.89 chroot build or an offline binary build from the vendor archive.
- `cargo test --locked --offline`: **76 passed, 0 failed, 3 ignored**, 27.67 seconds.
  Log: `target/ppa-tests.log`. Ignored tests require real desktop clipboard or
  loopback sockets and do not establish host desktop behavior.
- `rebuild_check`: **build passed**, **app-screenshot passed**, starts/draws at
  **440×540**. Screenshot: `checks/round-0/screenshot.png` in the parent project.
  Screenshot inspected; no application UI code changed for this task.
- Shell syntax, Python compilation and Git whitespace checks pass. Staged public
  identity substitution passed with a backslash-containing name; three invalid
  emails (placeholder, injected newline, malformed) were rejected without writing
  metadata. Unsupported arguments, missing orig archive and non-Noble targets
  were also rejected before packaging.

## Source artifacts and boundaries

Source preparation runs in a unique directory below `target/ppa-source/` and
updates `target/ppa-source/latest-path.txt` only after all five unsigned source
artifacts and `SHA256SUMS.unsigned` are generated. Read that pointer for the latest
successful output; `target/ppa-prepare.log` records the actual preparation run.
The first unsigned source build completed successfully (exit 0). A final source
rebuild includes the current guide and this verification snapshot.
A failed run must not be treated as a completed source package. The final
artifact validation record is saved alongside the successful outputs.

The repository's existing `debian/control`, `debian/rules`, changelog and
`3.0 (quilt)` format are retained. Because the owner's verified email was not
provided, local unsigned candidates use **maintainer@example.invalid** and must
be rebuilt with the public `DEBFULLNAME`/`DEBEMAIL` before signing or uploading.
No real signing key, keyring or passphrase was accessed or generated.
Common key files are ignored/excluded from source preparation; armored secret
keys in copied application inputs are rejected. Vendor license/checksum files
are retained. Preparation does not sign, register a key or upload.

All preexisting `dist/` artifacts and the earlier RPM source tarball were hashed
before work in `target/ppa-preserved-artifacts.json`; they must remain unchanged.
Preexisting Fedora/RPM edits are preserved. No commit, push or deployment is
performed by this work.

**Still required in the owner's regular Linux Mint Terminal:** install/check the
Noble Build-Depends; clean sbuild with Rust/Cargo 1.89, empty Cargo home and
vendored/offline compilation/tests; lintian; public maintainer identity; local GPG
key selection/creation and Launchpad public-key confirmation; source signing;
explicit dput upload; Launchpad acceptance, amd64 Noble build log and Published
state; signed APT index and disposable-system installation/upgrade check.

There is no evidence of an accepted upload or publication from this work.
