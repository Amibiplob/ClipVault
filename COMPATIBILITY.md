# ClipVault 2.0.0 — compatibility

Tested = that exact behavior was exercised. Limited = partial evidence or a known limitation. Untested = implementation/design only. Package extraction is not installation testing.

| Distribution / version | Desktop | Session | Clipboard capture | Global shortcut | Automatic paste | Package evidence | Overall |
|---|---|---|---|---|---|---|---|
| Linux Mint 22.3, x86-64 | Cinnamon | X11 | Untested against live apps; byte-transfer regression requires desktop access | Limited: Cinnamon GSettings integration and X11 fallback implemented; actual registration unverified | Untested: XTEST focus-checked attempt; no receiver acknowledgement | .deb and AppImage built; final extracted checks recorded in verification | Limited |
| Ubuntu 24.04 | GNOME | X11 | Untested | Untested: GSettings custom action | Untested: XTEST attempt | Untested installation | Untested |
| Ubuntu 24.04+ | GNOME | Wayland | Limited: needs ext/wlr data-control; stock sessions may not expose it | Untested: GNOME custom action; independent of clipboard support | Limited: manual Ctrl+V fallback | Untested | Untested |
| Mint 22.x | MATE | X11 | Untested | Untested: MATE custom keybinding | Untested: XTEST attempt | Untested | Untested |
| Mint 22.x | Xfce | X11 | Untested | Untested: xfconf custom command | Untested: XTEST attempt | Untested | Untested |
| Fedora 41+ | Plasma / GNOME | X11 / Wayland | Untested / requires data-control on Wayland | Plasma manual Keyboard setup; GNOME managed where available | X11 attempt / Wayland manual fallback | RPM recipe prepared; rpmbuild unavailable here | Untested |
| Arch / openSUSE and other Linux | Varies | X11 / Wayland | Capability-dependent; Untested | Desktop integration where supported, otherwise manual | X11 attempt / Wayland fallback | Untested | Untested |

The local build baseline is glibc 2.39, x86-64. Mint 21 / Ubuntu 22.04 and older glibc systems cannot run this binary merely by choosing AppImage. Build from source on an appropriate target baseline for older systems; that path is unverified. Graphics libraries and a working desktop are required. No Windows/macOS support.

Desktop-managed means ClipVault wrote and read back only its own action. It does **not** independently establish a working compositor registration. Settings reports this distinction. Cinnamon uses its `custom-list` and a `clipvault` custom action that runs the stable installed executable with `--toggle --data-dir …`; the internal grab is released. Conflicts are checked before replacement. GNOME uses `custom-keybindings`, MATE its custom keybinding paths, Xfce its custom command channel. KDE Plasma has an explicit command and Keyboard settings link; no guessed KGlobalAccel registration.

Physical keyboard repeat, first-press behavior, focus restoration and text/image insertion must be tested on each advertised desktop. Use a disposable desktop session. Never treat headless unit tests as that evidence.
