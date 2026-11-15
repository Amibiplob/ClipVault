# Automated releases

The repository root is `app/` in the Craft Anything workspace. `.github/workflows/packages.yml` is the only package publisher. Pushing `v*` tags starts it; manual dispatch requires an existing tag and defaults to **publish=false**. No releases were created or changed while developing this workflow. Application source version is 2.2.2; this preparation has not created or pushed a release tag.

## One-time setup

1. Enable Actions in GitHub **Settings → Actions → General** and allow checkout/cache/artifact, rust-toolchain, and Node setup actions. Organization policies must permit the publishing job’s explicit `contents: write`; every other release job has only `contents: read`. No personal access token is required. Protect version tags from deletion/retargeting and restrict who may push them.
2. Provision a durable **Ed25519** maintainer key outside the checkout using [UPDATER.md](UPDATER.md#trust-setup-external-maintainer-action-required). Commit only the corresponding raw 64-character public hex string in the JSON array `packaging/update-keys.json`. Review its fingerprint before distributing a build that trusts it. This checkout contains an embedded public key, preserved by the release-failure fixes; that alone does not verify that the corresponding private Actions secret is configured. Existing apps with an empty trust store need a manual trusted upgrade first.
3. In **Settings → Secrets and variables → Actions**, create repository secret **`CLIPVAULT_UPDATE_SIGNING_KEY`**. Its value is the complete unencrypted Ed25519 private **PEM**, including BEGIN/END lines and real line breaks. It is not base64, a public key, an OpenPGP key, or the APT key. From a secured machine with GitHub CLI authenticated, avoid printing the key:

   ```sh
   gh secret set CLIPVAULT_UPDATE_SIGNING_KEY --repo Amibiplob/ClipVault < "$HOME/.local/share/clipvault-signing/release.pem"
   ```

   The workflow writes it with private permissions to `$RUNNER_TEMP`, removes it on exit, verifies its public half is embedded, signs the final gathered manifest, and verifies the signature with the app’s public key. Never paste it into source, logs, issues, or an environment file. No key was provisioned by this task.
4. In **Settings → Pages → Build and deployment → Source**, select **GitHub Actions**. Keep only `pages.yml` as the Pages publisher. The expected project URL is `https://amibiplob.github.io/ClipVault/` after successful deployment; it has not been verified live here. Configure the `github-pages` environment to allow the default branch.
5. Hosted Ubuntu 24.04 runners need network access to crates.io, npm, GitHub/tool assets, Ubuntu repositories, and Fedora 44 repositories/image registry. The RPM job uses Docker; it builds and tests inside disposable Fedora 44, with installation tests confined to that container. Fedora support remains exactly ID `fedora` + VERSION_ID `44`. No PPA account is needed for direct releases. The existing optional unsigned PPA source candidate job is preserved as a separate manual `ppa-source.yml` workflow; it does not publish a PPA or block direct releases.

`Cargo.lock` and `package-lock.json` lock dependencies. Ubuntu Rust is 1.89.0, cargo-deb is 3.8.0. `packaging/tools.lock.json` pins both AppImage helper SHA-256 values. Their upstream continuous URLs can move: a reviewed cached copy is reused, but a changed download fails closed. To refresh, verify the upstream build/source and licensing, review its digest, update the lock, and rehearse packaging before tagging. Do not accept a changed digest automatically. Fedora/system packages are distribution-managed, so builds are not promised to be bit-for-bit reproducible; published-byte comparison protects retries.

## Prepare a version — current source 2.2.2

The source is prepared for 2.2.2 dated 2026-11-15. The checklist below documents the required edits for this and future versions; it does not authorize tagging or publication. See [local verification](../V2.2.2-VERIFICATION.md) for checks and limits.

1. Change `[package].version` in `Cargo.toml` to `2.2.2`.
2. Run `cargo check --offline` (or `cargo check` after fetching missing crates), which updates the root `clipvault` entry in `Cargo.lock`. Review the lock diff; unrelated dependency upgrades are unnecessary.
3. Add a first `<release version="2.2.2" date="YYYY-MM-DD">…</release>` entry to `packaging/linux/io.clipvault.desktop.metainfo.xml`. Use the real release date.
4. Add a nonempty `## 2.2.2 — YYYY-MM-DD` section to `CHANGELOG.md`. This section supplies both release notes and the signed manifest’s `release_notes`. Historical `CHANGELOG-v*.txt` and verification reports stay intact.
5. The app’s What’s New embeds `CHANGELOG-v2.2.2.txt` in `src/ui.rs`. Keep that include and the current-version document lists in `Cargo.toml` / `scripts/packaging-support.py` synchronized with each release, including its verification report. Keep `debian/changelog` and the package document checks current; `debian/rules` reads the expected version from Cargo.toml. Historical documents remain valid; do not overwrite them. Review user-facing source-version references in README/compatibility records when releasing.
6. Check everything locally:

   ```sh
   python3 scripts/release-support.py --tag v2.2.2
   cargo fmt -- --check
   cargo test --locked --offline
   cargo clippy --locked --offline --all-targets -- -D warnings
   python3 scripts/test-update-metadata.py
   python3 scripts/test-release.py
   npm ci
   npm run build
   npm test
   npm run test:browser
   ```

Only after maintainer setup, successful checks, review, and explicit authorization to publish, commit all intended release changes, then push and tag. These commands have not been run for 2.2.2:

```sh
git status --short
git add Cargo.toml Cargo.lock CHANGELOG.md CHANGELOG-v2.2.2.txt RELEASE-NOTES.md V2.2.2-VERIFICATION.md src/ui.rs packaging/linux/io.clipvault.desktop.metainfo.xml README.md COMPATIBILITY.md scripts/packaging-support.py scripts/test-linux-packages.sh debian/changelog debian/rules packaging/RELEASING.md packaging/README.md packaging/UPDATER.md packaging/VALIDATION.md .github/workflows/packages.yml .github/workflows/pages.yml .github/workflows/apt-artifact.yml .github/workflows/ppa-source.yml scripts/package-linux.sh scripts/package-rpm.sh scripts/check-release-upload.py scripts/test-packaging.py packaging/linux/build-packages.txt RELEASE-FAILURE-VERIFICATION.md
git commit -m "Prepare ClipVault 2.2.2"
git push origin main
git tag -a v2.2.2 -m "ClipVault 2.2.2"
git push origin v2.2.2
```

Do not move an existing release tag. If any required change is missed, prepare a new patch version. For prereleases use matching SemVer such as `2.2.2-beta.1` / `v2.2.2-beta.1`. DEB and RPM encode `-beta.1` as `~beta.1` internally to preserve package-manager ordering; source version and public filename retain SemVer. Prereleases are marked on GitHub, excluded from the stable website endpoint and ignored by the existing stable updater.

## What Actions does

| Stage | Behavior |
|---|---|
| Preflight | Resolves `refs/tags/TAG^{commit}`, checks out that exact commit, and compares tag, Cargo manifest, root lock entry, latest AppStream release, and changelog |
| Noble | Rust test/fmt/clippy, metadata/release tests, pinned tooling, release build, DEB + AppImage, identity/archive/runtime and isolated screenshot checks on Ubuntu 24.04 / glibc 2.39 |
| Fedora | Complete locked vendoring, source build and tests in Fedora 44, RPM identity/dependency checks, container install, native screenshot, and uninstall preserving isolated history |
| Sign | Requires all three exact package filenames, inspects actual files, hashes them, emits schema 1 JSON and raw detached Ed25519 signature, then independently verifies the bundle without API access |
| Publish | Revalidates bundle and exact commit; creates a draft, uploads and reads back every file, and publishes only after all bytes match |
| Pages | The existing website workflow builds static Tailwind output and deploys it from the default branch after a successful package workflow; browsers fetch/cache actual latest stable release data |

Expected release files for 2.2.2:

```text
clipvault_2.2.2-1_amd64.deb
clipvault-2.2.2-1.x86_64.rpm
ClipVault-2.2.2-x86_64.AppImage
clipvault-update.json
clipvault-update.json.sig
v2.2.2-SHA256SUMS
```

The manifest carries schema/repository/version/release notes plus each file’s name, URL, package type, architecture, distribution support, size, and SHA-256. Do not reformat signed JSON. RPM metadata permits Fedora 44 only; DEB metadata lists the existing Ubuntu 24.04/Mint 22.x compatibility rules. These rules describe updater eligibility, not proof of every distribution’s desktop acceptance. AppImage retains glibc requirements. See [UPDATER.md](UPDATER.md) for exact trust and selection behavior.

## Failure handling and retries

For the v2.2.1 packaging failures, install the shared
`packaging/linux/build-packages.txt` prerequisites (including `libxcb-shape0`)
locally and in Noble Actions. Run `python3 scripts/packaging-support.py preflight`
to check all required bundled runtimes/loaders and their linkage before building.
The Fedora upload preflight records exact files, modes/owners, readability,
symlinks, free disk space, and a temporary ZIP/CRC check. It uploads only the
current RPM and inspected diagnostic snapshots. Container build logs and exported
RPMs are readable by the host runner. Read the **complete original failed upload
step**, including the lines before the ZIP error, before identifying its cause;
a generic ZIP failure does not establish a permissions or disk-space problem.
See [the local fix verification](../RELEASE-FAILURE-VERIFICATION.md) for limits.

**The existing v2.2.1 tag is immutable.** Source fixes made after that tag are
local changes, not a repaired hosted run. Re-running a historical run uses its
historical workflow, and build jobs deliberately check out the exact tagged
commit; dispatching a newer workflow does not change those tagged scripts.
Do not move/overwrite v2.2.1, create a replacement tag, or publish as part of
this fix. A reviewed release recovery strategy and explicit authorization are
required before any subsequent release operation.

All required builds and signing finish before release creation. A failed build leaves no new draft at all; a draft from a prior attempt stays unpublished. Upload/read-back failure leaves the current draft unpublished. Actions logs show the failed command; `diagnostics-noble`, `diagnostics-fedora`, and signing diagnostics preserve useful logs. The final summary reports each job, including skipped ones. Missing signing secrets or an untrusted public key fail with a specific message and never produce verified release metadata.

Concurrency is keyed by tag with cancellation disabled, serializing publishing attempts for the same version. Retry failed jobs or manually dispatch the same tag. A matching draft skips identical files and replaces only differing draft assets, then verifies uploaded bytes before publication. Unexpected extra assets or a mismatched commit stop the retry. **An already published release is read-only**: every remote asset must match the rebuilt local bytes, or the job fails and requires a new version. Rebuilding can change bytes (toolchain, system package, archive timestamp); this is not permission to replace published files. For a transient publish failure, rerun the failed publishing job using the same run’s signed bundle while GitHub retains its artifacts.

Manual dispatch with `publish=false` runs real builds and signature checks but never creates/edits a release. It still needs the configured signing secret. Before secret setup, run the isolated tests below; they generate temporary fixture keys outside a temporary fixture repository and use a fake in-memory GitHub API. They are not production signing or publication evidence:

```sh
python3 scripts/test-update-metadata.py
python3 scripts/test-release.py
```

A maintainer with a complete gathered bundle can run the local nonpublishing validation:

```sh
python3 scripts/publish-release.py --tag v2.2.2 --commit "$(git rev-parse 'refs/tags/v2.2.2^{commit}')" --dist dist --dry-run
```

## After Actions finishes

Open **Actions → Release packages** and inspect the final result and diagnostics. Open the actual GitHub Release: check stable/prerelease status, notes and all six downloadable files. Download the bundle into a fresh directory and run `sha256sum --check v2.2.2-SHA256SUMS`. Validate the signature as documented in UPDATER.md. Test installation/launch and update behavior in disposable target systems; hosted package checks do not prove every real desktop’s shortcuts or paste behavior. Confirm the Website workflow and Pages URL, then refresh the page and check its stable version, sizes, and package links. Workflow artifacts alone are not the distribution channel.
