# ClipVault 2.2.1

ClipVault 2.2.1
• Automated releases validate the exact tagged source, build and check DEB/AppImage on Ubuntu 24.04 and RPM on Fedora 44, and sign update metadata with Ed25519.
• Publication verifies the complete bundle and uploaded bytes before making a release public; retries never replace published files. Manual runs default to a nonpublishing rehearsal.
• Redesigned responsive website uses compiled Tailwind, real app imagery, Light/Dark/System themes, keyboard-accessible controls, and actual latest stable release assets with safe GitHub Releases fallbacks.
• Updated documentation covers installation, compatibility, privacy, signing setup, release retries, website deployment, and screenshot provenance.
• Signing requires a maintainer-provisioned external private key in GitHub Actions secrets and its trusted public key in the app. The trust store remains empty; automatic installation stays disabled until setup and signed publication are complete.
Existing history, pins, preferences, shortcuts and data-directory arguments are preserved. No schema reset or data deletion is needed.

See [release automation and setup](packaging/RELEASING.md), [updater trust](packaging/UPDATER.md), [website documentation](docs/WEBSITE.md), and [local verification](V2.2.1-VERIFICATION.md). This source preparation does not publish a release or establish live Actions/Pages or installed-upgrade acceptance. No release tag has been created.
