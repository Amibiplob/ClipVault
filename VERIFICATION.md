> Historical v1.0.0 verification record. For the current 2.0.0 implementation, checks, package paths and limitations, read [V2.0.0-VERIFICATION.md](V2.0.0-VERIFICATION.md).

# Current feature update: 1.1.0

See [FEATURE-VERIFICATION.md](FEATURE-VERIFICATION.md) for current feature tests and package results. The 1.0.0 evidence below is historical.

---

# ClipVault v1.0.0 completion report

## Latest: activation and header correction — 2026-11-07

See [ACTIVATION-VERIFICATION.md](ACTIVATION-VERIFICATION.md) for the current cause, changed files, regression results, rebuild evidence, and manual desktop checklist. The source intentionally hid the window with a default-ON copy preference; no application crash was established. New settings default OFF and explicit saved preferences remain intact. The sections below record the earlier packaging run; their source/test counts and archive hashes predate this correction.

## Resumed final verification — 2026-11-07

The interrupted run stopped during the last packaging pass. This resume rebuilt
both 1.0.0 packages from the final source, including the focused-control Enter
fix and the fix that checks the saved pin state before deleting a stale preview.
No further application source changes were needed.

- `cargo test --locked --offline`: **23 passed, 0 failed, 1 ignored** (the real
  X11 clipboard test). Clippy with warnings denied and formatting checks passed.
- The offline release build, notice verification for 158 active Rust
  dependencies, script syntax and package/lock version checks passed.
- `rebuild_check`: **build passed; app-screenshot passed at 820×620**.
  `../checks/round-0/screenshot.png` was visually inspected against the attached
  references. Native rendering does not prove external clipboard pasting.
- Both final archives passed `scripts/test-linux-packages.sh`, including the
  launcher, original SVG and eight PNG icon sizes, dynamic SQLite operations,
  GIF/WebP decoding and relocated extracted AppImage runtime checks.
- The Debian executable is byte-identical to the final release binary. Its
  icon-cache hooks, MIT license and third-party notices match the source. The
  AppImage executable is byte-identical to the final patched AppDir executable
  and shares the source release's GNU build ID:
  `bec9150403a9d9ea456fd20f6a578ff3ad32bb93`.
- `sha256sum --check SHA256SUMS` passed for all three retained archives, including
  the historical 0.1.0 Debian package. The current release files are:

| File in `app/dist/` | Bytes | SHA-256 |
| --- | ---: | --- |
| `clipvault_1.0.0-1_amd64.deb` | 2798908 | `e4fdf4162510c4626d8640f2f27c564531b821ffe6fbd1b4ad32113b1c00e44a` |
| `ClipVault-1.0.0-x86_64.AppImage` | 8940024 | `3056693a88e18e769c793e9de08149ff801701e6ccc4b9f774f6151e51333e9c` |

A fresh read-only icon audit still finds the installed **0.1.0-1** package and
one related launcher with the old uppercase `StartupWMClass=ClipVault`.
The rebuilt 1.0.0 archives use lowercase `clipvault` and include all theme icons;
installation and the resulting desktop icon have not been verified. The
original desktop crash cause, external paste, Wayland, login/reboot and FUSE
checks remain unverified as detailed below. No system installation or upload
was performed. The existing rebuild/reinstall/manual verification commands
below remain applicable.

## 1. Implemented changes

Native Rust/egui application updated in place; `Cargo.toml` and the root Cargo.lock package entry are 1.0.0. The existing SQLite location, tables and stored payloads are retained. Added additive field migration, schema version 1, full-synchronous WAL, payload flushing, transactional settings/cleanup, and startup failure on corrupt records instead of silent data removal.

Redesigned history and responsive full preview, all five settings pages, edit-as-new text, successful-reuse recency updates, independent age/count cleanup with destructive-change confirmation, pin protection and deletion confirmation, persistent theme/density/topmost/hide preferences, login startup and window geometry. The same original logo now supplies UI and native window icons plus packaged scalable/eight PNG theme sizes. Fixed the X11 launcher class case, autostart icon, cache hooks, and AppImage menu installer. Added real developer hyperlinks and release notes with intended tag `v1.0.0`.

The original selection path was replaced with explicit scoped payload reads, empty/missing-payload errors, and acknowledged clipboard restoration; it remains visible until success. A saved-text click regression test is included. **The reported original desktop crash was not reproducible in the available sandbox and its original runtime cause remains unconfirmed.** A passing headless regression is not proof that the installed crash is fixed.

## 2. Tests run and actual results

- `cargo test --locked --offline`: **23 passed, 0 failed, 1 ignored**. Includes saved-text click dispatch, focused-control Enter isolation, missing payload errors, payload/settings/pin persistence on Store reopen, exact MIME roundtrips, text aliases, duplicate-free reuse, original-preserving editing, both independent and combined cleanup rules, pin exemptions, shared-image retention/deletion, rollback and legacy migration, fail-closed corrupt history, all settings/history layouts at 440×420, 820×620 and 1200×850. The ignored test requires a real X11 clipboard and is not counted as passed.
- `cargo clippy --locked --offline -- -D warnings`: passed.
- `cargo fmt -- --check`: passed.
- `rebuild_check`: build passed; native launch and screenshot passed at **820×620**. Actual capture: `../checks/round-0/screenshot.png`, copied unchanged to `docs/history.png` and visually inspected. This establishes native rendering, not external paste or installed desktop integration.
- Existing installed 0.1.0-1 inspected read-only: one related launcher in the usual checked application/autostart directories, `Icon=clipvault`, `StartupWMClass=ClipVault`, scalable SVG installed. No duplicate launcher was deleted. The source used no window icon. The upgrade aligns the actual lowercase X11 class and adds an explicit native icon and raster sizes.
- Shell syntax, package-notice generation and Cargo/lock/About/release/tag version consistency passed.
- `bash scripts/test-linux-packages.sh`: passed for both generated archives. Checks include version/architecture, executable linkage/help/diagnostics, matching desktop Icon/WM_CLASS, byte-identical source SVG and all eight PNGs, extracted/relocated AppDir launch, dynamic SQLite create/insert/read, and GIF/WebP thumbnail loading. This is archive/extracted-runtime evidence, not apt installation or FUSE mounting.
- AppImage deployment verified the non-host shared-library closure and collected distribution copyright/source metadata. The offline runtime was copied from the trusted cached linuxdeploy image using appimagetool's supported runtime-file input; no new runtime download or release upload was needed.

## 3. Remaining issues and untested behavior

Direct access to `DISPLAY=:0` is blocked from shell commands (`xclip`: cannot open display). Attempting the native X11 integration test through Craft Anything's command tool was refused because that tool requires approval and this session's approval policy is never. The ignored test was not successfully run. No Wayland compositor was available for verification.

Still manual: original installed click-crash reproduction/root cause; text/image Ctrl+V in external GUI applications; clipboard retention while hidden; real process restarts with clipboard owners; installed 1.0.0 menu/taskbar icon and cache behavior; shortcut conflict/grab checks; login startup after reboot; physical reboot/shutdown persistence; monitor-change recovery; Wayland/compositor-specific behavior; AppImage FUSE mounting on target desktops. Headless layout tests do not establish interactive mouse/keyboard usability on every desktop. Storage is unencrypted, application-name exclusions are approximate on X11 and unavailable on Wayland, and file references are not backups.

The installed system package remains **0.1.0-1** until the local reinstall command below is executed. No system files, actual user history, unrelated launchers, remote release, or Git tag were changed by this upgrade task. Packaging success does not mark v1.0.0 fully desktop-verified.

## 4. Packages actually generated

Both package formats were generated in this workspace (these are real files):

- `/home/biplob/Documents/Craft Anything/build-clipvault-a-modern/app/dist/clipvault_1.0.0-1_amd64.deb`
- `/home/biplob/Documents/Craft Anything/build-clipvault-a-modern/app/dist/ClipVault-1.0.0-x86_64.AppImage`

`dist/SHA256SUMS` records the actual final archive hashes; verify with `sha256sum -c SHA256SUMS` from dist. The AppImage uses the cached unmodified type-2 runtime from linuxdeploy, upstream commit `fedfad5`, with source/runtime hashes and original licensing information in its `usr/share/doc/clipvault/APPIMAGE-RUNTIME.json`. The separate runtime download was blocked; the supported cached-runtime input succeeded.

Old `dist/clipvault_0.1.0-1_amd64.deb` is retained as a historical artifact and is not a v1.0.0 output. No package was installed system-wide by this task.

## 5. Exact rebuild, reinstall and verification steps

Run in a local terminal on the Linux desktop:

```sh
cd '/home/biplob/Documents/Craft Anything/build-clipvault-a-modern/app'
cargo test --locked --offline
cargo clippy --locked --offline -- -D warnings
cargo fmt -- --check
python3 scripts/packaging-support.py notices
python3 scripts/extract-cached-runtime.py
LDAI_RUNTIME_FILE="$PWD/target/packaging-tools/runtime-x86_64" bash scripts/package-linux.sh all
bash scripts/test-linux-packages.sh
(cd dist && sha256sum -c SHA256SUMS)
```

For missing tools or first-time dependencies (network required), use the prerequisite commands in `packaging/README.md`, `cargo fetch --locked`, and `bash scripts/install-packaging-tools.sh`. The runtime helper uses the trusted linuxdeploy tool already downloaded; the normal packaging command otherwise asks appimagetool to download its runtime.

Fully quit the old instance before installing. Saved data stays in its existing user-data directory; do not remove it. A backup made while the old app is stopped should include the whole directory, not just history.sqlite3.

```sh
sudo apt install --reinstall ./dist/clipvault_1.0.0-1_amd64.deb
dpkg-query -W -f='${Version}\n' clipvault
python3 scripts/audit-icons.py
desktop-file-validate /usr/share/applications/io.clipvault.desktop.desktop
grep -E '^(Exec|Icon|StartupWMClass)=' /usr/share/applications/io.clipvault.desktop.desktop
find /usr/share/icons/hicolor -path '*/apps/clipvault.*' -print
sudo gtk-update-icon-cache -f -t /usr/share/icons/hicolor
sudo update-desktop-database /usr/share/applications
gtk-launch io.clipvault.desktop
xprop WM_CLASS _NET_WM_ICON
```

Click the ClipVault window after xprop. Expect lowercase `clipvault` in WM_CLASS and logo data in `_NET_WM_ICON`; visually verify the menu and taskbar. On Wayland, inspect the compositor's window identity tooling instead of xprop. If a custom launcher shadows the installed entry, inspect its path and Exec/Icon with the audit; disable only a confirmed stale ClipVault launcher. Log out/in if the shell retains an old icon.

```sh
# Optional AppImage menu installation; installs into your own home directory:
bash scripts/install-appimage.sh
# Package GUI smoke checks; needs a working desktop:
bash scripts/test-linux-packages.sh --gui
# Native X11 text/image ownership and selection-client byte roundtrips:
cargo test --locked --offline x11_text_and_image_activation_roundtrip -- --ignored --test-threads=1
```

The X11 integration test changes the clipboard; use a non-sensitive test session. It uses an isolated temporary data directory and does not read your history, but a separate clipboard manager may interact with test selections. Its success would demonstrate X11 byte transfer, not every GUI paste target.

Manual acceptance checklist:

1. In an editor, copy two multiline/Unicode texts. Open history, click the older text, then Ctrl+V into another editor. Confirm exact text, no crash, no duplicate, refreshed ordering, preserved pin, and visibility honoring Hide after copy. Repeat Enter activation. Preview/Pin/Edit/Delete must not copy.
2. Copy a PNG screenshot from an image app. Quit the source app, use the saved image and Ctrl+V into an image-capable app. Quit/reopen ClipVault and repeat. Verify exact saved image data remains and original files are not required.
3. Pin old text and image clips, change several settings and window geometry, then quit/reopen. Confirm pins/settings persist. Enable each cleanup rule separately, both, then neither. Confirm warnings before destructive changes and immunity of pins, including default clear. Check reuse refreshes age. Confirm pinned deletion requires confirmation.
4. Edit pinned and unpinned originals. Cancel, save unchanged, save changed, and Save and copy. Confirm originals remain byte-identical with unchanged position/pin, new clips start unpinned and appear first, and a copy failure retains the saved edit.
5. Toggle Start at login; close the window and copy another clip while hidden. Reopen with the shortcut and confirm capture. Explicitly Quit and confirm copies made while stopped are absent. Reboot, then shut down/power on; verify stored text, actual images, pins, preferences and enabled login startup. Check monitor removal/scale changes and 440×420/large-window scrolling/focus.
6. Repeat platform-specific checks on X11 and each intended Wayland compositor; document versions, shortcut setup, protocol availability and exact failures. No unsupported desktop should report successful capture.
