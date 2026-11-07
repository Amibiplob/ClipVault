# Clipboard activation and header correction — 2026-11-07

## Cause and investigation

The inspected application intentionally called `hide()` after `Notice::Activated`
when `settings.hide_after_copy` was true. `Settings::default()` set it true, and
the attached General screenshot also shows the option checked. `hide()` sends
`ViewportCommand::Visible(false)`; it does not stop the clipboard service,
destroy its selection window/source, or close the native event loop. A running
process with a disappeared window is therefore consistent with the implemented
auto-hide behavior. No application panic or desktop crash was reproduced, and
this change does not claim to fix a proven crash in the installed binary.

Text and images both use an ID plus a disk-loaded MIME snapshot. Text restoration
adds UTF-8 aliases; image restoration retains original image MIME bytes. X11
owns the selection on a separate service window, verifies ownership before
acknowledging success, and serves paste requests while the UI is hidden.
Wayland retains its service connection and data source while hidden; restoration
round-trips the compositor before acknowledgement. A cancelled source or finished
device now reports failure rather than successful activation. Neither rendering
an image thumbnail nor hiding the UI relinquishes clipboard ownership.

A separate confirmed UI defect was selection by list index: activating an older
clip updated recency and reordered history, leaving the old index pointing at a
different item. Rendering already cloned items and bounded access, so this was
not evidence of an out-of-bounds panic. Selection now follows the saved clip ID
across copy acknowledgements and concurrent history updates. Activating a stale
deleted item reports an error. Database/ownership errors keep the window open;
successful recency updates reuse the same record and preserve its pin.

## Files changed in this correction

- `src/storage.rs`: default auto-hide OFF; regression for new/missing-field
  settings and saved ON/OFF preferences with existing history, pins and shortcut.
- `src/ui.rs`: remove the redundant header menu; gear toggles Settings/history;
  preserve Close-as-hide and explicit Settings → About → Quit; brief Copied
  feedback; ID-based selection; one outstanding activation; correlate success
  and failure to that request so stale notices cannot hide after a failure;
  resolve the saved item before reading payloads; regression coverage.
- `src/clipboard.rs`: X11 success/failure notices include the clip ID and useful
  errors; retain the separate clipboard ownership/event loop.
- `src/wayland.rs`: matching ID-aware notices, reject empty data and compositor
  cancellation/revocation before reporting activation success.
- `README.md`, `VERIFICATION.md`, this report: explain intentional hiding,
  controls, current verification and desktop limits.
- `Cargo.toml`, `scripts/packaging-support.py`: include this report in the
  Debian and AppImage documentation.

No database schema, history directory, existing user settings, pins, shortcut,
or real history was reset or migrated by this correction. Tests use temporary
directories. Explicit saved true/false values remain true/false; absent fields
and new configurations use false.

## Automated verification

The UI tests use egui pointer/key events, real temporary SQLite history and
saved payload files, and **simulated clipboard-service acknowledgements**.
They do not establish system clipboard or external application behavior.

- `cargo test --locked --offline`: **33 passed, 0 failed, 1 ignored**.
  The four separate text/image × hiding ON/OFF tests verify exact dispatched
  bytes, visibility only after acknowledgement, Copied feedback, recency,
  no duplication, preserved pin and saved preference after reopening the store.
  Additional cases cover ownership/database failure notices, missing text/image
  payloads, deleted items, stale acknowledgements, rapid duplicate requests,
  gear clicks in both directions, header Close, row Pin/menu event isolation,
  preview/edit/delete handlers, and shortcut/second-launch notice handling.
- `cargo clippy --locked --offline -- -D warnings`: passed.
- `cargo fmt -- --check`: passed.
- `cargo build --release --locked --offline`: passed.
- `rebuild_check`: **build passed; app-screenshot passed at 820×620**.
  `../checks/round-0/screenshot.png` was visually inspected against the attached
  reference: status, Pause, one gear, Close; no redundant header menu. The
  capture also reports Super+V is already grabbed by another desktop client;
  quit the older running app before testing the replacement binary. This
  native render is not a successful global-shortcut or external-paste test.

The ignored X11 integration test was explicitly attempted in this session. It
**failed at desktop setup**: this sandbox cannot connect to `DISPLAY=:0`
(`xclip -selection clipboard -o -t TARGETS` also reports “Can't open display”).
The service thread returned before receiving a command; the test harness then
panicked on its `SendError` unwrap. This is a test failure caused by unavailable
desktop access, not an observed panic in the application's click path. The test
is still ignored in the ordinary suite and is not counted as a desktop pass.

## Required manual desktop verification

Run the rebuilt binary on an accessible desktop, using a separate non-sensitive
test history with `--data-dir` if desired. Use that same directory for subsequent
launches and for the Wayland desktop shortcut. Fully quit an older installed
binary first; no system-wide installation was performed here.

1. Set hide OFF; select older saved text, then an older saved image. Confirm the
   window stays visible with Copied feedback, each moves to the top once, and
   pins survive. Ctrl+V into a text editor and an image-capable app respectively.
2. Set hide ON; repeat both clips. Confirm hiding follows successful activation,
   then actually Ctrl+V into the destination while ClipVault is hidden. Confirm
   the service remains running and still captures a newly copied clip.
3. Reopen repeatedly with the configured global shortcut (X11 registered grab;
   Wayland compositor binding to the rebuilt executable with `--toggle`). Launch
   that executable again with the same data directory; confirm it shows the same
   instance and does not create another monitor. Automated checks cover notice
   handling, not real global key delivery, IPC sockets or desktop focus.
4. Exercise Preview, Edit, Pin, Delete and row dropdown actions with hide ON.
   Confirm none replaces the clipboard or hides. Verify gear/history toggling,
   absence of the extra header menu, Close continuing monitoring and explicit
   Settings → About → Quit terminating the app.
5. Test failure using a deliberately missing payload in isolated test history;
   confirm no hide and a useful error. Verify the saved hide preference, history
   and pins survive a full quit/relaunch. Repeat paste/reopen checks on each
   intended X11/Wayland desktop; compositor acceptance, physical keyboard focus,
   installed launcher behavior and third-party clipboard-manager interference
   remain unverified in this sandbox.
