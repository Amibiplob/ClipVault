# ClipVault

A native Linux clipboard history utility built with Rust, egui and SQLite. Clipboard payloads stay on this computer; no account or network connection is needed. There are no demo history entries.

## Build from source

Install the stable Rust toolchain with Cargo and the Linux desktop libraries described below, then open a terminal in this `app/` directory:

```sh
cargo build --locked
cargo run --locked
```

For an optimized executable, run `cargo build --release --locked`, then launch `./target/release/clipvault`. The first build downloads dependencies; add `--offline` to build without network access once they are cached.

The application code is in `src/`, bundled assets are in `assets/`, and `Cargo.toml` defines dependencies. Keep `Cargo.lock` in version control for reproducible dependency versions. The `.gitignore` excludes generated builds, local environment files, logs and editor files. Keep private clipboard history outside the source directory, including when using `--data-dir`.

## Open and use

Build with `cargo build --release --offline` when dependencies are cached, or `cargo build --release` for the first build. Launch `target/release/clipvault` (the development build is `target/debug/clipvault`). The system must provide `libsqlite3.so.0`, OpenGL and the usual X11 or Wayland desktop libraries. GdkPixbuf provides GIF/WebP thumbnails when its matching image loaders are installed. Original image data is retained even when a thumbnail decoder is unavailable.

1. Leave ClipVault running, then copy text, an image, or files in another application.
2. Press **Super+V** to open history. Search or use the arrow keys.
3. Click a clip or press **Enter**. ClipVault takes ownership of the actual system clipboard and hides its window.
4. Return to your application and press **Ctrl+V**.

Hiding the window keeps clipboard ownership alive. Quitting ClipVault ends its ownership; another clipboard manager may preserve the selection, but this is not guaranteed. Your saved history remains on disk for the next launch. Files and folders are saved as clipboard references, not copied into the vault, so moving or deleting the originals can break pasting.

Use **Ctrl+F** to search, **Ctrl+Shift+P** to pin, **Delete** to delete a highlighted clip, and **Escape** to dismiss. The pin and more-options controls are on each row. Settings contains appearance, history retention and size limits, shortcut customization, optional login startup, privacy exclusions and Quit. Clear-history confirmation preserves pinned clips unless you select the checkbox.

## Desktop support

- **X11:** native selection monitoring, multi-format restore and global shortcuts. If another app owns Super+V, ClipVault reports the conflict; choose another shortcut in Settings. A failed shortcut change preserves the previous binding, and successful changes are saved.
- **Wayland:** native `ext-data-control-v1` or `wlr-data-control-v1` is required. Configure your compositor's keyboard shortcuts to run the absolute executable path followed by `--toggle`, and use the same shortcut label in ClipVault. The compositor handles the binding. Use the same data directory for the background instance and toggle command.
- **GNOME Wayland without data-control:** background capture is unavailable. Use a compatible extension exposing the required protocol or an X11 session. ClipVault shows the access error.

For example, assign `/absolute/path/to/clipvault --toggle` to Super+V in your desktop shortcut settings. `clipvault --background` starts hidden; Settings can create the login startup entry after you choose a stable executable location. Run `clipvault --diagnostics` to see the detected desktop session. `--data-dir DIRECTORY` selects a separate history and application instance.

## Privacy and retention

History defaults to `$XDG_DATA_HOME/clipvault` or `~/.local/share/clipvault`. A private directory holds SQLite metadata and original MIME payload files. History is **not encrypted**. The default retention is 500 unpinned items for 30 days; pinned items remain until explicitly deleted. The default total payload limit is 32 MB per clip. Cleanup runs on launch, ingestion, settings changes and once a minute.

Pause monitoring before copying sensitive material. Content exclusions scan supported textual representations, including HTML and rich text. Password-manager MIME hints are ignored before reading payloads. Application-name exclusions are available on X11; Wayland data-control does not provide source application identity. Images cannot be filtered by words embedded in their pixels. Clearing history removes saved entries and payloads; it does not clear the active system clipboard or securely erase filesystem backups.

## Validation

`cargo test --offline` checks multi-format byte preservation, duplicate suppression, persistence, privacy exclusions, legacy text, Unicode filenames, retention, pinning, image thumbnails and size limits. `cargo fmt -- --check` verifies formatting. Craft Anything's `rebuild_check` builds and captures the running native window.

The current environment passed the build, nine unit tests and the native screenshot check. External application copy/select/paste round trips and cross-desktop Wayland behavior remain unverified: the host runtime tool requires approval that this session cannot grant. A rendered screenshot does not establish those workflows. The native implementation uses egui rather than the originally preferred Tauri stack.

## License

Licensed under the [MIT License](LICENSE). Third-party components retain their respective copyright and license notices.
