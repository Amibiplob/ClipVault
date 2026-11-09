# Compact / Comfortable display modes

Compact uses full-width 52 px rows with one-line text/link content, small saved-image thumbnails, and filenames. It hides timestamps, type/format labels, the separate preview pane, and the narrow-window preview toolbar. Search and all category filters remain available. The item menu provides Copy, Pin/Unpin, Edit for text/links, and Delete, including confirmation for pinned deletion. Comfortable retains the existing spacing, details, responsive preview, and editing layout.

The existing serialized `compact` preference saves each selection without migrating or rewriting clip payloads. The exact requested help text appears directly below the two choices.

Validation on 2026-11-10:

- `cargo test --offline --locked`: 50 passed, 0 failed, 2 existing environment-dependent tests ignored. Log: `target/appearance-tests.log`.
- New egui interaction tests click both appearance choices and reopen storage, compare all saved item metadata and payloads, and exercise Compact's Pin, Edit, and protected Delete menu actions without copying or hiding.
- Layout assertions cover both modes and themes at 440 and 820 px, including content, thumbnails, search, filters, metadata visibility, and horizontal fit. Link/file filtering and exact restore payloads are also checked. Software-rendered artifacts: `target/layout/appearance-{compact,comfortable}-{dark,light}-{440,820}.png`.
- `cargo clippy --offline --locked --all-targets -- -D warnings` and `cargo fmt -- --check` passed.
- `cargo build --release --offline --locked` passed; binary: `target/release/clipvault`.
- Craft Anything `rebuild_check`: build passed; running app screenshot passed at 820×620 (`../checks/round-0/screenshot.png`), visually inspected.

The software layout renders and simulated clipboard service acknowledgements are separate from the native launch capture. No physical reboot, external application paste, Wayland compositor test, package installation, or package rebuild was performed for this display-mode change.
