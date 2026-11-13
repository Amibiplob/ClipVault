# ClipVault 2.2.0 update protocol and release signing

The GUI remains the normal user. `pkexec /usr/bin/apt-get -y --no-remove install -- PATH`
or `pkexec /usr/bin/dnf -y --setopt=install_weak_deps=False install -- PATH` requests
system authentication. ClipVault never asks for, reads or stores an administrator password.
It checks process exit status, installed package metadata and the installed executable's
`--version`. Locks are never deleted, dependencies are never bypassed and packages are
never forced. A failed package transaction can require repair through the system package
manager; ClipVault cannot promise rollback of arbitrary maintainer scripts.

Detection uses `/proc/self/exe` through Rust's current_exe, package ownership of that file,
application identity and package architecture. AppImage additionally requires APPIMAGE,
APPDIR, the running executable inside APPDIR and the type-2 ELF runtime marker. Installed
package managers or distribution names alone do not establish ownership. APT configured
origins and cached DNF availability conservatively flag repository-managed installations;
these are directed to the system Update Manager. Detection is intentionally fail-closed.
Manually extracted builds and ambiguous ownership offer manual release downloads.

Canonical CPU names are x86_64 and aarch64. Only x86_64 artifacts are built by the current
scripts. RPM compatibility is an exact signed ID + VERSION_ID pair AND embedded package
Distribution field; current support is Fedora 44 only. Ubuntu 24.04/Mint 22.x DEBs use the
existing glibc 2.39 baseline. An AppImage retains that baseline; it does not run on every
Linux distribution. Broaden these claims only after testing actual packages.

## Trust setup (external maintainer action REQUIRED)

`packaging/update-keys.json` is an embedded array of **raw Ed25519 public keys**, each
64 hex characters. It is empty in this source candidate because no maintainer trust key
was supplied. Automatic installation is disabled until a maintainer provisions a key,
rebuilds and distributes a trusted app. Do not generate a throwaway key and call it
maintainer authentication. Never commit a private key or print it in build output.

On the maintainer's secured machine, OUTSIDE this repository:

```sh
install -d -m 700 "$HOME/.local/share/clipvault-signing"
openssl genpkey -algorithm ED25519 -out "$HOME/.local/share/clipvault-signing/release.pem"
chmod 600 "$HOME/.local/share/clipvault-signing/release.pem"
openssl pkey -in "$HOME/.local/share/clipvault-signing/release.pem" -pubout -outform DER -out /tmp/clipvault-public.der
python3 -c 'import pathlib; b=pathlib.Path("/tmp/clipvault-public.der").read_bytes(); assert b[:12].hex()=="302a300506032b6570032100"; print(b[12:].hex())'
```

Put ONLY that public hex string in the JSON array. Review its fingerprint independently,
rebuild all packages, test installation in disposable systems, and then generate and sign:

```sh
python3 scripts/update-metadata.py --tag v2.2.0 --signing-key "$HOME/.local/share/clipvault-signing/release.pem"
```

This command refuses a key inside the repository, a non-Ed25519 key or a key whose public
half is not embedded in the source trust store. Its metadata is generated from actual
current-version artifacts, with DEB/RPM identity inspection, bytes, file size and SHA-256.
Each packaging script regenerates unsigned metadata and **deletes any obsolete signature**;
sign only once all final artifacts have been gathered. Older dist artifacts are untouched.

Upload (as a separate maintainer action) the exact artifacts, `clipvault-update.json`,
`clipvault-update.json.sig` (raw 64-byte detached signature) and `v2.2.0-SHA256SUMS` to
`https://github.com/Amibiplob/ClipVault/releases/tag/v2.2.0`. These files belong in the same
release. JSON bytes must not be reformatted after signing. The updater accepts v2.1.0 and
legacy v.1.2.0 tag syntax, compares SemVer precedence and excludes prereleases/drafts.
The packages workflow gathers the actual DEB/AppImage/RPM candidates and prepares a signed metadata artifact using the external GitHub Actions secret `CLIPVAULT_UPDATE_SIGNING_KEY`. Configure that secret only after the public key is embedded and reviewed. It never publishes a release automatically. Release signing/publication and private-key provisioning were not performed here.

A **SHA-256 checksum** detects changed artifact bytes; it does not establish authenticity.
The **Ed25519 signature** authenticates exact manifest bytes with an embedded trusted key;
only then are URLs, version, package type, architecture and platform chosen. Missing or
invalid signatures never fall back to checksum-only automatic installation. HTTPS initial
URLs must belong to this repository's release downloads. Redirects may only target that
source or GitHub's release-assets.githubusercontent.com production-release-asset path;
HTTP, arbitrary hosts, credentials, unexpected paths and redirect loops are rejected.
Downloads are bounded by signed sizes and checked again before installation. No alternate
package type is downloaded if a matching artifact is absent.

For key rotation ship a release trusted by the old key that embeds both old and new public
keys, let users adopt it, and sign future manifests with the new key. Remove the old key
only after a migration window. Compromise requires out-of-band authenticated distribution
of a replacement trust store; signatures have no network revocation service. Keep an
inventory of public fingerprints and offline backups of secrets with access controls.

## Installation, cancellation and restart

Downloads, signature/digest/package checks and package operations run off the UI thread.
Only one update worker runs at once. Download progress counts real bytes against the signed
artifact size. Cancellation is checked between reads and stages; an outstanding HTTPS
operation can take time to return. Authentication and installation cannot safely be
interrupted by ClipVault. Cancel the system authorization dialog to deny authorization.
Errors distinguish missing metadata/assets, locks, space, canceled authorization and
incompatible identity/version/architecture. Retry reuses only a signed plan and redownloads.

AppImage updates canonicalize the running image path, stage in its own directory, verify
again, preserve executable permissions, fsync, create a hard-link rollback copy and atomically
rename into the original location, preserving launcher references. A failed stage leaves
the original file intact; a commit error attempts rollback. The `.clipvault-PID-rollback`
file remains for manual recovery. Read-only locations require a manual Save As download
(or the in-app save-copy action when a desktop file chooser is installed). Never overwrite
an existing image with an in-progress network download.

The existing installed-file watcher and integrated updater share one restart request. The
watcher cannot restart while the update worker is active. Open edits, pending changes and
paste actions defer restart. Storage is flushed, the exact installed executable is exec'd
as the same user, original launch arguments (including --data-dir) survive, and the existing
single-instance lock is reused. A per-data-directory receipt reports completion only when
the new running version matches. A mismatched receipt is consumed and reported once; it
never creates a restart loop. App updates do not alter Linux boot ID or trigger reboot cleanup.
