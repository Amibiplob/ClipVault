# Changelog

## 2.0.0 — 2026-11-11

- Select a row or press Enter to restore saved clipboard formats, hide the panel, and attempt automatic paste on X11. Clipboard failure leaves history open. Unsupported/failing paste keeps the copied item and gives a Ctrl+V fallback. XTEST submission is not an acknowledgement from the destination app.
- Desktop-owned shortcut setup for Cinnamon, GNOME, MATE, and Xfce; manual Plasma and other-desktop setup with the installed executable and custom data directory. Distinguish a configured desktop action from independently confirmed registration. Reject plain Ctrl+V and preserve the prior binding on failed replacement.
- Keep original capture and last-used times separately; stable clip identity and pins survive recency changes. Legacy original capture times that were overwritten by older versions are explicitly unknown.
- Remember independent Compact/Comfortable geometry and maximized states, with a persisted draggable preview divider. Add System theme, font size, clear-search control, readable descriptions, grouped settings and modal editing.
- Optional reboot cleanup uses Linux boot identity, preserves pins, and does not run on app-only restart. Editing always saves a separate clip; existing preferences and MIME payloads migrate additively.
- Read storage in batches, keep bounded summaries and a disk search index, virtualize rows, cache filtered metadata, decode previews off-thread and bound thumbnail/full-preview caches.
- Watch for a newer installed executable separately from optional GitHub checks. Defer restart until the panel is hidden and no edit/activation is pending; flush storage and retain single-instance startup. End-to-end package-replacement acceptance remains unverified.
- Accessible single-page website with actual app imagery, release-asset lookup and safe Releases fallback. Debian/AppImage scripts, RPM recipe, manual package-candidate CI, and signed APT artifact workflow. No repository is advertised as live.

See [verification](V2.0.0-VERIFICATION.md), [compatibility](COMPATIBILITY.md), and [performance](PERFORMANCE.md). A feature in source is not a claim that every Linux desktop or package installation was tested.

## 1.2.0

Native title bar preference, installed fonts, clean layouts, silent login startup, deletion preference and offline upgrade changelog. Historical evidence: V1.2.0-VERIFICATION.md.
