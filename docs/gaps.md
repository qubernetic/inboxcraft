# Where InboxCraft falls short

> **Last reviewed:** <date> · **Last updated:** 2026-11-21 · **Change:** minor

## Ranked
| # | Gap | B | Evidence | Impact | Hours | Doc |
|---|---|---|---|---|---|---|
| 1 | No front end hands the engine the computer's time zone: presets, send later, snooze, reminders and the calendar run in UTC | ✓ | `Session::set_offsets` has no caller | "Tomorrow 8:00" and every event are off by the UTC offset, an hour or two in Europe | 4 | |
| 2 | The composer has no attachments | ✓ | `ui-egui` has no attach field (#132); a draft closed earlier opens again from the drafts place (#314) | A mail with a file must go through another program | 6 | |
| 3 | Only the inbox is watched (IMAP IDLE, no NOTIFY); other folders change when the engine syncs them | | `net` watches one folder per account (#26) | Mail filed by a server rule or another device shows late | 6 | |
| 4 | Sign-in with a password only: no OAuth (XOAUTH2, OAUTHBEARER) for IMAP, SMTP or CalDAV | | `proto` and `dav` have no OAuth (#20, #144) | Providers that require OAuth cannot be added | 12 | |
| 5 | One message at a time: no selection of several messages | | `ui-egui` acts on the open message or the hovered row (#131) | Clearing a full inbox takes one act per message | 6 | |
| 6 | A flag changed here overwrites one changed on another device meanwhile | | `net` sends no `UNCHANGEDSINCE` (#115) | Rarely loses a flag change made elsewhere | 3 | |
| 7 | Repeating events change and delete as a whole series only | | no `RECURRENCE-ID` writes (#146, #148) | Moving one meeting of a series moves them all | 8 | |
| 8 | Search looks at subject, sender and recipients only, not the body text | | the search field runs `mail.search` per field (#27) | A message is found only by its header | 6 | |
| 9 | Tray, notifications, single instance and the control channel exist on Linux only | | no Windows or macOS code (#137, #138, #142) | InboxCraft is for Linux desktops for now | 30 | |

B = blocks the next stage (beta: the owner uses InboxCraft daily on both mailboxes). Hours are estimates of agent
time from the code as it is, not measured.

## Known rough edges (not ranked)
- `inboxcraft-web` answers `--version` and refuses everything else (#5).
- The app crates have no coverage floor of their own; their `main` counts only for its crate, which has none (#7).
- `cargo xtask ci` runs the tests only in the coverage-instrumented build (`cov` gate); a bug that shows only in
  an uninstrumented build goes unseen in CI (#66).
- `testkit` serves one client per fake server. A line script speaks implicit TLS only and matches whole lines
  exactly (#8); the fake IMAP and SMTP servers also speak STARTTLS (#23, #118).
- `testkit`'s `TestClient` checks certificates with the rustls WebPKI verifier and the test trust store, not the
  OS verifier `net` will use. It knows its clock is far off because the test sets the offset; the real
  `account.check` needs another reference for the clock (#8).
- The live tests (`crates/net/tests/live.rs`) cover the sign-in of both accounts, the IMAP round trip, the write
  path in the run folder, the account check, one message sent over SMTP and the CalDAV calendar list. Deleting the
  run folder leaves the empty `InboxCraft-Test` parent behind (#10).
- `just it-live` reads the credentials with `secret-tool` (Secret Service) or `security` (macOS Keychain) and needs
  bash; Windows Credential Manager is not supported yet. The leak scan of its output looks for the two secrets, the
  two addresses and the two login names, each verbatim and as Rust's `{:?}` prints it, not for encoded forms such
  as base64 in an `AUTHENTICATE` line. It runs after the output is on the screen, so it reports a leak and does not
  hide it; a short login name that also occurs in ordinary output fails the run (#10).
- A server without a hierarchy delimiter (`NIL` in `LIST`) has no run folder under `InboxCraft-Test`;
  `RunFolder` needs a delimiter (#10).
- `time` has no IANA zones and no lookup of the computer's own zone yet: `Offsets` takes a fixed offset or an
  `OffsetTable` someone filled, so a date shows in the sender's offset or UTC and the engine works in UTC until a
  front end hands it the computer's offsets. iCalendar text is read by `ical`, whose zones come from the
  calendar's own `VTIMEZONE`s; ISO 8601 text is not read yet (#15, #122, #143).
- RFC 5322 dates are read leniently where RFC 5322 section 4.3 allows it or real mail needs it: the day of the
  week is not checked against the date, second 60 reads as 59, and a missing zone, a military letter or an unknown
  zone name (`CET`) reads as `-0000`. Text after the zone other than a comment (`+0100 CET`) is refused (#15).
- `testkit`'s certificate `Clock` still reads the wall clock (through the `time` crate of crates.io), because the
  test certificates are made valid around the real now; it is not an `inboxcraft_time::Clock` (#15).
- `fuzz/` resolves its dependencies afresh on every run: it has no committed `Cargo.lock` and `cargo deny` does
  not see it (#15).
- `charset` reads the WHATWG charsets only: UTF-7 (RFC 2152), `unknown-8bit` and `x-unknown` are unknown labels,
  and ISO-2022-KR, ISO-2022-CN and HZ are refused as unsupported. Text in them is not shown yet; `mime` decides
  what to show instead (#16).
- Encoded words are read more leniently than RFC 2047 allows, as real mail needs: words inside a longer word
  (`Re:=?utf-8?q?x?=`), in quoted strings and longer than 75 characters, charset labels with any printable ASCII
  but `?` and `=`, and base64 without its padding. Text decoded from a word is not cleaned: it may hold CR, LF or
  control characters, which the reader and the header writer must handle (#16).
- `encode_text` writes UTF-8 words only, and does not fold the header line: a long plain part stays as long as it
  is, and folding at the spaces between words is left to the `mime` header writer (#16).
- Folder roles by name match the whole last part of the path against a fixed English and Hungarian list; other
  languages, and names with extra words (`Sent 2026`), are left to the owner's one question. When several folders
  could take a role, the first in the server's order wins; detection never moves a role once set, even when a
  SPECIAL-USE attribute appears later (#17).
- Mail addresses are compared without case in the local part too, which RFC 5321 does not promise; a header
  entry without a usable address (`undisclosed-recipients:;`, a bare name) is not an `Address`, and `mime` decides
  what to show for it (#17).
- A mail address with an invisible direction or zero-width mark (U+200B to U+200F, U+202A to U+202E, U+2060 to
  U+2069, U+061C, U+FEFF) is refused, so two addresses cannot look the same; that also refuses the rare
  internationalised domain that needs a zero-width joiner or non-joiner (#17).
- A folder name keeps such marks: the folder is the server's, and refusing its name would hide it (a Persian name
  may need a zero-width non-joiner). The sidebar must isolate each name and show the marks, or two folders can look
  the same (#17).
- Threads link by `Message-ID`, `In-Reply-To` and `References` only: a reply from a program that sends none of
  them starts a thread of its own; subjects are not used (#17).
- `model` caps what one message keeps (1000 addresses per field, 64 references, 64 keywords, subject 1000 and
  preview 256 characters) but not the size of a body or of a folder list in an event; `net` sets those caps (#17).
- `mime` reads no address lists, `Message-ID` or `References`: the read path takes those from the server's
  envelope (#18).
- `MessageBuilder` makes no `Message-ID` or `Date` of its own and requires none; the composer (E2) has to set both.
  It writes no `Bcc` (blind copies belong to the SMTP envelope), no group syntax, no `multipart/related` with inline
  pictures, no `Content-ID` and no `name` parameter on an attachment's `Content-Type`; every attachment is base64,
  so it refuses `message/rfc822` and every other `message` type but `message/global` (RFC 2046 allows those only
  in an identity encoding): forwarding a message as an attachment needs a writer of its own. Addresses outside ASCII are written as given (RFC 6532), so such a message needs a
  server with `SMTPUTF8`; the SMTP client says `SMTPUTF8` only when `MailOptions::utf8` is set or the sender is not
  ASCII; `net` sets it when the sender or a recipient is not ASCII (#19, #117).
- `MessageBuilder` keeps a display name of printable ASCII as a quoted string and writes any other as `B` encoded
  words, which a person reading the raw source cannot read; the text of a long word without spaces becomes encoded
  words throughout so that it can be folded (#19).
- `mime` refuses what it cannot read safely: a message over 128 MiB, a header block of more than 2000 fields or a
  field over 256 KiB, nesting deeper than 20, more than 1000 parts, a multipart without a boundary, with a boundary
  over 70 characters or one never found in its body, broken base64, an unknown transfer encoding or charset, and a
  delivery report with more than 1000 recipients. Any one such part, however deep, makes the whole message an
  error, so a good text part next to it is lost too; the reader has to show such a message as its raw source (#18).
- `Part::file_name` gives the name as the sender wrote it, control characters as spaces and invisible direction
  marks dropped, but path parts such as `../` kept. Whatever saves an attachment must cut the name down to a safe
  base name first (#18).
- A delivery report is read only from the direct parts of the `multipart/report`, never from an attached message
  inside it (#18).
- `mime` reads leniently where mail often breaks: a multipart without its closing delimiter keeps what arrived
  (`Multipart::closed` is false), base64 without padding or in padded chunks reads, an `=` in quoted-printable
  that starts no escape stays as it is, a `Content-Type` that cannot be read is `text/plain`, a line that is no
  field ends the header block, and text declared `us-ascii` that is valid UTF-8 reads as UTF-8 (#18).
- `mime` does not open an attached message sent in base64 or quoted-printable (RFC 2046 forbids it); it stays a
  leaf whose decoded bytes are the message. Text keeps its line ends as written; parameters past the 64th are
  ignored (#18).
- A delivery report keeps its dates as text and does not undo the RFC 3461 `xtext` in addresses; it needs one
  per-message block first, then at least one recipient block with `Final-Recipient`, `Action` and `Status` (#18).
- The `justfile` has no `assets` and `stats` recipes; they run as part of `just ci` (#6).
- `cargo xtask layers` checks direct dependencies only, and knows network and UI crates by name from two lists in
  `xtask/src/layers.rs`; a crate missing from them is not caught (#6).
- `cargo xtask stats` counts files and lines; the number of commands in the registry comes once the registry
  exists (#6).
- `cargo xtask test-files` reads lines, not a syntax tree: a module line inside a multi-line string counts as a
  module (#5).
- The IMAP client in `proto` sends one command at a time and only synchronising literals (no pipelining, no
  LITERAL+). It has no UTF8=ACCEPT, NOOP, ESEARCH, XOAUTH2 or OAUTHBEARER yet; those come when a caller needs
  them and with OAuth (#20, #21, #22).
- `proto` FETCH asks for UID, FLAGS, INTERNALDATE, RFC822.SIZE, MODSEQ and one of `BODY.PEEK[]`, `[HEADER]` and
  `[TEXT]`, and PREVIEW; no ENVELOPE, BODYSTRUCTURE or partial bodies. A FETCH response comes as an event whether a command asked
  for it or the server told of a change; the caller tells them apart by UID (#20).
- `proto` LIST always lists `""` `"*"`; names are decoded from modified UTF-7 only, and a name over 1024 bytes or
  with a control character is left out. A CAPABILITY that the server answers with OK and no list keeps the list
  from before; `RECENT` is only kept during SELECT and EXAMINE (#20).
- `proto` refuses PREAUTH on a connection that is not encrypted, which rules out a server that greets that way
  over a local tunnel (#20).
- `proto` ends the session on an EXISTS, EXPUNGE or FETCH response when no mailbox is open or being opened (RFC 9051
  section 7.4), also before TLS, rather than passing it over: a server that sends one after a refused SELECT is cut
  off. `[ALERT]` text that comes before TLS is dropped, not shown later, and BYE and refusal texts before TLS are
  empty: a server that explains in the clear why it has no STARTTLS is not heard, only that it refused (#20).
- `proto` hides the password as it was given, quoted, trimmed and as the PLAIN response. A server that writes back
  a cut or changed piece of it (a few characters, another case, other escapes) cannot be recognised, so that piece
  shows; a response code is dropped when it holds the password in any case (#20).
- `proto` takes an untagged CAPABILITY that comes during LOGIN or AUTHENTICATE as the list after the sign-in: the
  client cannot tell whether the server sent it before or after it signed the session in. Over TLS that can only
  mean an extension goes unseen (#20).
- `proto` CONDSTORE and QRESYNC: no SEARCH `MODSEQ`, no sequence match data in the QRESYNC parameter, and an untagged `[HIGHESTMODSEQ]` outside SELECT is not
  passed on; the modification sequences of the FETCH responses are. With QRESYNC enabled an EXPUNGE is still taken
  (RFC 7162 says a server must send VANISHED instead). Responses before `[CLOSED]` are passed over only with QRESYNC
  enabled, where the server must send it; without it they count towards the new mailbox until `[CLOSED]` comes, and
  a server that never sends it leaves them there (#21).
- `proto` IDLE: the client does not end an IDLE by itself; renewing it before the server's limit (often 30 minutes)
  is the caller's work, with its clock (#21).
- `proto` NOTIFY asks for one set of events: for the open mailbox new messages with UID and flags (and MODSEQ with
  CONDSTORE), removals and flag changes; for other mailboxes new and removed messages, and the names. No
  `SELECTED-DELAYED`, annotation, metadata or subscription events, no fetch attributes beyond those, and a rename
  comes as the new name only (the `OLDNAME` extended data is not read). STATUS and LIST that come without NOTIFY on
  are passed over; mailbox names in them are server data and, as in a LIST answer, the password is not hidden in
  them (#21).
- `proto` UID MOVE without MOVE: when the server refuses the STORE or the UID EXPUNGE after the copy was made, the
  messages are in both mailboxes and the client does not undo the copy; the answer says so (`Removal::NotMarked`,
  `NotExpunged`) and the caller decides. Without UIDPLUS the messages stay in the source marked `\Deleted` until
  something expunges the mailbox. The client does not check that the source and the destination differ (#22).
- `proto` COPYUID: one that does not fit what was asked for (UIDs not asked for, a UID twice, counts that differ,
  another UIDVALIDITY between the parts of one move) is dropped as a whole, and a COPYUID on the tagged OK of a
  MOVE is only taken when no untagged one came. The client does not check that the destination UIDs are new to the
  destination mailbox (#22).
- `proto` PREVIEW: the text is cut at 256 characters without a mark; it is not searched for the password, being
  message data like a fetched body. No `PREVIEW` search key or `FUZZY` (#22).
- `proto` COMPRESS=DEFLATE: only the negotiation; the stream, its inflate size cap and a server that sends a
  compression bomb are `net`'s (E1.7b). The client cannot tell a server that compresses at the TLS layer already;
  such a server answers `NO [COMPRESSIONACTIVE]` (#22).
- The fake IMAP server knows the UID forms only: FETCH, STORE, SEARCH, COPY and MOVE by sequence number are
  answered `BAD`. It has no RENAME, SUBSCRIBE, LSUB, UNSELECT, NAMESPACE or ID, no
  ENVELOPE, BODYSTRUCTURE, partial or `HEADER.FIELDS` fetch, and no SEARCH by sequence set, KEYWORD, MODSEQ, NEW,
  RECENT or sent date. SEARCH compares arrival days without their zone and text without ASCII case over the raw
  bytes, without decoding encoded words or transfer encodings. `RECENT` is always 0 (#23).
- The fake IMAP server compares mailbox names as UTF-8 and knows no modified UTF-7. Its PREVIEW is the first 200
  characters of the raw body with white space runs made one space, with no transfer decoding or HTML stripping,
  unless the fixture sets one (#23).
- The fake's NOTIFY takes every mailbox as personal and subscribed. It sends a STATUS line for any change to a
  covered mailbox that is not open, and never sends FlagChange for those, MailboxName or SubscriptionChange. The
  open mailbox's changes go out at once only under a SELECTED filter with MessageNew and MessageExpunge; under
  `SELECTED NONE` or without the filter they wait for the next command.
  ENABLE works only in the authenticated state (#23).
- A scripted change to a mailbox that does not exist, or to a UID that is not there, does nothing. Each `on` rule
  plays once. A compressed read that inflates past 1 MiB ends the session; the cap is checked every 64 compressed
  bytes, so the buffer can pass it by about 64 KiB first (#23).
- Between its STARTTLS answer and the client's handshake the fake records at most 16 lines sent in clear; more go
  to rustls, which ends the session. A line past 8192 bytes ends the session and is kept cut at 8192 (#23).
- `store` search: from, to and subject match a piece of the text without case (Unicode lower case, no accent
  folding); body text matches whole words as a phrase, without prefixes, operators or ranking. A word is a run of
  letters and digits; SQLite's `unicode61` tokenizer and the memory backend may split rare characters differently.
  The query language is E5.1 (#24).
- `store` keeps no lock on its directory: keeping a second process away from an open store is the app's single
  instance guard (`apps/inboxcraft/src/instance.rs`). The memory backend copies its rows on every batch, which is fine for tests and
  the browser build but not for a large mailbox (#24).
- `store`: a record whose own id is above `i64::MAX` is refused by the SQLite backend (`IdOutOfRange`); the memory
  backend keeps it. Such an id naming a thing to read, change or remove is an unknown id on both (#276). A source file
  that is missing or damaged is reported as `Corrupt` and is not fetched again by the store itself (#24).
- `store` makes its directories `0700` and its files `0600` on unix, whatever the umask, and narrows wider ones at
  open. The directories above the store directory are left as they are. On Windows it sets no ACL of its own: the
  files inherit the ACL of the directory they are made in, which for the profile directory is the owner's account
  alone; a store put in a shared folder there is as open as that folder (#24).
- `store` removes mail from its own files: removed rows are zeroed (`secure_delete`), removed words leave the FTS5
  index at once (`secure-delete`), the write-ahead log is emptied after a batch that removed mail, and a removed
  source file is unlinked. What the file system or the disk kept of those bytes (journal, copy-on-write snapshots,
  SSD blocks not yet erased, backups) may still be there; only full-disk encryption covers that (#24).
- `store` does not check its paths for symlinks. A link in place of `sources/` is followed, so the sweep at open
  works in the link's target; a link at a `.partial` or `.eml` name is followed when the file is written or read,
  and a FIFO there blocks `body()`. The sweep touches regular files only. Only something running as the owner
  inside the store directory can place such a link, since the directory is `0700` (#24).
- `store` reads a record and a body's text back from SQLite in full before it checks them; SQLite's default limit
  on one value is 1 GB. `Body` cuts the text to `MAX_TEXT_BYTES` only after it was read. A source length is checked
  before the file is read. Only something that writes the owner's database file can plant such a value (#24).
- `net` watches one folder per account, with IDLE; it does not use NOTIFY, so other folders change in the store
  only when the engine syncs them (#26).
- `net` resyncs a known folder without QRESYNC by fetching the flags of every known message. Without CONDSTORE too,
  every one of them is reported in `MessagesChanged`, changed or not, and a large folder costs one flags line per
  message at each resync (#26).
- `net` looks for jobs between reads of a quarter of a second during IDLE, so a job waits up to that long. A
  connection that dies without a word during IDLE is noticed when the IDLE is renewed (25 minutes) and the answer
  to DONE does not come within the read timeout (two minutes) (#26).
- `net` opens and resyncs the watched folder again after every job, also after a job that opened no other folder:
  one EXAMINE and one fetch more than needed. An `Idle` job for a folder this worker has not synced starts with a
  full sync, which reports every message as added again (#26).
- `net`'s back-off has no random spread; a read job that was running when its connection ended is reported failed and
  not run again (the engine decides); a write job runs again after the reconnect, three times at most (#115). The
  worker's offline queue is in memory; a job the engine keeps (`Outcome::with_kept_job`) is also in the store and
  goes out again at the next start, other jobs are gone. A certificate fault, a refused sign-in, a sign-in the server
  never answered (one more try after a dropped or timed-out one) or a server without a sign-in method stops the tries
  until the engine starts a new worker. A connection that drops during the sign-in twice for an ordinary network
  reason halts too, and needs a new worker; a missing password fails the queued jobs and ends the watch, and the next
  job asks again (#26).
- `net` compresses after the sign-in, so no password goes through COMPRESS. What the server sends (mail) is
  compressed together inside TLS; someone who can both put mail in the mailbox and watch the sizes of the encrypted
  traffic could in theory learn something of other mail in the same stream (as with CRIME). `WorkerSettings::compress`
  switches it off (#26).
- `net` keeps at most 10 000 changes and 10 000 removed-UID ranges from what QRESYNC tells while a known folder
  opens (`SyncConfig::max_told`); past it a resync costs the full flags fetch of the known messages. A resync without
  QRESYNC keeps the known messages still there as at most a million ranges (`SyncConfig::max_ranges`): a folder with
  more gaps than that among its UIDs, or a server that answers far out of UID order, fails the sync with `TooLarge`
  (#26).
- The live tests reach the server over implicit TLS on port 993 only; the `live-imap` round trip signs its helper
  client in with LOGIN; a password with characters outside ASCII cannot be quoted there and fails the test. The fake
  serves one connection at a time, so the step where another client appends while the worker idles runs only live
  (#26, #57).
- `net` tells a far-off clock from a bad certificate by the certificate's dates: it ended more than four years
  before the clock's time, or starts more than a day after it, or the clock is before 2026. Where the platform
  verifier gives no dates (Windows, Apple, Android) and checks at the system's own time, only the last test applies.
  A server whose certificate really expired more than four years ago is reported as a clock fault. The check
  against a server's time is `account.check` (E1.8) (#25).
- `net` does not fetch in the first sync a body larger than `proto`'s literal cap (64 MiB); asked for on its own,
  such a message fails with `TooLarge` and the connection closes (#25).
- `net` opens a folder by its name written back in modified UTF-7. A server name that is not valid modified UTF-7
  (taken as it was written by `proto`) is not written back the same way and the folder cannot be opened (#25).
- `net` waits up to two minutes for each read; a server that sends a byte now and then keeps a job running. The
  worker's event channel has no bound, so a slow engine lets events pile up in memory (#25).
- `net`'s `Password` overwrites its bytes when dropped, but `proto` keeps the password to hide it in what the server
  echoes, and `rustls` keeps the sent bytes in its buffers, until the connection is dropped (#25).
- `net` takes PREVIEW only from a server that offers it; others give summaries without a preview until the reader
  makes one from the body. Encoded words in a quoted display name are decoded, as most programs do, though RFC 2047
  says they should not be (#25).
- `net` keeps the Mozilla roots of `rustls-platform-verifier` (CDLA-Permissive-2.0, pulled only on wasm32, where
  `net` is never built) out of `cargo deny`'s graph with `[graph] exclude` in `deny.toml`, rather than allowing the
  licence (#25).
- `engine` keeps the folder roles and the roles to ask the owner about in memory: after a restart they are found
  again from SPECIAL-USE and the names, and a role the owner answered is asked about again. Nothing answers a
  question yet; that command comes with the folder pane (#27).
- `engine` turns an escaped panic into an error only where panics unwind; on wasm32 a panic still aborts. The
  default panic hook still prints the panic's message to standard error (#27).
- `engine`'s `mail.list` reads every message of the view's folders from the store and sorts them by date at each
  call, and `folder.list` reads every message to count them; a large unified inbox costs that much per list until
  the store gives sorted pages and counts (#27).
- `mail.search` looks at the words of a message's plain text part only, so a message with HTML alone is not found
  by its body words; `mail.read` gives the HTML part as text beside the plain one (#27).
- `engine` asks for a body once per message while that `FetchBody` job is out; a job the worker never answers
  (neither the body nor a failure) keeps the next `mail.read` from asking again until the session restarts. A body
  over 64 MiB is not kept and not asked for again until the session restarts, so such a message cannot be read in
  InboxCraft. Opening a
  message marks it opened in the store, so its body is kept, also when an agent opens it (#27).
- `engine` lets an agent run `safe` commands only; there are no per-account access levels, and an agent cannot ask
  the owner to approve an `owner` command (#27).
- `account.check` runs for saved accounts only, as a job of the account's worker: onboarding adds the account
  first (`account.add`) and checks it then; changing the settings after a failed check removes the account and adds
  it again under a new id. Detect is no command: the window runs `net`'s Detect on a thread of its own, so an agent
  cannot run it. The SMTP sign-in of an account without a submission server, and the CalDAV discovery of one without
  a CalDAV address, report "not set" (#28, #34, #119, #145).
- `account.check` starts nothing while a check of the account runs. A check whose worker ended before its last step
  stays running until the workers are started again (`Session::start`), which forgets it (#28).
- After a refused or unanswered sign-in, an agent's `account.check` stops before the sign-in (`needs_owner`) and only
  the owner's check sends the password again; a check that signs in ends the worker's halt. There is no minimum
  interval between an agent's checks of an account whose password works, so a looping agent still signs in once
  per check (#28).
- The TLS step of `account.check` passes when the server speaks TLS up to its certificate; a fault of the
  certificate is the certificate step's. On a STARTTLS port the greeting and the STARTTLS exchange count as part of
  the TLS step (#28).
- The blocked-mail-ports test opens TCP connections to ports 443, 993, 465 and 587 of the address that timed out
  and sends nothing on them; it can take up to four connect timeouts (30 seconds each by default) before it
  answers (#28).
- The clock check compares the clock with the certificate's own dates; there is no second reference such as the
  `Date` of an HTTPS answer yet, so a clock off by less than four years with a certificate expired in that time
  still reads as an expired certificate (#28).
- DNS for Detect reads `/etc/resolv.conf` (Linux, the BSDs, macOS). Windows has no resolver configuration yet, so
  Detect there goes on without MX and SRV records. No EDNS, no DNSSEC, no DNS over TLS or HTTPS; the answer is as
  trustworthy as the network, which is why nothing detected gets a password before the owner has seen it and TLS
  verified it (#28).
- Detect's MX domain is the MX host's last two labels, or three under a two-letter country label: a rule of thumb
  without the public suffix list, so a provider under a longer public suffix is not recognised by its MX (#28).
- Detect proposes no CalDAV address: the CalDAV SRV records and `/.well-known/caldav` at the address's domain
  (RFC 6764) are not asked, and no CardDAV (#28, #145).
- The ISPDB is off by default, so Detect tells Mozilla nothing unless the owner switches it on; with it off, a
  domain whose provider publishes no autoconfig and no SRV records is found only through its own common host names and
  its MX hosts' provider, when one of them answers the probe (#28, #260).

- `html` reads no CSS beyond whether an element is hidden and the few inline declarations and attributes of a
  block's look (alignment, a coloured background, a table's and a cell's width, a link styled as a button): fonts,
  sizes, text colours, background pictures, gradients, `rgba()` and `hsl()` colours and the message's `<style>`
  blocks are ignored, so a message keeps its words and structure but little of its look. Text hidden by other tricks
  (a zero font size, white on white, off-screen positions) is shown (#29, #383).
- `html` keeps no `data:` pictures (shown as unsupported, with their text) and no CSS background pictures, even
  with consent. `srcset`, `width` and `height` are not read, so a tracking pixel the owner allowed is a picture
  like any other (#29).
- `html` resolves no relative link (there is no base address), so a relative or fragment link is refused. Hosts
  are compared as written, without Punycode or the public suffix list: `bank.example` and its subdomains are
  the same site, a look-alike name in another script is not caught. Invisible format characters (Unicode's
  default ignorables) and the full-stop look-alikes are the only characters the comparison reads past; the
  shown text keeps the joiners, variation selectors and tag characters emoji and flags need (#29).
- `html` ignores start tags past 256 open elements and lays tables past 8 deep out as paragraphs; their text
  stays. A one-column table is laid out as its cells' blocks; `colspan` and `rowspan` are not read (#29).
- `--sample` shows an invented mailbox in memory that is forgotten on exit (#30). `inboxcraft` alone keeps the
  owner's store on disk with its accounts, so their passwords stay in the credential store
  (`io.github.qubernetic.inboxcraft`, `account-<store id>-<id>`) until the account is removed in "Change settings" or by "Remove account…" on its form;
  a crash or a store that refuses then leaves the entry behind, and `PRIVACY.md` says how to remove one by hand. An
  entry an older build left (`account-<id>`) is never read again and stays until removed by hand (#34, #113).
- Dates in the list are shown in UTC: the `time` crate has no local time zone yet (#30).
- The search field looks at subject, sender and recipients in every folder of the accounts shown, not at body text
  and not only in the open folder; it runs one `mail.search` per field and merges them. The query language comes
  with E5.1 (#30).
- The arrow keys step through the list only while no control has the keyboard focus; Tab moves the focus through
  every folder and row, and Enter or Space selects (#30, #33).
- A link in a message is never opened: the one browser hand-off is the cleaned copy of the reader's More menu (#388),
  in which a link is text (egui's `links` feature is off, and turning it on is the project owner's call), so a click copies the address and the float over the reader shows
  where it goes. Plain-text bodies show web addresses as text, not as links (#33).
- "Always for this sender" is kept for the session only (`remote.allow` keeps it in the engine's memory) until
  settings are kept on disk. It goes by the sender's address, which a forged `From` can borrow: a forger who
  puts an allowed sender's address on a message has its pictures load when it is opened, and so learns when it was
  opened and from which network address, without the owner being asked (#33).
- Saving goes into the downloads folder (`XDG_DOWNLOAD_DIR` or `~/Downloads`) without a file dialog; the Flatpak
  opens that folder (`--filesystem=xdg-download`) instead of asking through the file chooser portal (#33).
- Pictures: PNG and JPEG only (GIF, WebP and SVG show as a box with their alternative text), at most 8192 pixels a
  side and 8 Mi pixels; remote pictures over plain `http`, or on a host with only local addresses, are not fetched.
  Inline pictures show only from HTML bodies; a plain-text body's image parts are tiles (#33).
- Picture limits: one message asks for at most 32 inline pictures (only Content-IDs it has a part for) and 64 remote
  ones, and holds at most 64 MiB of decoded pixels; a picture past that shows as a box with "no room left". All
  messages together keep at most 256 MiB: a new picture first forgets the pictures of folded messages, and a
  forgotten remote picture is fetched again when its message shows it. Each inline picture is one `mail.attachment`,
  which reads the whole message again, so a large message with 32 inline pictures opens slowly (#33).
- Bold text in a message is set in IBM Plex Sans SemiBold; italic text is slanted by egui, as Plex's italics are not
  bundled (#83, #232). Tables are laid out as a grid without borders, four deep at most (#33).
- `mail.thread` reads every message of every account to group the thread; fast enough for the sample and small
  mailboxes, an index comes with the store on disk (#33).
- The next-message rule applies when the open message leaves the list after a refresh or an act (archive, delete,
  move). Bodies that arrive while a message is open show the next time it is opened (#33).
- The reader cannot be tested on a real double click in `egui_kittest`, so the list has no double-click shortcut to
  full width; the Full width button is the way (#33).
- The window's type is IBM Plex Mono and IBM Plex Sans, Regular and SemiBold, in twelve styles, with Audiowide for
  the product names (#83, #232). Noto Sans Hebrew, Arabic, Devanagari and Thai and Noto Emoji ship behind Plex in
  Regular only, so the SemiBold styles draw those scripts in Regular, and emoji are one colour (egui draws outlines,
  not colour emoji fonts) (#348). Other scripts (CJK and 18 more: Bengali, Tamil, Khmer, Ethiopic and others) are
  drawn with fonts found on the computer, searched on a worker thread once the window has drawn its first frame
  and placed behind the shipped faces; where the computer has none for a script, or a script is not among those
  looked for, they show as boxes.
  Only one sign per script is looked for, so a found font may still lack rarer signs of its script. The folders
  come from fontconfig's files, read by the app with limits (`<dir>`, `<remap-dir>`, `<include>`; nothing else of
  fontconfig's rules); in the Flatpak the host's folders should arrive through the runtime's `<remap-dir>` entries,
  which is not checked inside a Flatpak yet, nor on a real desktop. A configuration or font file swapped for a FIFO
  between the type check and the open blocks the search's worker thread (the window is not affected) (#233).
  The app's one-line labels, lines cut to their room and texts wrapped onto at most eight rows put right-to-left
  words in the order they are read (the Unicode bidi algorithm, each label a paragraph of its own) and are cut at
  their logical end (#356). egui itself has no bidi: text laid out by egui's own labels and text fields, a wrapped
  text of more rows (an open quote) and the reader's message text still stand their words left to right, and the
  cursor and selection in a right-to-left run follow the order of the letters, not the picture. A right-to-left line
  stands at its box's left, as a left-to-right one does; only the common bracket pairs are mirrored (#348, #356). egui's
  Ubuntu Light and Hack stand behind Plex only for the signs Plex lacks; they come
  with the `epaint_default_fonts` crate (Ubuntu Font Licence 1.0, not an OSI licence; allowed in `deny.toml` for that
  crate only, the project owner's to confirm). Most views take the styles through egui's own labels, which carry the
  size but not the line height or tracking; the drawn rows and controls carry all three. egui's text fields still
  take their height from Plex's own line, so the add-account form after Detect runs past the settings sheet and
  scrolls (#232).
- Text size (#83, #86, #87, #231, #341): the text scale (90 % to 130 %) is set on the appearance settings page, by
  Ctrl+Plus, Ctrl+Minus and Ctrl+0, or through `ui.text_scale.set`, and kept on disk; the type is IBM Plex, with no choice. The fit test draws the list, the
  reader, the add-account page and the settings page at 90 %, 100 % and 130 %, in a window 1200 points wide (and the settings page 820 points wide); a narrower window is not checked.
  Spacing and paddings stay
  on the grid at every scale; only text and the heights that hold it grow. Loading the owner's own font file is an
  idea only: it would be an outside file, needing the hostile-input rules (safe parsing, size caps, a fuzz target)
  and handling of missing glyphs.
- Icons (#84, #86, #231, #232): Phosphor Regular, one family and one weight, with no choice; a state never swaps
  in a filled glyph. The design draws an icon `muted`, and `ink` on the current row or a hovered button, never in
  the accent but on a selected icon button; the code still draws some in `cyan_text` (the pin on a row, the icons
  of the settings rows, the add-account steps and the empty list) until those components are restyled. The set has the ids the window and the triage actions need so
  far; a toolbar or a view that needs another meaning adds an id and its glyph. The Phosphor Regular font is bundled
  whole (every Phosphor icon) rather than cut down to the glyphs in use, so a new id needs no new font file. An icon
  is one colour token. The unread dot and the account dots and
  stripes stay shapes, as they are colour markers rather than icons. Loading the owner's own icon pack is an idea
  only (an outside file: safe parsing, size caps, a fuzz target, an SVG rasteriser).
- Appearance (#86, #87, #231): the four `ui.*.set` commands, the file that keeps their result and the settings sheet that
  runs them exist; every section of the sheet opens, and what is not built yet (Calendar, Privacy, About, AI, Plugins, rows on the others) is drawn coming soon (#238). The Accounts page lists the accounts and adds one; an account's row opens its form, where it is changed or removed; a check on demand, its folder roles and the account colours switch are not built, and a row shows no sign-in state (Connection state is drawn coming soon). The page has no command of its own: it opens from the rail's settings button, Ctrl+Comma (#88)
  or `InboxApp::open_settings`. Motion has its three levels on the page (#313); going back
  to no choice (the computer's wish decides) after a choice is possible only through `ui.motion.set` with
  `reduce: system`. Loading the
  owner's own font file or icon pack is an idea only and the page offers neither: both are outside files, needing
  the hostile-input rules (safe parsing, size caps, a fuzz target, an SVG rasteriser for icons) and handling of
  missing glyphs. The file sits in the config folder and is written in the sample window too: it
  holds no mail and nothing about an account, so the sample mode writes these files (the look, the keys and the inbox
  view of `triage.json`, never the screener's mode) and nothing else. A file kept by
  an older version that names an accent, a typeface or an icon set loads, and those fields are dropped at the next
  write. Two windows open at once each keep their own last change; the later write wins.
- Message list (#88, #134): the screener card and the grouped inbox show the engine's data (#134); "done all" on a
  group's head marks the whole group done with one Undo, but none for a group that takes more than the 100 acts the
  undo history holds (#135). In the grouped inbox (cards or grouped list) the arrow keys still step through
  the list in date order, not in the order of the groups, a card shows its first three rows while the keys can select
  one hidden under "Show all", and every row is drawn each frame (the simple list draws only the rows in view). The
  group menu is a right click on a row, Shift+F10 or the Menu key, or the reader's More menu.
- Hover actions (#89, #131): Done, read or unread and Delete act on a real account as on the sample, through the
  engine's commands (#131). Done marks the message read and done (`mail.done`, #135); on a done row under "Show
  done" it is "Not done". Flag and Archive have keys (S, A) but no hover button: the design's hover group is Done,
  read, Delete and Snooze; the hover Snooze opens the time picker at the row (#234). The act lands at once and the
  leaving row's gap closes over 180 ms (#236). The keys (D, A, R, S, Delete, Ctrl+Z) can be changed on the Shortcuts
  page (#130).
- Reader (#90, #131): Done, Archive, Delete and Move to a folder act on the open message, flag and read or unread
  from More (#131); Reply, Reply all and Forward open the composer filled in (#133). Snooze (H) and Pin act on the
  open message (#135). More also holds Full width,
  Close and the next-message rule; Escape leaves full width but does not close the
  reader. `ui_shot` cannot hover a link, so a hovered link has no picture; the UI tests cover it. The thread's earlier messages fold
  to one line, but the open message is the selected one, not always the latest of the thread.
- Components and motion (#85): the card, icon button, pill, tooltip, avatar, badge, hover action group, toast and
  popup exist and are shown in the `ui_shot` gallery; the list uses them since #88 and the reader since #90. The keymap knows the window's actions, the list's and the calendar's keys among them; Escape is still read directly and cannot be changed; the other keys are
  changed on the Shortcuts page (#130). The motion level is set with
  `ui.motion.set` (full, reduced or off) and kept on disk; the Appearance page offers the three (#313). Until each
  level has movements of its own, reduced and off both land every movement at its end at once and only full runs
  today's motion, so the window starts still. The desktop cannot read the system's wish for less motion (GNOME and
  KDE keep it differently and the portal has none), so there the owner's default level holds; the web build has no
  window yet, so nothing reads `prefers-reduced-motion` so far. A still level stops egui's fades and its scrolling to a place, but not its smoothing of a mouse wheel's notches
  (about 0.1 s, fixed inside egui 0.37 with no setting), so a wheel still scrolls with a short glide.
  A menu row shows its key as text, which a screen reader does
  not read with the row's name. Avatars show initials only; pictures of people (from a vCard or a sender's own
  picture) are not read.
- Account colours are six fixed bar colours per mode, the design file's, repeating from the seventh account; two
  of them can look alike at a glance. The stripe repeats what the folder pane says in words
  (#81, #231).
- `ui_shot` fills egui's triangles on the CPU and blends in gamma space, close to but not pixel-identical with the
  GPU window. The window itself was never opened in the agent sandbox (no display); the owner's first launch is
  E1.13c (#30).
- Floats (#82, #205, #231): egui's own menus and tooltips cast no shadow (egui's own shadow is off), so they stand
  out by their colour and edge alone.
- `cargo xtask side-by-side` matches only the height: a picture and its sketch keep their own widths and window
  sizes, the pair carries no labels, and the sketch is averaged down (or stretched up) without colour management.
  It needs the owner's mood sketches in `plan/mood/`; CI has none and does not run it (#81).
- `eframe` without its default features links `libwayland-client` at build time instead of loading it at run time
  (#30).
- The Flatpak is never built in the agent sandbox, which has no `flatpak`; the `flatpak` CI job builds the bundle on
  every full run (#32). Nobody has launched it yet: the owner's first launch is #35 (#31).
- The Flatpak builds with the Rust of the `rust-stable` SDK extension, not the exact `rust-toolchain.toml` version;
  it needs at least the workspace's `rust-version` (1.95). The runtime is pinned to freedesktop 25.08 (#31).
- The metainfo has no releases and no screenshots, which Flathub would ask for; `just flatpak` shows the linter's
  findings and builds anyway. The Secret Service permission is used for the account passwords since #34; the
  Background portal (since #137) and the notification portal (since #138) need none (#31).

- Onboarding (#34):
  - Detect fills in its first IMAP and its first outgoing proposal; the others are not offered to choose from, and
    the form does not propose a CalDAV address. `account.update` and `calendar.server` are `owner` commands, which the
    CLI and the control channel refuse: an account's settings change in the window only (#214).
  - The check's failure sentences and the explanation of a step that did not run are the engine's English text,
    not yet through the i18n macros.
  - The password field edits a buffer made once for 256 characters, so typing never moves it, and the window
    overwrites the whole buffer when the password is added or the page closes. egui's text layout caches hold only
    the mask. The field keeps no undo history (it is emptied after every frame), so undo cannot bring back a cleared
    password or an earlier account's. What remains: egui's undo history holds plain copies for the rest of a frame,
    and those, like any other short-lived copy egui makes, are freed without being overwritten.
  - A new account starts syncing when its first check signs in; a refused first sign-in leaves it unsynced until a
    check signs in. Without a running credential store (no Secret Service on the desktop) no account can be added.
  - Workers stop off the window's thread when their account is removed, so a closing worker can still finish its
    last job for a moment; its late events name an account the session no longer holds and are dropped. Account ids
    are never given twice in a session.
  - The window wakes on each worker event; the events channel has no bound (#25), and the window applies at most
    eight rounds of jobs and events a frame.

- **The app's own window frame** (#104) has round corners (12 points) and a one-pixel outline but no shadow of its
  own, and the corners need a compositor that shows a window's see-through pixels. It resizes from a 5-point band
  at each edge and corner. The command line is the title bar: a drag moves the window where neither the field nor a
  window control takes the press (the mark's column, the gap before the controls), also while the command line's
  results or the small event editor show; a press there leaves them open. The desktop's own snapping and its window menu
  (right click on a title bar) are not offered. Pictures and the web build draw no
  frame, so the screens in `docs/images` show the window's content only.
- **The index** (#104) opens the places Home, Mail, Calendar, Later and Settings. Calendar shows the calendars, or their empty state while there is none (#130, #147). The owner's initials at its foot come from the first account's address, as no account holds a
  name of the person yet.
- **The folder pane** (#104) lists only an account's own folders under it; its inbox, sent mail and the like are
  reached through the unified views, narrowed to the account with the picker.
- The IMAP client hides the password as sent, as LOGIN quotes it and in base64 (the PLAIN token and the password
  alone); it is shown as it comes in any other encoding a server writes it back in (base64 at another offset,
  quoted-printable, percent encoding), with an invisible mark inside it, in another case of ASCII letters (as
  itself or as a base64 token), and as a token cut short (#64).
- `cargo xtask changes --fold` checks everything before it writes, but writes the documents one by one: a write
  that fails halfway (a full disk) leaves the documents before it folded and every fragment in place. A new
  command module, unlike a new command, still adds its name to the `modules!` line in `cmd/mod.rs` (#111).
- `store` keeps senders across accounts: removing an account leaves what is known of its senders, which goes only
  with the data folder (no command removes a sender yet). Schedule entries and alerts are ordered by the second
  they are due, then id; the fraction of a second is kept but not sorted on. Senders are told apart by the address in Unicode lower case, as `EmailAddress` compares them (#113).
- A kept job runs again after a restart under its old id, also when its worker had already run it before the app
  ended: kept jobs are for acts that can be done twice (#113).
- A message moved on a server without `COPYUID` (UIDPLUS) comes back from the next sync of its folder under a new
  id, without what the owner did to it meanwhile; a message still on its way cannot be deleted for good yet. A sync
  of the old folder that runs before the move can show the message there again until the move is done (#114).
- The undo history lives for the session only (#114).
- A write that a connection cut off runs again after the reconnect. APPEND cut off after the server took the message
  but before its answer adds it a second time. A move on a server without MOVE cut off after its UID COPY is not run
  again: it fails and the engine takes it back, but the copy stays in the target folder until the owner removes it
  (#115).
- A delete for good on a server without UIDPLUS (and not IMAP4rev2) fails before anything is sent, so the message
  stays: EXPUNGE there would remove every message marked `\Deleted`. A move on such a server leaves the moved
  messages in the old folder marked `\Deleted` (#115).
- `net` does not use UNCHANGEDSINCE yet: a flag the owner changes overwrites a change made on another device
  meanwhile (#115).
- `proto` takes an unclosed list at the end of a line (`* 1 FETCH (`) in an open mailbox as an empty FETCH instead
  of refusing it (#115).
- A draft copy goes to the server without Bcc. On a server without UIDPLUS the copy cannot be named: the draft's
  first copy stays in the Drafts folder, no later one goes up, and discarding the draft leaves it there (#116).
- A draft copy whose APPEND answer is lost (a restart, a cut connection) stays unnamed in the Drafts folder for good.
  Finding and removing it needs a stable `Message-ID` per draft in the copy (#116).
- A failed draft upload is not tried again until the draft changes (#116).
- The SMTP client sends one command at a time (no PIPELINING), has no CHUNKING or BINARYMIME, no DSN parameters, no
  HELO after a refused EHLO, and signs in only with AUTH PLAIN and AUTH LOGIN (no XOAUTH2 or OAUTHBEARER). It checks
  a message against the server's SIZE as given, before its line ends become CRLF and its lines are dot-stuffed
  (#117).
- The fake SMTP server reads one command after the other, though it lists PIPELINING; it knows no CHUNKING,
  BINARYMIME, DSN parameters, XOAUTH2 or VRFY, and reads command lines up to 8192 bytes where RFC 5321 asks for 512.
  Its delivery reports carry a fixed date, quote the header block of the message as text even when it is not ASCII,
  and name one recipient each. A client idling on the fake IMAP server hears of a report only at its next command
  (#118).
- A send is not held for undo and not retried on its own: one cut off by a dropped connection after the message went
  (new `ErrorKind::SendUnanswered`) or by a restart may have gone, and its outbox item says so until the owner sends
  it again. Only the outbox item holds the reply, which the window's outbox shows (#119, #136).
- The Sent copy is always appended, also on servers that keep a copy of what they send themselves, which then hold
  two. A send whose answer is lost leaves no Sent copy (#119).
- The live send test takes the account's IMAP host on port 465 as its submission server; the live configuration
  names no SMTP host of its own (#119).
- A reply or forward of a message without a plain-text part quotes its preview, not its HTML; a forward carries the
  attachments but not the inline pictures of an HTML message (#120).
- The quote line and the forward header are in English whatever the window's language, and the date in them is
  written `5 Nov 2026 at 08:00` (#120).
- Templates belong to one account each; saving or deleting one is not undoable (#120).
- Reply all knows the owner's addresses only as each account's address and login; aliases and other addresses the
  owner receives mail at are not modelled yet, so they stay on a reply all (#120).
- A bounce is linked only to a message in the Sent folder of the account the report came to, by the `Message-ID` of
  the report's returned header; a report without one, a report that names no kept Sent copy, and an account without
  a Sent folder mark nothing, nor does a report kept before the Sent copy came in with a sync (#121).
- A delivery report passes the screener and has its body downloaded on arrival only when its sender's local part is
  `MAILER-DAEMON` or `postmaster`, so a sender that calls itself that gets past the screener with a report header;
  a report from any other sender is screened like other mail and links once it is let in and opened. Reports that
  come with a folder's first sync, or into the trash, junk or Sent folder, are not fetched (#121).
- Only the failed recipients the Sent copy names in To or Cc count, by the report's final or original recipient
  address; a report about a Bcc recipient, or about an address a forward or alias turned it into without an
  `Original-Recipient`, marks nothing (#121).
- A bounce mark cannot be cleared and is not undoable; it outlives its report, which it then no longer names, and
  a later delivery report for the same message about its recipients replaces the reason (#121).
- No front end hands the session the computer's offsets, so presets and dates are in UTC there (#122, #123).
- Undo send is its own command (`mail.unsend`), not a step of `history.undo`, and a scheduled draft cannot be edited
  in place: take the send back, change the draft, schedule it again (#123).
- A send later that can no longer go at its time (its account lost the submission server, a forward's body is gone)
  is refused in the outbox with why, and no notice tells of it (#123).
- A send held when InboxCraft stops goes at the next start, late, with a notice, not taken back (#123).
- A minimized window's wake is not tried on a real Linux desktop yet. From eframe 0.37's code: on X11 a minimized
  window runs only the app's logic, which ticks and asks for the next wake, at most every 100 ms; on Wayland the app
  cannot tell it is minimized, the frame waits for the compositor, and eframe runs the logic after 250 ms without
  one. Both are by reading the code, not by a check by hand (#123).
- The screener's mode is kept by the window in `triage.json` (#134); the CLI and the MCP server read it from no
  file and start with the screener off. A delivery report is known
  by its sender only (`MAILER-DAEMON`, `postmaster`), not by its `multipart/report` type. An account whose folders
  have no hierarchy delimiter gets no screener or blocked folder: its new mail stays in the inbox, marked (#124).
- Accept-and-done files the held mail in the archive with the done state, as `mail.done` does (#126). A held message
  whose move to the screener folder is still on its way follows the owner's later decision only once the server
  answered the move (#124).
- The grouped inbox knows a `text/calendar` part only once the message's body is kept: an invitation sent as a
  multipart whose body was never fetched (older than the sync's cutoff, or too large) stays in the group its header
  gives. The classifier reads no message text and keeps no learned weights; the owner's per-sender choice is the
  only correction. A group choice holds for an address across the accounts, not per account. Mail the owner sent to
  themselves is People unless its header says otherwise (#125).
- `mail.list` with `group` works out the group of every message of the view before it pages, which reads the sender
  record of each sender once per call; fast enough for one mailbox, not measured on a large one (#125).
- Done, park and pin are kept in this computer's store only: another InboxCraft or another mail program sees a done
  message as archived and nothing of a park or a pin. A done message moved on later by hand still says done and lists
  under "show done" in its old place, until done files it again from there. On a server without UIDPLUS, done keeps
  only the move: the done mark, a pin kept through done, "show done" and the way back are lost once the server
  answers, as the message comes back from the archive's sync under a new id. Done refuses on an account without an
  archive folder; there is no fallback to All Mail. Whether done keeps pins is kept by the window in `triage.json` (#135) (#126).
- Snooze writes its time into the server's copy only for a message whose body is kept, on a server with UID EXPUNGE
  (UIDPLUS or IMAP4rev2) that InboxCraft has signed in to since it started: elsewhere, and for a message whose move is
  still on its way, the message goes to the snoozed folder without the field, and another InboxCraft cannot tell when
  it comes back (it stays there until the owner moves it). A server with UIDPLUS that names no UID for the copy
  (UIDNOTSTICKY) gives it back from the folder's sync under a new id: its time is read from the field, but a park or
  a pin on it is lost. The copy goes up without the old one's arrival time (APPEND sends no date), so the server
  records it as received at the snooze, and the field stays in it after the return, an unsnooze or an undo. A copy the
  server refused leaves the message in its old folder with its time kept, and the time then only clears it. The
  returned mark stays until the message is snoozed again; the window shows it only while the message is unread
  (#135) (#127).
- A follow-up reminder is kept in this computer's store only: another InboxCraft does not see it. A reply is told
  only by its `In-Reply-To` or `References`, so a reply from a program that writes neither, or to a sent message
  without a Message-ID, does not cancel it; a reply that arrives while InboxCraft is off cancels it only if the sync
  brings it before the next tick, and an answer from the owner's own address never does (#128).
- The time picker reads typed times in English words only and on one fixed offset from UTC: a time typed across a
  daylight saving change is off by the change, and the app still passes UTC until the computer's offset is read.
  The presets come from the caller; the scheduler's presets join with the snooze and send later screens. The
  alert switch keeps its value while the app runs, not between runs (#129).
- The command line's `>` list and the Shortcuts page list the window's own actions (`keymap::actions`), each running the engine's
  commands it needs; the engine's commands that need a message, an account or other parameters are not listed one
  by one. Changing a key is not an engine command yet, so the control channel and MCP cannot change one. A key
  given to an action may also be one the window reads directly (the arrows, Escape, Enter); both then run. egui
  matches a key with Shift or Alt held to the key without them when no other action has that one. The calendar
  screen shows its empty state while no account has a calendar (#130, #147).
- Acting on mail (#131): one message at a time, no selection of several. The toast's Undo takes back every act made
  after its own, so a message opened (and marked read) after it is unread again. The Move menu lists every folder of
  the account that holds mail but the message's own; it has no search. The
  window counts the acts it put into the engine's history from the commands' answers; when the engine holds fewer (it
  keeps 100), Ctrl+Z stops quietly. A failed write the server refuses is taken back by the engine, and its toast still
  shows until it goes.
- Composer (#132): one message at a time; a draft closed or left from an earlier run opens again from the drafts
  place or Home (#314); it stays in the store (`compose.list`) and in the account's Drafts folder. Plain text only: no formatting, no attachments. Suggestions come from the senders and recipients of the mail kept here (`mail.search`), not from an
  address book; a name that would move the address (`<`, `"`, `,`, `@`) is taken as the address alone. The engine
  takes a domain without a dot (`ansel@example`) as an address, so only a recipient with no `@`, two of them or an
  empty part is marked as none.
- Reply, reply all and forward have no keys yet, only the reader's buttons and the engine's commands (#133).
  Templates are written, edited and removed on Settings, Sending, Templates (#317).
- A blank is asked for at the foot of the composer, one at a time; its mark in the text is tinted but cannot be
  clicked to fill it in (#133).
- The Let in card's group pills are forgotten when another message opens; a sender's group is chosen on the card or
  later in the group menu, not when the sender is blocked. New senders lists the held mail of the accounts shown, at
  most 1000 messages of the screener folders and 1000 of the inboxes (#134).
- Pin and Park have no keys. The picker's alert switch is not shown, because notifying of a returned snooze is one choice for all mail
  (`notify.set`), not one per message. "Show done" is forgotten at a restart and is not offered while searching or
  in New senders. The rail's Later opens on the pins; it shows the snoozed, pinned and parked mail of the accounts
  shown, at most 1000 of each (#135).
- The countdown in the status line follows the last send the window made: a second send takes its place, and the first one is
  counted down in the outbox only. Undo send in the status line and in the outbox works only in the hold; a scheduled
  send's Edit takes it back, and it is scheduled again from the composer. The Outbox opens from the folder pane only,
  with no button on the rail and no key. A hold set to another value than 5, 10, 30 or 60 seconds through `send.set`
  shows with none of them chosen (#136).
- The bounce banner gives the report's reason only: it has no "Edit and resend" (no command makes a new draft from a
  sent message yet) and does not open the delivery report. The note that a send went late is a notice in the status line
  for four seconds, shown once (#136).
- No tray outside Linux yet, and outside Unix no socket: on Windows a later start says that InboxCraft is already
  running instead of bringing its window forward, and closing the window ends InboxCraft, since nothing could bring
  it back. Start at login is XDG autostart and the Background portal only; Windows and macOS have none
  yet. A desktop without a StatusNotifierWatcher (GNOME without the AppIndicator extension) shows no tray icon:
  InboxCraft then runs on without a window all the same, and a start from the menu brings it back (#137).
- Without a window InboxCraft looks for the workers' events every half second rather than at once. A window opened
  again starts with egui's state fresh (scroll positions, open popups). On Wayland a later start may only mark the
  running window as wanting attention rather than raise it, since no activation token is passed on (#137).
- The tray's tooltip counts the unread mail of every account's inbox; the icon only shows that mail is unread (its
  flap turns cyan), not how much, and its menu has no entries per account (#137, #232). The icon is always the
  light drawing meant for a dark panel: the app does not know the panel's colour, and the symbolic icons the brand
  ships for desktops that recolour tray icons are not used yet. The tray gives the panel only its own pixmaps and
  no themed icon name (the name only when no pixmap could be read), so a host that draws only themed names shows
  no different icon for unread mail; this is not checked on a real panel yet (#232).
- No notifications outside Linux. New mail notifies also while the window is open and in front. A click on a
  notification reaches InboxCraft only while it runs (it is not D-Bus activatable), and without a window it is
  answered within half a second. A desktop that shows no actions shows no buttons and may send no click either (#138).
- No choice to notify without the sender and subject; a notification always hands the desktop who the mail is from
  (or, for a reminder, to) and its subject, and the desktop may keep it in its list and show it on the lock screen as
  its own settings say (#138).
- The Notifications page has no way to turn notifications off for a while (the design file's "Snooze notifications
  for an hour") and no choice to notify without the sender and subject. "Start with the computer" shows what the
  autostart entry says; in the Flatpak a desktop that later revokes the Background portal's permission is not
  noticed until the page asks again. A start at login while running in the background is off (an entry left from
  an earlier choice) still starts without a window, and its window then ends InboxCraft when closed (#139).
- `inboxcraft-cli` runs no job a command asks for: a body not kept yet is not fetched and `account.check` checks
  nothing on a store (in the running app, through the control channel, the app runs it) (#140).
- On a store the MCP server runs no job: `mail_read` does not fetch a body that is not kept and `account_check`
  checks nothing (bridged to the running app, the app runs them). Its results are text
  only (no `outputSchema`, not the CLI's JSON), `mail_list` has no group column yet, and an HTML-only message gives
  no text (#141).
- The store takes no lock, and opening it tidies away files the running app may not have committed yet: until it
  takes one, `inboxcraft-cli mcp` (and every CLI command) runs only while the app is closed, or on a copy of the
  store (#141).
- The control channel works on Linux only: the user id comes from `/proc/self`, so on the other Unix systems the app
  runs without a channel and the CLI trusts no socket, and on Windows it is not built; `--store` works everywhere. The
  CLI does not check the user id of the app it connects to (the standard library cannot read it yet, and the channel
  adds no crate for it); the owner and mode of the folders stand in for that check. It does not check the client's
  process, so any process of the owner's that can read the credential
  store can use it. It has no approvals, so an `owner` command cannot be asked for over it. Without a window a call
  waits up to half a second for the app's next look (#142).
- An agent's act over the control channel ends the window's Undo for the owner's acts before it, rather than keeping
  both apart (#142).
- `ical` has no IANA zone data: a `TZID` without a `VTIMEZONE` in the same calendar object is read on the owner's
  own wall clock (UTC's own names excepted), and `ical` cannot write a `VTIMEZONE` for a zone it has not read. A
  UTC offset's seconds are dropped (#143).
- Recurrence: `DTSTART` is always the first instance, also where the rule would not make it, and `COUNT` counts the
  rule's own occurrences; `RECURRENCE-ID;RANGE=THISANDFUTURE` changes only its own instance; an `RDATE` period
  counts by its start, its own end dropped; second 60 reads as 59. An expansion stops after 100 000 empty periods
  in a row, 100 000 candidates in one period or 2 000 000 steps of work for the whole calendar (rule periods, days
  and candidates, ten for each event, date and instance it looks at), keeps the earliest 10 000 occurrences, and
  says so; with more work than that in a span, which events are left out depends on their order (#143).
- A `VTIMEZONE` is read only when its rules are yearly at one time of day, at most 16 rules without a `COUNT`, a
  `COUNT` that ends within 2 000 steps, and at most 10 000 onsets; any other zone is kept as it came and its
  `TZID` is read on the owner's own wall clock. A zone rule's onsets are looked for in this year and the year
  before only (#143).
- `Event` keeps the properties and components it does not read, but parameters it does not read on the properties
  it reads (`ALTREP` or `LANGUAGE` on `SUMMARY`, an `X-` parameter on `DTSTART`) are lost when an event is written
  from its fields; the component tree keeps everything. `VTODO`, `VJOURNAL` and `VFREEBUSY` are kept unread (#143).
- `dav` signs in with Basic auth only: no Digest and no OAuth bearer token yet, so Google's CalDAV cannot be used. It
  reads no SRV records (RFC 6764 section 3) and no `DAV:current-user-privilege-set`, so a calendar the owner may only
  read is found out at the first refused write. A redirect to another origin ends discovery with
  `DavError::OtherOrigin` and the caller decides. No scheduling (RFC 6638), no MKCALENDAR, no time-range
  `calendar-query`, no CardDAV (#144).
- `dav` sends no `Accept-Encoding`, so answers come uncompressed, and reads one response at a time on a connection.
  A resource a sync lists but `calendar-multiget` does not return leaves the new token out of `Changes`, so the next
  sync compares the whole calendar; members that are collections are skipped. A `DAV:displayname` keeps at most 256
  characters and a colour 32 (#144).
- The fake CalDAV server knows one principal and one home set, applies no `calendar-query` filter, has no access
  control, and records the head lines of each request in its transcript, not the bodies (#144).
- An account without a CalDAV address has no calendar: the address comes from the add-account form or the account's
  settings (`calendar.server`, `owner`), and until then its calendar jobs and the CalDAV step of the check do not run
  (#145, #146, #214).
- A calendar job runs once, failing on a dropped connection, and the engine does not retry it: a write that failed
  leaves the event as edited here until a sync brings the server's copy or the owner changes it again. Calendars sync
  at the start, on `calendar.sync` and after a conflict, not periodically (#145, #146).
- A calendar job opens a connection of its own each time and closes it at its end; a connection the server closes
  between two requests of a job fails the job (`ConnectionDropped`) instead of being opened again. The password is
  copied into `dav`'s Basic auth credentials for the length of a job, a copy that is not overwritten when dropped
  (#145).
- Only a 401 holds the CalDAV password. A 403 is also how a server answers a write to a calendar the owner may only
  read, so a server that answers a locked account with 403 gets the password again with each calendar job, twice
  per discovery (the well-known path, then `/`) and twice per sync (the report, then the comparison by entity tags)
  (#145).
- The live calendar test takes the account's IMAP host over `https` as its CalDAV server; the live configuration
  names no CalDAV address of its own (#145).
- A new event is written in UTC, a repeating one on the computer's wall clock (floating), never in a named zone: the
  engine knows the computer's offsets, not its zone's name or rules, so it writes no `VTIMEZONE`. A repeating event
  shared with guests elsewhere shows at their own wall-clock time (#146).
- Changing or deleting a repeating event changes or deletes the whole series: no single instance (`RECURRENCE-ID`,
  `EXDATE`) is edited or deleted, changed instances stay as they were when the series moves, and a repeat preset
  replaces the series' `RDATE`s (#146).
- `notify` only counts `SEQUENCE` on, and a new event's guests are only named in it: nothing is mailed to them until
  the invitations send iTIP over iMIP. Changing an event someone else organises changes the copy on the owner's
  server only (#146).
- An edit or delete the server refuses because the event changed elsewhere (412) is replaced by the server's copy at
  the full sync the conflict starts: the owner's change is lost, with no notice yet. A failed calendar job is not
  shown to the owner (#146).
- A write to an event whose earlier write the server took without an entity tag waits for the next sync of its
  calendar; if that sync is lost without a failure, the write waits until the next start (#146).
- A CalDAV server that ignores `SCHEDULE-AGENT=CLIENT` may still mail the guests of an event InboxCraft writes (#146).
- A delete within its undo window when InboxCraft quits goes to the server at the next start, without the window
  (#146).
- `event.list` answers at most 1000 occurrences, and a discovery keeps at most 100 calendars of an account (#146).
- The calendar's views read the computer's clock as the engine knows it, which is UTC until a front end hands the
  engine the computer's offsets; the line at the time now moves only when the calendar is read again (a view, a
  step, a sync). Weeks start on Monday and the hours have no setting for the start of the day (#147).
- An event over more than one day shows as a block over whole days in the day, week and month views, even when it
  has times (a night from 22:00 to 02:00); its times show in its details. Overlapping timed events share their
  group's width in equal columns, however short the overlap (#147).
- The calendar's views have no keys of their own for steps, views or today (#147).
- Hiding a calendar lasts until the window closes; it is not kept between runs (#147).
- The event editor takes times typed as `2026-11-06 13:00` at the computer's present offset; it has no date or time
  picker yet, and the place and the event's own zone are not edited in it (#148).
- Changing the times of one occurrence of a repeating event moves the whole series to them; there is no "this
  occurrence only" (#148).
- Telling the guests of a change only counts the event's `SEQUENCE` on: InboxCraft answers invitations (#149) but
  sends none of its own yet, and a new event's guests are named in it and not told (#148).
- An invitation is read only once its body is kept: one older than the sync's cutoff shows its card when it is
  opened, and the versions of an event are linked only among the messages whose bodies were read (#149).
- An answer to one occurrence of a repeating event (`RECURRENCE-ID`) is sent, but leaves the calendar as it is; a
  cancellation of one occurrence offers no Remove from calendar (#149).
- `COUNTER`, `REFRESH`, `ADD`, `PUBLISH` and `DECLINECOUNTER` show a card that asks for no answer; a guest's answer
  to an event the owner organises is shown, not written into the event's guests; delegating is not offered (#149).
- An answer carries no comment, and its message text is English; Going and Maybe put the event into the account's
  first calendar, with no choice of calendar (#149).
- An alert notification has no snooze or dismiss button; mail (`EMAIL`) alarms and alarms more than a year from their
  event are left out; the default alert of an all-day event goes off at midnight the day before (#150).
- A selected icon button and a section label that folds its section have no pressed state in the design; the app
  borrows the nearest one: the accent edge of a pressed primary on the selected button, the muted edge of a pressed
  icon button round the label's hover fill (#233)
- Check boxes, radio buttons, switches, segmented controls, pills and selects have looks the design does not draw yet,
  and the app chooses them: pressed, a chosen or on control takes the accent wash with the `cyan_edge` edge (a chosen
  pill's word turns `ink`, a chosen segment keeps its foot bar), an unchosen pill and a select take the `stage` fill
  and `ink` edge of a pressed secondary button; a chosen radio button and an on switch hovered turn to `ink` as a
  ticked check box does; a chosen radio button disabled keeps its square in `dim`, a ticked check box disabled its
  cross in `muted` (#233)
- The design's quote fold (a `muted` line in a 2 px edge) is not built yet: a small link folds quoted text, and the
  reader's notice keeps its own layout with a small link (#233)
- The menu dividers span the menu's content, 4 px in from its edge on each side; the design's run edge to edge
  (#233)
- Fields, menus and pickers have looks the design does not draw yet, and the app chooses them: a pressed menu row,
  choice, suggestion or recipient chip takes the press of a secondary button (`stage` and an `ink` edge), a pressed
  chosen row or ticked choice the accent wash with the `cyan_edge` edge; a focused empty text field puts its hint
  after the caret as the typed time does; the caret stands 2 after the character before it, over the next one when
  it is inside the text (#234)
- A menu row from a plugin is drawn coming soon, without the plugin's tag; the time picker reads a date that has
  passed only when it is written with its year (`2026-11-02 09:00`), as `2 nov` always means the next one (#234)
- A caret moved into the 8 px above or below a scrolling multiline field's text scrolls it into the text's room (#234)
- While a screen reader listens, egui hands every glyph of a text to it every frame: the composer's message text
  costs about 2 ms a frame at 256 KiB and 12 ms at 1 MiB at rest, 3 and 18 ms while typing, over the budget of 4 ms
  near its 1 MiB cap (a field of several lines holds 64 KiB, under 1 ms); without one it costs under 0.5 ms at rest
  and 3.1 to 4.3 ms a frame typing at 1 MiB (p95 over three runs on a busy machine), at or over the budget (#234)
- A frame with an undo or a redo in a field copies the field's whole text once more, into the field's own record of
  the frame's changes, so that a refused frame can be put back; other frames copy nothing of it (#234)
- egui copies a field's whole text every frame it is drawn, and twice more while it has the keyboard, a password's
  too, and frees those copies without wiping them; the field wipes its own copies of a password, but egui's are out
  of its reach (#234)
- The composer says a refused paste over its message text, and brings that line into view, so a person typing at
  the end of a long draft sees the page move up to it (#234)
- A field refuses a frame's input as a whole: when anything in a frame would take the text past its limit, all of
  that frame's change to the field goes, so a character typed in the same frame as a paste that is too long goes too
  and is typed again. The limit of a text opened over its cap is the size it opened with until the text changes
  from outside, drawn or not in between; such a change also clears the field's undo history, so Ctrl Z never brings
  back the text before it. A change from outside that keeps the length is not seen: the older, larger limit and the
  history stay. When something else moved the selection inside an unfinished input method word, a
  refused commit leaves that word in the text (until the next word of the input method takes its place). If egui
  has already used its second pass in that frame, the "too long" line shows one frame late (#234)
- The Sending page shows a stored signature whole; the account itself still cuts a signature at 2 000 characters
  when it is read from the store or saved, so a longer one written into the store by hand is cut there, before any
  field sees it (#234)
- Turning typed text into recipient chips, and saving a signature the account tidies (white space at its end),
  change the field's text from outside, which clears its undo history (#234)
- A recipient line stops growing at four rows of chips and scrolls; the design's line grows to hold them all, with
  no limit drawn (#234)
- A recipient line splits what is typed at every comma and semicolon, inside a quoted name too: where the line has
  room, `"Doe, Jane" <jane@example.org>` becomes two chips, and where it is full it counts as two (#234)
- The focused scrolling field's board differs from the field in two ways the field does not follow: the board's text
  is scrolled by whole lines (154 = 7 × 22) where the field scrolls to the very end of its text (152), so the
  caret and the text stand 2 px lower than the board's; and the board's thumb is 148 long where the text in
  proportion gives 140 (#234)
- Right-aligned mono runs follow the design's rule (the run at the left of a box as wide as the run rounded up,
  `paint::run_right`) in menus, floats and the recipient line; the list's, the reader's and the settings' dates still
  put the run's exact right end on the pad until the list (#236), reader (#237) and screens (#238) slices (#234)
- The real menus are not yet the design's frames: More, Move, From and Templates are 252 or 308 wide where the
  frames are 288, 256, 360 and 256 (Group is the frame's 256 × 212 since #236); More's Park, Show the full header and Full width have no icon,
  Move's folders no icon or key and its title no caption, Templates' title no count, and From lists plain choices
  where the frame draws account rows (colour bar, name, address in mono 11, tick). The reader's menus come with the
  reader (#237), the composer's with the screens (#238) (#234)
- The wrapped recipient line's board gives `Studio, everyone` a 106 box although the run is exactly 105.000 wide,
  kerned, in the design's font too; every other hugged name on the boards is the ceiling of its kerned width. A board
  artefact, left for the owner: the app's chip is 137 where the board's is 138, and its cross and the caret after it
  stand 1 px left (#234)
- The password's dots: the board centres them on y 42.5 in its cell and draws them 4 rows high (41 to 44); the app's
  glyphs land on whole rows, 0.55 px lower, 3 rows high (42 to 44); x 15 to 104 the same (#234)
- The composer's subject and message text are plain edits without the field's box until the composer is restyled
  (#234)
- A time typed or picked for a date past a change of daylight saving time is read on today's offset, an hour off
  (#234)
- A hover action held down has no pressed look in the design; the app gives it the press of a secondary button, the
  `stage` fill and an `ink` edge (#235)
- The hover actions stand at the right end of the row's first line, right of its middle (what does not fit gives way
  from the end of the list's order, Delete first, then Read and Snooze, Done last; beside the reader all four show on
  a hovered row); a hovered row hides its time, the keyed row keeps it and its actions stand left of it, so at the
  list's width beside the reader the keyed row gives up Delete (Done, Snooze and Read show, Delete stays on its
  key); the reader's banners keep their own layout until they move to the banner component (#235)
- The design draws no flagged or pinned row under the pointer: the app keeps the flag and the pin on the first line,
  8 left of the hover actions and 8 apart, and cuts the sender 4 short of them (#235)
- A settings row whose control has the focus shows the row's focus edge and the control's own ring; the design shows
  the row's edge alone (#235)
- A banner's link is drawn whole while it fits the banner's room; the design's banner board cuts the second link of
  the remote-pictures banner (`always for news@brightshelf.example.org`) at 240 wide with an ellipsis, though the
  room would hold it (#235)
- egui draws a glyph on whole pixel rows only: a caption line stands 0.30 to 0.40 px above the design's place, whose
  baseline falls half way between two pixels (it stood 0.60 to 0.70 px under it) (#235)
- Text drawn by egui's own widgets (`ui.label` and the like: the list pane's screener note and the sidebar's
  addresses, the settings dialog's lines) keeps egui's baseline, so at 100 % it stands 1 px under caption text drawn
  through the components, until those views move onto the components in the lists, reader and screens slices (#235)
- At 125 % the second row of a wrapped run can land about 1 px off the design's line; the whole-pixel baseline is
  measured at 100 % (#235)
- The running step of an account's check is the design's `[ .. ]` and stands still; the running mark of a progress
  line goes round. With motion off (not a level of its own yet) the ring will stand still and the bar give way to
  its words (#235)
- Motion has two levels in the app, on and off; "on" plays the design's reduced level: a row leaving by Done,
  Delete, Archive, Move or Park closes over 180 ms on the design's curve, a snoozed row fades over 125 ms. The full
  level (Done's words dissolving, Snooze's slide of 24 px and its 300 ms fade) comes with stage I-2.7 (#236)
- The mail row's paperclip and invitation marks are drawn on the gallery only: the list does not know yet whether a
  message carries attachments or an invitation (#236)
- A snoozed message found by a search is a plain row: the engine's search output carries no snoozed time. Listed
  in its snoozed folder it is the sheet's snoozed row (the clock and "back …" in `warn`), in Later the later row (#236)
- Bulk select is planned: the mail row's checked states are drawn on the gallery with the check box coming soon, and
  the list's rows have no check box (#236)
- A sent message that was not delivered keeps its sender's name in its row; the design writes "To" and the
  recipient (#236)
- The screener card's line under it is drawn inside its 64, where the design stands a 1 px line under a card of 64:
  every line height stays a whole even number of points, so every edge is a whole pixel at 1.5× (#236)
- A group's head the keyboard is on shows "done all", which the sheet's focus state does not draw, so `Tab` reaches
  it (#236)
- A row of Later the keyboard is on shows "bring back now", which the sheet's focus state does not draw, so `Tab`
  reaches it (#236)
- A send on its way says "sending…" without the sheet's progress, which the engine does not report (#236)
- The rows of the pins and the park pile offer no link while hovered: the sheet draws one for a snoozed row only (#236)
- Drafts' rows have no check boxes and no bulk bar: the way in is drawn as "Select", coming soon (#236)
- Whether Smart shows all of a group's rows is kept while the app runs, not between runs, and under the group's
  translated name, so a change of language folds every group again (#236)
- A screen reader sees only the rows in sight (and two above and below) as items; the rest of the list is there as
  its size and each row's place in it, not as items it can move to without scrolling (#236)
- The engine lists at most 1 000 messages of a place; the list says "1000 messages" and shows no more (#236)
- Showing all of a group's rows or fewer, and changing Later's pile, change at once: the design's motion names none
  for them (only M5, M6 and M11 touch the lists), so none is played (#236)
- The queue of New senders plays M11 at the reduced level, the one the app's motion has: the full level (the card
  leaving 24 px up or down over 300 ms) comes with stage I-2.7. The queue's own head stands under the list's head
  until the head is the design's, and a card's time stands after its subject, not at the end of the name's line (#236)
- Back from a held message in one pane, New senders' queue takes no keyboard landing: its cards have no row to
  land on, so the focus starts over (#236)
- The rows' cut and `cut_within` (the fields, the floats) measure a line the same way but do not always keep the same
  letters: right-to-left text is cut at opposite ends (the rows keep its beginning, `cut_within` its end), and
  `cut_within` can pass its room by a fraction of a point (0.13 pt the most seen) (#236)
- In one pane, a draft behind the list shows no sign that it waits: `C`, "New message" or a message opened from the
  list brings the composer back with all it holds (#236)
- The current card of New senders ends its why-line without the design's "open log →": the app keeps no log of the
  screener's answers to open (#236)
- The list's head is not yet the design's: the design draws a head of 48 (the place's name in `strong` mono, how
  many to read, the view switch at its right), a line, a row of 36 with the pills and a `panel_line`; the app keeps
  the title, whose mail and how many, and the search field over one line of the view switch and the pills. The
  design's head comes with stage I-2.2 (#236)
- A search's line under the list's head does not offer the design's "all mail" and "clear" links at its right; it
  says what was searched and where, and the search in message text stands there coming soon, which the design does
  not draw (#236)
- New senders' "SCREENER" stands about 6 px high beside its mode switch: the design centres the label on the
  switch's 32 high row (the label 16 high at +8) (#236)
- A card of New senders puts the time of the newest message after its subject on the second line; the design puts it
  at the right end of the name's line (the 620 wide line's last 33) (#236)
- More's second head ("When a message leaves the list") keeps a 1 px line under it, which the design's More menu
  does not draw (the head 28, the choices straight under it) (#236)
- At 130 % the current card's why-line cuts the time it was held back since: the answers grow and leave the text less
  than the design's 620 (#236)
- Beside the reader at 1300 the keyed row keeps its time, so its hover actions stand left of it and Delete gives way
  first (Done, Snooze and Read show; Delete stays on its key); the design's hovered row shows all four, 197 wide, at
  the right end of line one with the time hidden, and its focus state draws none (#236)
- Beside the lists' 441 at 1024 px the reader is about 279 wide: an attachment box shows no name; at 130 % text
  "Accept and reply" in the let-in card is cut with `…` inside its button, the subject keeps a word or two a line and
  a thread's line shows no sender (#237)
- The epic's "Back to the list" centred on the sender's avatar has no place: the reader's head has no avatar (#237)
- The reader stands with nothing open only after the server took the open message away; with no message picked since
  the app started, the list still takes the reader's width (#237)
- After a message left, the first step from the gap it left follows the list's flat order, not the order the cards
  show, and can open a row a card keeps hidden behind "Show all"; every step after it follows the cards (#237)
- The reader's attachments stand as wide as what they hold, where the design draws a lone attachment across the
  reader (#237)
- A thread's message and a quote open and fold at once: the design gives the folds no timing (#237)
- In a narrow content bar, once `open in browser ↗` has given way, the coming-soon tag gives way too when the words
  need its room (#237)
- The content bar counts no trackers: the HTML cleaner does not count what it removes, so the bar says only what
  pictures were held back or loaded; a mailing list's bar and its `unsubscribe` wait for the list headers (#237)
- An attachment never shows its saving state: the save writes the file at once, in the frame it is asked for (#237)
- The More menu's Park, Show the full header and Full width rows have no icon: the app's 45 icons hold none for them
  (#237)
- The subject the reader holds is cut at 300 characters before it is drawn, so the copy and a screen reader get
  those 300 (#237)
- An open line of a thread has a menu with "Show the full header" that the design does not draw: it is the way to
  an older message's header where its line has no room for `details ▾` (#237)
- A bounced message's banner has no `open the report` and no `edit and send again` yet (#237)
- While a message's text is fetched the progress line's bar stands empty and has no size: the fetch does not say
  how far it is (#237)
- The invitation card's WHO names only the organiser and its CAL stays empty: what the reader reads of an
  invitation holds neither the guests nor the calendar; a card never shows a clash yet (#237)
- The frames draw Reply primary under an invitation card whose Going is primary too; the reader keeps one primary
  button and makes Reply secondary while the card asks (#237)
- Keep my answer, beside the three answers while the owner changes an answer, is not drawn by the design (#237)
- The Note banner of the reader's notice has `dismiss`, which the sheet does not draw: a notice would otherwise stay
  until the next one (#237)
- A held sender's invitation card says it cannot be answered in the look of a card that cannot be answered (`line`
  edge, the reason in `caption` and `muted`): the frames draw no card for a held sender (#237)
- A picture that is here but whose data could not be drawn says so in the box of a picture not shown (`line` edge,
  the picture icon): the sheets draw no box for it (#237)
- A failed picture's box too narrow or too short for `try again` whole leaves the link out; the frames draw no box
  that small (#237)
- The box of a remote picture not loaded has the app's dashed edge, 2 on and 2 off, where the frames draw 4 and 4; a
  picture not shown does not name its kind, and a loading one shows no count of what came (#237)
- In a fitted HTML message a screen reader hears what lies within a pane's height of what is in sight; the rest comes
  as the pane scrolls (#237)
- A fitted table sets every cell in the message's words: the hairline table's figures in mono, right-aligned, are not
  guessed for a message's columns (#237)
- A fitted table is no table for a screen reader (each cell is heard alone) (#237)
- A change of the reader's width lays a fitted message out again whole: for one paragraph of two megabytes that is
  a frame of about half a second (#237)
- In a thread whose unfolded HTML messages hold more than four million letters together, a message that does not
  fit beside the fitted ones kept is shown in the plain view, under a line that says why, until a message folded
  leaves it room (#237)
- The fitted and the plain view of a message share the reader's scroll offset, so a switch keeps the offset, not the
  place (#237)
- An open quote of a plain text is hashed whole each frame to see that it is the one kept (#237)
- The composer's frames lay paragraphs 20 apart; the text field lays a blank line as a row of 26, and a blank's mark
  in the text has no room of 4 beside it (#238).
- The frames draw no pointer over and no press on the From line, the floating panel's title or its folding: over and
  open they take the `stage`, pressed the secondary button's press (#238).
- The floating panel folds by its title: the frames draw no control for it (#238).
- While the reader shows its answers (Reply, Reply all, Forward), the floating panel stands 16 over them, not 16 over
  the window's foot where its frames put it, so it never covers them, folded or open (#238).
- Attaching a file is drawn coming soon in the composer's foot; the drop target for files has no frame and is not
  drawn. Writing in a window of its own is a setting of the composer's place, drawn with the settings pages (#238).
- Of the composer's motion the wipe into the reader's place and sending (M10) are built; with reduced motion the
  wipe is not the design's 125 ms fade but at once, and the floating panel folds at once (the design gives it no
  timing) (#238).
- What is typed in the field that fills a template's blank is not kept with the draft: closing the composer,
  answering, opening another draft, quitting or a crash loses it; keeping it needs a field of the stored draft
  (#238).
- While a field of the composer has the keys, the draft is kept once a second on the window's thread: each keep
  writes the whole draft and waits for SQLite's sync to the disk. Measured at the store on a fast laptop's SSD, a
  keep costs about 5 ms at 16 KiB of text and about 6 ms at 1 MiB (median; p95 about 8 ms); measured through
  `compose.update` while other builds ran, a 1 MiB keep took 14 ms (median; p95 15 to 20 ms). Typing shows one
  long frame a second; the frames' p95 stays under the budget because it is one frame in sixty. On the in-memory
  store (the sample, the browser) a keep costs under 1 ms at 1 MiB (#243).
  A minimised or hidden window may draw that keep late, so what is typed stays at risk a little longer than a
  second, though never more than a second of it (#238).
- "This event only" for an occurrence of a series is drawn coming soon; an edit changes the whole series (#238).
- Dragging to create or move an event is not built and has no surface yet; New event and Edit do it (#238).
- The details of an invitation lack the design's "Your answer" control, "Remove from my calendar" and the organiser's
  mark: the calendar's data has no answer of the owner's yet (#238).
- A day picked by the arrows in the month view has no design state; it takes the mini month's picked look (#238).
- A picked agenda row has no design state; it takes the picked row of the day's float (stage, cyan bar) (#238).
- The day's float has no `Enter` key chip on its picked row (#238).
- In the side panel a calendar's name stands 12 points from its box, the check box's own gap; the design has 8 (#238).
- Week and day show at most 2 lanes of bands and 4 overlapping events side by side, the month the chips a cell holds
  and the agenda 50 rows a day; the rest are counted in the day's "+N more" (#238).
- Quitting the app while the event editor holds typing throws the typing away without a word (closing the window
  hides it to the tray and keeps it) (#238).
- A description longer than 10 000 characters is cut there, and the details show at most 4 096 wrapped rows of it;
  an event's title and place longer than 1 000 characters (the engine's own limit; only a server can send one) are
  cut there, and an Edit that changes such a title saves the cut (#238).
- While the event editor is open, the view under it does not scroll and shows no hover: the editor's ground takes the
  wheel and the pointer as well as presses (#238).
- The app starts on the inbox view Smart, the design on Grouped (#238)
- Undo send set to Off is a coming-soon row of its own, "Send at once", not an Off option of the hold's choice (#238)
- About shows its facts as coming soon, without the marks, the lockup and the emblem of the design (#238)
- What is typed on the settings pages and not saved, a signature or a left account form, lives only while the window is
  open: quitting loses it; a signature typed for an account that is then removed stays in memory, unseen, until the
  window closes (#238)
- A settings page with a row for every account lays out only the rows near the view, and the first and the last: the
  others have no node for a screen reader, whose browse mode does not reach them; `Tab` and `Shift+Tab` reach every
  one (#238)
- The Calendar page's rows differ from the design's: each account's name is `muted` with the coming-soon mark (the
  design draws a working row, its name in `ink`), and the line under it says where the calendars will come from
  ("Calendars from the CalDAV address of …"), not the design's count, names and address of the calendars, which are
  not read yet (#238)
- Check and add and Detect are drawn enabled when the form is not ready; the frames draw them disabled (#238).
- A check step has no duration at its right, and the TLS step shows the host and port, not the TLS version: the
  engine reports neither (#238).
- The DNS step's line shows the IMAP server only; the frames list the IMAP and SMTP hosts (#238).
- A guess's other candidates ("proposed, not asked") are not listed in Detect's column (#238).
- Detect's progress line has no fill and no current query: Detect reports nothing until it ends (#238).
- The welcome has no character field behind its box (stage I-2) (#238).
- The "← Accounts" link above the page keeps its old look (#238).
- An error under a single field stands 6 under it, as the screen frames place it; the text-field sheet says 8 (#238).
- A port field's hint is the usual port (993, 465); the frames show "optional", or the port as a value (#238).
- Detect's column has no muted answer line under a question that succeeded ("found: IMAP, SMTP") (#238).
- A row with both a host and a port error says both, one under the other; the frames draw one error (#238).
- The note when Detect leaves a server the owner typed out of the form, and the line when it replaced a picked
  encryption, have no frame; their words are the app's (#238).
- An account's form kept unsaved opens as it was kept, even if the account was changed elsewhere meanwhile, and
  saving it writes the form as it was kept, fields left alone too, over that change; nothing changes an account
  behind the form today (#238).
- The add-account page lies in the settings sheet's 720 column: the form is at most 436 wide beside Detect's column
  of 200 at +476 (the frames: 560, and 278 at +600; the design gives the page the whole place), the address 353 (477),
  the names 212 each (274), the host 184 (308). Three texts wrap that the frames keep on one line: the Mozilla check
  box's label (+20), the outgoing server's caption (+16; the row's boxes stand level all the same) and the password's
  caption (+18). So at 1440 × 900 the form's foot, Check and add and Cancel, lies under the fold of the sheet, reached
  by scrolling, and no picture shows it (#238).
- A server row's encryption that does not fit its port (STARTTLS on 993, TLS on 587) is said in one caption under
  the row and does not hold the save back; the connection is then refused, never made without TLS or STARTTLS: no
  value of the form leads to a login without them (#238).
- What an earlier Detect found comes back with a kept add form, its column of questions as it was, and nothing says
  it is from an earlier visit unless the address has changed since (#238).
- At 125 %, 150 % and 175 % the 1 px edges of a field, a select, a check box, a button, the segmented control, a
  menu, the palette, a tooltip and a toast still show a half-tone pixel beside their solid one; only the settings
  sheet's outline stands on whole pixels (#238).
- The error banner under a failed check is one line, cut with an ellipsis where its words are longer (a screen
  reader hears them whole); the frames wrap it on two lines (#238).
- The IMAP sign-in's line names its server, not how many folders it found, the SMTP sign-in's not its encryption, and
  the calendars' line not how many calendars it found ("sign-in · 6 folders", "· 2 found"): the engine reports none
  of them (#238).
- The Shortcuts page's intro says that one key runs one action per screen, and that giving a key takes it from its
  holder there; the frame's says one action, from the action that had it: the app's keys have a screen each (#238).
- The Shortcuts page's scroll bar stands in the page's room right of the 720 column, 24 right of the frame's, which
  lays it over the column's last pixels (712 to 715) with the rows' lines ending at 711 (#238).
- The Shortcuts page lists the calendar's keys right after "Show the calendar"; the frame goes on with the message
  keys there (#238).
- The add form says two things no frame has: that accounts are not kept yet and their passwords leave the store when
  InboxCraft closes, and "Sign in at the provider", coming soon (#238).
- In a window narrower than 980 the composer in the reader's place stands in the list's place, as the one pane of the
  lists has it (Esc in the composer, or Back, brings the list in front of it). From 980 up the list keeps its 440 beside
  the composer, which has the rest (#312). Closing the composer, or moving it to the floating panel, brings the list back with its scroll place and its pick;
  when it goes its floats (a row's time picker, a group's menu) close and do not come back with it, and the keyboard in
  the list or in one of them moves to the composer's text; while it is hidden, and in one pane while the composer
  stands in front of it, nothing of it takes Tab, and nothing acts on it or its rows: Ctrl F, whose field is the
  list's, the list's keys, the inbox views, Done, Archive, Read, Flag, Delete, Snooze and Undo cannot run, by their
  keys or from the palette. Beside the list the composer is never narrower than 391 (405 at 130 %), what its foot and
  a recipient line with its widest chip, 176, beside "Cc Bcc" need; at 720 the composer is 392 wide (405 at 130 %),
  and a chip wider than its line's room is cut to it. A folder name too long for the list's 440 still stands over the
  search field, at every width. In the composer's foot, as the room runs out, attaching a file and its coming-soon tag
  give way, then Send's key chip (the key still sends), Templates and Discard; from a window of 720 (the app's least)
  up, Send and its caret are always drawn whole inside the composer, Send's words cut to the room; a control that gave
  way is not drawn and Tab does not reach it. Discard has no key and no palette command, and
  Close keeps the draft. In its head, when the draft was kept gives way before the title would be left less than 80,
  and on the From line the account's tag gives way before the sender would be left less than its name or 80 (#238).
- The send-later picker offers the engine's time presets still ahead, This evening among them, and Snooze offers the
  same; its frame offers Later today, Tomorrow morning, This weekend and Next week, without This evening (#238).
- A template blank's mark in the composer's text is 65 × 22, its wash tight round its words; its frame's is 71 × 26,
  the line's whole height, the words 4 in and the mark 4 from the words beside it (#238).
- With the keys in an empty recipient line, the hint "Name or address" stands 2 left of its frame's place: its ink
  starts at 128 where the frame's starts at 130, 13 after the caret's left edge (#238).
- The docked composer counts its 1 px edge inside its pads: its head, lines, text and foot stand 39 clear of the edge
  beside them and 23 above and below, where its frames keep 40 and 24 (#238).
- In week and day, events that share a day are always drawn compact: the title 3 after the bar and no second line,
  however wide the event. The design keeps the full look (title 8 after the bar, the times and place under it) on a
  shared event 294 wide in the day, and goes compact only at 43 wide; at 64 wide it keeps the 8 but drops the second
  line (#238).
- In the month, the weekday names over columns 2 to 7 stand 1 point left of the design's, and the design's
  `panel_line` hairlines between the names (1 × 28) are not drawn (#238).
- In the gallery's calendar page, the event block's "row" cell shows the agenda's row (its bar at x 114) rather than
  the design board's row (bar at x 67) (#238).
- The outbox row's addresses, its Cc and Bcc on its line and "Changed since it was refused: …" before the reply are
  not drawn by the design file, whose outbox row has To by name and the reply only; with the address the line is
  cut sooner at the list's width (#245)
- A composer chip showing an address whose domain is wider than the chip's 176 grows to the line's width, and a
  recipient line Send has grown shows eight rows of chips before it scrolls; the design file draws neither (its
  chip stops at 176, its line at four rows). In a composer too short for its lines (the 800 × 480 window, a short
  floating panel) the lines show fewer rows and the message text goes out of sight under them (#245)
- The dev container's compile cache holds the dependencies only: the workspace's own crates in the dev profile
  (built incrementally), binary crates and linking still build in every target dir, the coverage run
  (`CARGO_INCREMENTAL=1`) does not use the cache at all, and the first build after the cache is emptied is no faster. Only the x86_64 image is built and checked; the aarch64 checksum is the release's
  own, untried (#246)
- `docs/design.md` gives the body at 1440 × 900 as 1207 × 828 and the reader as 766 wide; the design file's frame
  draws the body 1205 × 826 at (234, 46) and the reader 764: the window's own outline takes one point at each side,
  and one hairline stands under the command line and one over the status line. The frame's regions are measured
  from the design file (#254).
- After the server takes away the open message that was the only row of its card, the card goes with it and the
  keyboard goes to the row under it in the next card (or above it in the card before); the design draws no empty
  card head to land on (#254).
- A window narrower than 800 (a tiling window manager can give one) is laid out 800 wide and clipped at its right:
  what stands there cannot be reached by the pointer, among them the docked composer's Close, the reader toolbar's
  right end and the window's own Close; the keyboard still reaches them (#254).
- In an invitation from someone other than the organiser, the organiser's address is cut at a 1024 window and 130 %:
  beside the index the card's answers have about 116 points beside its date; from about 1100 it stays whole (#254).
- The calendar's week view at 1200 shows no week number: its toolbar's title takes a short form (2 – 8 Nov) and the
  number gives way first; the frames draw both at 1440 (#254).
- A connection that drops under a job is signed in again at once, and `sync.status` reads `offline` (or `failing`
  beside a signed-in account) for that sign-in: the worker tells the drop and the new attempt, nothing that marks
  it as brief (#255).
- A folder that keeps failing but is synced only in whole rounds, not the watched one, reads `synced` again after
  the second clean resync of the watched folder, until the next round tries it (#255).
- `mail.why` cannot say when the owner let a sender in or moved a message, nor that a move was the owner's: the
  engine keeps no date for those decisions and no history of moves. Like `mail.thread`, it reads every message of
  every account to find the thread. It is a `safe` command, offered through the CLI, the control channel and the
  read-only MCP tool `mail_why` (#256). So the reader's why
  line leaves out what the engine cannot say: the date of a let-in ("let in by you on 3 Nov") and "synced 14:02",
  which the frames `screen-shell--reader-open` and `screen-shell--everything-failing` draw in it, and "moved by
  you" and "SPF and DKIM pass", which the why-line sheet's `folder` and `screener` states draw (#267).
- The design file draws a mail row's hover actions four at a time; where the 440 list holds three (130 %, or a
  keyed row beside its time) the app shows Done, Snooze and Delete, which no frame draws (#259)
- The add-account form's line that its servers come from an earlier Detect is not drawn in the design file: it is
  the form's muted caption hint under Detect's note, 18 high (#259)
- The line under a server row whose encryption is unusual for its port is not drawn in the design file: a muted
  caption line under the row, after Detect's note on the encryption when both stand (#259)
- The invitation card's line for an organiser who did not send the message is not drawn in the design file: the
  same caption line as "Your answer goes to … at once.", other words, or two or three caption lines in a narrow card
  (#259)
- Detect's provider domains come from the MX hosts without a public suffix list: one for each MX host, its last two
  labels, or three under a two-letter country label when the second last is three letters or fewer, at most two in
  all (`mx1.mail.example.net` gives `example.net`, `mx.example.co.uk` gives `example.co.uk`). So `mx.abc.de` gives
  `mx.abc.de` and `abc.de` is never tried, and a provider whose mail hosts are not under its MX hosts' domain is not
  found this way (#260).
- One of the domain's own common host names (`mail.`, `imap.`, `smtp.`) whose probe timed out is not guessed again in
  the run, on any port: such a host that drops only its port 993 and answers on 143 is missed, unless the domain
  names that server (#260).
- The system's name lookup does not tell a temporary failure (its resolver not answering for a moment) from a name
  that does not exist: both count as "its name was not found", so a named server whose lookup failed for a moment
  counts as gone, and the guesses of its kind are tried in its place (#260).
- On a network that blocks the mail ports, Detect proposes only what the domain names (unchecked) and finds nothing
  for a domain that names nothing; the account form and its check stay the way to enter it. The probe connects to
  names made up from the domain and the MX hosts, so the system looks those names up (#260).
- A domain whose stale autoconfig or SRV records name a host that drops packets gets that host proposed as not
  checked, and its common host names and MX provider are not tried: the owner types the servers by hand. A network
  that refuses connections to the mail ports instead of dropping them makes a named server look gone, so a guess
  that answers may be proposed in its place (#260).
- Detect's 120 seconds are a deadline every lookup and probe is given, and each probe has 12 seconds of its own for
  its name lookup, at most four addresses, TLS, the greeting and the capabilities, however slowly a server sends or
  reads. The system's name lookup of a probe has no timeout of its own: it runs on a thread that is waited for
  until the deadline and then left to end by itself; at most eight such threads run at once, across runs, and past
  that a lookup fails as timed out (#260).
- Detect proposes one IMAP and one submission server, the first that answered, not a list to choose from (#260).
- Servers named by SRV records and autoconfig files keep the port they name (port 25 is left out); only the hosts
  Detect makes up from a domain are limited to 993, 143, 465 and 587 (#260).
- With Saturday as the first day of the week, This weekend and Next week are the same moment on most days; the
  time picker shows both, as the engine gives them (#265).
- The why line's reasons stand as plain words: a rule's or group's name does not lead to its settings, and
  `open log →` is coming soon. A thread's count in it is the one asked when the message opened or last moved; a reply
  that comes in later does not change it (#267).
- Choosing for a sender the group its message already shows, or letting InboxCraft sort it into that same group,
  changes the reason ("you put … in People") but not the message's place in the list, so the open message's why
  line keeps the old words until the message is opened again (#267).
- A time in a why line's reason ("back from a snooze, 14:02", "snoozed until Sat 09:00") is worded when the reasons
  are asked, against the clock of then: while the message stays open past midnight or into another week it can read
  stale until the message is opened again or moves (#267).
- Calendar sync line (#271): the time shown is the last read the window heard since it started, as hours and
  minutes without a date; the engine keeps no last-read time, so after a start the line says "calendars not read
  yet" until the start's own sync brings a calendar. "open log" under a failure is drawn coming soon without its tag
  (the 222-point line has no room for it): there is no log to open. A failure names the first account that failed,
  not how many did. Synced keeps its full bar until the next press, as the frame draws it, without the "sync now"
  link; the line itself takes the press.
- Calendar sync line (#271): the window sets no time limit on a sync. A job that is never answered (a window
  without workers, such as the web app or the bare sample, or a worker that loses a job) keeps the line at "syncing"
  for as long as the window runs, and while the calendar shows, its ring asks for a frame every 125 ms.
- The character field is shown on Home only: the first start, the cover and the panes' empty areas do not use it
  (#275)
- The field's gallery page shows a plain place holder of the emblem's 76 × 40 cells, not the mark Home draws (#275)
- At 130 % text the field's cells are 8 × 20 points (the width stays, the height follows the text to the whole
  point) and the dot keeps its 13.33 point size; the design draws the field at 100 % only (#275)
- Which of the field's cells flip under the pointer, how long each takes to cool and the glyph it flips to are
  fixed hashes, not chance: about 45 in 100 of the cells within reach flip, the nearest most often, and about half
  of the emblem's (#275)
- At most 4 096 of the field's and the emblem's cells are warm at once: past that no further cell flips until some
  have cooled, so a fast sweep at a high refresh rate (some 100 new cells a frame at 240 Hz) can leave the cells past
  the cap unflipped: dots on the field, the emblem's own characters on the emblem (#275)
- The design file does not draw the add-account form's note for servers guessed from the MX provider
  (`account-detected/*`): it is the careful note the form shows for a guess, other words (#279).
- The design file does not draw the line under a server row whose server Detect did not reach
  (`account-detected/*`): it is the form's muted caption hint under the row, 18 high a line, after the encryption
  lines when they stand (#279).
- A Detect run stopped by Cancel ends at its next lookup or probe, or within the wait of the one under way; a name
  lookup the system runs for a probe is not interrupted and ends by itself, as before (#279).
- A store failure met outside a command the window runs (the engine's timed work, the workers' events, the account
  list read while handing out jobs) shows its fixed sentence in the status line and its database words are written
  nowhere (#284)
- Leaving the settings by their key gives the keyboard to no control: it is nowhere until a press or Tab (#284)
- The About page is one column with 24 under it; the frame lays it out in columns 40 apart (#284)
- The settings' frames are drawn at 1440 wide only: under about 1060 the page's side room and the sections' column
  give way by the code's own rule (#284)
- The narrow-window numbers are chosen in the code, not taken from a frame: the room beside the page goes down to
  24, the sections' column to a quarter of the body, a row stacks under 160. At 800 wide the page is about 375,
  under its least of 520, and the section names and the head's count are cut with "…"; at 1000 wide and 130 % the
  segmented controls cut their option labels (#284)
- Command field (#286): with the focus a text longer than the field scrolls to the caret instead of being cut with
  `…` at its end as the `long` frame draws it; without the focus it is cut with one `…`. An empty field of commands,
  people or mailboxes shows no hint (the sheet draws none); an empty field searching all mail shows the general hint
  "search mail or run a command" (the sheet draws no empty all-mail state). A mailbox's name in the scope mark keeps to a third of
  the field and is cut with `…` (the sheet gives it no bound). The field keeps the 1 px edge and the 13 points to
  its text at 130 % text; its mark is 20 high there and the caret 9 × 20.
- Window controls (#286): the sheet draws no pressed state and no focus ring; a focused cell shows the focus ring
  inside its 36 × 32 cell. At 130 % text a cell is 36 × 42 and its glyph 20.
- The status item has no pressed state in the design: a pressable item pressed takes the `stage` fill and `ink`
  edge of a pressed secondary button; under the pointer its `muted` words turn `ink`, as the hover board's do (#287)
- The status item's `●` and `○` are drawn in code (IBM Plex Mono holds neither): a disc 0.75 of the text size
  across, and a 1 px ring of the same size; the waiting ring stands still, as its board draws it (#287)
- The status message's key chip is the chip of every other place, 20 high and 3 down; the sheet squeezes it to 18
  high, 4 down (#287)
- The error message's "open log" stands coming soon, unavailable, without its `coming soon` tag: there is no log to
  open yet and no room on the line for the tag (#287)
- A status message narrower than its parts clips them at its edge; it does not yet drop the countdown or the
  seconds first (#287)
- Home's sets (#288): the sheets give no pressed state: a
  pressed count tile keeps its hover fill and a pressed block its `muted` edge. A count tile's label takes at most
  two lines and a block line's empty text at most three, each cut with `…` after them. At 130 % text a tile, a block
  head and a more line take the scaled heights (98, 46, 42) while their paddings stay, so the room the scaling
  adds stands under the text. A count is written as the index writes its counts, groups of three apart by a narrow
  no-break space.
- Text in a script that neither the app's fonts nor a font found on the computer holds is drawn as boxes, one per
  sign, in the window, the reader, the composer and the toasts; it never stops the window (checked for Hebrew,
  Arabic, Devanagari, Thai, Chinese, Japanese, Korean, emoji sequences, combining marks, private-use, unassigned and
  non-characters, bidi and control characters, 10 000 of each, at 100 % and 130 %). The app's fonts hold Latin,
  Greek, Cyrillic, Armenian and Georgian, and since #348 Hebrew, Arabic, Devanagari, Thai and emoji, but no Syriac,
  Thaana, other Indic scripts, Lao, Tibetan, Myanmar, Khmer, Ethiopic, Cherokee, CJK, Hangul, Bopomofo or IPA
  letters. The computer is searched for one sign of each of these but IPA (#348); a script with no sign looked for
  stays boxes even where the computer has a font for it. The mono styles lack U+FFFD
  (a box is drawn for it). Audiowide, the product-name style, draws a missing sign as blank space, not a box; it
  draws only the app's own words. egui's test harness panics on a missing sign by default; a test that draws such
  text says `allow_missing_glyphs()`, as the app's policy is (#290).
- Home (#299): the counts' fourth tile turns to "not sent" while the server refused a send. "Back today" counts the snoozed mail
  of one page of `mail.list` (the engine's list limit). An agenda line's detail says where and how long, never "in
  58 min". The greeting's sentence is written from the counts and the next event, plainer than the file's words.
- Home (#299): the file's "MISSING UI" note on the drafts' block is not drawn.
- Home (#299): where the brief does not fit, the emblem's place goes first, then the brief takes the whole width
  under 600 and scrolls when it is taller than the body; the file draws neither.
- Home (#299): the greeting names the first word of the account's sending name, so a name written family name first
  is greeted by the family name; there is no setting for it yet. The date, the greeting and "the rest of today" go by
  the window's clock, which is UTC like the rest of the app until a front end hands the engine the computer's zone.
- Home (#303): "Look around with sample mail" on Home before the first account is drawn coming soon and cannot be
  pressed: the sample is another start of the app (`just run-sample`). The start place with no account is the
  add-account card, not Home's first-run form. "open account" in the failing banner shows Settings › Accounts, not the failing account.
- Home (#303): while a discard waits behind its Undo, the draft still shows in the Drafts mailbox on the mail.
- Command line (#305): the results are an area of their own, not a float of the float module (whose modal layer
  would take the field's keyboard); while they show they hold the window's keys as the palette did. Tab keeps the
  keyboard in the field while the results show: the rows are reached by the arrows, not by Tab, and the row picked is said selected, the field's active
  descendant (the field's role stays a text input, not a combobox).
- Command line (#305): plain text searches the mail shown, as the list's field did, and only while the mail shows:
  typed on Home, the calendar or the settings it changes no place until Enter shows the mail. `@` then Enter searches
  the person's mail. The results' rows are the menu rows (32 high), their head and edge as the float module draws
  them.
- Title bar (#305): with the window maximised the frames draw the command line from the window's edge (field 1083
  wide, controls at 1328); the code keeps the 1 px edge's room there too (field 1081, controls at 1327). The desktop's
  window menu on a right press of the title bar is not offered.
- Status line (#309): the frame's "Personal synced 14:02" is "synced 14:02" for all accounts; the syncing note is the share done ("54 %"), not the account's name.
- Status line (#309): the messages keep the window's words ("Marked as done", where the frames write "done · Draft
  logo for the bakery"); the error's "open log" and the problems' "open log" are drawn coming soon.
- Status line (#309): the message shows at once, without the frame's motion; its bar and seconds change once a second.
- Widths (#312): in one pane a picked event's details bring the whole side panel into the pane (the mini month and
  the calendars above them), drawn at the pane's width; `Esc` leaves the keyboard on nothing, as it does beside the
  week. While the event editor is open the week stays in the pane under it.
- Widths (#312): under 1280 the status line keeps its first three facts whatever they are: with a failing account
  "open account" or "open log" counts as one of them.
- Widths (#312): the compact mark in the narrow command line stands 16 in from its left, where the frames centre it
  in the index's 57.
- Drafts place (#314): the list holds the drafts kept here. A draft written in another program lives only in the
  account's Drafts folder on the server: the row "Drafts folder on the server · N messages" opens that folder, where
  it is mail that opens in the reader, not in the composer; N is the folder's size, the uploaded copies of the
  drafts kept here included, so the row does not say how many drafts only the server holds. The list reads at most 1 000 drafts (`MAX_LIST`).
- Drafts place (#314): the list's head is the list's title over the count (the design: "Drafts" and "2 drafts" on
  one 48-high line). The row has no account mark, as the design's
  `draft-row` has none. The server folder's row is the index's mailbox row (30 high), which the design does not
  draw in this place.
- Templates page (#317): a delete is confirmed in place, where the row stood, not in the frames' dialog over a scrim;
  it adds "This cannot be undone." because the engine cannot undo `template.delete`.
- Templates page (#317): the groups follow the engine's order of accounts (Personal before Studio in the sample),
  where the frames show Studio first; the form's title is "New template" for a new one, "Edit a template" for an
  edit; the intro and the empty state finish the sentences the frames cut with "…".
- Scheduled page (#317): a cancel is confirmed in place, where the row stood, not in the frames' 480×250 dialog over
  a scrim (no "CONFIRM" label); it says "This cannot be undone." first because the engine keeps no undo for
  `schedule.cancel`, and adds when the entry is due. The intro's second sentence finishes the frames' cut one (a
  snooze's cancel brings the message back now, through `mail.unsnooze`; a reminder's or an alert's takes only its
  time off the schedule); the page has no picture of its empty state yet.
- Scheduled page (#317): times within the coming week are a short weekday (`Fri 11:00`), further on the day and month
  (`12 Nov 09:00`); the frames show only today and weekdays.
- Privacy page (#317): a sender's row shows its address alone, in the frames' name column; the frames' name (180 wide,
  strong) and address (424 wide, mono) need a name the engine does not keep for an allowed sender.
- Removing an account (#317): the question stands in place of the form's buttons, not in the frames' 480-wide dialog
  over a scrim (no CONFIRM label, no shadow); the form's title is still "Account settings", without the account's bar
  and name the frame has; the frame's line about the drafts gives their number, the question does not. A send handed
  to the account's worker just before the removal may still go out.
- Triage page (#317): "After an act" is a group of its own, `[ C ]`, before the grouped inbox (now `[ D ]`); the
  frames have no row for it.
- Calendar page (#317): the frame's Week group has one row, "Week starts on", whose help says it cannot be changed;
  the page has two, a select of the seven days (160 wide) and a segmented choice of the first week, with help of
  their own. The locale's default knows a short list of Sunday regions and no Saturday ones (another Sunday region,
  such as Colombia or Thailand, gets Monday), and every region off that list counts week 1 by ISO 8601. The headless
  CLI and MCP count weeks from Monday by ISO 8601 until the owner chooses: the locale's default is the window's only.
- Motion (#318): the hover scramble (M3) runs on text buttons only; links, index rows, tabs and segmented controls
  keep their labels. Home's headline resolves at an even pace (the motion frames draw the first word slower at
  140 ms). A change of place (M12) scrambles what the body painted, icons excepted, and skips a body that painted
  more than 20 000 shapes; text in a float over the body (a menu, a picker) does not take part. A new row (M4) is
  told from the rows its list held since it was opened, unread only; past 64 new rows at once the rest are there
  at once. A row scrolled into sight while it would still move moves for the rest of its time.
- Motion (#318), a change of place (M12) at full: a text already scrambling (Home's headline, a hovered button)
  is scrambled again by the change, so its characters change twice; a press in the first 125 ms lands on the new
  place under the old body still shown; a place changed by the window itself after the frame's commands, or in a
  frame already drawn again, shows a frame late without the movement.
- Motion (#318): a message opened while nothing was open wipes in over the ground (nothing was drawn to keep); a
  draft sent from the floating panel leaves without M10; at full the line that goes into the status line is the
  composer's head, not a line with the subject and who it goes to; a change of mode the computer makes by itself
  is there at once, without M14.
- Motion (#318), send (M10): in the frame of Ctrl Enter the status line's Undo stands 56 pt right of where it stands
  from the next frame on, so a press aimed in that one frame misses it.
- Motion (#318), the command line (M8): the line run is not held in cyan for 125 ms at full; the results close in
  the frame of Enter.
- Later's heads over the snoozed pile (Back today, Back this week, Later) are the inbox's group heads; the design
  file draws Later as one page with heads for today, this week, the scheduled sends and the pins, which the app does
  not have (#323)
- The focus ring of a wide table that has the keyboard is not drawn in the design file: the usual ring, inside the
  table's box (#323)
- At 130 % text the list's view switch stands at the start of the pills' line, where the design keeps it in the
  head (#325)
- The index keeps a Blocked row while an account has InboxCraft's Blocked folder; the design draws none, and blocked
  mail has no other place (#325)
- The list and all in it stand 1 px right of the design: at 1440 the line between the list and the reader is at
  x 675, the design's at 674, and the rows' text at 270 (269) (#325)
- Where the status line has too little room (1000 wide, or one account at 1200) its key hints lose their first ones
  ("K previous · Enter open · Ctrl K commands": previous without next) (#325)
- The screener card keeps its 1 px line inside its 64 where the design draws it under (65 in all), so what stands
  under the card is 1 px higher than the design (#325)
- The drafts list shows the row of the server's Drafts folder, which the design does not draw (#325)
- The pills' line shows the planned Select (coming soon) after "Show done" and the park pile; the design draws only
  those two (#325)
- No picture of the status line's notice, account-refused and everything-failing states or of the whole window
  failing: the picture script cannot make an account fail or a notice show (#325)
- The account picker's head shows no account count; the design shows one with six accounts and none with two, and
  no rule says from how many (#325)
- At 125 % the index's rows lie on half pixels: 34 pt rows are 42.5 px (#325)
- A screen reader is given the full rect of an index row scrolled out of sight, reaching under CALENDAR's row (#325)
- The why line of a held message says "held back by the screener" alone; the design's says the first mail from the
  address and its SPF and DKIM result too, which the engine does not keep (#325)
- At 1024 wide and 130 % text the reader beside the 440 list is about 524 wide: everything fits (acts on one line,
  subject on two, From cut with …), but the design draws this text size only at 1440; its width there waits for a
  ruling (#325)
- The 1 point edges of fields, selects, check boxes, buttons and the segmented control still fall between pixels at
  175 % (#325)
- Settings: the sections list has 11 rows, the design's nine and AI and Plugins (coming soon) after them (#325)
- Settings, Templates: the groups follow the accounts' order (Personal before Studio in the sample); the design
  draws Studio first and names no rule (#325)
- Settings, Privacy: a sender's row leaves the design's name column (180) empty: `remote.list` keeps only the
  address (#325)
- Settings, Calendar: Week starts on is a select of the seven days and First week of the year a choice; the design
  draws one fixed row ("Fixed: it cannot be changed yet.") (#325)
- Settings, Triage: "After an act" has no frame; its look is not measured (#325)
- Settings, Shortcuts: the intro says a key runs one action per screen; the design's sentence leaves out "per
  screen" (#325)
- Settings, Appearance: under 1440 the page's column is narrower than 720 and the Window frame's help wraps (two
  lines at 1200, three for Motion at 130 %); the design draws 1440 only (#325)
- The calendar's sync foot's "open log" is coming soon without its tag (#325)
- In one pane a picked event's details bring the whole calendar side panel (mini month, calendars, details) at the
  pane's width; the design draws no one-pane calendar with details (#325)
- The calendars' sync line, failed, draws "open log" coming soon without its tag: the tag does not fit the 222 of
  the side panel beside "try again"; the frame draws "open log" as a plain link (#328)
- The why line still gives up its link before it cuts its first reason (a narrow reader at 130 % shows
  "groupe…" with no link); the frames draw only the 684 wide line, where both fit (#328)
- The text size slider's names keep 4 apart at least; at 130 % "120 %" and "130 %" stand about 6 apart (#328)
- Home (#335): the frames draw it only in a window 900 high; in a lower one the emblem and the names under it are
  pulled up toward the top of the emblem's place, and leave only where even that leaves no room. The frames' line
  "↳ the field reacts under the pointer here, and only here" under the tagline is a note on the design and is not
  drawn; the names' box keeps its room.
- Privacy (#337): `remote.list` keeps no sender name, so each row shows the address at 16 in up to 616 of a 720 row,
  where the frames draw the name at 16 in 180 and the address at 208 in 424; a row with a name would keep the
  frames' columns.
- A draft kept is written behind: a kill (SIGKILL) or a power loss in the few milliseconds of its write loses that
  keep, and a second close of the window in a row with a draft the disk refused loses its newest text; the disk holds the draft
  as it was before (#338)
- A new event's Alert says the default lead it rings with (10 min before, 1 day before an all-day event) where the
  design's new-event frame says None (#343).
- The local alerts read an event's first 16 alarms, while `event.list` and the `alert` parameter look at all of them:
  an event with more than 16 alarms may show or move one that does not ring here (#343).
- How other calendar programs treat the `ACTION:NONE` alarm (RFC 9074) written for an alert of "none" is not checked
  yet; one that does not know it may ring for it or show an odd lead (#343).
- A full `event.update` (more than the alert) writes every alarm again and drops what is nested inside one, such as a
  location alarm's `VLOCATION`; an update of the alert alone keeps them (#343).
- The small editor lacks the frames' "more room" link over the description, the "also set" line (place and alert,
  which it does not show) and the two buttons that move the typing to the full editor or a window (#344).
- Links in the editor's description are not drawn as links (no cyan underline, no hand pointer, no target on hover);
  a press with Ctrl held copies one all the same (#344).
- A description longer than the 10 000 characters the calendar reads opens read-only in the editor, so the cut text
  never replaces the whole one (#344).
- `event.update` still changes someone else's event when an owner-class client asks; only the editor holds it
  read-only (#344).
- The Flatpak does not install the fonts' licence texts (SIL Open Font Licence 1.1 for Noto, IBM Plex and
  Audiowide) under `/app/share/licenses/`; the notices travel only in the fonts' own name tables (#348, #335).
- Home's greeting (#349): no frame of the design file draws the choice of the word Home greets by; it stands on
  Settings › Sending as a group of its own, "D Your name", in the segmented style of the page's other choices. A
  name written family name first is greeted by its family name with the first word: the order is not guessed, only
  a comma turns it round.
- No frame of the design file draws Settings › Calendar's "All-day alerts ring at" row: it is drawn in the page's
  own row style, with a select under the alerts switch (#353).
- An all-day event's alerts ring here at the hour on the clock, also on the two days a year the clock is put forward or
  back; a device that counts the hours exactly from midnight rings the same alarm an hour earlier or later on those
  days (#353).
- When the hour all-day alerts ring at changes after the old or the new hour has passed today, the event without an
  alarm of its own gets no extra ring and no make-up for that day (#353).
- The event editor's Alert offers the four all-day hours only as the default; one event cannot pick another of them
  (#353).
- A right-to-left word that egui splits across two faces (a Hebrew letter with a point the first face lacks) is
  shaped face by face, and its parts stand in egui's order, left to right (#356).
- Two notes a screen reader hears as laid out, not as written: the note beside a calendar head's title and the
  note under a table left out of a fitted message (`note.text()` of the galley, its words in drawn order and, when
  wrapped, its rows joined by line breaks). Both are UI text and left to right in English; under a right-to-left
  translation they would be heard out of order (#356).
- An address inside a right-to-left line (a translated sentence in a right-to-left language) keeps its parts in
  order but stands as a run of that line: the app has no isolates in the text it draws yet (#356).
- The full editor and the event editor's window have no Place field and no buttons at the foot that move the editor
  to the other two hosts; the small editor has no "more room" link; the `event-editor-full` and `-window` frames draw
  them (#370).
- The description in the full editor and the window grows from 96 to 240 high as in the small editor; the frames give
  it the room left under the fields (314 and 258 high) (#370).
- The event editor's window has no status line of its own ("not saved yet · into Studio" and the cancel key in the
  `event-editor-window` frames) (#370).
- The day beside the full editor shows the events of the days the calendar has read (the view's): a Starts typed far
  from them shows that day's hours without its events; the draft's own block (`draft-picked` in the frames) is not
  drawn in it (#370).
- The event editor's window is not compared with the `event-editor-window` frames pixel for pixel: no picture draws a
  second window (#370).
- A quit from the tray while no window is open ends the app without asking: an event typed in the editor and not
  saved is lost (#370).
- Sending's Undo send has no Off segment (Send at once is a row of its own, coming soon), so at 1000 with 130 % text
  its control keeps beside its words where the Sizes page's `settings-1000-130` frame, with five segments 311 wide,
  stacks it under them (#373).
- In a narrow window the page's head stays put while the rest of the page scrolls under it; Esc on Templates and
  Scheduled, which lead back to Sending, does not go back to the list of sections (#373).
- Appearance › Layout › Write a message offers Pane, Floating and Window, and its help names all three; the
  `settings-appearance` frames draw two segments, Pane and Window (#379).
- The composer's window shows Send's key (schedule, with a time picked) at the right end of its status line where
  the `composer-window` frames show the command line's: the command line, Undo and every key of the list are the main
  window's and rest in the composer's window (#379).
- The main window's command line cannot send a composer in its window of its own: its Send row says no message is
  being written; Send's key and button in the window send it (#379).
- The composer's window is not compared with the `composer-window` frames pixel for pixel: no picture draws a second
  window (#379).
- The reader's window has the subject in its title bar where the design puts the command line, and no status line
  with key hints at its foot; the hairline over its answers spans the reader's 760 column, not the window's 958
  (`screen/reader-window/default`, `unfocused`) (#380).
- In a reader's window Snooze, Move and More are the buttons' only: their keys, the palette and Undo are the main
  window's (#380).
- Each reader in a window of its own keeps its own decoded pictures and laid-out letters under the per-reader caps,
  so the pane and eight windows can hold up to nine readers' worth at once (#380).
- Where messages open in a window of their own, the pane with nothing open still says "Pick a message. It opens
  here." (#380).
- A mail's tables used for layout are drawn as hairline tables, 12 between their columns; the Messages page draws
  them without lines, as sent (`screen/message-receipt/fitted` "two columns of 280", `screen/message-marketing`
  "three columns of 192" 12 apart) (#383).
- A coloured table cell round a link (a common way of drawing a button in mail) is a band round the link's
  paragraph, not the mail's button (#383).
- Every band of a kept background stands 14 above, 16 below and 32 in from the sides of its blocks (at most a sixth
  of a narrow column), 6 between them;
  the receipt's band is 12 above and 14 below (`screen/message-receipt/fitted`) (#383).
- The mail's own button shows a 1 px edge in its ink under the pointer and the focus ring; the Messages page has no
  frame of a hovered mail button. Its target shows in the status float, not in a tooltip of its own (#383).
- Alignment is not read in list items and preformatted text, and quoted blocks keep no background (#383).
- The copy keeps no remote picture, even one the owner loaded for the message, and no link can be followed from it
  (#388).
- Under the portal, a desktop that has no browser for the file says so in its own way; the status line says only
  that the copy was handed on. Outside the Flatpak, `xdg-open` is waited for 0.8 s at most (#388).
- Only Linux opens the original; macOS and Windows do not offer the row (#388).
- A run that is killed, rather than closed, leaves its cleaned copies in their folder until the next start (#388).
- One original opens at a time: a press while one is still being handed on is not queued, the status line says to
  wait (#388).
- Status line (#396): "try again" when offline is coming soon: no command signs the accounts in before their wait
  ends. "Offline since" is the time the window first read the accounts offline, not when the connection dropped.
- Status line (#396): a send the server refused counts as a problem but is never the line's first fact; the line
  names only failing accounts there.
- Status line (#396): on the settings in two panes the frames draw `back` (`Esc`) between mail and commands; there
  `Esc` keeps the settings shown, so the line leaves it out. The sections' list alone in one pane draws it too; the
  line leaves it out there for the same reason.
- Command line (#397): a refused command's reason shows in the status line as a notice that goes after its seconds,
  where the frame `command-line/refused` draws the error state; the words are the window's own ("Not run: Snooze or
  remind me needs an open message. Open one, then try again."), the row's full name where the frame says "Snooze".
  LAST COMMANDS holds the commands of this run of the app only, at most three (the frame draws two); GO TO lists the
  inbox, New senders and the calendar (none of them over the event editor), and with nothing to say leaves a caption
  out. With the line narrowed to the mailbox shown (`Ctrl F`) it lists no results but the prefixes, as before. The
  MAIL head's count is of the messages the list's search finds, which reads at most the engine's list limit per field
  (then "N+"); the PEOPLE count is of at most six people, the most the lookup keeps. The refused state is not among
  the pictures. Over the event editor Enter on a typed search still shows the mail searched, as it did before, and
  leaves the calendar.
- The index shows a Junk row, and a Blocked row while InboxCraft's Blocked folder exists, which the frames do not
  draw (`index/with-counts`: Inbox, New senders, Drafts, Outbox, Sent, Archive, Bin): their mail has no other place
  (#398).
- `sync.status` says whether a sync round runs, not whose: while one account syncs, every account signed in and not
  failing says syncing, and while another account fails, one in its first round says so only once a worker has told
  its progress (#398).
- At 130 % New message is 42 points high, 52.5 pixels at 125 %: the button is not on whole pixels as the index's
  rows are (#398).
- Home (#399): the banner says "offline since" only after a sync of this run: before the first one (an account
  connecting at startup) it says nothing of it, and the status line says offline. Of the failures that stop an
  account until the owner acts, only a refused sign-in is in the banner; an unanswered sign-in, a missing password or
  a certificate refused are the status line's.
- Home (#399): "Offline since" gives the time of the last finished sync, not the moment the connection dropped: a
  mailbox watched quietly from 08:10 until the network went at 13:00 says "Offline since 08:10". `account.list` does
  not give when an account went offline.
- Control channel (#399): an agent's `ui.settings.open` or `ui.home.open`, held for the frame's end, acts after an
  owner's own call of either that came over the channel in the same tick, and so wins over it.
- The first start after the update to schema version 6 indexes the links of every message before the window shows,
  with nothing shown meanwhile: about 1.4 s for 100 000 messages on a fast laptop, more on an ordinary one (#406)
- Later's snoozed pile has a third head, "Later", for what comes back after this week; the frames (`later/snoozed`)
  draw "Back today" and "Back this week" only. Scheduled sends are not in Later: no frame of Later draws them, and
  they stay in the Outbox (#410).
- The screener card's line is drawn on the first point of the line under it, where the frame gives it a point of its
  own (a 64 card, its line, then the rows): every line of the list stays on whole pixels at 125 %, 150 % and 200 %,
  and the lines under the card stand 1 higher than the frame's (#410).
- A desktop that does not keep to a window's least size (some tiling ones) can still make the window narrower
  than 800; it is then laid out 800 wide and cut at its right. No frame draws a narrower window (#412)
- Pictures against frames (`compare.py`'s measure, 125 scenes in both modes): on the frames' sample, with the own
  window frame, the mean is 0.860 (0.850 on the present sample with the system frame) and 90 scenes stay under 0.90
  in a mode. The window controls now match (0.98 in that corner, from 0.51); what keeps the scenes there is the
  status line (0.82 in that strip: the place's hints where the frames say next, previous, done, snooze) and the
  body's own differences each scene's pair lists. The outbox and Scheduled pictures list the waiting message for
  Tuesday 08:00, as their frames do, so their status line says "one goes at 17 Nov" where those frames say
  "outbox 1" (#416).
- The frames' sample has no mail in the Studio folders Clients (4), Invoices 2026 (1) and Open day (12) the index of
  the frames counts, and the Drafts place lists the message in the outbox ("Thank-you cards: the proof", written
  yesterday) beside "Price list for the winter workshops", where the frame lists "Re: Kiln service: two dates to
  choose from": a message waiting to be sent stays a draft until it goes (the drafts list holds every draft), so
  the index's "Drafts 2" holds only with one kept draft (#416).
- The frames' sample's why-line says "grouped as People · a sender you know" where the frames say "let in by you on
  3 Nov · grouped as People · synced 14:02", and the attachment's size is the file's own (1.2 MB of a blank PDF);
  the month and agenda frames' events outside the week of 16 November are not in it (#416).
- Home's emblem first twinkles a quarter second after Home shows, where the motion source starts at once: the
  headless pictures draw a few frames of Home and must show it at rest (#418).
- The character field flips as strongly next to Home's text as away from it: the motion source calms its cells
  near text (to 0.2 of the chance and of the opacity, over 80 points), the field knows nothing of the text above it
  (#418).
- At the reduced level Home's field answers within 30 points, as the design says, and with 0.4 of the chance, as
  the motion source does; the source keeps its full 60 points there, so about a quarter as many cells flip (#418).
- The account form's note that saving signs in to a moved server with the stored password stands in `warn` in the
  caption's place, as `account-edit/new-password` draws the note for a new password; no frame draws the stored
  password's case (#422).
- When an account is removed but the system's credential store keeps its password, the status line says so only in
  a notice that goes after 4 seconds; nothing keeps it on screen, and nothing says later whether the password was let
  go when it is tried again (#422).
- Short of room, a segmented switch's names keep 4 on each side instead of the design's 12; no frame draws the
  control in less room than its names and pads want (#427).
- At 130 % the screener's "Before the inbox" is cut by the 136 the design sets an option's name to, though the room
  is there; whether that 136 grows with the text size waits for the owner (#427).
- In New senders the screener's switch stands under its label, 8 below it, where the line has no room for both, as
  at 130 % in a 1024 window; `screen/new-senders/before-inbox` draws it on the label's line only (#427).
- Beside the reader the search line shows only the search's words when "in message text" and its mark do not fit
  after them; `screen/search-results/results` draws the scope line alone (#427).

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
| 2026-11-04 | minor | Rough edges of charsets and encoded words (#16) |
| 2026-11-04 | minor | Live test harness: the wider leak scan, a server without a hierarchy delimiter (#10) |
| 2026-11-04 | minor | Rough edges of the mailbox model (#17) |
| 2026-11-04 | minor | Invisible marks in addresses and folder names (#17) |
| 2026-11-04 | minor | Rough edges of the message reader (#18) |
| 2026-11-04 | minor | Message reader: whole-message errors, unsafe file names, reports read from direct parts (#18) |
| 2026-11-04 | minor | What the message builder does not write (#19) |
| 2026-11-04 | minor | The message builder refuses attached messages other than `message/global` (#19) |
| 2026-11-04 | minor | Rough edges of the IMAP client core (#20) |
| 2026-11-04 | minor | The IMAP client refuses message data outside a mailbox and drops alerts before TLS (#20) |
| 2026-11-04 | minor | IMAP server words before TLS, password pieces, capabilities during a sign-in (#20) |
| 2026-11-04 | minor | Rough edges of the IMAP sync extensions (#21) |
| 2026-11-04 | minor | Rough edges of IMAP move, UIDPLUS, preview and compression (#22) |
| 2026-11-04 | minor | Rough edges of the fake IMAP server (#23) |
| 2026-11-04 | minor | Limits of the local-first store and its search (#24) |
| 2026-11-04 | minor | Who may read the local-first store on unix and Windows (#24) |
| 2026-11-04 | minor | What is left of removed mail on the disk (#24) |
| 2026-11-04 | minor | Symlinks in the store and values read back unbounded (#24) |
| 2026-11-05 | minor | Rough edges of TLS, the IMAP sign-in and the first sync (#25) |
| 2026-11-05 | minor | Live updates, resync, back-off and COMPRESS in `net` (#26) |
| 2026-11-05 | minor | The caps on what a resync keeps (#26) |
| 2026-11-05 | minor | An unanswered sign-in halts the tries (#26) |
| 2026-11-05 | minor | The engine read path and the command registry (#27) |
| 2026-11-05 | minor | Rough edges of the account check and Detect (#28) |
| 2026-11-05 | minor | An agent's check after a refused sign-in waits for the owner (#28) |
| 2026-11-05 | minor | The live tests use port 993 on both accounts (#57) |
| 2026-11-05 | minor | Rough edges of the safe HTML display (#29) |
| 2026-11-05 | minor | What the link comparison reads past (#29) |
| 2026-11-05 | minor | Rough edges of the first window and the sample mailbox (#30) |
| 2026-11-05 | minor | Rough edges of the dev Flatpak (#31) |
| 2026-11-05 | minor | The Flatpak is built by the CI bundle job (#32) |
| 2026-11-05 | minor | Rough edges of the reader and remote pictures (#33) |
| 2026-11-05 | minor | The Flatpak opens Downloads for Save (#33) |
| 2026-11-05 | minor | The reader forgets decoded pictures past a budget (#33) |
| 2026-11-05 | minor | Picture limits per message: inline cap and decoded-bytes budget (#33) |
| 2026-11-05 | minor | What a forged `From` of an allowed sender learns (#33) |
| 2026-11-05 | minor | `cargo xtask ci` runs the tests once, under coverage (#66) |
| 2026-11-05 | minor | Onboarding's rough edges; the window applies worker events (#34) |
| 2026-11-05 | minor | Passwords go at window close; the password field keeps no undo history (#34) |
| 2026-11-06 | minor | Rough edges of the theme tokens and presets (#81) |
| 2026-11-06 | minor | Limits of the side-by-side pictures (#81) |
| 2026-11-06 | minor | Limits of glass and floats (#82) |
| 2026-11-06 | minor | Glass and floats: resizes no longer rasterise the shadows; no float shadow over sharp text (#82) |
| 2026-11-06 | minor | Figtree and the text scale: what is left, the fallback scripts, Ubuntu Light still compiled in (#83) |
| 2026-11-06 | minor | Icons: one set, the ids so far, whole fonts bundled, no gradient through a glyph (#84) |
| 2026-11-06 | minor | Components and motion: what is used yet, one key, the reduced-motion default (#85) |
| 2026-11-06 | minor | Reduced motion: the mouse wheel's smoothing keeps running (#85) |
| 2026-11-06 | minor | Appearance: the commands and keeping them exist, the controls wait for the settings page; Lucide's disc; the kept file (#86) |
| 2026-11-06 | minor | Appearance settings page: what it lacks, the owner's own font or icon files an idea only; the fit test's reach (#87) |
| 2026-11-06 | minor | Message list: the showcase-only parts, what waits for E2.1, E3.1 to E3.3 and E1.17c; the rail's reach (#88) |
| 2026-11-06 | minor | Hover actions: on the sample only, Done's stand-in meaning, one undo, the keys fixed (#89) |
| 2026-11-06 | minor | Reader: which toolbar actions act, what waits for which task, the inner buttons, no picture of More (#90) |
| 2026-11-06 | minor | The app's own window frame, the rail's views not built yet, the folder pane's own folders (#104) |
| 2026-11-06 | minor | The own frame: round corners, resizing on Wayland, moving in the top row (#104) |
| 2026-11-06 | minor | The settings' Accounts page and what it lacks (#104) |
| 2026-11-06 | minor | The settings sheet: only Appearance among its sections (#104) |
| 2026-11-06 | minor | Lucide's Fill without a disc (#104) |
| 2026-11-08 | minor | Encodings of the password the IMAP client does not recognise (#64) |
| 2026-11-08 | minor | The fold writes document by document; a new command module edits `cmd/mod.rs` (#111) |
| 2026-11-08 | minor | The store on disk: passwords stay, kept jobs, the schedule and senders, two windows (#113) |
| 2026-11-08 | minor | Moves without COPYUID, the undo history per session (#114) |
| 2026-11-08 | minor | Writes run again after a reconnect; APPEND twice, no UIDPLUS, no UNCHANGEDSINCE yet (#115) |
| 2026-11-08 | minor | Draft copies without Bcc or threading headers; unnamed copies; a failed upload waits (#116) |
| 2026-11-08 | minor | What the SMTP client leaves out; who sets SMTPUTF8 (#117) |
| 2026-11-08 | minor | The fake SMTP server: STARTTLS, what it leaves out, its delivery reports (#118) |
| 2026-11-08 | minor | Sending without undo or retry, the Sent copy, the SMTP step; the form asks for no SMTP server (#119) |
| 2026-11-08 | minor | Quotes of HTML-only mail, English quote lines, templates per account, aliases on reply all (#120) |
| 2026-11-08 | minor | Bounces linked by the Sent copy's `Message-ID` only, reports let past the screener (#121) |
| 2026-11-08 | minor | `Offsets` without a zone lookup (#122) |
| 2026-11-08 | minor | Undo send outside the undo history, refused send laters told only in the outbox, a minimized window's wake not tried by hand (#123) |
| 2026-11-08 | minor | The screener's mode per session, delivery reports by sender, flat folder lists (#124) |
| 2026-11-08 | minor | The grouped inbox: calendar parts known from kept bodies, choices per address, list cost (#125) |
| 2026-11-08 | minor | Done, park and pin in the local store only, lost without UIDPLUS, no archive fallback (#126) |
| 2026-11-08 | minor | Snooze without the field for mail not downloaded or without UIDPLUS, the copy's arrival time (#127) |
| 2026-11-08 | minor | Follow-up reminders in the local store only, told by In-Reply-To and References (#128) |
| 2026-11-08 | minor | The time picker: English words, one fixed offset, presets from the caller, the alert value per run (#129) |
| 2026-11-08 | minor | The palette lists the window's actions; keys are no engine command yet; the calendar's empty state (#130) |
| 2026-11-08 | minor | Hover actions and the reader's toolbar act on a real account (#131) |
| 2026-11-08 | minor | Composer limits; Reply waits for #133 (#132) |
| 2026-11-08 | minor | The reader's Reply row works; reply keys and the template settings page wait (#133) |
| 2026-11-08 | minor | The screener and the groups show the engine's data; the sample writes the inbox view too (#134) |
| 2026-11-08 | minor | Done all, Done, Snooze and Pin work; the hover Snooze waits (#135) |
| 2026-11-08 | minor | The countdown follows the last send, the outbox opens from the pane only, the bounce banner without Edit and resend (#136) |
| 2026-11-08 | minor | Tray and autostart outside Linux, the windowless half second, the Background portal in use (#137) |
| 2026-11-08 | minor | Notification choices not kept, the scheduler not driven, Linux only, no notice without names (#138) |
| 2026-11-08 | minor | No pause of notifications, a revoked portal permission not noticed (#139) |
| 2026-11-08 | minor | The CLI runs no jobs headless; the `--version`-only line names the web app alone (#140) |
| 2026-11-08 | minor | The MCP server: store only, no jobs, text results, no group column, no HTML text, no store lock (#141) |
| 2026-11-08 | minor | The control channel: Linux only, no peer or process check, no approvals; an agent's act ends the window's Undo (#142) |
| 2026-11-08 | minor | `ical`: zones, recurrence, what an event drops (#143) |
| 2026-11-08 | minor | `dav` and the fake CalDAV server: Basic only, no privileges, sync limits (#144) |
| 2026-11-08 | minor | The CalDAV step of the check, Detect proposes no CalDAV address, calendar sync limits (#145) |
| 2026-11-08 | minor | Calendar commands: no named zone for new events, whole series only, guests not mailed, conflicts, a write waiting on a lost sync, servers that ignore SCHEDULE-AGENT (#146) |
| 2026-11-08 | minor | The calendar's views: UTC clock, Monday weeks, timed blocks over days, no keys, hidden calendars not kept (#147) |
| 2026-11-08 | minor | The event editor: typed times, no place or zone, whole series only, guests not mailed (#148) |
| 2026-11-08 | minor | Invitations: read from kept bodies, occurrences, methods without an answer (#149) |
| 2026-11-08 | minor | Event reminders: no switch on the Notifications page, left-out alarms (#150) |
| 2026-11-08 | minor | The ranked table filled for the daily-use epic; stale lines on the scheduler, alerts and the composer edited (#151) |
| 2026-11-08 | minor | The solid float: the old float and selection lines are gone (#205) |
| 2026-11-08 | minor | The add-account form asks for the SMTP server and the CalDAV address (#214) |
| 2026-11-21 | minor | Presets, custom accent, glass, Inter and Lucide gone from the rough edges (#231) |
| 2026-11-21 | minor | IBM Plex type, its fallbacks and row height; Phosphor Regular only, its colours as drawn against the design; the tray's cyan flap and its one drawing (#232) |
| 2026-11-21 | minor | Text IBM Plex does not hold takes the computer's own fonts (#233) |
| 2026-11-21 | minor | A selected icon button shows the accent, as the design has it (#233) |
| 2026-11-21 | minor | Pressed states the design does not draw yet (#233) |
| 2026-11-21 | minor | Looks of the toggles and choices the design does not draw yet (#233) |
| 2026-11-21 | minor | The quote fold and the menu dividers still to come (#233) |
| 2026-11-21 | minor | The looks of fields, menus and pickers the design does not draw yet; the hover Snooze opens the time picker (#234) |
| 2026-11-21 | minor | Shared rows and blocks: pressed hover action, Snooze, old rows, two rings, motion off, a banner link's cut, where the caption's baseline holds (#235) |
| 2026-11-21 | minor | The rows to the design: a row's act at once, the gaps of the lists' rows (#236) |
| 2026-11-21 | minor | Lists build only the rows in sight: what a screen reader sees, the folded card's rows, the 1 000 cap (#236) |
| 2026-11-21 | minor | The reader with nothing open, its why line, content bar, attachments and wipe (#237) |
| 2026-11-21 | minor | The composer drawn as its frames: what differs and what is not drawn (#238) |
| 2026-11-21 | minor | The settings sheet's sections and their coming-soon rows; the preview is gone (#238) |
| 2026-11-21 | minor | The outbox row's addresses and changed note, a grown chip and line are not in the design (#245) |
| 2026-11-21 | minor | What the dev container's compile cache does not hold (#246) |
| 2026-11-21 | minor | The keyboard after a server removal of a card's only row; the palette over the event editor; the body's size in design.md; the index and the regions (#254) |
| 2026-11-21 | minor | The brief `offline` of a reconnect; a failing side folder (#255) |
| 2026-11-21 | minor | What `mail.why` cannot say yet (#256) |
| 2026-11-21 | minor | The rulings' new lines the design file does not draw (#259) |
| 2026-11-21 | minor | Detect probes its candidates; the provider domains of the MX hosts and their limits (#260) |
| 2026-11-21 | minor | The two presets on the same Saturday (#265) |
| 2026-11-21 | minor | The why line: no links yet, the thread count and the times as asked (#267) |
| 2026-11-21 | minor | Calendar sync line: what the time means, the log, synced (#271) |
| 2026-11-21 | minor | The character field's five rough edges (#275) |
| 2026-11-21 | patch | Ids above `i64::MAX`: unknown when looked up, refused when kept (#276) |
| 2026-11-21 | minor | The add form's Detect lines the design file does not draw; how soon a stopped run ends (#279) |
| 2026-11-21 | minor | A store failure outside a window command leaves its database words unlogged; the settings as a place: the keyboard on leaving, About's columns, narrow windows (#284) |
| 2026-11-21 | minor | Command field and window controls: what the code does not follow yet (#286) |
| 2026-11-21 | minor | The status line's sets: pressed item, drawn marks, key chip, log, narrow message (#287) |
| 2026-11-21 | minor | Home's sets: pressed states, cut labels, the room at 130 % (#288) |
| 2026-11-21 | minor | Scripts no font holds are boxes; which ones (#290) |
| 2026-11-21 | minor | A toast with Undo covers the reader's answers and takes the presses on them; a toast's words do not reach a screen reader (#292) |
| 2026-11-21 | minor | Home: the numbers without a source (#299) |
| 2026-11-21 | minor | Home: a place now; sample mail, the failing account, the composer's Discard, no control channel (#303) |
| 2026-11-21 | minor | The command line: what of the frames is not built yet; the palette's line names it; the own frame's title bar is the command line (#305) |
| 2026-11-21 | minor | The status line: what of the frames is not built yet; the countdown and the late send's note are the line's (#309) |
| 2026-11-21 | minor | The widths: the index narrows, the lines give up room, the one pane swaps the calendar's side panel (#312) |
| 2026-11-21 | minor | Motion: the three levels on the Appearance page, what each does until it has its own movements (#313) |
| 2026-11-21 | minor | Drafts open again from the drafts place; attachments stay a gap (#314) |
| 2026-11-21 | minor | Templates, Scheduled, the always-load senders and the calendar alerts switch: what of the frames the pages do not follow; removing an account (#317) |
| 2026-11-21 | minor | Motion: the hover scramble on text buttons only; a change of place, icons excepted (#318) |
| 2026-11-21 | minor | Later's heads and a wide table's focus ring, which the design file does not draw (#323) |
| 2026-11-21 | minor | The picture round's remaining differences from the design (#325) |
| 2026-11-21 | minor | The picture round's leftovers: the sync line's open log, the why line's link, the slider (#328) |
| 2026-11-21 | minor | Home: the emblem in a window lower than the frames' and the caption under the names (#335) |
| 2026-11-21 | minor | Privacy rows without a name (#337) |
| 2026-11-21 | minor | A kill loses a keep in flight (#338) |
| 2026-11-21 | minor | Text size: the keys that step it (#341) |
| 2026-11-21 | minor | The Alert field waits for the full editor and the window (#343) |
| 2026-11-21 | minor | The event editor's hosts, its small editor and its description (#344) |
| 2026-11-21 | minor | The shipped Noto faces; the computer is searched for 18 more scripts (#348) |
| 2026-11-21 | minor | Home's greeting has no frame; the name's order is not guessed (#349) |
| 2026-11-21 | minor | No frame draws the all-day alert row (#353) |
| 2026-11-21 | minor | Right-to-left words are ordered and cut at their logical end in the app's labels; three rough edges of it (#356) |
| 2026-11-21 | minor | The event editor's three hosts: what the full editor and the window do not draw yet (#370) |
| 2026-11-21 | minor | Narrow Settings: Undo send without Off, the head that stays, Esc under Sending (#373) |
| 2026-11-21 | minor | The composer in a window of its own: the row's three hosts, its key hint, the palette's Send (#379) |
| 2026-11-21 | minor | A window opened again uploads the reader's pictures again: the #165 clause is gone; the reader's window (#380) |
| 2026-11-21 | minor | `html` reads a block's alignment, background, widths and button links (#383) |
| 2026-11-21 | minor | The cleaned copy is the one browser hand-off; what it keeps (#388) |
| 2026-11-21 | minor | The status line: offline since and try again, the worst first, the place's keys, dismiss by Esc, a live region (#396) |
| 2026-11-21 | minor | The command line: what of its frames' search, GO TO and refusal is not as drawn (#397) |
| 2026-11-21 | minor | The index: Blocked and Junk rows, whose sync a row says, New message at 125 % (#398) |
| 2026-11-21 | minor | Home: when the banner says offline, which failures it names, what "since" means; an agent's held ask after the owner's (#399) |
| 2026-11-21 | minor | The first start after schema version 6 indexes the links with nothing shown (#406) |
| 2026-11-21 | minor | Later's third head and scheduled sends; the screener card's line (#410) |
| 2026-11-21 | minor | A desktop that ignores the least size; the title bar under the results and the editor (#412) |
| 2026-11-21 | minor | The pictures against the frames on the frames' own sample (#416) |
| 2026-11-21 | minor | The emblem's first twinkle, the field near text and its reach at reduced (#418) |
| 2026-11-21 | minor | The account form's note for a moved server; a kept password's removal notice (#422) |
| 2026-11-21 | minor | Lines checked against the code at the epic's end: the fixed parts of the live tests, IMAP, the fake server, the index, the reader, the composer and the calendar taken out (#425) |
| 2026-11-21 | minor | Segmented names short of room, the 136 at 130 %, the screener switch and search line beside the reader (#427) |
