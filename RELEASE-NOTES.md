# ClipVault 2.2.0

ClipVault 2.2.0
• History toolbar separates clip count and Clear unpinned, with hidden-filter scope in tooltip and confirmation.
• Larger image previews preserve aspect ratio, with copied time, dimensions for new images, lazy visible rows and a bounded thumbnail cache.
• Centered Back icon/label and compact wrapping About links.
• Integrated Linux updater detects running package ownership or AppImage runtime and matches package, CPU and supported distribution release.
• Signed Ed25519 release metadata, SHA-256 artifact checks, system authorization, atomic AppImage replacement with rollback, cancellation and safe restart coordination.
• Automatic installation requires maintainer-provisioned signing keys and published signed metadata. Unsupported installations use manual downloads.
Existing history, pins, preferences, shortcuts and data-directory arguments are preserved. No schema reset or data deletion is needed.

See [update signing and publication](packaging/UPDATER.md) and [verification](V2.2.0-VERIFICATION.md). Artifacts are local candidates until independently tested, signed and published.
