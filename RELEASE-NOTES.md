# ClipVault v1.2.0

Native Linux clipboard manager update. Application version **1.2.0**, Debian revision **1.2.0-1**. No installation, tag, publication, or upload is performed by the build scripts.

A cleaner clipboard, with a window that feels at home.

• Choose your desktop’s title bar and borders, or ClipVault’s own movable, resizable window controls.
• Pick an installed app font, preview it, and return to System default. Missing fonts fall back automatically.
• Compact keeps a single clean list with small image thumbnails. Comfortable adds one full-content preview, with no repeated title or duplicate text.
• Scrollbars have a separate lane. Pins use a filled accent icon; unpinned clips use a muted outline.
• Copy text, Copy image, Pin/Unpin, Edit text, and Delete are available where appropriate.
• Start hidden at login while monitoring continues. The shortcut or app menu opens the window normally.
• Confirm before deleting is on by default and applies to menus, previews, and keyboard Delete. Clear history has its own confirmation; automatic cleanup protects pins.
• This offline changelog appears once per version when you open the window. Reopen it from About → View changelog.

Your existing clipboard history, original image data, pins, shortcuts, and saved preferences are retained.

Desktop limitations: native decorations, focus, placement and resizing are ultimately controlled by the compositor; Wayland still needs supported data-control and a desktop shortcut. Fontconfig supplies installed fonts; absent/invalid fonts fall back safely. See [current verification](V1.2.0-VERIFICATION.md) for actual test/build evidence and unverified physical desktop behaviors.

---
Historical release notes below describe their original versions only.

# ClipVault v1.1.0

Prepared feature update; no release, tag, push, upload, or system installation performed.

- Native press-to-record X11 shortcuts, including Super, with release-to-save, Escape cancellation, confirmed registration, and conflict-safe persistence.
- Physical-key repeat filtering, explicit minimized-window restoration, immediate event-loop wakeups, and serialized single-instance IPC startup.
- Redesigned Shortcuts and responsive About pages with developer emphasis, bundled licensed brand icons, and expandable compatibility and license information.
- Manual GitHub stable update checks; optional once-daily checks off by default, persistent scheduling/backoff/dismissals, semantic version comparisons, and validated manual download links.
- Accurate update-network privacy wording. Clipboard data never enters update requests. Existing history, images, pins, and preferences are preserved.

Application version 1.1.0; Debian revision 1.1.0-1. See [FEATURE-VERIFICATION.md](FEATURE-VERIFICATION.md) for exact evidence and unverified real-desktop boundaries. Older builds without update checks require this first upgrade to be installed manually.

---
Historical 1.0.0 notes follow; their verification claims apply to that earlier build only.

# ClipVault v1.0.0

Intended Git tag: `v1.0.0`. No tag, push, publication, or upload is performed by the build scripts.

- Redesigned native dark/lavender history, searchable type filters, wide full preview, narrow detail navigation, and five complete settings pages.
- Existing logo throughout the app and Linux packages, embedded window icon, corrected X11 launcher class, theme-size PNGs from the SVG, icon-cache hooks, read-only launcher audit, and explicit AppImage menu installer.
- Existing SQLite history retained, additive schema/JSON migration, full-synchronous WAL and payload flushing, transactional cleanup, and failure on unreadable records to protect history.
- Reused text and images update the existing record's recency, keep pins, and hide only after successful activation according to settings.
- Independent age/count cleanup, confirmation before immediately destructive settings, protected pins, and shared-payload-safe garbage collection.
- Multiline edit-as-new with Save and copy, original preservation, and persistent appearance/window preferences.
- Developer links, MIT notices, operational documentation, regression tests, and explicit platform/testing limits.

Debian package revision: `1.0.0-1`; application version: `1.0.0`. See [verification](VERIFICATION.md) for actual build/test outcomes and unverified desktop checks. In particular, the original reported desktop crash root cause is not established by the headless click regression. Version 1.0.0 is a release candidate until the remaining manual desktop/install/reboot checks pass.
