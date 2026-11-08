# Testing a fork build

## Windows test packages

Push a `fix/**` branch to run **Windows Test Build**. The workflow runs the
frontend and Rust workspace tests, then uploads the NSIS installer and standalone
`pebble.exe` as an Actions artifact. The artifact name includes the commit SHA.
Artifacts expire after 14 days; this workflow does not publish a Release.

The regular PR CI also uploads Windows, macOS and Linux installers after its
formatting, lint, test and build checks pass.

## OAuth configuration is not inherited

Forks do not inherit the upstream repository's secrets. Test packages deliberately
do not embed OAuth client credentials. IMAP/SMTP accounts can be tested without
Google or Microsoft OAuth configuration. Gmail/Outlook OAuth login and refresh
require the appropriate client configuration; do not overwrite an existing
OAuth-enabled installation with an unconfigured test build.

See `.env.example` and the README's OAuth section. Keep secrets out of commits,
test fixtures, logs and public build artifacts. A desktop executable cannot
protect a secret embedded at compile time.

## Safe local integration testing

- Back up the executable and profile before testing. Use SQLite's backup API
  for a consistent database snapshot while the original process is running.
- Set `PEBBLE_PROFILE_DIR` to an **absolute path to a disposable profile**.
- Use test accounts or a local copy of the intended profile. Do not run two
  versions against the same profile simultaneously.
- A copied profile still connects to the real remote mailbox. Disable pending
  mail operations, rules and automatic backup tasks in the **copy** before a
  read-only sync investigation. Never clear these in the original profile.
- Do not enable protocol-level tracing: it can expose credentials and email
  contents. Narrow metadata logging can be enabled with
  `RUST_LOG=pebble=info,pebble_mail::imap=debug,pebble_mail::sync=debug`.
- Keep profile copies and logs outside version control and never upload them
  as Actions artifacts or attach them to public issues.
- Restore the original executable/profile when testing is finished unless a
  replacement has been validated, including OAuth refresh when applicable.

## Recovery regressions

`crates/pebble-mail/src/test_imap.rs` implements a loopback-only, credential-free
IMAP fixture. Tests cover server BYE/reconnect, permission failures, missing
Inbox, silent IDLE DONE/LOGOUT, transport loss during login, startup retries,
manual-only behavior and cancellation during backoff. They never connect to
real email providers.

Deletion regressions additionally cover an unsynchronized 163-style `已删除`
folder, retaining an existing folder ID, cached discovery, missing Trash,
LIST errors and timeouts (including an incomplete LIST stream), tracked MOVE
destination UIDs, and idempotent pending-operation commits. Normal IMAP deletion
discovers and caches the server Trash when it is missing locally; it never
falls back to permanent deletion. Permanent deletion remains a separate action
for messages already in Trash. Folder discovery does not rewrite the user's
configured sync selection.

Frontend regressions cover zero/partial results, backend error details, query
refresh after partial failure, stale selections, confirmation cancellation and
duplicate submission. Test deletion only against disposable messages or the
loopback fixture, not a user's real mailbox.

Run:

```sh
pnpm test
pnpm build:frontend
cargo test --workspace --all-targets --locked
```

Passing these checks does not prove every provider-specific issue is resolved.
Verify initial sync, new-message arrival, sleep/resume, reconnection, and both
message and thread views with a test mailbox before replacing a daily-use build.
