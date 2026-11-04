# Where InboxCraft falls short

> **Last reviewed:** <date> · **Last updated:** 2026-11-04 · **Change:** minor

## Ranked
| # | Gap | B | Evidence | Impact | Hours | Doc |
|---|---|---|---|---|---|---|
| 1 | <gap> | ✓ | <how we know> | <who it hurts> | <h> | |

B = blocks the next stage.

## Known rough edges (not ranked)
- Every crate is an empty skeleton; the apps answer `--version` and refuse everything else (#5).
- The `ui-egui` drawing code is not left out of the workspace coverage total yet: there is none. Its files join
  `workspace-exclude` in `.config/coverage.toml` when it lands (#7).
- The app crates have no coverage floor of their own; their `main` counts only for its crate, which has none (#7).
- `cargo xtask ci` runs the tests twice, once under nextest and once under `cargo llvm-cov nextest` for the
  coverage gate (#7).
- `testkit` serves one client per fake server and speaks implicit TLS only; STARTTLS comes with the fake SMTP
  server. A script matches whole lines exactly (#8).
- `testkit`'s `TestClient` checks certificates with the rustls WebPKI verifier and the test trust store, not the
  OS verifier `net` will use. It knows its clock is far off because the test sets the offset; the real
  `account.check` needs another reference for the clock (#8).
- The live test harness has one live test so far, which checks the configuration only; the IMAP, SMTP and CalDAV
  round trips, and the code that creates and removes the run folder on a server, come with `net`. Deleting the run
  folder leaves the empty `InboxCraft-Test` parent behind (#10).
- `just it-live` reads the credentials with `secret-tool` (Secret Service) or `security` (macOS Keychain) and needs
  bash; Windows Credential Manager is not supported yet. The leak scan of its output looks for the two secrets, the
  two addresses and the two login names, each verbatim and as Rust's `{:?}` prints it, not for encoded forms such
  as base64 in an `AUTHENTICATE` line. It runs after the output is on the screen, so it reports a leak and does not
  hide it; a short login name that also occurs in ordinary output fails the run (#10).
- A server without a hierarchy delimiter (`NIL` in `LIST`) has no run folder under `InboxCraft-Test`;
  `RunFolder` needs a delimiter (#10).
- `time` knows fixed offsets only: no IANA zones and no lookup of the computer's own zone yet, so a date shows in
  the sender's offset or UTC. ISO 8601 and iCalendar text come with `ical` (#15).
- RFC 5322 dates are read leniently where RFC 5322 section 4.3 allows it or real mail needs it: the day of the
  week is not checked against the date, second 60 reads as 59, and a missing zone, a military letter or an unknown
  zone name (`CET`) reads as `-0000`. Text after the zone other than a comment (`+0100 CET`) is refused (#15).
- `testkit`'s certificate `Clock` still reads the wall clock (through the `time` crate of crates.io), because the
  test certificates are made valid around the real now; it is not an `inboxcraft_time::Clock` (#15).
- `fuzz/` resolves its dependencies afresh on every run: it has no committed `Cargo.lock` and `cargo deny` does
  not see it (#15).
- The `justfile` has no `assets` and `stats` recipes; they run as part of `just ci` (#6).
- `cargo xtask layers` checks direct dependencies only, and knows network and UI crates by name from two lists in
  `xtask/src/layers.rs`; a crate missing from them is not caught (#6).
- `cargo xtask stats` counts files and lines; the number of commands in the registry comes once the registry
  exists (#6).
- `cargo xtask test-files` reads lines, not a syntax tree: a module line inside a multi-line string counts as a
  module (#5).

## Revision history
| Date | Change | Summary |
|---|---|---|
| <date> | major | First version |
| 2026-11-04 | minor | Rough edges of the workspace skeleton (#5) |
| 2026-11-04 | minor | Rough edges of the `layers`, `wasm`, `assets` and `stats` gates (#6) |
| 2026-11-04 | minor | Rough edges of the coverage gate (#7) |
| 2026-11-04 | minor | Rough edges of the fake server frame (#8) |
| 2026-11-04 | minor | Rough edges of the live test harness (#10) |
| 2026-11-04 | minor | Rough edges of the clock and RFC 5322 dates (#15) |
| 2026-11-04 | minor | Live test harness: the wider leak scan, a server without a hierarchy delimiter (#10) |
