# Personal fork maintenance

- `origin` is `xkcore123/Pebble`; `upstream` is `QingJ01/Pebble`.
- Keep `master` an upstream-only, fast-forward mirror. Never merge personal fixes into it.
- Work and build on `personal`; new `fix/**` branches should start from and target `personal`.
- Do not rewrite the existing shared commits or force-push. Preserve the old fix branch for recovery.
- Use `scripts/sync-upstream.mjs` for guarded local sync; read `FORK-MAINTENANCE.md` first.
- Push only after reviewing changes; follow Personal Build through completion. No upstream PR unless requested.
- Keep fork-only CI in `personal-build.yml`, not upstream's `ci.yml`.
- Keep real profiles, messages, credentials and diagnostics out of Git. Never test destructive mail operations on the user's mailbox.
- Do not replace the daily-use executable with a fork build lacking the required OAuth configuration.
