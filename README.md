# ClipVault 1.1.0

<img src="assets/clipvault.svg" width="80" alt="ClipVault logo">

A native Linux clipboard manager written in Rust with egui and SQLite. Keep useful text, images, links, rich text, and file references on this computer, then recall them from history and paste with your usual **Ctrl+V**. No account, telemetry, or cloud sync. The original logo is `assets/clipvault.svg`; bundled PNGs are rasterizations of that file.

![ClipVault 1.0.0 native history and preview](docs/history.png)

Actual 820×620 native capture from Craft Anything rebuild_check; the capture workspace contains existing clipboard history, not bundled demo records.

## Install, build, package

The build host used here is Linux Mint 22.3, x86_64 (Ubuntu 24.04/noble baseline). This does not establish compatibility on every desktop. See [verification and remaining checks](VERIFICATION.md) and [packaging prerequisites](packaging/README.md).

From this `app/` directory:

```sh
cargo fetch --locked                   # first build, requires network
cargo build --release --locked --offline
./target/release/clipvault
cargo test --locked --offline
cargo clippy --locked --offline -- -D warnings
cargo fmt -- --check
python3 scripts/generate-icons.py       # requires system librsvg + Cairo
python3 scripts/packaging-support.py notices
bash scripts/package-linux.sh all
# If appimagetool cannot download its runtime, reuse the trusted cached tool runtime:
python3 scripts/extract-cached-runtime.py
LDAI_RUNTIME_FILE="$PWD/target/packaging-tools/runtime-x86_64" bash scripts/package-linux.sh all
bash scripts/test-linux-packages.sh
```

If tools are missing, run `bash scripts/install-packaging-tools.sh` with internet access; the packaging guide lists the apt prerequisites. Output names are `dist/clipvault_1.1.0-1_amd64.deb` and `dist/ClipVault-1.1.0-x86_64.AppImage` after successful packaging. The Debian revision is `-1`.

```sh
sudo apt install ./dist/clipvault_1.1.0-1_amd64.deb
# Or run the portable application:
chmod +x dist/ClipVault-1.1.0-x86_64.AppImage
./dist/ClipVault-1.1.0-x86_64.AppImage
# Optional explicit AppImage user-menu installation:
bash scripts/install-appimage.sh
```

AppImage menu installation copies the image to `~/.local/bin/clipvault.AppImage`, installs the same icons under the user's hicolor theme, and creates the matching desktop entry. Running an AppImage alone does not install a menu launcher. No release is automatically uploaded or published. Release notes: [ClipVault v1.1.0](RELEASE-NOTES.md); intended Git tag: `v1.1.0`.

## Capture and reuse

1. Leave ClipVault running with monitoring enabled and copy text, an image, or files in another app.
2. Open history with **Super+V** (configurable) or launch ClipVault again.
3. Search and filter by type; use arrows to select. Click an item or press **Enter** to restore its saved formats to the actual system clipboard.
4. Return to the destination application and press **Ctrl+V**.

A successful reuse refreshes the existing record's last-used time and moves it to the top without duplicating it or changing its pin. The window hides only after the clipboard service reports success, if enabled in General. It stays open with an error if the saved data cannot be read or clipboard activation fails. Use the separate **Preview** action to inspect full content without copying. Row menus expose Copy, Preview, Edit (text/links), and Delete; pins have a dedicated button. Child controls do not activate the clip.

Supported text/plain, HTML, rich-text and image MIME representations are retained within the size limit; PNG/JPEG thumbnails use the Rust decoder and GIF/WebP thumbnails use installed GdkPixbuf loaders. Image bytes are saved, not just image paths. An unavailable thumbnail does not discard the original image. Available representations depend on the source app. File/folder entries remain **references**, not backups of the original files. Moving or deleting a source file may break pasting. Unsupported or oversized clipboard data is reported. Privacy-excluded copies are skipped.

## Persistent storage and upgrades

The existing SQLite database remains at `$XDG_DATA_HOME/clipvault/history.sqlite3`, normally `~/.local/share/clipvault/history.sqlite3`. Original MIME bytes and thumbnails live under `items/` in the same private directory. Settings, pins, recency, and window geometry are stored in SQLite. Additive JSON fields and schema version 1 preserve legacy history; unreadable records cause a safe startup error instead of silently deleting payloads.

SQLite uses WAL with full synchronization. Payload files are flushed before metadata is committed, settings/cleanup writes are transactional, and changes are saved promptly. History survives process restarts and is intended to survive Linux restarts and power cycles; physical reboot/power-loss testing remains manual. Back up the **whole data directory** while ClipVault is stopped; copying only the database may omit payloads or WAL changes. Do not put private history in this repository.

Closing the window hides it and keeps monitoring and clipboard ownership running. **Quit ClipVault** in Settings → About stops the process. Enable **Start at login** in General to create an autostart entry for the current stable executable and data directory. Copies made while ClipVault is not running cannot be captured. After explicit Quit, Linux clipboard ownership ends; another clipboard manager may retain the current selection, but this is not guaranteed. Saved history remains available on the next launch.

`clipvault --background` starts hidden. `clipvault --toggle` toggles the existing window. `--data-dir DIRECTORY` isolates a separate history and instance; use that same directory in your desktop shortcut. `--diagnostics` prints desktop environment information.

## Record the history shortcut

On X11, open **Settings → Shortcuts** and click the large shortcut control. It displays **Press your shortcut**. Hold Super (the Windows key) and press V to see **Super + V**, or combine Ctrl, Alt, Shift, and Super with one letter or number. Release every key to register and save automatically; there is no Apply button. A modifier alone is never saved. **Escape** cancels and restores the previous binding. Invalid combinations, conflicts, and save errors retain the previous setting; the UI reports a binding as active only after registration succeeds. If the desktop takes the old binding while recording and restoration fails, the UI reports it unavailable rather than claiming it still works.

Native X11 capture includes Super even though egui’s Linux modifier events omit that key. During recording, local shortcuts, global toggles, and incoming toggle commands are suppressed. Settings persist after restart and the saved binding is registered again, with any new conflict reported accurately. X11 servers without detectable key repeat require a desktop binding instead of an unreliable grab.

One physical shortcut press shows and focuses hidden history; the next hides it. A minimized window restores and focuses on the next press. Held-key repeats and key releases do not toggle again. Close, Escape, and optional hide-after-copy keep monitoring and clipboard ownership alive; **Quit ClipVault** stops the app. A second launch and `clipvault --toggle` use the existing instance for the same canonical data directory. Simultaneous starts serialize IPC socket setup.

On Wayland, expand **Wayland & desktop shortcut setup**. Direct global registration and native Super capture are unavailable through this app’s data-control integration. Configure `clipvault --toggle` in GNOME/KDE’s shortcut settings or your compositor configuration, using the installed binary’s absolute path when needed. For Sway, use `bindsym --no-repeat $mod+v exec /absolute/path/to/clipvault --toggle`; for Hyprland, use `bind = SUPER, V, exec, /absolute/path/to/clipvault --toggle`, not a repeating binding. Use the same `--data-dir` if customized. Wayland clipboard protocol availability is a separate limitation from shortcut configuration.

## App updates and privacy

**Clipboard history stays on this device. Checking for updates contacts GitHub. Clipboard content is never sent.** No saved clips, excluded phrases, personal file paths, or analytics are transmitted. Your IP address and the app’s versioned User-Agent are visible to GitHub as part of the HTTPS request. Opening developer or release links also contacts those sites through your browser.

In **Settings → About → App updates**, click **Check for updates**. **Automatically check once a day** is optional and **OFF by default**. With it off, ClipVault starts no automatic update requests. The preference, last-attempt status, daily scheduling, rate-limit backoff, and dismissed release version are saved locally. An already started request may finish if you turn the option off; no further automatic requests start. Automatic failures stay in the update status and do not interrupt clipboard work or retry repeatedly.

ClipVault checks public stable releases from [Amibiplob/ClipVault](https://github.com/Amibiplob/ClipVault) using GitHub’s documented HTTPS API. Optional leading `v` is accepted; semantic versions are compared numerically (1.10.0 is newer than 1.9.0). Drafts and prereleases are ignored. Newer installed builds are never offered a downgrade. Malformed release versions, offline requests, timeouts, and rate limits are shown as unavailable checks, never as proof the app is current. The latest-release endpoint’s 404 response is shown as no stable release available. Requests run outside the UI thread, with a 5-second connection / 12-second overall timeout, a 512 KiB body limit, restricted redirects, and no concurrent checks.

For an update, **View release / Download** opens a validated official ClipVault GitHub release page. Choose the `.deb` or AppImage and **install it manually**; ClipVault never downloads, executes, or installs updates. **Remind me later** suppresses automatic reminders for that same version across restarts. A later version can notify again.

**Older builds without update checking must first be updated manually to a build containing this feature.** They cannot receive the first in-app notice retroactively. Keep the existing data directory when upgrading; history, image payloads, pins, and settings remain local and are preserved.

## Pins, cleanup, and editing

Pinned clips never expire automatically, never count toward the unpinned limit, and survive the default **Clear unpinned history** action. Deleting an individual pinned clip requires explicit confirmation. Unpinning makes it eligible for cleanup.

Two independent switches control automatic deletion:

- **Age-based cleanup:** delete unpinned clips older than the configured days.
- **Maximum-item cleanup:** retain at most the configured number of unpinned clips, removing least recently used clips first.

Enable either, both, or neither. Defaults are 30 days and 500 unpinned clips. Both rules use the **last captured or reused timestamp**, not original creation time. Reusing a clip refreshes that timestamp. Cleanup runs at startup, after capture/reuse and relevant setting changes, and every minute. Settings that would immediately remove history require confirmation before being applied. Unused payload directories are collected only after the database commit; data referenced by another clip is retained.

The multiline editor offers **Save as new clip**, **Save and copy**, and Cancel/Back. The original content, recency and pin remain unchanged. Edited copies begin unpinned. Saving unchanged text creates nothing. Identical edited payloads are rejected without changing an existing record. Save and copy saves first; if clipboard activation fails, the newly saved clip remains in history and an error appears. Explicit editing is available while monitoring is paused, while respecting content exclusions and size limits.

## Settings and keyboard use

All five pages scroll within the resizable window:

- **General:** start at login, pause, consecutive duplicate policy, hide after successful copy, independent cleanup rules, clip size, and confirmed unpinned clearing.
- **Appearance:** dark/light theme, compact/comfortable density, and always-on-top.
- **Shortcuts:** press-to-record global history shortcut, confirmed registration feedback, keycap reference, and expandable desktop setup help.
- **Privacy:** textual content exclusions, X11 application exclusions, sensitive MIME hints, location, and accurate unencrypted storage status.
- **About:** package-derived version, existing logo, developer links, optional update checks, and expandable MIT, third-party license, and desktop compatibility information.

Identical supported-format snapshots always share one record. With **Skip consecutive duplicates** enabled, repeating the current most recent snapshot leaves recency unchanged; disabling it refreshes recency. Capturing an older duplicate refreshes its existing record. ClipVault's own clipboard writes are suppressed.

| Key | Action |
| --- | --- |
| Super+V, configurable | Toggle history on X11; compositor binding on Wayland |
| Up / Down | Select a history item |
| Enter | Copy selected clip |
| Ctrl+F | Focus search |
| Ctrl+Shift+P | Pin/unpin selected clip |
| Delete | Delete selected clip; pinned deletion asks first |
| Escape | Cancel a dialog, return from detail/settings, or hide |
| Tab / Shift+Tab | Move keyboard focus among controls |
| Ctrl+V | Paste in the destination application |

Default client size is approximately 820×620, with a 440×420 minimum and monitor-size clamping. Wide windows show history beside full preview; narrow windows prioritize history with Back navigation for preview/editing. Native window borders allow resizing. Size and position are saved while visible; a saved offscreen position is recovered. Wayland ultimately controls placement.

## Linux compatibility and privacy limits

X11 uses native selections (including incremental transfers), format preservation, and registered global shortcuts. A conflicting shortcut reports an error and retains the prior binding. Text restoration adds standard UTF-8 aliases for clients that request them. The service remains alive while the UI is hidden.

Wayland requires **ext-data-control-v1** or **wlr-data-control-v1**. Configure your compositor's shortcut to execute `/absolute/path/to/clipvault --toggle`; recording in a desktop’s own shortcut settings is required; ClipVault does not claim to register a compositor shortcut. GNOME Wayland commonly does not expose data-control. Use an appropriate extension or an X11 session; unavailable access is reported instead of claiming capture. Data-control acknowledgement is not proof that every destination app will accept a paste.

Content exclusions scan textual representations, not words inside images. Supported password-manager/sensitive MIME hints are skipped before payload capture. X11 application exclusions use the active window's class, which can differ from the actual clipboard owner. Wayland does not expose the source application name. These exclusions cannot guarantee protection for every sensitive app; pause monitoring when needed.

History is **not encrypted**. Linux account file permissions provide local access protection. Clearing saved history is not secure erasure and does not clear active clipboard data or backups.

## Troubleshooting and verification

**Missing/gear icon:** the old executable supplied no native window icon; its launcher also used `StartupWMClass=ClipVault` while X11's executable-derived class is `clipvault`. The 1.0.0 build embeds the existing logo, aligns that class, retains Wayland app ID `io.clipvault.desktop` with desktop file `io.clipvault.desktop.desktop`, and packages scalable plus eight PNG theme sizes. Debian post-install/removal hooks update available caches. AppImage user installation updates user caches.

```sh
python3 scripts/audit-icons.py          # read-only; identifies stale/duplicate launchers
sudo apt install --reinstall ./dist/clipvault_1.1.0-1_amd64.deb
sudo gtk-update-icon-cache -f -t /usr/share/icons/hicolor
sudo update-desktop-database /usr/share/applications
desktop-file-validate /usr/share/applications/io.clipvault.desktop.desktop
gtk-launch io.clipvault.desktop
xprop WM_CLASS _NET_WM_ICON               # X11: click the ClipVault window
```

Expected X11 class includes `clipvault`, and `_NET_WM_ICON` should contain logo data. Fully quit the old app before launching the new binary. Check menu and taskbar visually; log out/in if the desktop caches the old icon. An old custom launcher with a different Exec or Icon may shadow the packaged launcher. Inspect the audit output and disable only entries you identify as stale; the audit never removes anything. Installed 0.1.0 was inspected here, but installing 1.0.0 and verifying menu caches remain manual.

**Clipboard activation errors:** inspect the in-app error and `clipvault --diagnostics`; check X11/Wayland capability, avoid a second manager aggressively taking ownership, and ensure saved payload files remain readable. Keep ClipVault running, retry Copy, then Ctrl+V in the target app. Do not delete your database to troubleshoot.

The inspected selection path intentionally hides the window after acknowledged clipboard activation when **Hide after successfully copying a clip** is enabled; this is not evidence of a crash. New configurations now default this setting to **OFF**; explicitly saved ON/OFF preferences are preserved. When OFF, selecting text or an image keeps the window open with brief **Copied** feedback. Copy failures keep it open with an error. Selection follows clip identity when the list reorders. The header gear toggles Settings/history and Close only hides; use Settings → About to quit. See [the activation investigation](ACTIVATION-VERIFICATION.md) for the separate text/image checks and desktop limitations. Native launch/capture checks and automated tests do not establish installed external-application paste behavior or reboot persistence. See [VERIFICATION.md](VERIFICATION.md) for historical evidence and [FEATURE-VERIFICATION.md](FEATURE-VERIFICATION.md) for this 1.1.0 update’s checks and remaining desktop verification. Only current screenshots that were actually captured are included under `docs/` when available; reference mockups are not application screenshots.

## Bug reports, developer, license

Report at [ClipVault issues](https://github.com/Amibiplob/ClipVault/issues). Include ClipVault version, distribution, desktop/compositor, X11/Wayland session, exact steps, terminal errors, and whether text/images or particular destination apps fail. Use a non-sensitive sample and redact clipboard content and private paths. Never attach your history database or payload directory by default.

Developer: **Md Biplob Hosain** · [GitHub](https://github.com/Amibiplob) · [Project](https://github.com/Amibiplob/ClipVault) · [Portfolio](https://amibiplob.vercel.app) · [LinkedIn](https://linkedin.com/in/amibiplob).

[MIT License](LICENSE). Existing third-party copyright and license notices are preserved in [THIRD-PARTY-NOTICES.txt](THIRD-PARTY-NOTICES.txt) and package documentation.
