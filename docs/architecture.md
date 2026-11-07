# Architecture

> **Last reviewed:** <date> · **Last updated:** 2026-11-06 · **Change:** minor

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
| `Check` | the account check, at once and on a connection of its own, also while offline or halted: DNS, TCP, TLS, the certificate, the IMAP sign-in | one `CheckStep` per step of `CheckStep::ALL`, in order |

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
| `BodyFetched` | the body kept with its plain text for the search; the fetch ends; a body over 64 MiB is not kept, and the message is marked so it is not asked for again in this session |
| `FolderSynced` | the folder's state recorded for the next sync |
| `Connection`, `Failed` | shown by `account.list`; online again clears the failure; a failed `FetchBody` job ends its fetch |
| `CheckStep` | the step takes its place in the account's running check (from `account.check`), shown by `account.list`; the last step ends the check; a step of another job is dropped |

An event for an account or folder the store does not hold, or with UIDs of another UIDVALIDITY than the folder's
current one, is dropped; only a failure of a `FetchBody` job still ends its fetch. The read commands are `account.list`, `folder.list` (with the unified views: one role
across every account), `mail.list` (one folder or one view, merged by date, newest first, narrowed to one account
with `account`), `mail.read` and `mail.search`, all `safe`. Opening a message changes nothing on the server; it
marks the message opened in the store so its body is kept, and asks the worker for a body not kept yet, once. A
body too large to keep is not asked for again; `mail.read` then answers `body_error: "too_large"`.

The reader adds `mail.thread` (the thread of a message oldest first, linked by `Message-ID`, `In-Reply-To` and
`References` across folders and accounts), `mail.attachment` (one attachment or inline part of a kept body, in
base64) and `remote.list`, all `safe`, and `remote.allow` and `remote.forget`, `owner`: whether a sender's remote
pictures load without asking changes what leaves the computer. `mail.read` answers the HTML part as text (for
`html`'s display, never run), the inline parts by Content-ID, numbered attachments and `remote_allowed`. No command
fetches a remote picture: the front end asks its fetch for the pictures the owner allowed, and the desktop app's
fetch is `net::picture` on threads of its own (HTTPS only, a fixed user agent, no cookies or referrer, public
addresses only), so the engine stays free of network I/O.

The appearance commands (`ui.theme.set`, `ui.accent.set`, `ui.font.set`, `ui.icons.set`, `ui.text_scale.set`,
`ui.motion.set`, all `safe`) change one choice each of the session's `Appearance` (`engine::appearance`) and
answer the whole of it. The engine checks the values and makes a custom accent readable; it keeps nothing on disk.
The front end installs a changed appearance in the frame it finds it and keeps it in a file of the config folder
(`ui-egui::keep`), handing it back to the session with `Session::set_appearance` at the next start.

`sample.mail.set` (`safe`) marks a message read or unread and moves it to another folder of its account, or both,
and answers where it was and whether it was read, so the front end can undo it. It runs only in a session made with
`Session::sample` over the invented sample mailbox; any other session refuses it (`sample_only`). It queues no job,
so nothing reaches a server; the sample's hover actions use it until the commands for a real mailbox exist.

Onboarding adds `account.add` and `account.remove`, both `owner`. `account.add` takes the settings the owner
entered or saw (address, names, user name, the IMAP server) and never a password; it saves the account under an id
no account had in this session and queues the owner's first check. The front end puts the password into the OS
credential store (`ui_egui::onboard::Keychain`; `keyring` in the desktop app) under the account's id, and the
worker reads it from there each time it signs in, so a password never passes through a command, an answer, the
store or the log. The desktop app keeps its accounts in the store on disk, so their passwords stay when the window
closes; a front end whose accounts are forgotten removes them then (`InboxApp::on_exit`). A new account's first check that signs in queues its folder list, so the sync starts after the
password was accepted and not before. `account.remove` removes the account with its mail and drops its state and
queued jobs. Each frame the window hands the queued jobs to the app's workers (`ui_egui::workers::Workers`; one
`net::Worker` per account, started at its first job, stopped off the window's thread when its account is gone) and
applies their events; Detect runs on a thread of its own in the window, outside the engine.

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
- The account check (`net::check_account`, behind `account.check`) runs the steps one after the other and reports
  each as it ends, so a failure is named by its cause. A port that times out is told apart by trying port 443 and
  the other mail ports of the same address (the network blocks mail, or the port is wrong, or the server is down).
  The certificate's issuer and dates come from our own small DER reader, fed by a verifier that keeps the
  certificate and asks the real one; the dates give the clock check where the platform's verifier gives none.
- Detect (`net::detect`) only proposes: MX, autoconfig at the domain over HTTPS, the ISPDB when switched on (off by
  default), SRV, and common host names as a hint. DNS is `hickory-proto` messages over our own blocking UDP and TCP
  exchange with the resolver configuration of the system, HTTPS is `ureq` with `rustls` and the same trust as the
  mail connections, following no redirect and sending nothing but the URL. A candidate is detected settings
  (`CheckTarget::detected`): the check signs in to it only after the owner has seen it and its certificate
  verified.
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
  that removed it. The store does no network I/O and parses no mail: the caller hands in each body's text.
- Beside the mail the store keeps the local records (`model::local`, schema version 4): the job queue, drafts, the
  outbox, the schedule, senders (known, the screener's decision, the group), each message's own state (done,
  parked, pinned, snoozed, follow-up) and the calendars with their sync token, events (ETag and iCalendar text) and
  alerts. Each row holds its record as JSON, so a new field needs no migration; removing an account, folder,
  message, draft, calendar or event removes what hangs below it. The desktop app keeps its store in
  `inboxcraft/store` in the owner's data folder (`XDG_DATA_HOME`, `~/.var/app/<app id>/data` in the Flatpak); the
  sample stays in memory.
- Kept jobs: a command that queues a job with `Outcome::with_kept_job` (the acts on mail) writes it into the
  store's job queue in the same batch as its changes, so a kill keeps both or neither. The job leaves the queue with
  its last event (the answer, or `Failed`); `Session::start` hands the jobs an earlier run left over again, under
  their ids, before the start's own. Read jobs are not kept: the start syncs again anyway.
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
| 2026-11-05 | minor | The account check, its job and events, and Detect (#28) |
| 2026-11-05 | minor | The reader commands and where remote pictures are fetched (#33) |
| 2026-11-05 | minor | Adding an account: `account.add`, `account.remove`, the credential store and the window's workers (#34) |
| 2026-11-05 | minor | The window removes the session's passwords when it closes (#34) |
| 2026-11-06 | minor | The appearance commands and where their result is kept (#86) |
| 2026-11-06 | minor | `sample.mail.set` and the sample session (#89) |
