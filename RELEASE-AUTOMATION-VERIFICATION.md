# Release automation and website verification — 2026-11-15

Implemented in `app/`, preserving the existing Rust app and release artifacts. Application version remains **2.2.0**. The npm version is private website tooling, not the app version. No commit, push, real release modification, production signature, Pages deployment, or privileged package installation was performed.

## Delivered

- Tag-triggered and manually dispatched release pipeline, with exact tag/commit/version preflight; Ubuntu 24.04 DEB/AppImage jobs and separate Fedora 44 source RPM/container tests; locked Cargo/npm dependencies, pinned Rust/cargo-deb, hash-pinned AppImage tools and keyed caches.
- All required builds and final signature verification gate publishing. A draft is created only after the complete bundle passes; uploads are read back before publication. Failed uploads retain an unpublished draft. Same-tag concurrency is serialized. Identical published retries are read-only; changed published bytes, wrong commits and unexpected assets are rejected. Workflow artifact overwrites are permitted for rebuilding an attempt, independently of immutable published release files; diagnostic names retain the run attempt.
- Existing Ed25519/schema 1 updater protocol preserved. Per-job metadata only inspects that job’s formats; final release metadata requires all three packages. Actual byte sizes, SHA-256, exact URLs, architecture and existing distribution eligibility are generated. Tag-derived checksum filenames replace the hardcoded v2.2.0 filename. DEB/RPM prerelease payload versions use `~` while public filenames retain SemVer. Canonical changelog sections feed both release notes and the manifest.
- Production Tailwind single-page redesign with the existing logo, actual current native app screenshot, labelled search illustration, four-step usage, implemented features, compatibility/privacy details, complete FAQ, three theme choices, and verified-asset-only download links/sizes. Native HTML controls retain the static stack; no React or component runtime was introduced. Public release data is validated and cached for up to 24 hours, with fresh lookup and an explicit GitHub fallback.
- Existing Pages workflow now builds/tests `target/site`, uses relative asset URLs for `/ClipVault/`, and refreshes after successful package workflows using `workflow_run`. Only deployment receives Pages write/OIDC permission. The optional PPA source candidate job is preserved in its own manual workflow; APT preparation remains separate and is not advertised as live.
- Welcoming README and focused [maintainer release](packaging/RELEASING.md), [updater](packaging/UPDATER.md), [website](docs/WEBSITE.md), and [screenshot provenance](docs/SCREENSHOT.md) documents. New guides are included in source/package assets. One existing UI regression assertion now uses the current package version so a version bump does not fail it merely because the expected title was hardcoded.

## Passed locally

| Check | Actual result |
|---|---|
| Native release build | `cargo build --release --locked --offline` passed after the final source edit |
| Rust suite | **89 passed, 0 failed, 3 existing ignored**; later version-portable changelog assertion retested separately: **1 passed**, not an additional unique test |
| Rust formatting/lints | `cargo fmt -- --check` and `cargo clippy --locked --offline --all-targets -- -D warnings` passed |
| Metadata/signing fixtures | **5 passed**: byte hashes, stale-signature removal, package-format isolation, prerelease DEB ordering/URLs, mocked Fedora identity/platform rejection, external fixture-key trust/signature/tamper checks |
| Publishing fixtures | **10 passed**, all using an in-memory fake API: exact commit/version matching, required files, complete upload/read-back, failed-upload draft safety, identical/different published retries, draft repair, prerelease flagging, unexpected assets/commit rejection |
| Website production | Locked offline `npm ci --ignore-scripts`, `npm run build`, and `npm test` passed. CSS is compiled/minified, about 20 KiB. npm dependencies were restored from the existing local cache; no live npm connectivity is claimed |
| Website logic | Static local asset/anchor checks and DOM/release fixtures pass: exact filenames/version/architecture/URLs, missing packages disabled, stable/draft/prerelease handling, actual-size rendering, theme preference persistence/invalid-value fallback, cache expiry and offline disclosure |
| Workflow checks | All four YAML files parse; all embedded Bash blocks parse. Static assertions cover tag/dispatch triggers, publish default false, same-tag concurrency, required-job gates and permissions. **actionlint and hosted Actions execution were not available** |
| Documentation | README/focused-guide local Markdown links resolve; feature/compatibility descriptions were checked against the existing implementation |
| Packaging | Fresh DEB/AppImage built, extracted and checked: architecture/version/desktop/AppStream/linkage/help/diagnostics plus dynamically loaded SQLite and GIF/WebP loaders. Current DEB executable matches the final source build byte-for-byte; packaged README/new guides match source bytes |
| Checksums | Current tag checksum file and unsigned manifest match the actual final DEB/AppImage; generic historical DEB/AppImage checksums refreshed too |
| Craft Anything | `rebuild_check` passed native **build** and **app-screenshot at 440×540**, including a final check after packaging/website changes. This does not test the website |

The unique Rust + Python suite pass count is **104**, plus the website assertion suites. The three ignored Rust tests remain ignored; no extra desktop or installation proof is inferred from these counts.

## Final local candidates

| File | Bytes | SHA-256 |
|---|---:|---|
| `dist/clipvault_2.2.0-1_amd64.deb` | 3,822,864 | `04bb168a6730f85a8a613ddb8763b82911a1c74b6c8e8a8fa02018aa2c5b189a` |
| `dist/ClipVault-2.2.0-x86_64.AppImage` | 10,402,296 | `e673785e3cd15c5ed0b0883d9796feddbcf6ee64ca7f57c6d7d8c82f3163754d` |

The final native executable SHA-256 is `59fb441a7d658fffa4375f4c5dff0da07b0d990e422fabccb00275777bed3b3d`. Previous DEB/AppImage candidates and metadata were copied to `target/release-automation/previous-packages/` before rebuilding. Earlier versions remain in `dist/`.

A preexisting `dist/clipvault-2.2.0-1.x86_64.rpm` (4,949,294 bytes, SHA-256 `498f457ac6379761b46386eb21c3412a3d0ac0b6a2364ae187f341cd98d8d9c8`) was preserved unchanged. It was **not** rebuilt or independently validated in this task, and is excluded from the new local partial manifest. The release workflow accepts only fresh jobs’ gathered packages, not this local older candidate.

## Blocked or still unverified

- **Browser run did not complete.** The initial local server attempt was refused (`listen EPERM`); the final in-process asset-routing test avoids opening a server, but Chrome itself exits with `setsockopt: Operation not permitted`. Craft Anything’s `build_command_run` also returned “requires approval, but approval policy is never.” Screenshots at 1440/768/390/320, actual mobile overflow/layout, browser theme flash/persistence, keyboard focus, and FAQ interactions remain **unverified**, despite passing DOM/CSS checks. The prepared browser suite is mandatory in Pages CI and writes a failure result rather than reporting a pass.
- **Fresh Fedora build blocked before compilation**: Podman cannot create `/run/user/1000/libpod` on the read-only runtime. No fresh RPM, Fedora installation, or Fedora screenshot success is claimed. Docker-based hosted CI still needs an actual run.
- **Production signing remains unconfigured**: the real `packaging/update-keys.json` is `[]`; no private key was supplied or generated for the maintainer. Only isolated temporary fixture keys were used. Local metadata is explicitly unsigned and no `.sig` claims verification. The signing job will fail safely until setup is completed.
- **No hosted publishing/deployment proof**: live GitHub API asset downloads/uploads, API permissions, exact Fedora dependency resolution, pristine runner packaging, workflow reruns, release status/latest selection, and Pages routing need a GitHub-hosted rehearsal/run. Local API calls were fakes; repository browsing/network availability was restricted. Existing AppImage tool hashes are pinned from the existing cached executables; moved upstream downloads fail closed and require review.
- Extracted packages and native rendering are not proof of apt/dnf installation on the user’s machine, FUSE, physical shortcuts, external-app paste, Wayland, privileged upgrades or AppImage replacement/restart. Existing app verification limits are preserved.

## Useful local evidence

- `target/release-tests.log`, `target/release-build.log`
- `target/release-automation/version-portable-test.log`, `clippy-final.log`, `build-final.log`
- `target/release-automation/packages-final.log`, `final-doc-refresh.log`, `package-checks-final.log`
- `target/release-automation/npm-ci.log`
- `target/release-automation/browser-attempt.log`, `target/website-checks/result.json` (**passed: false**)
- `target/release-automation/rpm-attempt.log`; detailed preserved RPM attempt logs under `dist/`
- `docs/history.png` is an unchanged copy of this task’s actual earlier native capture; its hash/provenance is in `docs/SCREENSHOT.md`. `../checks/round-0/screenshot.png` is the latest subsequent native check and can differ because of cursor/tooltip state.

## Next maintainer actions

Follow [RELEASING.md](packaging/RELEASING.md) for the exact next-version commands. Provision/review a durable Ed25519 public key in source; put its private PEM in **CLIPVAULT_UPDATE_SIGNING_KEY**; permit the narrowly scoped publishing job; set Pages source to **GitHub Actions**. Confirm the default branch’s Pages environment is allowed. Run manual **Release packages** with an existing prepared tag and **publish=false** for a nonpublishing hosted rehearsal after setup. Check the separate website/browser build. For a real next release, update application/package versions and changelog, commit/push, then create/push the matching example tag `v2.2.1`; Actions uploads all six finished files automatically after success. Verify the published release/checksums and target-system behavior. Do not retag an already published version or manually replace its packages.
