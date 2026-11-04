# InboxCraft roadmap

**Stage: <pre-alpha|alpha|beta>** · next: <stage>, <what is left, in points or hours>

> **Last reviewed:** <date> · **Last updated:** 2026-11-04 · **Change:** minor

The detail is in [docs/gaps.md](docs/gaps.md) (what is missing, ranked) and [docs/architecture.md](docs/architecture.md).

## Headline numbers
| Number | Value | Kind |
|---|---|---|
| Commands in the registry | <n> | measured (`cargo xtask stats`) |
| Tests | <n> | measured |
| Fuzz targets | <n> | measured |
| Hours to beta | <n> Opus 5.5 agent-hours | estimated (<how>) |

## By area
| Area | Ready | Hours left | Doc |
|---|---|---|---|
| IMAP sync (IDLE, CONDSTORE/QRESYNC, offline queue) | <%> | <h> | |
| SMTP sending | <%> | <h> | |
| CalDAV calendars | <%> | <h> | |
| Google Calendar | <%> | <h> | |
| Account autodetect | <%> | <h> | |
| Tray and background mode | <%> | <h> | |
| Screener / grouped inbox / snooze / send later | <%> | <h> | |
| Reading mail safely (HTML) | <%> | <h> | |
| Search | <%> | <h> | |

## Upcoming
1. <item> (<hours>)

## Progress log
| Date | What landed |
|---|---|
| 2026-11-04 | Workspace skeleton: every crate and app as an empty crate, `cargo xtask ci`, `spdx`, `test-files` (#5) |
| 2026-11-04 | The `layers`, `wasm`, `assets` and `stats` gates, all in `cargo xtask ci` (#6) |
| 2026-11-04 | The coverage gate `cargo xtask cov --check`, floors in `.config/coverage.toml`, in `cargo xtask ci` (#7) |
| 2026-11-04 | `testkit`: the fake server frame with the drop, delay, garbage and certificate faults (#8) |
| 2026-11-04 | Opt-in live test harness: `just it-live`, nextest profile `live`, `testkit::live` (#10) |
| 2026-11-04 | `time`: injectable clock (system and fake), RFC 5322 dates with the obsolete forms, first fuzz target (#15) |
| 2026-11-04 | `charset`: WHATWG charsets to UTF-8, RFC 2047 encoded words read and written, two fuzz targets (#16) |
| 2026-11-04 | `model`: accounts, folders with roles (SPECIAL-USE, English and Hungarian names, ask once), messages, threads, the read path jobs and events (#17) |
| 2026-11-04 | `mime`: messages read into a tree of parts (headers, RFC 2231 parameters, base64 and quoted-printable, multiparts, attached messages, attachments, delivery reports), capped, three fuzz targets (#18) |
| 2026-11-04 | `mime`: messages built (fields, text and HTML, attachments) behind a header injection guard, build-then-parse property tests (#19) |
| 2026-11-04 | `proto`: the sans-I/O IMAP client core (greeting, CAPABILITY, STARTTLS, LOGIN and AUTHENTICATE PLAIN over TLS only, LIST with SPECIAL-USE, SELECT, EXAMINE, UID FETCH, UID SEARCH, LOGOUT), golden transcripts, fuzz target (#20) |

## Revision history
| Date | Change | Summary |
|---|---|---|
| <date> | major | First version |
| 2026-11-04 | minor | Progress log: workspace skeleton and first gates (#5) |
| 2026-11-04 | minor | Progress log: the remaining E0 gates (#6) |
| 2026-11-04 | minor | Progress log: the coverage gate (#7) |
| 2026-11-04 | minor | Progress log: the fake server frame (#8) |
| 2026-11-04 | minor | Progress log: the live test harness (#10) |
| 2026-11-04 | minor | Progress log: the clock and RFC 5322 dates (#15) |
| 2026-11-04 | minor | Progress log: charsets and encoded words (#16) |
| 2026-11-04 | minor | Progress log: the mailbox model (#17) |
| 2026-11-04 | minor | Progress log: the message reader (#18) |
| 2026-11-04 | minor | Progress log: the message builder (#19) |
| 2026-11-04 | minor | Progress log: the IMAP client core (#20) |
