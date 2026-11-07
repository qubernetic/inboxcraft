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
| `StoreFlags` | sets and clears flags of messages (`UID STORE`) | `MessagesChanged` with the flags the server now has, `FlagsStored` last |
| `Move` | moves messages to another folder of the account (`UID MOVE`, or `UID COPY`, `\Deleted` and `UID EXPUNGE`) | `Moved`, with the new UIDs where the server gives them (`COPYUID`) |
| `Expunge` | removes messages for good: `\Deleted` on them and `UID EXPUNGE` of their UIDs only | `Expunged` |
| `Append` | adds a message to a folder (`APPEND`) | `Appended`, with its UID where the server gives it (`APPENDUID`) |
| `CreateFolder` | creates a folder (`CREATE`) | `FolderCreated` |

The write jobs (`JobKind::writes`) are the owner's acts: they are kept in the store's queue until their last event.
The worker runs them on the IMAP write path (`net::write`): it opens the folder with SELECT and checks its UIDVALIDITY
against the job's (a changed one is told as `UidValidityReset`, and nothing is written); after a store it reads the
flags back, so a message gone meanwhile comes as `MessagesExpunged`, not as a failure; a move names no new UID for
it. It never sends CLOSE or EXPUNGE, which would remove every message marked `\Deleted`; a delete for good needs
UID EXPUNGE. A write that a dropped or timed-out connection cut off runs again after the reconnect, three times at
most; each write ends in the state one run leaves, except APPEND, which can add the message twice. A move on a server
without MOVE is not run again: its fallback may have copied the messages before the cut, and a second run would copy
them again. Any other failure is `Failed`, and the engine takes the act back.

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
| `Abilities` | kept for the account until the next sign-in: whether the server removes only the messages named (UID EXPUNGE), which a snooze chooses its way by |
| `CheckStep` | the step takes its place in the account's running check (from `account.check`), shown by `account.list`; the last step ends the check; a step of another job is dropped |
| `FlagsStored`, `Expunged`, `Appended` | the write's job ends; `Expunged` removes the messages if a sync has not yet |
| `Moved` | each moved message takes its server UID in the folder it went to; one the owner moved on or flagged meanwhile gets the jobs for that now; with no new UIDs it goes, and that folder is synced again |
| `FolderCreated` | the folder list is asked for, which brings the new folder |
| `Failed` of a write | the act is taken back: flags as they were, a moved message back where the server keeps it (both folders synced again), a whole resync of the folder after a failed delete for good |

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

The acts on messages are `mail.flag`, `mail.seen` (read and unread), `mail.open` (`mail.read`, and the message
marked read), all `safe`, and `mail.move`, `mail.archive` (to the account's Archive folder) and `mail.delete`
(to the account's Trash; a message in the Trash goes for good), all `owner`. An act changes the store at once and
queues a kept write job (`engine::act`). A moved message keeps its id: it goes to the new folder under a UID of
ours, from the top of the UID range down (servers count up from 1), with `Message::moving` noting the folder, UID
and flags the server has; the `Moved` answer gives it the server's UID there (`engine::write`). An act on a message
whose move is on its way only changes the store: once the answer names the message's place, the engine sends the
flags it has and moves it on to the folder it is in by then, so an undo of an archive made offline works. Server
events never touch a UID of ours, and a server UID that takes one moves ours aside. A message still on its way
cannot be deleted for good yet (`still_moving`), and its body is fetched once it has arrived.

Each act but a delete for good stores its inverse in the session's undo history (`engine::undo`, at most 100 acts,
for the session only); `history.undo` (`safe`) takes the newest back and can be run again to walk further. An agent
undoes only an act an agent may make, and an act whose message or folder is gone is passed over. In the sample
mailbox, and in a folder before its first sync, the acts change the store and queue no job.

New messages go through the arriving-mail pipeline (`engine::arrive`) after the sync put them in its batch: each
message passes each stage in order, and a stage adds store changes, kept jobs or an act (a move, say) to the same
batch. A stage is a file of its own in `crates/engine/src/arrive/` and one name in the `stages!` line; the screener
is the first and the notify stage the second, the outbox and calendar slices bring more.

The screener (`engine::arrive::screener`, commands in `engine::cmd::triage`) keeps the known-sender index in the
store's senders table, one record per address across the accounts: a sender is known once the owner received mail
from it (the first sync of a folder that is not junk, trash, drafts, a virtual folder or our own; mail let into the
inbox while the screener is off), wrote to it (every recipient of a message in the sent folder) or accepted it. The
owner's own addresses and the senders of delivery reports (`MAILER-DAEMON`, `postmaster`) are never screened. Later
mail into the inbox from a sender neither known nor decided goes by the mode (`screener.mode`, `owner`; the session
keeps it and the front end hands it back at the start like the appearance): `before` holds it in our own
`InboxCraft/Screener` folder, `inside` leaves it in the inbox with `new_sender` set in `mail.list`, `off` lets it in
and learns the sender. Turning the screener on learns every sender the kept mail names. Our own folder is made on
the server the first time (a kept `CreateFolder` job before the moves; a folder list that comes before the answer
keeps it and does not sync it), beside the inbox with its delimiter, or under it where every folder lives there; an
account without a hierarchy holds nothing. `screener.list` (`safe`) lists the senders waiting with their mail, and
`mail.read` is the preview. `screener.accept` (`owner`, with a group, and `open` to mark the newest read or `done` to
file the held mail in the archive as done) moves the held mail (also from the blocked folder) into the inbox and lets
the sender's future mail through; `screener.block` (`owner`) moves the held mail, and the future mail in every mode,
to `InboxCraft/Blocked`. A blocked sender's message is never fetched: `mail.read` shows the kept body or
`not_downloaded`, and its remote content never loads. `history.undo` takes either decision back: the sender record
as it was and the mail where it was.

The grouped inbox (`engine::arrive::group`, commands in `engine::cmd::triage`) puts each message of a grouped account
in People, Updates, Newsletters or Invitations. `net` reads the signs of a message's kind from its header block into
the envelope (`model::Signs`: a list field, `Precedence`, `Auto-Submitted`, a `text/calendar` type), and the engine adds
whether a kept body holds a `text/calendar` part of its own when the body comes. The group is worked out when a list
asks and never stored: the owner's choice for the sender first (`group.set`, `owner`, kept in the sender record with
the screener's knowledge, so it moves the kept and the future mail at once, and `history.undo` takes it back;
`auto` hands the sender back), then our classifier, first match wins: a calendar part, `Auto-Submitted` other than
`no`, a list field or `Precedence: list` or `bulk`, `Precedence: junk` or a no-reply or delivery-report sender, and
People for the rest. Grouping is on per account until the owner turns it off (`group.account`, `owner`, kept with the
account). `mail.list` says each row's `group` (`null` for an account not grouped) and lists one group (`group`, or
`none` for the mail no group holds); `group.list` (`safe`) counts the inbox's mail and its unread mail per group.

Done, park and pin (commands in `engine::cmd::triage`) are the owner's own state of a message (`model::MessageState`,
the store's message state table), which no server keeps. `mail.done` (`owner`) moves a message to its account's
archive like `mail.archive` and keeps the folder it came from; done `false` moves it back there and clears the mark.
A done message out of the archive (the server refused the move) is filed again by the next done.
"Show done" is `mail.list` with `done`: the done mail whose old place is in the view is listed among the mail there,
by date, with `done` set. `mail.park` (`safe`) puts a message on the park pile: `mail.list` leaves the pile out and
counts it in `parked` (the window's pill), `parked` lists it too, and `park.list` (`safe`) lists the pile across the
accounts. `mail.pin` (`safe`) pins a message: it stays in its lists, and `pin.list` (`safe`) is the pins view across
the accounts. Done takes the pin off unless the owner keeps pins through done (`pin.set`, `owner`; the session keeps
it and the front end hands it back at the start, `Session::pins_outlast_done`). Each list row says `done`, `parked`
and `pinned`. `history.undo` takes each act back: the state as it was and, for done, the message back where it was
when that folder still holds mail.

Snooze (`mail.snooze`, `safe`, in `engine::cmd::triage`; the stage and the return in `engine::arrive::snooze`) puts
a message away until a time, given as `until` or a preset of `schedule.presets`. The message goes to the account's
`InboxCraft/Snoozed` (made on the server the first time) in a new copy whose header has InboxCraft's own field on
top, `InboxCraft-Snoozed-Until` (`model::SNOOZED_UNTIL`) with the time as an RFC 5322 date in UTC: the copy goes up
with `APPEND` (`JobKind::Append` naming the message), and only the answer queues the delete for good of the old copy
and gives the message the new copy's UID (`APPENDUID`); a failed copy deletes nothing and leaves the message where the
server keeps it. A server that names no UID gets the copy from the folder's next sync under a new id. The copy is
written only where the old one can go: on a server that removes only the messages named (UID EXPUNGE, UIDPLUS or
IMAP4rev2, told by `net` after each sign-in as `Abilities`). Elsewhere, before the first sign-in of the session, for a
message whose body is not kept yet, and for one whose move is still on its way, the message is moved without the field
(`written` is `false`). A field an earlier snooze left on top is replaced, not added to. The message's state keeps the
time (`snoozed_until`) and a schedule entry brings it back. The snooze stage reads the field (`net` puts it in the
envelope, and only when it is the first field of the header, where InboxCraft writes it) of mail that arrives in the
snoozed folder, so a message another InboxCraft snoozed,
or our own copy back without a UID, is snoozed until its time here too; elsewhere the field is ignored. When the
entry fires, a message still in the snoozed folder comes back to the inbox unread, with `returned` set to the time
(`mail.list` sorts it as if it arrived then and says `returned` and `snoozed_until` on each row); one the owner moved
on stays where it was put. `mail.unsnooze` (`safe`) brings a message back at once; `history.undo` takes a snooze
back, its time and schedule entry included.

Notifications (`engine::notify`, commands in `engine::cmd::notify`): the engine decides and the front end shows. The
notify stage, after the screener, tells of a message new in an inbox after the folder's first sync that is unread,
still in the inbox after the screener (held or blocked mail tells nothing), not from one of the owner's own
addresses, and not the owner's own move coming back: a moved message a server names no UID for is remembered by its
Message-ID (or sender, subject and date) when it went to an inbox, so the sync that brings it back, as after
`screener.accept`, tells nothing. The account's level (`notify.set`, `owner`; `notify.settings`, `safe`) has the last
word: `all`, `people` (a sender known or accepted whose message is in People, by the owner's group for the sender or
else the classifier) or `none`; `notify::tells` is the decision table. `Session::tick` adds a notice for a returned
snooze and a follow-up reminder unless the owner turned that off. The session queues the notices (at most 100, the oldest go) until the front end takes them
(`Session::take_notices`), more than three new messages of one account as one count. A notice carries the sender's
name and subject as one clean line of at most 120 characters, never markup.

`sample.mail.set` (`safe`) marks a message read or unread and moves it to another folder of its account, or both,
and answers where it was and whether it was read, so the front end can undo it. It runs only in a session made with
`Session::sample` over the invented sample mailbox; any other session refuses it (`sample_only`). It queues no job,
so nothing reaches a server. The window no longer uses it: its acts run the `mail.*` commands and `history.undo` on
the sample as on a real account (#131).

The scheduler (`engine::schedule`) works on the store's schedule: each entry is due at a moment and sends a draft,
brings a snoozed message back, reminds of a reply that did not come or shows a calendar alert. `Session::tick` fires
every entry whose time has come on the session's clock, in time order: the entry leaves the schedule in the same
batch as what it changes (a returned snooze or a fired reminder clears its time in the message's state, and a
returned snooze comes back to the inbox with its jobs), and comes
back as a `Fired` event, `missed` when it was due before `Session::start` or is handled more than a minute late (the
computer was asleep). `Session::next_due` tells the front end when to tick next. Commands add entries with an id from
`Context::next_schedule_id`. `schedule.presets` (`safe`) answers the presets still ahead: later today (three hours
on, rounded to the nearest half hour of the wall clock, while still today), this evening (20:00) and tomorrow
morning (08:00). They are in the computer's wall-clock time, from the `inboxcraft_time::Offsets` the front end
hands over with `Session::set_offsets` (UTC until then); a wall-clock time the clock skips in spring lands after the
change, one it shows twice in autumn is the first. `schedule.list` (`safe`) lists the entries and `schedule.cancel`
(`owner`) takes one off.

Drafts (`engine::draft`) are kept in the store at every change, so a kill loses nothing typed. `compose.new` starts
one from the account that last sent (the newest message in a Sent folder, else the first account) with that
account's signature below `-- `; `compose.update` changes the fields it is given and keeps the draft at once;
`compose.show` and `compose.list` read them; `compose.discard` removes one here and its copy on the server where
the server named it; all
`safe`. A draft on its way (its outbox item waits for the worker's answer) is neither changed nor discarded, so the
answer finds it as it went. `compose.signature` (`owner`) sets an account's signature. Recipients are kept as typed and checked as
`Name <address>` or a bare address: one that cannot be an address is marked with what is wrong, and it blocks Send
(`blocks`), like a draft without a recipient. `mail.thread` lists the drafts that answer a message of the thread.
`Session::tick` hands a draft that changed and has been still for ten seconds to its account's worker as a kept
`Append` to the Drafts folder (`\Seen \Draft`, without Bcc and the recipients that are no address), and
`Session::next_due` counts it. Where the server has UIDPLUS, the `Appended` answer names the copy's UID; the copy
before it is then removed for good (`Expunge`), and a copy of a draft discarded or moved to another account meanwhile
goes too. An answer without a UID marks the draft (`Draft::unnamed`): that copy can be neither replaced nor taken
back, so no further copy of the draft goes up to that account and it leaves at most one there. A draft with nothing
typed, or of an account without a Drafts folder, stays here; a failed upload waits for the draft's next change.

Sending (`cmd::send`, `engine::send`): `mail.send` (`owner`) makes a draft that nothing blocks into a message with its
own `Message-ID` and a `Date` in the computer's offset, puts it in the outbox (`Outgoing`, state `Sending`, the
message kept for the Sent copy) and queues a `Send` job with the envelope: the account's address and the To, Cc and
Bcc addresses, each once. Bcc is in the envelope alone, never in a header. The job is not kept in the job queue: a
send cut off may have gone, so nothing sends it again on its own. The worker sends it at once on a submission
connection of its own (`net::SmtpConnection`, implicit TLS on 465 or STARTTLS on 587, AUTH PLAIN or LOGIN only after
TLS, `EHLO [127.0.0.1]`), also while the IMAP connection is down, and answers `Sent` or `SendFailed` with the cause
and the server's reply. A connection that ends after the message went and before the server answered it is
`SendUnanswered`: the message may have gone. On `Sent` the draft and its outbox item go, its Drafts copy is removed for good and the
message is appended to the Sent folder (`\Seen`), both kept jobs; on `SendFailed` the draft stays and its outbox
item holds the reply (`Refused`), and `mail.send` can send it again. After the IMAP server refused the password, the
worker sends nothing until it is started anew. After the submission server alone refused it or left it unanswered,
in a check or a send, a check the owner did not start reports the SMTP step as `NeedsOwner` and sends it nothing; an
owner's check or a send that signs in there ends the hold. `JobKind`'s `Debug` shows a message's length and a send's
number of recipients, never the message or an address. `account.add` takes the submission server; the account check signs in
to it after the IMAP sign-in passed, with the same password, as its `SmtpLogin` step (`NotSet` without one).

Onboarding adds `account.add` and `account.remove`, both `owner`. `account.add` takes the settings the owner
entered or saw (address, names, user name, the IMAP server) and never a password; it saves the account under an id
no account had in this session and queues the owner's first check. The front end puts the password into the OS
credential store (`ui_egui::onboard::Keychain`; `keyring` in the desktop app) under the account's id, and the
worker reads it from there each time it signs in, so a password never passes through a command, an answer, the
store or the log. In the desktop app the entry is named with the store's own random id as well
(`account-<store id>-<account id>`, `Sqlite::id`), so two stores on one computer never share an entry. The desktop
app keeps its accounts in the store on disk, so their passwords stay when the window closes; a front end whose
accounts are forgotten removes them then (`InboxApp::on_exit`). A new account's first check that signs in queues
its folder list, so the sync starts after the password was accepted and not before. `account.remove` removes the account with its mail and drops its state and
queued jobs. Each frame the window hands the queued jobs to the app's workers (`ui_egui::workers::Workers`; one
`net::Worker` per account, started at its first job, stopped off the window's thread when its account is gone) and
applies their events; Detect runs on a thread of its own in the window, outside the engine.

Folder roles come from `model::RoleMap`: SPECIAL-USE first, then the English and Hungarian names, then the owner,
asked once per role. Inbox, Sent and Drafts stay fixed once set; Trash, Junk and Archive can be changed.

## Decisions worth knowing
- The engine does no I/O; protocols are sans-I/O (`proto`, `dav`); sockets and TLS live only in `net`.
- No async runtime; one worker thread per account (`net::Worker`). It opens one connection at its first job, signs
  in, runs the jobs in order and opens folders read-only for the read path (EXAMINE, `BODY.PEEK`), so reading changes
  nothing on the server; only the write jobs open a folder with SELECT. Answers stream out as events while they come
  in. Jobs wait in an offline queue while it reconnects after a back-off with a cap. Between jobs it watches one
  folder with IDLE, reading in short ticks so a job ends the IDLE; every change, and every reconnect, brings a resync
  of the folder from the state the last sync left (QRESYNC where offered), so a change seen live and one made while
  offline are reported the same way.
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
  Code that works in the owner's wall-clock time takes an `inboxcraft_time::Offsets` the same way: a fixed `Offset`,
  or an `OffsetTable` of changes, which the summer-time tests use.
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
  message, draft, alert, calendar or event removes what hangs below it. A schedule entry hangs below the draft,
  message or alert it names, so it never acts on a later record that got the same id. Senders are kept across
  accounts. The store has an id of its own, random and kept for good (`Sqlite::id`). The desktop app keeps its
  store in `inboxcraft/store` in the owner's data folder (`%LOCALAPPDATA%` on Windows, else `XDG_DATA_HOME`,
  `~/.var/app/<app id>/data` in the Flatpak); the sample stays in memory.
- Kept jobs: a command that queues a job with `Outcome::with_kept_job` (the acts on mail) writes it into the
  store's job queue in the same batch as its changes, so a kill keeps both or neither. The job leaves the queue with
  its last event (the answer, or `Failed`); `Session::start` hands the jobs an earlier run left over again, under
  their ids, before the start's own. Read jobs are not kept: the start syncs again anyway. Only a job with a last
  event is kept: a check or a watch runs in its session only, and one found in the queue is removed at the start.
- Single instance and background mode (`apps/inboxcraft`): a start on the owner's mailbox claims the store's folder
  (`instance`): the first holds `instance.lock` (the system's file lock, gone with the process) and listens on
  `instance.sock` (Unix); a later start sends `show` (or `quit` for `--quit`) and leaves, so two runs never open
  one store. Closing the window closes it for real (a Wayland window cannot be hidden) while InboxCraft keeps
  running (`desktop::Desktop`): without a window the main thread waits for requests and pumps the workers every
  half second; a request opens a new window on the same `InboxApp` on eframe's kept event loop. `--background`
  starts that way. The sample, or a run nothing could bring a window back to (no socket, no tray), ends with its
  window. `ui-egui` knows none of this: the app's own `eframe::App` wrapper calls `Desktop::tick` from
  `App::logic`.
- Tray (`tray`, Linux): a StatusNotifierItem over the session bus (`ksni`) with the unread count of every
  account's inbox in its tooltip, a click to show the window and a menu with Open and Quit; each is a request to
  the window, as from a later start. In the Flatpak it goes by its connection's name.
- Autostart (`autostart`): an XDG autostart entry outside the Flatpak; inside it the Background portal
  (`RequestBackground`, over `zbus`), which each start asks again so a desktop that ends background apps lets
  InboxCraft run, with the same entry in the app's own config folder as the record of the choice.
- Notifications (`notify`, Linux): `Desktop` takes the session's notices after each pump and shows them through the
  notification portal in the Flatpak or `org.freedesktop.Notifications` elsewhere (`notify::bus`, over `zbus`), with
  the body escaped where the server reads markup and the buttons (Done, Mark read) only where it shows actions. A
  thread per connection listens for the clicks on what was shown; each comes to the window as a `Click`: a click
  opens the message in the full-width reader and brings the window forward (or opens one), the buttons run the same
  acts as the window. A later notice with the same key (the count of an account) takes the place of the first.

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
