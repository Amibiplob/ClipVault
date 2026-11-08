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
