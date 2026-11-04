# Architecture

> **Last reviewed:** <date> · **Last updated:** 2026-11-04 · **Change:** minor

## Layers
<table L0–L6 + apps, the rule: a crate depends only on lower layers; enforced by `cargo xtask layers`>

## Data flow
UI, CLI and MCP call `Session::run(id, params)`. The engine changes the local state at once and emits jobs; the
account's `net` worker runs them and reports events; the engine applies the events, `store` writes them atomically
and the UI repaints.

```text
UI / CLI / MCP --Session::run--> engine --Job--> net worker (one thread per account) <--IMAP--> server
                                   ^                         |
                                   +--------- Event ---------+
                                   |
                                   +--> store (atomic writes) --> UI repaints
```

The jobs and events are types of `model` (layer 1), not of `engine`, because `net` (layer 3) may not depend on
`engine` (layer 4); both speak `model`, and no protocol type crosses the line. A job carries everything the worker
needs (the folder's path, the UIDVALIDITY and the state the last sync ended at), so the worker never reads the
engine's state.

| Job (`model::JobKind`) | What the worker does | Events it answers with |
|---|---|---|
| `SyncFolderList` | `LIST` with SPECIAL-USE | `Folders` (the whole list) |
| `SyncFolder` | a full sync, or a resync from the known `FolderState` (CONDSTORE, QRESYNC) | `MessagesAdded`, `MessagesChanged`, `MessagesExpunged`, `UidValidityReset` if the UIDVALIDITY changed, `FolderSynced` last |
| `FetchBody` | fetches one whole message | `BodyFetched` |
| `Idle` | watches the folder (IDLE, or NOTIFY) until the next job | the same change events as `SyncFolder` |

On its own, without a job, the worker reports `Connection` (offline, connecting, online, watching, backing off)
and `Failed` with one `ErrorKind` per cause (host not found, each certificate fault, STARTTLS missing, login
refused, folder gone, ...), so the owner is told the cause. Every event with UIDs carries the UIDVALIDITY they
belong to; the engine drops an event for an older one.

Folder roles come from `model::RoleMap`: SPECIAL-USE first, then the English and Hungarian names, then the owner,
asked once per role. Inbox, Sent and Drafts stay fixed once set; Trash, Junk and Archive can be changed.

## Decisions worth knowing
- The engine does no I/O; protocols are sans-I/O (`proto`, `dav`); sockets and TLS live only in `net`.
- No async runtime; one worker thread per account.
- Code that needs the time takes an `inboxcraft_time::Clock`; tests hand it a `FakeClock`. `Instant::now` and
  `SystemTime::now` are banned in `clippy.toml` (they trap on wasm); `SystemClock` is the one place that reads them.
- Local-first: state that must follow the person between devices lives in the mailbox (hidden folders), because
  the reference server has no IMAP METADATA.
- The store (`inboxcraft-store`, layer 1, the one crate allowed to depend on `model` inside its layer) is the
  `Storage` trait with two backends. `Memory` is always built and is the store on wasm. `Sqlite` is native only
  (feature `sqlite`): `store.sqlite3` in WAL mode with `synchronous=FULL` and foreign keys on, its schema version in
  `PRAGMA user_version` with one migration per version, an FTS5 index of the body text, and each message source as
  `sources/<body id>.eml` beside it. A source is written and synced before the row that names it commits and
  removed only after that row is gone; opening the store removes the files a kill left behind. The store does no
  network I/O and parses no mail: the caller hands in each body's text.
- Tray/background mode: the window can close while the engine keeps running.

## Start-up

## Testing and gates

## Revision history
| Date | Change | Summary |
|---|---|---|
| <date> | major | First version |
| 2026-11-04 | minor | The injectable clock (#15) |
| 2026-11-04 | minor | Data flow: the read path jobs and events in `model`, folder roles (#17) |
| 2026-11-04 | minor | The local-first store and its two backends (#24) |
