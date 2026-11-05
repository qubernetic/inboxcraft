# Architecture

> **Last reviewed:** <date> · **Last updated:** 2026-11-05 · **Change:** minor

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
| `Idle` | watches the folder between jobs (IDLE, or a resync every few minutes without it) until another `Idle` moves the watch | the same change events as `SyncFolder`, after each change and each reconnect |

On its own, without a job, the worker reports `Connection` (offline, connecting, online, watching, backing off)
and `Failed` with one `ErrorKind` per cause (host not found, each certificate fault, STARTTLS missing, login
refused, folder gone, ...), so the owner is told the cause. Every event with UIDs carries the UIDVALIDITY they
belong to; the engine drops an event for an older one.

The engine (`inboxcraft-engine`) is the `Session`. Every command is a `CommandSpec` in `crates/engine/src/cmd/*.rs`
with a stable id, an agent class (`safe`, `owner` or `never`) and a function; the `Registry` refuses a command
without a class, so an unclassified command never runs, and an agent (`Caller::Agent`) runs `safe` commands only.
Parameters are a JSON object (unknown fields refused), answers are JSON (snake_case keys, ISO 8601 dates in UTC).
A command only reads (`Context`: the store, the session's state, the time) and returns an `Outcome`: the answer,
the store changes as one batch and the jobs. The session makes them after the command returned, so a failed
command changes nothing; a command that panics despite the rules is caught and becomes `EngineError::Panicked`,
with nothing changed. Events go the same way: `Session::apply` plans the changes, then makes them.

| Event | What the engine does |
|---|---|
| `Folders` | held folders keep their ids (by name, the inbox in any case), new ones get one, missing ones go; roles are detected again; every folder that holds mail is synced from its known state, the inbox first, and the inbox is watched (`Idle`) |
| `MessagesAdded`, `MessagesChanged`, `MessagesExpunged` | messages put (a known UID keeps its id), flags set, messages removed |
| `UidValidityReset` | every message of the folder removed; the new UIDVALIDITY is the current one |
| `BodyFetched` | the body kept with its plain text for the search |
| `FolderSynced` | the folder's state recorded for the next sync |
| `Connection`, `Failed` | shown by `account.list`; online again clears the failure |

An event for an account or folder the store does not hold, or with UIDs of another UIDVALIDITY than the folder's
current one, is dropped. The read commands are `account.list`, `folder.list` (with the unified views: one role
across every account), `mail.list` (one folder or one view, merged by date, newest first, narrowed to one account
with `account`), `mail.read` and `mail.search`, all `safe`. Opening a message changes nothing on the server; it
marks the message opened in the store so its body is kept, and asks the worker for a body not kept yet, once.

Folder roles come from `model::RoleMap`: SPECIAL-USE first, then the English and Hungarian names, then the owner,
asked once per role. Inbox, Sent and Drafts stay fixed once set; Trash, Junk and Archive can be changed.

## Decisions worth knowing
- The engine does no I/O; protocols are sans-I/O (`proto`, `dav`); sockets and TLS live only in `net`.
- No async runtime; one worker thread per account (`net::Worker`). It opens one connection at its first job, signs
  in, runs the jobs in order and opens folders read-only (EXAMINE, `BODY.PEEK`), so the read path changes nothing on
  the server. Answers stream out as events while they come in. Jobs wait in an offline queue while it reconnects
  after a back-off with a cap. Between jobs it watches one folder with IDLE, reading in short ticks so a job ends the
  IDLE; every change, and every reconnect, brings a resync of the folder from the state the last sync left (QRESYNC
  where offered), so a change seen live and one made while offline are reported the same way.
- TLS in `net`: `rustls` with `ring`, the operating system's verifier (`rustls-platform-verifier`), certificates
  checked at the time of the injected clock. Implicit TLS, or STARTTLS before anything else; a password goes to
  `proto` only on an encrypted connection, and `proto` refuses it before as well.
- Code that needs the time takes an `inboxcraft_time::Clock`; tests hand it a `FakeClock`. `Instant::now` and
  `SystemTime::now` are banned in `clippy.toml` (they trap on wasm); `SystemClock` is the one place that reads them.
- Local-first: state that must follow the person between devices lives in the mailbox (hidden folders), because
  the reference server has no IMAP METADATA.
- The store (`inboxcraft-store`, layer 1, the one crate allowed to depend on `model` inside its layer) is the
  `Storage` trait with two backends. `Memory` is always built and is the store on wasm. `Sqlite` is native only
  (feature `sqlite`): `store.sqlite3` in WAL mode with `synchronous=FULL` and foreign keys on, its schema version in
  `PRAGMA user_version` with one migration per version, an FTS5 index of the body text, and each message source as
  `sources/<body id>.eml` beside it. A source is written and synced before the row that names it commits and
  removed only after that row is gone; opening the store removes the files a kill left behind. Removed mail is
  zeroed in the database (`secure_delete`, FTS5 `secure-delete`) and the write-ahead log is emptied after the batch
  that removed it. The store does no
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
| 2026-11-04 | minor | The store zeroes removed mail (#24) |
| 2026-11-05 | minor | The account worker and TLS in `net` (#25) |
| 2026-11-05 | minor | Live updates, the offline queue and back-off in `net` (#26) |
| 2026-11-05 | minor | The engine: the `Session`, commands with agent classes, events applied (#27) |
