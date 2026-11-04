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

## Revision history
| Date | Change | Summary |
|---|---|---|
| <date> | major | First version |
| 2026-11-04 | minor | Progress log: workspace skeleton and first gates (#5) |
| 2026-11-04 | minor | Progress log: the remaining E0 gates (#6) |
