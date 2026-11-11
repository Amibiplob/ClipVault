# Upgrading safely to 2.0.0

Keep the existing data directory. The application still uses `history.sqlite3`, `items/`, settings JSON and private Linux account permissions. No normal package install/uninstall script deletes user history.

- Existing text, HTML, rich formats, images and file references remain byte-for-byte payload files. Pins, custom shortcuts, login and deletion preferences are preserved.
- Settings gain additive fields for per-mode geometry, divider, font size, theme and reboot cleanup. The active mode inherits old geometry; the other starts at a practical size. The old Light/Dark choice is used until the visitor chooses a theme. The old hide-after-copy behavior remains available for the explicit Copy action; activation now hides and attempts paste.
- `time` is no longer overwritten on reuse. New records also have `captured_at`; `last_used` is independent. Older versions overwrote `time`, so migrated records without a reliable original capture timestamp say that it is unavailable. No invented original date is stored. Recency/age cleanup uses last-used, otherwise saved `time`.
- Opening a database creates a local search table in batches of 128 records. In-memory summaries are capped at 512 characters; full text stays in payload files, and the disk search index preserves the formerly stored search content. Existing schema version remains 1 for additive compatibility.
- Reboot cleanup is off by default. A separate runtime table records `/proc/sys/kernel/random/boot_id`. Unknown/first-known/empty boot identities never cause deletion. A changed known ID clears only unpinned history, transactionally with the new marker. A process restart or package update in the same boot does not trigger cleanup.
- Clipboard and settings writes remain synchronous transactions. A failed read never causes a fresh empty database to replace unreadable history. Back up the entire stopped data directory before upgrade; older binaries do not understand all new settings, so downgrading is not a supported way to preserve new preferences.

Fresh/legacy/reopen fixtures, exact original MIME bytes, pin immunity, recency and reboot transitions are covered by automated tests. Physical reboot, actual installed upgrades and package-manager replacement need manual desktop acceptance. Test from a copy of data, never the only backup.

When upgrading from 1.2.0 or earlier, quit the old running instance once and open 2.0.0 after installing. Older binaries cannot retroactively acquire the new installed-update watcher. Future replacement detection is implemented in 2.0.0; real package-manager restart acceptance remains unverified.
