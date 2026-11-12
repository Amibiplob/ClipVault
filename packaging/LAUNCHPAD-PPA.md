# Launchpad PPA `clipvault` — prepared, not published

No Launchpad account, PPA, signing key or upload was used. A PPA builds **source packages**;
uploading the existing binary `.deb` does not publish a PPA. Do not advertise APT
installation until the real PPA has successfully built and published this version.

## Target and toolchain

| Prepared target | Corresponding Mint | Architecture | Rust / Cargo | Build status |
|---|---|---|---|---|
| Ubuntu 24.04 `noble`, updates/security + universe | Mint 22, 22.1, 22.2, 22.3 | amd64 | `rustc-1.89`, `cargo-1.89` | Recipe prepared; clean PPA build unverified |

This is the sole prepared source target. Ubuntu 22.04/Mint 21.x, newer Ubuntu
series, LMDE and other architectures are not claimed. They require separate
changelog versions, dependency/toolchain checks and clean builds. Mint uses the
Ubuntu base suite (`noble`), not its own codename, for this PPA.

The locked Linux graph requires Rust >=1.88 (including `image` and ICU).
Noble's default Rust 1.75 is insufficient. Versioned Rust/Cargo 1.89 packages
are available in Noble's universe updates/security archives; `debian/control`
requires them and `debian/rules` selects their absolute executable paths.
Enable the appropriate archive components/pockets on the clean build host and
PPA; verify candidates with `apt-cache policy rustc-1.89 cargo-1.89`.
No rustup or dependency downloads are attempted by the build rules.

## Reproducible source preparation

Use an Ubuntu 24.04 amd64 preparation host with network access, Rust >=1.88,
`dpkg-dev`, `devscripts`, `debhelper`, `lintian`, `sbuild`, `dput` and GnuPG.
Keep private keys outside this repository. Replace the maintainer name/email in
`debian/control` and `debian/changelog` with the identity registered in Launchpad.
Use a higher `~ppaN~nobleN` revision for every subsequent upload of changed source.

```sh
export SOURCE_DATE_EPOCH=1794484800
cargo fetch --locked
python3 scripts/packaging-support.py notices
bash scripts/prepare-ppa-source.sh --online
```

`prepare-source.py` copies an explicit source allowlist, vendors the complete
Cargo.lock graph (including other-target crates), retains upstream licenses,
notices and Cargo checksums, and validates resolution with an empty Cargo home.
It normalizes source tar ownership/timestamps and uses sorted archive entries.
No binaries, target directories, Git credentials, `.env` files or personal history
are included. Pin the build-image digest and Rust/system-package versions when
comparing binary reproducibility; a locked graph alone is not a bit-for-bit promise.

`--online` is needed only while preparing missing crate sources. Later preparation
can run without it if the complete Cargo cache is available. The source contains
`vendor/` and `.cargo/config.toml`; builders use `--locked --offline` with an isolated
Cargo home. Keep all vendor files and licenses; do not prune non-Linux crates from
Cargo.lock just to work around an incomplete cache. Upload no incomplete archive.

Unsigned outputs are under `target/ppa-source/`:

```text
clipvault_2.1.0.orig.tar.xz
clipvault_2.1.0-1~ppa1~noble1.debian.tar.xz
clipvault_2.1.0-1~ppa1~noble1.dsc
clipvault_2.1.0-1~ppa1~noble1_source.changes
clipvault_2.1.0-1~ppa1~noble1_source.buildinfo
```

The source-only build uses `dpkg-buildpackage -S -sa -us -uc -d -nc`: `-d` is
only for preparation without binary build dependencies; `-nc` skips cleaning the freshly extracted source. It does not prove a
clean binary build. Inspect all files, then on a configured disposable Noble
build host run:

```sh
lintian target/ppa-source/clipvault_2.1.0-1~ppa1~noble1_source.changes
sbuild --dist=noble --arch=amd64 target/ppa-source/clipvault_2.1.0-1~ppa1~noble1.dsc
```

Provision the sbuild chroot's declared Build-Depends from Ubuntu before restricting
its network. Confirm Cargo build/test succeeds without network and without the
preparer's cache. Inspect the resulting package's dependencies, icon sizes,
launcher, AppStream metadata, license notices and `clipvault --version`. Test
installation, isolated launch, upgrade with fixture history/pins/preferences,
removal and desktop behavior before declaring the target supported. Debian
maintainer scripts refresh desktop/icon caches and never delete personal history,
including on purge; no new storage migration is needed for 2.1.0.

## One-time account and signing setup (owner action)

1. Create a Launchpad account and verify the email used in the changelog.
2. Create the public PPA named **clipvault** under `LAUNCHPAD_USERNAME`.
3. Generate or select your own GPG signing key locally; register its **public**
   key/fingerprint on Launchpad and complete Launchpad's verification. Keep the
   private key, passphrase and credential files outside the source tree.
4. Configure upload authentication as needed. Launchpad documents FTP and SFTP;
   if using SFTP, register your public SSH key and use a local `~/.dput.cf` profile
   with `method = sftp`, `fqdn = ppa.launchpad.net`, `login = LAUNCHPAD_USERNAME`,
   and `incoming = ~LAUNCHPAD_USERNAME/ubuntu/clipvault/`. Check the current
   Launchpad guide for endpoint/authentication requirements before uploading.

After all clean-build checks pass, sign the reviewed source artifacts locally:

```sh
debsign -kGPG_SIGNING_KEY_FINGERPRINT \
  target/ppa-source/clipvault_2.1.0-1~ppa1~noble1_source.changes
gpg --verify target/ppa-source/clipvault_2.1.0-1~ppa1~noble1.dsc
gpg --verify target/ppa-source/clipvault_2.1.0-1~ppa1~noble1_source.changes
dput ppa:LAUNCHPAD_USERNAME/clipvault \
  target/ppa-source/clipvault_2.1.0-1~ppa1~noble1_source.changes
```

Replace `LAUNCHPAD_USERNAME` and `GPG_SIGNING_KEY_FINGERPRINT`; they are explicit
placeholders. An SFTP profile uses its chosen profile name in place of `ppa:…`.
Alternatively build/sign together inside the prepared source directory using
`dpkg-buildpackage -S -sa -kGPG_SIGNING_KEY_FINGERPRINT`. Review signatures and
upload the `_source.changes` file with its referenced source archives, not a `.deb`.

Check Launchpad's upload acceptance/rejection email, PPA Builds page, amd64 Noble
build log and Published status. Confirm the signed APT index contains 2.1.0 and
test installation/update from that index in a clean Noble/Mint fixture system.
Only then add the **real** PPA installation command and supported suite to the
README/website. A successful `dput` transfer alone is not publication.

## Current blockers and troubleshooting

In this sandbox `cargo vendor --locked --offline` fails because
`wasi 0.11.1+wasi-snapshot-preview1` is not cached. This cross-target dependency
is still required for complete source resolution; native Linux binary builds
work from the available cache. Network DNS is unavailable, so fetching it and
any further missing crates requires another host. Debhelper, lintian, sbuild,
dput and the versioned Ubuntu Rust 1.89 toolchain are not installed here.
Source metadata checks alone do not establish an offline clean build or a
Launchpad-accepted source package. See the current verification record.

If Launchpad reports missing Build-Depends, check universe/updates/security and
the versioned toolchain candidates. If Cargo requests network, reject that build
and regenerate the complete vendor archive. If an upload revision is rejected as
already used, increment the Debian revision and rebuild/sign; do not rename files.

References checked during preparation:
[Launchpad source uploads](https://ubuntu.com/docs/launchpad/user/how-to/packaging/ppa-package-upload/),
[PPA source package requirements](https://ubuntu.com/docs/launchpad/user/reference/packaging/ppas/ppa/),
[Noble Rust 1.89](https://packages.ubuntu.com/noble/rustc-1.89),
[Mint release bases](https://linuxmint.com/download_all.php).
