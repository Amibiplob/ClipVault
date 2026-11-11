# ClipVault v2.0.0

Your clipboard, with a memory. A native Rust clipboard manager by **Md Biplob Hosain**.

This source upgrade adds automatic paste attempts with clear fallback, safer desktop shortcuts, separate capture/last-used timestamps, independent layouts, reboot-aware optional cleanup, grouped settings, bounded preview loading and an accessible website. Existing history, pins, supported MIME payloads and preferences stay in their existing data directory. Original capture times lost by older versions cannot be reconstructed.

Package-manager updates and optional GitHub release checks are separate. Installed-executable replacement detection defers restart for unsaved edits. See [CHANGELOG.md](CHANGELOG.md) for changes and [V2.0.0-VERIFICATION.md](V2.0.0-VERIFICATION.md) for exact outputs, checks and blockers. The version is 2.0.0 in source; no GitHub release, PPA or signed APT repository has been published by this task.

Before upgrading, stop ClipVault and back up the **whole** local storage folder (normally `~/.local/share/clipvault`), including SQLite WAL files and `items/`. Do not delete this directory during package installation or removal. See [MIGRATION.md](MIGRATION.md).

Desktop support varies: [compatibility table](COMPATIBILITY.md). Automatic paste on X11 is an input request, not proof the target accepted content; Wayland uses manual Ctrl+V where safe automation is unavailable. No “all Linux distributions” or fixed RAM promise.

MIT License. [Project](https://github.com/Amibiplob/ClipVault) · [Issues](https://github.com/Amibiplob/ClipVault/issues) · [Portfolio](https://amibiplob.vercel.app) · [Developer GitHub](https://github.com/Amibiplob) · [LinkedIn](https://linkedin.com/in/amibiplob)
