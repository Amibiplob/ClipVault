# ClipVault 2.2.2

ClipVault 2.2.2
• Packaging dependency fixes share the complete Noble build prerequisites, including libxcb-shape0, between local builds and GitHub Actions.
• Clearer library checks report all missing or unusable bundled libraries, image loaders and linked dependencies together, with installation guidance before building or replacing staged packages.
• Artifact-upload safeguards check the exact current RPM and diagnostic logs for readability, permissions, symlinks, disk space and ZIP integrity; container exports are readable and diagnostic uploads are narrowly scoped.
• Updated GitHub Actions use Node.js 24-compatible versions, including the Pages artifact chain. Mandatory package validation, Ed25519 signing, signature verification and upload/read-back checks remain required.
• The Fedora RPM failure is not confirmed resolved. Full original upload-log review and a successful hosted Fedora build/upload are still pending; local safeguards are not hosted verification.
The embedded update keys are unchanged. Existing history, pins, preferences, shortcuts and data-directory arguments are preserved; no schema reset or data deletion is needed.

See [release automation and setup](packaging/RELEASING.md), [updater trust](packaging/UPDATER.md), and [local verification](V2.2.2-VERIFICATION.md). This is source preparation only: no v2.2.2 tag, push, signing or publication has been performed.
