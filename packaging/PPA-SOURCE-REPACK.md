# Linux PPA source repacking

This policy applies to fresh, unsigned ClipVault 2.1.0 Noble/amd64 PPA source
candidates. Earlier outputs are retained. No candidate from this work has been
uploaded. Once an orig archive has been uploaded, never replace it under the same
upstream version: use a new upstream repack version for source-content changes.
`--orig` deliberately reuses exact trusted archive bytes and does not apply this
policy retroactively. Omit `--orig` to prepare the corrected source candidate.

## Website

`docs/index.html` is authored HTML with external readable `site.js` and
`style.css`, not generated JavaScript. The original long lines triggered
`source-is-missing`. It is now indented and wrapped without changing the page's
elements, attributes, copy, assets or behavior. No override is needed.

## Windows files investigated

The upstream `winapi-i686-pc-windows-gnu` and
`winapi-x86_64-pc-windows-gnu` 0.4.0 crates contain **1387** and **1416** `lib/*.a`
files respectively. Running `ar t` over every one finds **11** and **12** failures.
Those 23 files are ASCII GNU linker scripts containing `INPUT(...)` lists of
other libraries. They are valid linker inputs named `.a`, not malformed ar
archives. Every other file in these two sets passes `ar t`. Lintian's archive
inspection misclassifies those linker scripts. Both crates' `build.rs` add their
library search path only for their corresponding Windows GNU target; they are
irrelevant to the native Linux build.

`libloading` 0.8.9 `tests/nagisa32.dll` and `tests/nagisa64.dll` are real prebuilt
Windows PE test fixtures. `tests/windows.rs` starts with `#![cfg(windows)]` and
documents their ordinal-export test purpose. Linux ClipVault neither compiles
that Windows test module nor loads these DLLs. Their Lintian finding is valid.

The PPA repack removes **only** those 2803 winapi `lib/*.a` files (including the
23 linker scripts) and the two named DLLs. No crate directories, Rust sources,
build scripts, manifests, features, license/notice files or Cargo.lock entries
are removed. Other Windows crates are retained. This bundle supports the Linux
PPA; use complete upstream vendoring for Windows builds/tests. General source
preparation and Fedora preparation retain their existing complete vendor policy.
There are no Lintian overrides.

## Reproduction and integrity

From `app/`, with a complete Cargo cache, run:

```sh
SOURCE_DATE_EPOCH=1794484800 bash scripts/prepare-ppa-source.sh
```

Use `--online` only to fetch missing crates during preparation. The PPA script
calls `prepare-source.py --linux-ppa`; after `cargo vendor --locked
--versioned-dirs`, `repack-ppa-vendor.py` verifies the **entire** vendor inventory
and every file SHA256 against `.cargo-checksum.json`, including all registry
package checksums against Cargo.lock. It requires the reviewed versions,
upstream package checksums and exact exclusion counts before any removal.

Only the excluded files' entries are removed from the three affected
`.cargo-checksum.json` `files` maps. Their `package` values remain the original
registry `.crate` checksums expected by Cargo.lock; they are not checksums of the
repacked directory. Every retained per-file hash is unchanged and verified again.
No checksum is cleared and Cargo source replacement/offline verification stays
enabled. `packaging/PPA-VENDOR-REPACK.json` inside the orig archive records the
lock hash, original/repacked checksum-manifest hashes, and every excluded path
and original SHA256. This report has stable sorted output and no timestamps or
absolute paths. The tar archive normalizes timestamps/owners and traversal order.
Identical source inputs, Cargo cache, file modes, epoch and tool versions produce
identical orig bytes; this does not promise identical `.buildinfo` timestamps.

Both preparation stages run complete `cargo metadata --locked --offline` with
an empty Cargo home (without platform filtering). Verify extracted bytes with:

```sh
PPA_WORK=$(cat target/ppa-source/latest-path.txt)
python3 scripts/repack-ppa-vendor.py --verify "$PPA_WORK/clipvault-2.1.0"
lintian --tag-display-limit 0 "$PPA_WORK/clipvault_2.1.0-1~ppa1~noble1_source.changes"
```

## Noble build dependency

Noble main provides `pkgconf` **1.8.1-2build1**. Its installed file list includes
`/usr/bin/pkg-config`, which resolves to `/usr/bin/pkgconf`; native library
queries still work. `debian/control` now depends directly on `pkgconf`.
References: [Ubuntu Noble pkgconf](https://packages.ubuntu.com/noble/pkgconf)
and [amd64 file list](https://packages.ubuntu.com/noble/amd64/pkgconf/filelist).

## Build and next local command

Check `target/lintian-fix/RESULTS.md` for this run's exact evidence and any
remaining findings. If a clean chroot is unavailable in Craft Anything, use
the Noble chroot setup in [LAUNCHPAD-PPA.md](LAUNCHPAD-PPA.md), then run in your
regular Linux Mint Terminal from `app/`:

```sh
PPA_WORK=$(cat target/ppa-source/latest-path.txt)
mkdir -p "$PPA_WORK/build-results"
(cd "$PPA_WORK/build-results" && sbuild --dist=noble --arch=amd64 "$PPA_WORK/clipvault_2.1.0-1~ppa1~noble1.dsc")
```

This does not sign or upload. Public maintainer placeholders still require the
owner's verified identity before any later signing or publication.
