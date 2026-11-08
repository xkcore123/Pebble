# Single-branch patch maintenance

- User chose one maintained branch: `master`. Do not recreate personal/fix branches unless requested.
- `origin` is `xkcore123/Pebble`; `upstream` is `QingJ01/Pebble`. No upstream PR unless requested.
- Keep application source at the upstream version. Store fixes AND their tests in `patches/pebble-fixes.patch`.
- Read `patches/README.md`. Patch manifest records provenance, file list and SHA256; regenerate it when changing the patch.
- `Patched Build` applies/checks the patch before testing/packaging. Upstream CI/Release remain unpatched.
- Never commit expanded patch changes to source. Never force-push or silently skip failed patch hunks.
- Sync upstream on master with a clean, unpatched working tree; validate patch applicability and follow cloud builds through completion.
- Preserve the backup tag. Do not upload real mail profiles, logs, credentials or destructive mailbox test data.
- Do not overwrite the daily-use executable with a fork build missing OAuth configuration.
