# Software Manager metadata verification

Checked on Linux Mint 22.3 (noble), amd64, with mintinstall 8.4.0,
mint-common 2.5.1, and appstreamcli 1.0.2, on 2026-11-10 local time.

## Package checks

`bash scripts/package-linux.sh deb` rebuilt
`app/dist/clipvault_1.1.0-1_amd64.deb` successfully, including a release build,
locked offline dependencies, license-notice verification, desktop validation,
and strict offline AppStream validation.

`dpkg-deb -f dist/clipvault_1.1.0-1_amd64.deb Description` now prints:

```text
Private native Linux clipboard history
 ClipVault keeps a local history of copied text and images. Open it with a
 keyboard shortcut to search, pin, and reuse previous clips.
```

This replaces the previous Markdown/HTML README in the Description field.
The extracted README still matches the source README byte for byte.
The package installs `usr/share/metainfo/io.clipvault.desktop.metainfo.xml`,
with component ID `io.clipvault.desktop`, name `ClipVault`, package `clipvault`,
desktop launcher `io.clipvault.desktop.desktop`, a short summary, and a default
screenshot. The packaged XML matches the source XML and passes
`appstreamcli validate --no-net --strict`.

`bash scripts/test-linux-packages.sh` passed the description, separate README,
AppStream XML, Debian contents/linkage/help/diagnostics, extracted existing
AppImage smoke checks, and SQLite/GdkPixbuf runtime checks. Only the Debian
package was rebuilt for this task; the existing AppImage is from the prior
feature build. `rebuild_check` passed the build and native 820×620 app capture;
that capture is of ClipVault, not Software Manager.

`dist/SHA256SUMS` records the rebuilt package checksum. Check it from `dist/`
with `sha256sum --check SHA256SUMS`; paths in that manifest are relative to
that directory. Historical feature-build reports describe their earlier
artifact hashes, not this rebuilt Debian package.

## Public screenshot: unverified

Configured URL:
`https://raw.githubusercontent.com/Amibiplob/ClipVault/main/docs/history.png`.
This is a raw HTTPS image URL, not a GitHub HTML file-view page. The local
`docs/history.png` is an 820×620 PNG with SHA256
`a514450d32dce698199689320b97c2b04a5edae199479bc47e68983f052d688b`.

The public response could not be verified: curl failed with
`Could not resolve host: raw.githubusercontent.com`; the web fetch tool
returned cache misses. The Craft Anything command tool refused the public
curl check because it requires approval and the active approval policy is
never. These failures do not establish whether the URL is live or returns a
PNG. No screenshot was uploaded or published.

On a network-enabled host, run from `app/` to verify the response body matches
the actual local PNG, rather than trusting an HTTP status or filename:

```sh
curl --location --fail --proto '=https' --proto-redir '=https' \
  --max-time 30 --dump-header /tmp/clipvault-history.headers \
  --output /tmp/clipvault-history.png \
  https://raw.githubusercontent.com/Amibiplob/ClipVault/main/docs/history.png
file /tmp/clipvault-history.png
cmp docs/history.png /tmp/clipvault-history.png
```

Stop if curl or cmp fails; verify the published image before declaring this
check passed. These commands have not succeeded in this environment.

## Installed Mint listing: blocked

The installed package database reports ClipVault 1.1.0-1, but that is not
evidence of this rebuilt artifact being installed. The requested reinstall
was attempted with
`sudo -n apt install --reinstall --no-download ./dist/clipvault_1.1.0-1_amd64.deb`.
It failed because the sandbox's no-new-privileges flag prevents sudo becoming
root. The rebuilt package was therefore not installed.

`xprop` could not open display `:0`; `mintinstall show clipvault` emitted a
GTK screen error and timed out. No Software Manager screenshot appearance
or absence was observed after installing the rebuilt package.

To finish that acceptance check on the actual desktop, reinstall the package
with the command above (without `-n` if a password is needed), close and
reopen Software Manager, search for ClipVault, and inspect its description
and screenshot area. Record the Software Manager version and visible result
separately from package validation.

## Separate Mint catalog limitation

Inspection of this host's installed Software Manager code shows that its
AppStream screenshot path applies to Flatpaks, not Debian packages:

- `/usr/lib/linuxmint/mintinstall/imaging.py:221-249` handles AppStream
  screenshots only for package hashes starting with `f` (Flatpak).
- The Debian branch at lines 250-277 downloads from the Mint community
  screenshot service and screenshots.debian.net; optional Hamonikr handling
  follows. It does not read the metainfo shipped in the `.deb`.
- `/usr/lib/python3/dist-packages/mintcommon/installer/pkgInfo.py:210-211`
  returns an empty screenshot list for APT packages.

Consequently, the metadata is valid and available to AppStream consumers,
but this Mint version's Debian listing cannot acquire its screenshot just
from a standalone `.deb`. This is a source-code observation, separate from
the blocked installed-GUI test and the unverified public image response.
Refreshing AppStream caches alone does not change this screenshot branch.
Getting a screenshot into this Mint listing would require the relevant
catalog/screenshot service or a Software Manager change; neither was
published, submitted, or modified in this task.
