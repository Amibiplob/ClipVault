# Resume verification — 2026-11-12

Recovered the saved checklist and `core_unknowns_show`. The v2.0.0 implementation was already complete; no source defect was found in this verification pass.

Current checks:

- `cargo build --release --locked --offline`: passed (`target/resume-build.log`).
- `cargo test --locked --offline`: 72 passed, 0 failed, 3 ignored (`target/resume-tests.log`). Ignored cases require live X11, loopback sockets, or an explicitly invoked performance workload; no new performance measurement was made.
- `cargo clippy --locked --offline --all-targets -- -D warnings`, formatting, Git whitespace checks, website fixtures and JavaScript syntax checks: passed (`target/resume-clippy.log` for lint).
- `bash scripts/test-linux-packages.sh`: passed extracted Debian/AppImage metadata, assets, linkage, diagnostics, SQLite and image-loader checks (`target/resume-packages.log`). No packages were installed.
- Debian payload matches the rebuilt release executable byte for byte. AppImage staging modifies ELF paths, so its executable is not expected to have the same raw hash; the extracted runtime checks passed.
- Craft Anything `rebuild_check`: build and native 440×540 screenshot passed. Visually inspected `../checks/round-12/screenshot.png`; it shows an empty history in isolated Preview/screenshot mode with monitoring disabled. It does not prove external clipboard interaction.

Existing package archives remain unchanged:

| Candidate | SHA256 |
| --- | --- |
| `dist/clipvault_2.0.0-1_amd64.deb` | `8d1d72b2b5b94ab2df468fea15cce9afeeb4404307cca4d3a82feece55669ca1` |
| `dist/ClipVault-2.0.0-x86_64.AppImage` | `03ec49c891aee3a3fd8f7ad757974392ab09eafe81d913688ee76acd1237b078` |

Live desktop shortcuts/copy/paste, Wayland sessions, installed upgrades, reboot behavior, native performance and browser interactions remain unverified as described in V2.0.0-VERIFICATION.md. `rpmbuild` remains unavailable. Publishing requires maintainer credentials and approval; nothing was installed, uploaded or published. This resume report is outside the existing archives.
