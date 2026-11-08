# ClipVault 1.1.0 feature verification

This is a prepared local feature update, not a published release. Existing history tables, payload paths, pins, exclusions and settings are retained; update preferences are additive JSON fields with automatic checking disabled by default. No installation, push, upload, release overwrite, or private-history reset was performed.

## Shortcut investigation and changes

Observed in the original source: X11 `Event::KeyPress(_)` unconditionally sent `Notice::Open`, including held-key auto-repeat, with no key identity or press latch. An ordinary hold could enqueue alternating show/hide operations before the user saw the window. The UI also treated minimized windows as visible and sent hide instead of unminimize/focus. IPC and service notices depended on periodic UI polling instead of waking the event loop. Wayland incorrectly acknowledged a requested label as a registered shortcut. These are source-confirmed defects; this environment cannot reproduce the person's physical-key sequence on their compositor.

The service now enables XKB detectable repeat and filters the registered key through a press/release latch. Without that X11 capability, it reports desktop setup required. Native keyboard capture suspends the global binding during recording and retains clipboard monitoring and ownership. Input is handled in the shared X11 event pump, including while clipboard format transfers are in progress, so concurrent clipboard activity cannot swallow recording key releases. Native X11 modifier state supplies Super (egui-winit 0.31.1 deliberately omits Linux Super from its exposed modifier fields). Registration/persistence either both succeed or restore the old binding. Restoration failure is reported as inactive. The UI has no manual text input or Apply button. On Wayland, configuration is delegated accurately to desktop shortcuts; no successful global registration is invented.

Show/hide now share one state transition, restore minimized windows explicitly, and focus on show. Service/IPC messages wake egui immediately. The existing canonical-data-directory instance lock remains; a brief separate startup lock serializes socket creation before a second launch connects. IPC accepts only complete bounded `show` or `toggle` messages. Desktop Wayland commands must be configured without compositor repeats, as shown in the UI/README.

## Update protocol

GitHub documentation was read live during implementation on 2026-11-08:

- [Latest stable release endpoint and response fields](https://docs.github.com/en/rest/releases/releases#get-the-latest-release).
- [User-Agent and request headers](https://docs.github.com/en/rest/using-the-rest-api/getting-started-with-the-rest-api).
- [Rate-limit handling and serial requests](https://docs.github.com/en/rest/using-the-rest-api/best-practices-for-using-the-rest-api).
- [Current API calendar version 2026-03-10](https://github.blog/changelog/2026-03-12-rest-api-version-2026-03-10-is-now-available/).

Implementation uses a public unauthenticated GET to `https://api.github.com/repos/Amibiplob/ClipVault/releases/latest`, `Accept: application/vnd.github+json`, a versioned ClipVault User-Agent, and `X-GitHub-Api-Version: 2026-03-10`. Public release reads require no token. Redirects are bounded and limited to HTTPS api.github.com. Requests run on a worker, with connect/overall timeouts, bounded body reading and concurrent-check prevention. Primary/secondary limits respect Retry-After and exhausted X-RateLimit-Reset; manual retries have a short request cooldown and automatic attempts are persisted before dispatch with a daily schedule. The endpoint's documented 404 is displayed as no stable release; offline, malformed, oversized and unsuccessful responses are unavailable checks. SemVer comparison ignores build metadata and never offers a downgrade. Only official HTTPS repository release-tag pages can be opened.

Update code accepts no clipboard data, store, content exclusions or file paths. UI integration stores only scheduling, result, link and dismissal metadata. No automatic downloads/installations exist. Disabling automatic checking prevents new automatic requests; an in-flight request may finish. Automatic failures do not open dialogs or disrupt clipboard operations.

## Verification results

Final automated suite: **47 passed, 0 failed, 2 explicitly ignored**. `cargo clippy --offline --locked --all-targets -- -D warnings`, `cargo fmt -- --check`, and Python syntax checks passed. `rebuild_check` passed both build and native 820×620 app screenshot checks after the final code changes. Release and package build logs are in `target/feature-release.log` / `target/feature-packaging.log`; tests and lint are in `target/feature-tests.log` / `target/feature-clippy.log`. Eight layout artifacts were generated and inspected, with a fixed About footer keeping Quit available even when the page scrolls.

Prepared artifacts (all paths relative to `app/`):

- Release binary: `target/release/clipvault`.
- Debian: `dist/clipvault_1.1.0-1_amd64.deb`.
- AppImage: `dist/ClipVault-1.1.0-x86_64.AppImage`.
- Package checks: `target/feature-package-check.log`; hashes: `dist/SHA256SUMS`.
- Software layout artifacts: `target/layout/about-{dark,light}-{440,820}.png` and `target/layout/shortcuts-{dark,light}-{440,820}.png`.
- Native rebuild capture: `../checks/round-0/screenshot.png`.

Package validation results are also recorded in `dist/FEATURE-BUILD-RESULTS.txt` after generation; historical 1.0.0 artifacts remain untouched. Archive checks establish local package contents/runtime behavior, not a system installation or compositor acceptance.

Git status/diff could not be obtained: `app/.git` contains an empty object and reports `fatal: bad object HEAD`. No Git repair, commit, push, or history rewrite was attempted.

Repeatable tests include Super recording and full release, all modifiers, Escape/focus cancellation, invalid unmodified/non-alphanumeric combinations, a modifier alone, held repeats, conflict and disk-save rollback, restoration failure, 30 window toggle/restore cycles and IPC actions, recording suppression, confirmed active status, shortcut/settings restoration, daily opt-in/backoff/dismissal persistence, semantic comparisons including 1.9.0/1.10.0, equal/older/newer/build metadata, malformed tags and payloads, draft/prerelease filtering, official URL validation, bounded/erroring response readers, rate limits, and duplicate-check prevention. Existing text/image payload, pin, retention, history reorder, clipboard acknowledgment, and migration regressions remain in the suite.

The socket-free update tests supply mocked HTTP status/headers/body readers to the same production response handler. Network errors are represented by failing readers. A separate complete loopback HTTP mock test remains available but is ignored by default because `TcpListener::bind` returns Operation not permitted in this sandbox. It was attempted in the initial full run and failed at setup, not at an update-comparison assertion. The existing real-X11 clipboard integration test also remains explicitly ignored. No test is counted as a pass merely because it was started.

Eight actual egui mesh/font-atlas software rasterizations are generated under `target/layout/` for About and Shortcuts at 440 and 820 px, in light/dark themes. These are local layout artifacts, not compositor captures. The layout test asserts horizontal fit inside the render pass. Existing minimum/normal/large layout coverage also runs at 440×420, 820×620 and 1200×850. Native rebuild_check separately builds and captures the running app.

## Remaining real-desktop checks

Direct desktop/command tools are unavailable under this session's approval policy, and sandbox X11/loopback access is restricted. Real physical Super+V recording, desktop conflict delivery, key-repeat behavior, first-press focus, WM minimize/Close/Escape, second-process activation and external Ctrl+V text/image pasting require a real accessible X11 desktop. Wayland GNOME/KDE/Sway/Hyprland registration instructions and clipboard protocol availability require compositor-specific verification. No physical reboot or installed .deb/AppImage upgrade was performed. App HTTPS against the live GitHub repository was not run; live documentation verification is separate from an actual app update request. Offline/timeout/rate-limit evidence is mocked.

Manual acceptance: quit old ClipVault builds first; use an isolated non-sensitive data directory; record a free Ctrl+Alt combination then Super+V after removing any desktop conflict; test Escape, modifier-only and conflicting combinations; restart and confirm settings; press/release once to open, once to hide, hold to verify no repeats; minimize and restore; repeat after Close/Escape/hide-after-copy; launch `clipvault --toggle` from another process; verify copied text and image paste while the UI is hidden; test optional update checks and rate-limit feedback; inspect both themes at the minimum size. Preserve production history and do not attach it to bug reports.
