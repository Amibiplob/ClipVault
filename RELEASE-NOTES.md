# ClipVault v2.1.0

Settings now provide ← Back, responsive navigation/content and a scrollbar close to the right edge. Pinned clips use a larger filled accent icon with contrast in both themes. Clear unpinned states its global scope, keeps pins, disables when no unpinned clips remain, and follows Confirm before deleting.

Existing history, pins, preferences, MIME payloads, shortcut behavior and storage formats are preserved. Shared clear handling drops retained preview resources; repeated settings JSON serialization was removed. No dependencies were removed without evidence of being unused.

The existing Debian and AppImage workflows are rebuilt for 2.1.0. Fedora RPM and offline Debian source/PPA workflows are prepared, with environment blockers recorded in [verification](V2.1.0-VERIFICATION.md). See [packaging](packaging/README.md) and [Launchpad setup](packaging/LAUNCHPAD-PPA.md). No release, PPA or website has been published by this task.

Stop ClipVault and back up the whole storage directory before upgrading. Install the newer package using the same source and data path; normal removal never deletes personal history. Existing cleanup preferences still apply. [Migration](MIGRATION.md) and [compatibility](COMPATIBILITY.md) document the boundaries; external clipboard/paste/shortcut behavior still requires real desktop acceptance.
