# ClipVault 2.1.0 → amibiplob/clipvault (Noble)

**Prepared for upload; not signed, uploaded, accepted or published by this work.**
Launchpad username: **amibiplob**. Destination: **ppa:amibiplob/clipvault**.
A source package is uploaded; the existing binary `.deb` is not a PPA upload.

All terminal blocks below are **OWNER ACTION — run in your regular Linux Mint
Terminal**, not Craft Anything's restricted terminal. Browser steps are marked.
Run blocks in order, stop on errors, and keep the same terminal unless told to
open a new one. No private key or passphrase should be pasted into Craft Anything.

## Target and verified requirements

Ubuntu **24.04 Noble**, **amd64**, is the source target for Linux Mint **22.3**
(and the other Ubuntu-based Mint 22.x releases). Use `noble`, not Mint's `zena`.
LMDE, Mint 21.x and other architectures are outside this recipe.

The existing `debian/` packaging is reused: `3.0 (quilt)`, debhelper 13,
`2.1.0-1~ppa1~noble1`, `Architecture: amd64`. `debian/rules` selects
`/usr/bin/rustc-1.89`, `/usr/bin/rustdoc-1.89`, `/usr/bin/cargo-1.89` and uses
`--locked --offline` with a separate `debian/cargo-home`. No rustup/bootstrap
or crate downloads occur on the Launchpad builder.

The 256-package resolved Linux graph declares a maximum minimum Rust version
of **1.88** (`image` 0.25.10 and ICU 2.4); Noble's original 1.75 is insufficient.
APT metadata and the Ubuntu package index list versioned Rust/Cargo **1.89**,
`1.89.0+dfsg~24.04-0ubuntu0.24.04.2`, in Noble updates/security universe.
A declared minimum-version check is not a successful compiler/build check.

The local source/build results and remaining blockers are recorded separately in
[PPA verification](PPA-VERIFICATION.md). Check live candidates again before upload.

## 1. Install tools and create or reuse your GPG key

**Linux Mint Terminal:** go to the existing source folder (adjust this path if you
moved the project). These installs change your local machine and require sudo.

```bash
cd "$HOME/Documents/Craft Anything/build-clipvault-a-modern/app"
. /etc/os-release
printf 'System: %s; Ubuntu base: %s\n' "$PRETTY_NAME" "${UBUNTU_CODENAME:-unknown}"
dpkg --print-architecture
```

Continue only for Ubuntu base `noble` and `amd64`. Enable universe if missing;
keep the standard Noble updates and security repositories enabled in Software
Sources. Do not add a different Ubuntu release to get a newer compiler.

```bash
sudo add-apt-repository universe
sudo apt update
apt-cache policy rustc-1.89 cargo-1.89
sudo apt install build-essential dpkg-dev devscripts debhelper lintian sbuild \
  schroot debootstrap dput gnupg ripgrep pkgconf python3 ca-certificates \
  rustc-1.89 cargo-1.89 libstd-rust-1.89-dev desktop-file-utils \
  libsqlite3-0 libgdk-pixbuf-2.0-0 libwayland-dev libxkbcommon-dev libegl-dev libgl-dev
/usr/bin/rustc-1.89 --version
/usr/bin/cargo-1.89 --version
dpkg-checkbuilddeps
```

Both executables must report 1.89, candidates must exist, and `dpkg-checkbuilddeps`
must exit successfully. On a fresh preparation host select these tools for
vendoring too (this does not install anything with rustup):

```bash
export RUSTC=/usr/bin/rustc-1.89
export RUSTDOC=/usr/bin/rustdoc-1.89
mkdir -p target/ppa-tools
ln -sf /usr/bin/cargo-1.89 target/ppa-tools/cargo
ln -sf /usr/bin/rustc-1.89 target/ppa-tools/rustc
export PATH="$PWD/target/ppa-tools:$PATH"
cargo --version
rustc --version
```

Keep your keyring at its normal location outside the repository. List existing
secret keys **locally**; reuse an unexpired, unrevoked key with signing capability
and access to the secret key. It needs the email you have verified on Launchpad.

```bash
export GNUPGHOME="$HOME/.gnupg"
install -d -m 700 "$GNUPGHOME"
export GPG_TTY=$(tty)
gpg --list-secret-keys --keyid-format long --with-fingerprint
```

If no suitable key exists, run this interactive command. Select **RSA and RSA**,
**4096** bits, an expiry such as **2y**, your public name and Launchpad-verified
email, and a strong passphrase in the GnuPG prompt. The RSA encryption subkey
lets you decrypt Launchpad's confirmation email.

```bash
gpg --full-generate-key
gpg --list-secret-keys --keyid-format long --with-fingerprint
```

Copy the full primary fingerprint, not the short key ID. Enter it and your public
identity below when prompted. These values are public; your private key stays in
`~/.gnupg`, and the passphrase goes only into GnuPG's prompt.

```bash
read -r -p 'Full GPG primary fingerprint: ' GPG_KEY
export GPG_KEY
read -r -p 'Your public maintainer name: ' DEBFULLNAME
export DEBFULLNAME
read -r -p 'Your Launchpad-verified public email: ' DEBEMAIL
export DEBEMAIL
gpg --fingerprint "$GPG_KEY"
gpg --list-secret-keys "$GPG_KEY"
```

Back up your keyring/revocation certificate securely outside this project.
Never export secret keys into the repository, source archive, or upload folder.
`.gitignore` and source exclusions cover common key files, and preparation rejects
armored private keys in copied input; these safeguards do not replace keeping
keys outside the project. No signing key was created or inspected by this work.

## 2. Register the public key and create/reuse your PPA

**Linux Mint Terminal:** publish only your public key to Ubuntu's keyserver.
Your name/email in that public key become publicly discoverable.

```bash
gpg --keyserver hkps://keyserver.ubuntu.com --send-keys "$GPG_KEY"
```

**Browser, signed in as amibiplob:**

1. Verify the email used above on your Launchpad account.
2. Open <https://launchpad.net/~amibiplob/+editpgpkeys>, paste the full fingerprint,
   and import it. Allow time for keyserver propagation if it is not found yet.
3. Launchpad emails an encrypted challenge. Save its encrypted attachment as
   `~/Downloads/launchpad-confirmation.asc`, then decrypt it locally:

```bash
gpg --decrypt "$HOME/Downloads/launchpad-confirmation.asc"
```

**Browser:** follow the confirmation link from the decrypted message. Confirm the
key appears on your profile. Open <https://launchpad.net/~amibiplob/+archive/ubuntu/clipvault>;
if it does not exist, use **Create a new PPA** on your profile and name it exactly
`clipvault`. Review/accept Launchpad's terms yourself. Check **Edit PPA dependencies**:
Noble builds must have access to updates/security and universe. No PPA/account
existence or key registration is assumed here.

The default `dput ppa:…` route uses FTP; the source signatures authenticate your
upload, and it does not require an SSH key. Optional SFTP setup is in Launchpad's
upload guide and requires a registered public SSH key and a local dput profile.

## 3. Build, check and sign the Debian source package

**Linux Mint Terminal, from `app/`:** keep the identity variables from step 1.
The script substitutes them in the **staged** control/changelog files; the
repository retains explicit placeholders because your verified email is unknown.
Without both variables it produces an unsigned placeholder candidate only.

```bash
export SOURCE_DATE_EPOCH=1794484800
bash scripts/prepare-ppa-source.sh --online
PPA_WORK=$(cat target/ppa-source/latest-path.txt)
DEB_VERSION=$(dpkg-parsechangelog -l debian/changelog -S Version)
SOURCE_CHANGES="$PPA_WORK/clipvault_${DEB_VERSION}_source.changes"
SOURCE_DSC="$PPA_WORK/clipvault_${DEB_VERSION}.dsc"
cat "$PPA_WORK/clipvault-2.1.0/debian/control"
cat "$PPA_WORK/clipvault-2.1.0/debian/changelog"
lintian --tag-display-limit 0 "$SOURCE_CHANGES"
```

`--online` allows the **preparation** step to fetch any missing crates. It vendors
all of Cargo.lock, including other-platform crate metadata and upstream licenses,
and resolves the complete graph with an empty Cargo home offline. The Linux PPA
[repack policy](PPA-SOURCE-REPACK.md) excludes exactly the unused winapi `lib/*.a`
files and two libloading DLL test fixtures, updating only their per-file checksum
entries while verifying all retained files and preserving registry checksums.
Do not prune dependencies or ignore a missing crate. Once the complete Cargo
cache is available, omit `--online` to repeat preparation offline.

Outputs are in a **new unique folder** under `target/ppa-source/` per successful
attempt; `latest-path.txt` is updated only on success. The five upload artifacts
are `.orig.tar.xz`, `.debian.tar.xz`, `.dsc`, `_source.buildinfo`, `_source.changes`;
`SHA256SUMS.unsigned` records their hashes before signing. Earlier outputs and
`target/source-release/` are preserved. The script uses
`dpkg-buildpackage -S -sa -us -uc -d -nc`; `-d -nc` is only unsigned source
preparation from a fresh extraction without installed build dependencies. It
**does not** establish a clean binary build.

### Check a clean Noble builder before signing

**Linux Mint Terminal, one time:** sbuild needs a disposable chroot and membership
in its group. This setup uses sudo on your own machine.

```bash
sudo sbuild-adduser "$USER"
```

Log out and back in to activate the group. Open a new regular Terminal, return to
`app/`, and repeat the tool-selection and public-variable blocks in step 1. Then:

```bash
sudo sbuild-createchroot --arch=amd64 --components=main,universe \
  --include=ca-certificates noble /srv/chroot/noble-amd64 \
  http://archive.ubuntu.com/ubuntu
sudo tee /srv/chroot/noble-amd64/etc/apt/sources.list >/dev/null <<'SOURCES'
deb http://archive.ubuntu.com/ubuntu noble main universe
deb http://archive.ubuntu.com/ubuntu noble-updates main universe
deb http://security.ubuntu.com/ubuntu noble-security main universe
SOURCES
sudo chroot /srv/chroot/noble-amd64 apt update
sudo chroot /srv/chroot/noble-amd64 apt-cache policy rustc-1.89 cargo-1.89
```

Confirm the versioned toolchain candidates exist. **Linux Mint Terminal:**

```bash
PPA_WORK=$(cat target/ppa-source/latest-path.txt)
DEB_VERSION=$(dpkg-parsechangelog -l debian/changelog -S Version)
SOURCE_CHANGES="$PPA_WORK/clipvault_${DEB_VERSION}_source.changes"
SOURCE_DSC="$PPA_WORK/clipvault_${DEB_VERSION}.dsc"
mkdir -p "$PPA_WORK/build-results"
(cd "$PPA_WORK/build-results" && sbuild --dist=noble --arch=amd64 "$SOURCE_DSC")
```

sbuild installs declared Ubuntu Build-Depends first, then compiles/tests the
vendored source with Cargo networking disabled and its own empty Cargo home.
Read the build log and fix errors; require a successful clean build before
uploading. Confirm `clipvault --version` is 2.1.0, inspect the produced `.deb`,
and test installation/upgrading in a disposable Noble/Mint system. Local native
builds or the existing release `.deb` are not a substitute for this source build.

Sign the reviewed **source** `.changes` using your local key. `debsign` signs the
`.dsc` and `.buildinfo` and updates the checksums in `.changes` correctly; signing
these files independently with raw `gpg --clearsign` would break those references.

```bash
if rg -n 'example\.invalid' "$PPA_WORK/clipvault-2.1.0/debian/control" \
  "$PPA_WORK/clipvault-2.1.0/debian/changelog"; then
  echo 'STOP: rebuild with your public DEBFULLNAME and Launchpad-verified DEBEMAIL.'
else
  debsign -k"$GPG_KEY" "$SOURCE_CHANGES"
fi
gpg --verify "$SOURCE_DSC"
gpg --verify "$PPA_WORK/clipvault_${DEB_VERSION}_source.buildinfo"
gpg --verify "$SOURCE_CHANGES"
lintian "$SOURCE_CHANGES"
(cd "$PPA_WORK" && sha256sum ./*.orig.tar.xz ./*.debian.tar.xz ./*.dsc \
  ./*_source.buildinfo ./*_source.changes > SHA256SUMS.signed)
```

`ripgrep` (the `rg` command) is installed in step 1 for the placeholder check.
All three signatures must be
good and match the registered fingerprint. The unsigned hash file naturally no
longer matches files changed by signing. As an alternative, on a host with all
Build-Depends installed, `(cd "$PPA_WORK/clipvault-2.1.0" &&
dpkg-buildpackage -S -sa -k"$GPG_KEY")` cleans, builds and signs the source together;
re-run the inspections and signature checks afterwards.

### Later packaging revisions

Never reuse an uploaded Debian version for changed source. For a packaging-only
fix, increment it (for example `2.1.0-1~ppa2~noble1`) with `dch --newversion
2.1.0-1~ppa2~noble1 --distribution noble`, then reuse the **exact original** trusted
`clipvault_2.1.0.orig.tar.xz` from the first upload:

```bash
bash scripts/prepare-ppa-source.sh --orig /absolute/path/to/clipvault_2.1.0.orig.tar.xz
```

Only `debian/` changes are applied in this mode; upstream changes are not copied.
Do not regenerate a different orig archive under the same upstream version.
For upstream changes, increment the application/upstream version and regenerate
its source archive. Recheck, clean-build and sign each new revision. Filenames
come from the changelog automatically; do not rename old signed artifacts.

## 4. Upload with dput (explicit owner action)

**Linux Mint Terminal, only after all checks/signatures pass and the public key
and PPA are confirmed in your account:**

```bash
dput --simulate ppa:amibiplob/clipvault "$SOURCE_CHANGES"
dput ppa:amibiplob/clipvault "$SOURCE_CHANGES"
```

The first command is a dry run; the second performs the real public upload.
Keep all referenced source files beside `.changes`. Upload **`_source.changes`**,
not a binary `.deb`, `_amd64.changes`, RPM or AppImage. A completed transfer is
not an accepted upload, successful build or publication. This work did not run it.

## 5. Read Launchpad errors and confirm publication

**Browser:** open
<https://launchpad.net/~amibiplob/+archive/ubuntu/clipvault/+packages> and
<https://launchpad.net/~amibiplob/+archive/ubuntu/clipvault/+builds>.
Read the acceptance/rejection email sent to the signer. For the exact uploaded
version, select **Noble / amd64**, open the build entry and download its build log.

| Status/error | What to do |
|---|---|
| No accepted source / rejected upload | Check registered GPG fingerprint, verified email, signature/checksum errors, source `.changes`, `noble`, exact PPA destination and revision. |
| Dependency wait | Check Build-Depends candidates in Noble updates/security universe and PPA dependency settings; no rustup/network bootstrap in rules. |
| Failed to build | Open the build log; find the first real compiler/test/debhelper error above the final failure. Reproduce with the clean sbuild command, fix and increment the Debian revision. |
| Cargo asks for network / missing crate | Regenerate a complete vendor source archive for a new upstream version as necessary; never bypass offline checks or prune the lockfile. |
| Pending / building / built but not published | Wait for the builder/publisher. A green local build or `dput` receipt does not mean published. |
| Published | Verify the exact source version **and amd64 Noble binary** are published, then test the signed APT index and installation on a disposable system. |

Only after Launchpad confirms publication, run on a **disposable Mint/Noble test
machine**, not automatically on your daily machine:

```bash
sudo add-apt-repository ppa:amibiplob/clipvault
sudo apt update
apt-cache policy clipvault
sudo apt install clipvault
clipvault --version
```

Require the expected version from this PPA and `ClipVault 2.1.0`. If an existing
manual `.deb` is installed as `2.1.0-1`, the prepared `2.1.0-1~ppa1~noble1` sorts
lower, so APT may retain the manual package. Validate on a clean system and plan
an explicit version transition before migrating those users; do not silently
force a downgrade or claim they will automatically update.

## Official references

Checked during preparation; the verification record distinguishes actual tests.

- [Mint 22.3 release notes, Ubuntu 24.04 base](https://linuxmint.com/rel_zena.php)
- [Ubuntu Noble Rust 1.89 package](https://packages.ubuntu.com/en/noble-updates/rustc-1.89)
- [Ubuntu Noble Cargo 1.89 package](https://packages.ubuntu.com/en/noble-updates/cargo-1.89)
- [Launchpad OpenPGP import](https://documentation.ubuntu.com/launchpad/user/how-to/import-openpgp-key/)
- [Launchpad source package requirements](https://ubuntu.com/docs/launchpad/user/reference/packaging/ppas/building-a-source-package/)
- [Launchpad dput/upload guide](https://ubuntu.com/docs/launchpad/user/how-to/packaging/ppa-package-upload/)
