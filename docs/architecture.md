# Architecture

> **Last reviewed:** <date> · **Last updated:** 2026-11-04 · **Change:** minor

## Layers
<table L0–L6 + apps, the rule: a crate depends only on lower layers; enforced by `cargo xtask layers`>

## Data flow
<UI/CLI/MCP → Session::run(id, params) → engine emits Jobs → net worker per account (IDLE/NOTIFY, back-off)
 → Events → engine applies → store (atomic writes) → UI repaints>

## Decisions worth knowing
- The engine does no I/O; protocols are sans-I/O (`proto`, `dav`); sockets and TLS live only in `net`.
- No async runtime; one worker thread per account.
- Code that needs the time takes an `inboxcraft_time::Clock`; tests hand it a `FakeClock`. `Instant::now` and
  `SystemTime::now` are banned in `clippy.toml` (they trap on wasm); `SystemClock` is the one place that reads them.
- Local-first: state that must follow the person between devices lives in the mailbox (hidden folders), because
  the reference server has no IMAP METADATA.
- Tray/background mode: the window can close while the engine keeps running.

## Start-up

## Testing and gates

## Revision history
| Date | Change | Summary |
|---|---|---|
| <date> | major | First version |
| 2026-11-04 | minor | The injectable clock (#15) |
