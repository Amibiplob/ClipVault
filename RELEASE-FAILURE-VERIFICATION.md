# v2.2.1 release failure fixes — local verification

Verified on 2026-11-15 in the Craft Anything checkout. These are local source
fixes, not a successful hosted release or permission to publish.

## Changes

- Noble Actions and local instructions install the same explicit package list
  in `packaging/linux/build-packages.txt`, including `libxcb-shape0` and every
  explicit DEB runtime dependency. AppImage/all/stage packaging checks all 20
  required runtime libraries plus the query tool and GIF/WebP loaders before
  compilation/DEB creation, and again before deleting/rebuilding the AppDir.
- Resolution skips stale, unreadable and wrong-architecture cache entries,
  accepts real x86-64 ELF files reached through library symlinks, and reports
  all unavailable explicit inputs and unresolved linkage with installation
  instructions instead of indexing a missing SONAME and emitting a bare KeyError.
- Fedora container build logs use 0644 instead of mktemp's root-only 0600;
  exported RPM permissions are explicitly 0644. The upload checker selects only
  the exact current-version RPM and build logs, checks regular files,
  readability, modes/owners, symlinks and workspace/scratch free space, and
  creates/reads back an actual temporary ZIP. Diagnostic uploads use readable
  snapshots of inspected logs; rejected symlinks/unreadable originals are never
  uploaded by the always-run diagnostic step. Compressed RPM uploads use ZIP
  compression level 0. No source/build tree is uploaded.
- Release artifacts are limited to package extensions, the signed updater
  manifest/signature, and versioned checksums. Signing's temporary private PEM
  remains outside all uploaded paths, with private permissions and EXIT cleanup.
  Build/install/runtime checks, all-three-package validation, embedded-key
  verification, mandatory signing, final verification, and publishing gates
  remain required. No continue-on-error or dependency-validation bypass added.

## Original Fedora ZIP failure: cause still unverified

The complete original hosted upload-step logs were not supplied and could not
be retrieved: local GitHub API requests fail DNS resolution, GitHub CLI is not
available here, and web retrieval of the repository's Actions page/API failed.
Full step output was requested during this task. The quoted ZIP error alone
does not distinguish permissions, symlinks, disk pressure, or an upload-service
problem. No claim is made that the original failed step uploaded the whole tree;
the inspected workflow already selected `dist/*.rpm` and `dist/*.log`.

The root-only container log issue is established independently by the code:
Docker runs the native build as root and mktemp defaults to 0600. Correcting
that defect and adding narrow input diagnostics are hardening changes, not a
verified explanation of the historical error. Review the **full** original
upload-step output before choosing any further cause-specific fix.

## Actions runtime compatibility

Verified the upstream action definitions while making these changes:

| Action | Selected version | Runtime/source |
|---|---|---|
| checkout | v5 | [Node 24 definition](https://github.com/actions/checkout/blob/v5/action.yml) |
| cache | v5 | [Node 24 definition](https://github.com/actions/cache/blob/v5/action.yml) |
| upload-artifact | v6 | [Node 24 definition](https://github.com/actions/upload-artifact/blob/v6/action.yml) |
| download-artifact | v7 | [Node 24 definition](https://github.com/actions/download-artifact/blob/v7/action.yml) |
| setup-node | v6 | [Node 24 definition](https://github.com/actions/setup-node/blob/v6/action.yml) |
| configure-pages | v6 | [Node 24 definition](https://github.com/actions/configure-pages/blob/v6/action.yml) |
| deploy-pages | v5 | [Node 24 definition](https://github.com/actions/deploy-pages/blob/v5/action.yml) |
| upload-pages-artifact | v5 | [Composite uses upload-artifact v7](https://github.com/actions/upload-pages-artifact/blob/v5/action.yml) |

The Pages composite's v4 still embeds the Node 20 upload-artifact v4, so upgrading
only direct actions would leave a warning. Ubuntu 24.04 hosted runners are retained;
self-hosted runners would need versions compatible with these Node 24 actions.
Checkout refs, credential behavior, cache paths/keys, artifact names/retention,
Pages build/deployment permissions and release signing/publication conditions
are preserved. Selecting Node 24 for website builds alone would not update the
runtime used to execute old action versions. No deprecation-warning suppression
environment flags were added.

## Completed checks

- `cargo test --locked --offline`: **89 passed, 0 failed, 3 ignored**.
- `cargo fmt -- --check`, clippy with `-D warnings`, and locked/offline release
  build: passed. Logs: `target/release-failure-checks/`.
- `python3 scripts/test-packaging.py`: **12 passed**. Covers aggregate missing
  libraries/actionable errors, stale/wrong-architecture entries, missing
  transitive dependencies, retaining prior staging on failure, ZIP/CRC roundtrip,
  exact version allowlist, missing/empty RPMs, unreadable inputs, symlinks
  (including diagnostic exclusion), low disk space and ZIP I/O failure.
- `python3 scripts/test-release.py`: **10 passed** using an in-memory GitHub API.
- `python3 scripts/test-update-metadata.py`: **5 passed**, with isolated temporary
  fixture signing keys. These do not prove production secret configuration.
- Four workflow YAML files parsed and every inline shell block passed `bash -n`;
  both modified shell scripts passed `bash -n`. Canonical prerequisites were
  compared against every explicit Cargo DEB runtime dependency. Required job
  dependencies, `--require-all --signing-key`, final dry-run verification,
  narrow upload paths and absence of failure bypasses were checked.
  `actionlint` is not installed; hosted runner execution remains untested.
- `npm run build` and `npm test`: static production site and existing static/DOM
  logic checks passed. The existing Browserslist database-age notice is unrelated
  to the Actions Node runtime warnings; no dependency refresh was performed.
- `rebuild_check`: **build passed; application starts and draws at 440×540**.
  Screenshot: `../checks/round-0/screenshot.png`. This does not verify physical
  desktop clipboard/paste behavior or Fedora installation.

## Fresh local packages

`LDAI_RUNTIME_FILE="$PWD/target/packaging-tools/runtime-x86_64" bash
scripts/package-linux.sh all` completed successfully, including runtime preflight,
release compilation, DEB creation, AppImage staging, verified non-host library
closure, source/license metadata and AppImage creation. The resolved and bundled
`libxcb-shape.so.0` exists in the generated AppDir. Logs are
`target/release-failure-checks/packages.log` and `package-tests.log`.

`bash scripts/test-linux-packages.sh` passed on both fresh artifacts: package
identity/version, x86-64 ELF/linkage, archive contents, desktop/AppStream metadata,
version/help/diagnostics, SQLite create/insert/read and GIF/WebP thumbnail decoding.
It used extracted packages; apt installation, FUSE and other desktop systems were
not tested. Generated versioned checksums match these exact bytes:

| File | Bytes | SHA-256 |
|---|---:|---|
| `dist/clipvault_2.2.1-1_amd64.deb` | 3,824,584 | `22a3542fc53f4d855ae77114dd7736cf4d186d9842e62f79b291a5bb54558209` |
| `dist/ClipVault-2.2.1-x86_64.AppImage` | 10,398,200 | `ecf4661c090600b2e19d1304b3b298999f224bcec1c7c5b9e9b1d1aaafded1ef` |

These ignored local build outputs are unsigned, are not published release assets,
and do not modify the existing tag. The maintainer recovery guide was updated
after package staging; these packages demonstrate the packaging/runtime fixes,
not a final release documentation snapshot.

## Fedora and publication limits

A fresh `bash scripts/package-rpm.sh container` attempt failed before Fedora
compilation: Podman reports `mkdir /run/user/1000/libpod: read-only file system`.
There is **no current v2.2.1 RPM output and no RPM build/install pass**. Existing
historical RPMs remain untouched and are not accepted as current outputs.
The actual upload preflight correctly rejected the missing current RPM, retained
the current readable 0644 failure log, and reported free disk space. Actual GitHub
artifact upload is untested; fixture ZIP success is not hosted upload success.

`packaging/update-keys.json` is byte-for-byte unchanged (one existing public key),
SHA-256 `53ed7c03b73ec0f37062eb4a4423fa8a00cb96b7b492237920b0c244b215c275`.
Production private signing material was not accessed, provisioned or packaged.
Complete all-package production signing/verification remains untested because a
fresh RPM and production secret are unavailable here. Local package metadata is
unsigned and must not be published as a verified update.

The existing annotated tag object remains
`0815a466bf39205265e6c95cecd0bd0e5432b7ae`; its target commit remains
`ad43aced50663060e2c3332c34063549c8d9967a`. No commit, tag creation, tag movement,
push, workflow dispatch, release upload, publication or deployment was performed.
These local fixes do not retroactively modify the workflow/scripts at that tag;
re-running its old workflow will still use old code. Release recovery needs a
reviewed, explicitly authorized strategy that preserves the tag's immutability.
