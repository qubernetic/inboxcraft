# Architecture

> **Last reviewed:** <date> · **Last updated:** 2026-11-08 · **Change:** minor

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
batch. A stage is a file of its own in `crates/engine/src/arrive/` and one name in the `stages!` line: the bounce
stage is the first, then the screener, snooze, follow-up and the notify stage last; the calendar slices bring more.

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

Follow-up reminders (`mail.follow_up`, `safe`, in `engine::cmd::triage`; the stage in `engine::arrive::followup`):
"remind me if no reply by" a time, given as `until` or a preset of `schedule.presets`, on a message the owner sent (in
a Sent folder, or from one of the owner's own addresses). The reminder is ours alone and sends nothing to the server:
the message's state keeps the time (`follow_up`, said on each `mail.list` row) and a schedule entry fires it. The
follow-up stage, after snooze and before notify, cancels it when a reply arrives while it waits: a message whose
`In-Reply-To` or `References` names the sent message's Message-ID, not from one of the owner's own addresses and not
in a Sent or Drafts folder. At the time `Session::tick` fires the entry, clears the time and adds a reminder notice
that opens the sent message. `mail.cancel_follow_up` (`safe`) takes a reminder off; `history.undo` takes either back,
time and schedule entry included.

Notifications (`engine::notify`, commands in `engine::cmd::notify`): the engine decides and the front end shows. The
notify stage, after the screener, tells of a message new in an inbox after the folder's first sync that is unread,
still in the inbox after the screener (held or blocked mail tells nothing), not from one of the owner's own
addresses, and not the owner's own move coming back: a moved message a server names no UID for is remembered by its
Message-ID (or sender, subject and date) when it went to an inbox, so the sync that brings it back, as after
`screener.accept`, tells nothing. The account's level (`notify.set`, `owner`; `notify.settings`, `safe`) has the last
word: `all`, `people` (a sender known or accepted whose message is in People, by the owner's group for the sender or
else the classifier) or `none`; `notify::tells` is the decision table. `Session::tick` adds a notice for a returned
snooze, a follow-up reminder and a calendar alert unless the owner turned that off (`returns`, `reminders`,
`alerts`). An alert's notice names its account, calendar and event, the occurrence's start and end, all-day or not,
and the event's title and place as clean lines; one that went off while InboxCraft was off tells late (`missed`),
and not at all once the event is over. The session queues the notices (at most 100, the oldest go) until the front end takes them
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
`Context::next_schedule_id`.

Event alerts (`engine::calendar::alert`): an event's `DISPLAY` and `AUDIO` alarms, or without any alarm the defaults
(ten minutes before a timed event, a day before an all-day one), go off at times the store holds as `Alert`s, each
with a schedule entry. A trigger's days follow the wall clock of the event's zone (the computer's for a floating
time or a whole day) and its hours do not, so a day before noon is noon the day before across a change of the clock.
A cancelled event, one the owner declined and an alarm further than a year from its event alert nothing. The store
holds the alerts of the next two weeks, at most 64 an event: `Session::commit` works them out for every event a
batch puts, in the same batch (an alert at its old time keeps its id and entry), `Session::start` and then a sweep a
day (`Session::next_due` counts it while a calendar is held) for every event held, so a repeating or far event comes
into the horizon in time; an alert whose time came stays for the tick. A fired alert leaves the store with its
entry.

`schedule.presets` (`safe`) answers the presets still ahead: later today (three hours
on, rounded to the nearest half hour of the wall clock, while still today), this evening (20:00) and tomorrow
morning (08:00). They are in the computer's wall-clock time, from the `inboxcraft_time::Offsets` the front end
hands over with `Session::set_offsets` (UTC until then); a wall-clock time the clock skips in spring lands after the
change, one it shows twice in autumn is the first. `schedule.list` (`safe`) lists the entries and `schedule.cancel`
(`owner`) takes one off.

The calendars in the engine (`engine::calendar`, `cmd/calendar.rs`) keep each event as the iCalendar text the server
sent and read it with `ical` whenever it is shown, so nothing of it is lost on the way back. The start, and
`calendar.sync` (`safe`), ask each account with a CalDAV address (`calendar.server`, `owner`) for its calendars; the
`CalendarReport`s land in the store's calendar tables: a calendar or an event keeps its id by its URL, a sync token is
kept beside its calendar, an event without an entity tag is not named as known so the next sync brings its tag, and a
conflict (412) starts the calendar afresh so the server's copy replaces ours. `event.list` (`safe`) expands every event
in a span of at most 400 days on the computer's clock: each occurrence with its moments in UTC and on the wall clock,
its first and last day, its repeat preset, and the event's own zone with its wall-clock times when that clock shows
another time than the computer's. `event.create`, `event.update` and `event.delete` (`owner`) change the store at once
and write to the server as kept jobs (`Put` new or over the copy seen, `Delete`); a new single event is written in UTC,
a repeating one on the computer's wall clock (floating) so it keeps its time of day across a summer-time change, and an
edit keeps the event's own form (its zone, the wall clock or UTC). A delete is a kept job the session holds back for
the undo window (`Outcome::with_held_job`, six seconds, as long as the toast with Undo shows): `Session::tick` hands it
to the worker once the window is over, `history.undo` before then withdraws it from the queue and puts the event back,
and a restart hands it out at once. Writes to one event go out one at a time (`engine::writes`): a second write (an
edit, an undo, a delete whose window is over) waits until the server has taken the first, then goes with the entity
tag that one got, or, when the server sent none, the tag the next sync of its calendar brings; a sync meanwhile keeps
the owner's copy. So the owner's changes never conflict with each other. Editing an event with guests counts its
`SEQUENCE` on only when the owner asks to tell them (`notify`); every organiser and guest InboxCraft writes carries
`SCHEDULE-AGENT=CLIENT` (RFC 6638) so a server does not mail them on its own, and the attendee parameters `ical` does
not read go back as they came. Clearing an account's CalDAV address, or moving it to another origin (host or port),
drops the account's calendars and every calendar write of it in the queue; a discovery carries the address it is for,
the app starts a worker afresh when an account's servers change, and a `Found` from another origin, a `Found` for an
account without an address and an event URL outside its calendar's collection are dropped.

Invitations in mail (iTIP, RFC 5546, in mail, iMIP, RFC 6047) are read once a message's body is kept: the sync hands
the body to `arrive::invite`, which reads its first `text/calendar` part of its own with `ical` and marks the message
(`MessageState::invitation`: the method, the event's `UID`, `RECURRENCE-ID`, `SEQUENCE` and `DTSTAMP`, the title,
times, repeat rule and place, and the owner's answer); mail in the Blocked folder is not read, and a body kept before
is read when its card is first asked for. Marks with the same `UID` and `RECURRENCE-ID` in one account are versions of
one event, the later one the higher `SEQUENCE`, then the later `DTSTAMP`. `invite.show` (`safe`) gives the card: the
version before and what changed since, whether a later version made it out of date, the answer given, the event in
the owner's calendars by its `UID`, and whether it can be answered. `invite.answer` (`owner`) builds the owner's
`METHOD:REPLY` (only the owner among the attendees, with `PARTSTAT`) into a draft with the iTIP object as its
`text/calendar; method=REPLY` part beside the text (`Draft::calendar`, `MessageBuilder::calendar`), sends it at once
with the checks `mail.send` makes but without its hold (`cmd::send::sending`; the answer is taken back by changing
it, not by `mail.unsend`), and writes the event into the account's first calendar
(going or maybe; a copy held, a server's own included, is changed in place and keeps its reminders) or deletes it
(not going), without a `METHOD` and with `SCHEDULE-AGENT=CLIENT`. `invite.remove` (`owner`) deletes the event a
cancellation is about as `event.delete` does, with its undo window. Nothing in an invitation changes the calendar on
its own.

Drafts (`engine::draft`) are kept in the store at every change, so a kill loses nothing typed. `compose.new` starts
one from the account that last sent (the newest message in a Sent folder, else the first account) with that
account's signature below `-- `; `compose.update` changes the fields it is given and keeps the draft at once;
`compose.show` and `compose.list` read them; `compose.discard` removes one here and its copy on the server where
the server named it; all
`safe`. A draft on its way (its outbox item waits for the worker's answer) is neither changed nor discarded, so the
answer finds it as it went; the draft of an answer to an invitation (`invite.answer`) is not changed either, so it
cannot go elsewhere or into the Drafts folder. `compose.signature` (`owner`) sets an account's signature. Recipients are kept as typed and checked as
`Name <address>` or a bare address: one that cannot be an address is marked with what is wrong, and it blocks Send
(`blocks`), like a draft without a recipient. `mail.thread` lists the drafts that answer a message of the thread.
`Session::tick` hands a draft that changed and has been still for ten seconds to its account's worker as a kept
`Append` to the Drafts folder (`\Seen \Draft`, without Bcc and the recipients that are no address), and
`Session::next_due` counts it. Where the server has UIDPLUS, the `Appended` answer names the copy's UID; the copy
before it is then removed for good (`Expunge`), and a copy of a draft discarded or moved to another account meanwhile
goes too. An answer without a UID marks the draft (`Draft::unnamed`): that copy can be neither replaced nor taken
back, so no further copy of the draft goes up to that account and it leaves at most one there. A draft with nothing
typed, or of an account without a Drafts folder, stays here; a failed upload waits for the draft's next change.

Replies and forwards (`engine::reply`) are drafts made from a message, from the account that holds it, all `safe`.
`compose.reply` goes to the message's `Reply-To`, else its `From`, and a reply to the owner's own message to its `To`;
`compose.reply_all` adds the other `To` and `Cc` recipients, each address once and none of the owner's own: the
address and the login (where it is an address) of each account. A message's `Bcc` is never read. The subject gets `Re: ` or `Fwd: ` unless it starts with `Re:`, `Fw:` or `Fwd:` already. A reply keeps
`In-Reply-To` (the message's `Message-ID`) and `References` (the message's `References`, else its `In-Reply-To`, then
its `Message-ID`; only ids the builder can write, of a long thread the first and the newest 19) in the draft, so the
sent message and its Sent copy join the thread (`model::group_threads`). It quotes the message's plain text, or its
preview, below a line of our own (`Name <address> wrote on 5 Nov 2026 at 08:00:`), above the signature.
`compose.forward` puts the text inline below the original's sender, date, subject and recipients and names the
message in `Draft::forwards`: its attachments are read from the kept body whenever the draft is built (its Drafts
copy, the send), each with its own media type where the builder writes it, else `application/octet-stream`. A
forward needs the body kept; one whose body has gone since shows the block `forward_body`, and `mail.send` refuses
it.

Templates are kept on their account (`Account::templates`, in the account record): `template.list` (`safe`),
`template.save` and `template.delete` (`owner`, a setting). `compose.template` (`safe`, `engine::template`) puts one
into a draft: its text in front of the draft's, its subject where the draft has none, `{name}`, `{first_name}` (the
first `To` recipient's) and `{my_name}` (the account's display name) filled in. A blank (`{?prompt}`), and a name
that is not known, becomes a mark (`[[prompt]]`) the draft remembers in `Draft::blanks`; while one is still in its
subject or text, `blocks` holds `blank` and Send is refused. A mark the draft or a filled-in name holds already
(from a quote, or a sender's name) gets a number, so the sender's words do not keep Send blocked.

Sending (`cmd::send`, `engine::send`): `mail.send` (`owner`) puts a draft that nothing blocks in the outbox
(`Outgoing`, state `Waiting`) with a schedule entry (`Due::Send`) at the end of the owner's hold (`Hold`, 5 to 60
seconds, 5 by default; `send.set`, `owner`, and `send.settings`, `safe`; the front end keeps it and hands it back
with `Session::set_send_hold`); `mail.send_later` (`owner`) does the same for a time or a preset of
`schedule.presets`. Until then `mail.unsend` (`owner`), or `schedule.cancel` on the entry, takes the send back: the
outbox item and the entry go, the draft was never touched and is answered as it is. A waiting draft is neither
changed, discarded nor copied to the Drafts folder. When `Session::tick` fires the entry, the draft is made into a
message with its own `Message-ID` and a `Date` of that moment in the computer's offset, its outbox item becomes
`Sending` (the message kept for the Sent copy) and a `Send` job is queued with the envelope: the account's address
and the To, Cc and Bcc addresses, each once; a draft that can no longer go then (no submission server, a forward's
body gone) is `Refused` with why. Nothing is sent while InboxCraft is not running: a send waiting at a stop stays in
the store, its time missed fires at the next start's first tick (late, `Fired::missed`), and the owner is told
(`Notice::SentLate`). The window ticks from its pump (`Session::tick_if_due`, which keeps the next due time until
something changes) and asks egui for a frame at that time (`Session::due_at`, `InboxApp::wake_when_due`), from its
drawing and from the logic eframe runs for a hidden window, since an idle window draws no frame of its own. Bcc is in the envelope alone, never in a header. The job is not kept in the job queue: a
send cut off may have gone, so nothing sends it again on its own. The worker sends it at once on a submission
connection of its own (`net::SmtpConnection`, implicit TLS on 465 or STARTTLS on 587, AUTH PLAIN or LOGIN only after
TLS, `EHLO [127.0.0.1]`), also while the IMAP connection is down, and answers `Sent` or `SendFailed` with the cause
and the server's reply. A connection that ends after the message went and before the server answered it is
`SendUnanswered`: the message may have gone. On `Sent` the draft and its outbox item go, its Drafts copy is removed for good and the
message is appended to the Sent folder (`\Seen`), both kept jobs; on `SendFailed` the draft stays and its outbox
item holds the reply (`Refused`), and `mail.send` can send it again, held again. `outbox.list` (`safe`) shows each outbox item with
where it stands (`waiting`, `sending`, `refused`), a refusal with the server's reply, its enhanced status code
(RFC 3463) and whether it is permanent (a `5.x.x` code, or a `5xx` reply without one); `outbox.retry` (`owner`)
sends a refused draft again as `mail.send` does, and refuses an item that is not refused; a waiting item shows the
time it goes (`at`).

Bounces (`engine::arrive::bounce`): a message whose header is a delivery report (`Signs::report`: `multipart/report`
with `report-type=delivery-status` or `global-delivery-status`) from a sender of reports (local part `MAILER-DAEMON`
or `postmaster`, the screener's own rule) passes the screener, which comes after the bounce stage and lets every
report it found in without learning its sender; a report header from anyone else is screened like any mail. Its
body is fetched at once unless it came with a folder's first sync or into a folder whose mail teaches the screener
nothing or the Sent folder. Once the body is
kept, the report's returned header (`Part::reported_message_id`: `text/rfc822-headers`, `message/rfc822` and their
RFC 6533 forms) names the message by `Message-ID`; a message with that id in the Sent folder of the account the
report came to gets `MessageState::bounced`: the report and the reason, each recipient the report says failed with
the remote server's reply or the status code, at most 1000 characters. Only failed recipients the Sent copy names in
To or Cc count (by `Final-Recipient` or `Original-Recipient`); delayed, delivered and relayed recipients mark
nothing, and a report held by the screener or blocked links nothing. `mail.list` and `mail.read` show it as
`bounced`, naming the report only while that id is still a report of the same account (ids are given again). After the IMAP server refused the password, the
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
- CalDAV in `dav`, sans-I/O over HTTP/1.1: `Discovery` (the well-known path, the principal, the calendar home set),
  `Sync` (a sync report from the last token, or a listing of entity tags where the server has none, then
  `calendar-multiget`), and `put` and `delete` with `If-Match` or `If-None-Match: *`, so a change made elsewhere comes
  back as a conflict and is never overwritten. Each exchange is a state machine: `request()` gives the next request
  for its origin, `receive()` takes the `Response` that `ResponseReader` read. Basic auth is written into a request to
  an `https` origin only, and a redirect to another origin is named, never followed. Its XML reader is its own, with no
  document type declarations; the fake CalDAV server in `testkit` reads requests with readers of its own.
- The calendar jobs (`model::CalendarJob`: discover, sync, put, delete) run in `net` (`crates/net/src/dav/`) like a
  send: at once, on an HTTPS connection of their own, kept alive for the job's requests, never in the IMAP queue. Each
  answers with one `model::CalendarReport` in the shape of the store's calendar tables: the calendars found, or what
  changed with the next sync token, or the new entity tag, or a conflict (412). Every place is an absolute `https`
  URL on the origin of the account's CalDAV address (`Account::caldav`), where discovery starts; a job URL elsewhere
  fails with `OtherServer` before the password is asked for. After the CalDAV server refused the
  password, the worker sends it nothing until the owner's check finds the calendars again; after the IMAP server
  refused it, nothing either. The account check finds the calendars as its last step, with the password the IMAP
  sign-in used.
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
  window, and so does the owner's when they turned "Run in the background" off: `desk::OwnerDesk` shares
  `Desktop`'s flag and keeps the choice in `background.json` beside `appearance.json`, read at the next start.
  `ui-egui` knows only the `ui_egui::desk::Desk` seam the Notifications page draws (`InboxApp::set_desk`): the
  app's own `eframe::App` wrapper calls `Desktop::tick` from `App::logic`.
- Tray (`tray`, Linux): a StatusNotifierItem over the session bus (`ksni`) with the unread count of every
  account's inbox in its tooltip, a click to show the window and a menu with Open and Quit; each is a request to
  the window, as from a later start. In the Flatpak it goes by its connection's name.
- Autostart (`autostart`): an XDG autostart entry outside the Flatpak; inside it the Background portal
  (`RequestBackground`, over `zbus`), which each start asks again so a desktop that ends background apps lets
  InboxCraft run, with the same entry in the app's own config folder as the record of the choice. The
  Notifications page's "Start with the computer" changes it through `OwnerDesk`: at once outside the Flatpak; inside
  it on a thread of its own, since the portal may ask the person, with the window woken by the answer.
- Notifications (`notify`, Linux): `Desktop` takes the session's notices after each pump and shows them through the
  notification portal in the Flatpak or `org.freedesktop.Notifications` elsewhere (`notify::bus`, over `zbus`), with
  the body escaped where the server reads markup and the buttons (Done, Mark read) only where it shows actions. A
  thread per connection listens for the clicks on what was shown; each comes to the window as a `Click`: a click
  opens the message in the full-width reader and brings the window forward (or opens one), the buttons run the same
  acts as the window. A calendar alert says when its occurrence takes place on the window's clock and where; a
  click on it opens the calendar on the occurrence's day with the event picked (`InboxApp::open_event`). A later notice with the same key (the count of an account) takes the place of the first.

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
| 2026-11-08 | minor | The local records, the desktop store on disk and kept jobs (#113) |
| 2026-11-08 | minor | The write jobs, the acts on mail with undo, and the arriving-mail pipeline (#114) |
| 2026-11-08 | minor | The IMAP write path in `net` (#115) |
| 2026-11-08 | minor | Drafts, the compose commands and the Drafts folder copy (#116) |
| 2026-11-08 | minor | Sending: `mail.send`, the `Send` job, the submission connection and the answers (#119) |
| 2026-11-08 | minor | Replies, forwards and templates (#120) |
| 2026-11-08 | minor | The outbox commands and the bounce stage before the screener (#121) |
| 2026-11-08 | minor | The scheduler, its presets and the `Offsets` hook (#122) |
| 2026-11-08 | minor | Undo send, send later, missed sends and the window's tick (#123) |
| 2026-11-08 | minor | The screener, the known-sender index and the `screener.*` commands (#124) |
| 2026-11-08 | minor | The grouped inbox, its classifier and the `group.*` commands (#125) |
| 2026-11-08 | minor | Done, park and pin, and their commands (#126) |
| 2026-11-08 | minor | Snooze: the copy with our own header field, the stage, the return; the `Abilities` event (#127) |
| 2026-11-08 | minor | Follow-up reminders: the commands, the stage that cancels, the notice (#128) |
| 2026-11-08 | minor | The window no longer uses `sample.mail.set` (#131) |
| 2026-11-08 | minor | Single instance, background mode, the tray and autostart (#137) |
| 2026-11-08 | minor | Notifications: the notify stage, the decision table and the desktop delivery (#138) |
| 2026-11-08 | minor | The desk seam: running in the background and starting with the computer from the window (#139) |
| 2026-11-08 | minor | CalDAV in `dav`: discovery, sync, writes with entity tags, Basic auth over `https` only (#144) |
| 2026-11-08 | minor | The calendar jobs in `net`: discover, sync, put and delete on an HTTPS connection of their own (#145) |
| 2026-11-08 | minor | The calendars in the engine: reports applied, the event commands, the held delete, one write per event at a time, scheduling left to the client, the old server's answers dropped (#146) |
| 2026-11-08 | minor | Invitations: the mark, `invite.show`, `invite.answer` and `invite.remove` (#149) |
| 2026-11-08 | minor | Event alerts: the store's alerts, the sweep, the notice and its click (#150) |
