# Where InboxCraft falls short

> **Last reviewed:** <date> · **Last updated:** 2026-11-08 · **Change:** minor

## Ranked
| # | Gap | B | Evidence | Impact | Hours | Doc |
|---|---|---|---|---|---|---|
| 1 | The add-account form asks for no SMTP server and no CalDAV address, and nothing else the owner can reach sets them | ✓ | `onboard.rs` sends `account.add` with the IMAP fields only; `account.add` and `calendar.server` are `owner`, which the CLI refuses | An account added today cannot send and has no calendar | 4 | [Onboarding](#known-rough-edges-not-ranked) |
| 2 | No front end hands the engine the computer's time zone: presets, send later, snooze, reminders and the calendar run in UTC | ✓ | `Session::set_offsets` has no caller | "Tomorrow 8:00" and every event are off by the UTC offset, an hour or two in Europe | 4 | |
| 3 | The composer cannot open a draft closed earlier or left from an earlier run, and has no attachments | ✓ | `ui-egui` has no drafts list and no attach field (#132) | A reply started yesterday or a mail with a file must go through another program | 6 | |
| 4 | Only the inbox is watched (IMAP IDLE, no NOTIFY); other folders change when the engine syncs them | | `net` watches one folder per account (#26) | Mail filed by a server rule or another device shows late | 6 | |
| 5 | Sign-in with a password only: no OAuth (XOAUTH2, OAUTHBEARER) for IMAP, SMTP or CalDAV | | `proto` and `dav` have no OAuth (#20, #144) | Providers that require OAuth cannot be added | 12 | |
| 6 | One message at a time: no selection of several messages | | `ui-egui` acts on the open message or the hovered row (#131) | Clearing a full inbox takes one act per message | 6 | |
| 7 | A flag changed here overwrites one changed on another device meanwhile | | `net` sends no `UNCHANGEDSINCE` (#115) | Rarely loses a flag change made elsewhere | 3 | |
| 8 | Repeating events change and delete as a whole series only | | no `RECURRENCE-ID` writes (#146, #148) | Moving one meeting of a series moves them all | 8 | |
| 9 | Search looks at subject, sender and recipients only, not the body text | | the search field runs `mail.search` per field (#27) | A message is found only by its header | 6 | |
| 10 | Tray, notifications, single instance and the control channel exist on Linux only | | no Windows or macOS code (#137, #138, #142) | InboxCraft is for Linux desktops for now | 30 | |

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
  ASCII, and nothing sets it from the message yet (#19, #117).
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
  LITERAL+). It has no UTF8=ACCEPT, NOOP, CLOSE, UNSELECT, ESEARCH, APPEND, STORE (other than inside a move),
  XOAUTH2 or OAUTHBEARER yet; APPEND and APPENDUID come with drafts (E2), the rest when a caller needs them and with
  OAuth (#20, #21, #22).
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
- `proto` CONDSTORE and QRESYNC: no STORE `UNCHANGEDSINCE` and `[MODIFIED]` (STORE is not there yet), no SEARCH
  `MODSEQ`, no sequence match data in the QRESYNC parameter, and an untagged `[HIGHESTMODSEQ]` outside SELECT is not
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
  answered `BAD`. It has no APPEND, CREATE, DELETE, RENAME, SUBSCRIBE, LSUB, UNSELECT, NAMESPACE or ID, no
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
  instance guard, which is not built yet. The memory backend copies its rows on every batch, which is fine for tests and
  the browser build but not for a large mailbox (#24).
- `store` ids above `i64::MAX` are refused by the SQLite backend; the memory backend takes them. A source file
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
- `engine`'s `mail.read` shows the plain text part only; a message with HTML alone shows no text (`has_html` says
  so) until the reader shows the display blocks of `html`. Its body words are not searched either (#27).
- `engine` asks for a body once per message while that `FetchBody` job is out; a job the worker never answers
  (neither the body nor a failure) keeps the next `mail.read` from asking again until the session restarts. A body
  over 64 MiB is not kept and not asked for again until the session restarts, so such a message cannot be read in
  InboxCraft. Opening a
  message marks it opened in the store, so its body is kept, also when an agent opens it (#27).
- `engine` lets an agent run `safe` commands only; per-account access levels and the owner's approval for `owner`
  commands come with the first `owner` command (#27).
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
- Detect stops at the first source that names an IMAP server and does not probe the candidates; the common host
  names are hints, never checked until the owner asks. Detect proposes no CalDAV address: the CalDAV SRV records and
  `/.well-known/caldav` at the address's domain (RFC 6764) are not asked, and no CardDAV (#28, #145).
- The ISPDB is off by default, so Detect tells Mozilla nothing unless the owner switches it on; with it off, a
  domain whose provider publishes no autoconfig and no SRV records gets only the common host names as hints (#28).

- `html` reads no CSS beyond whether an element is hidden: colours, fonts, sizes, alignment and the message's
  `<style>` blocks are ignored, so a message keeps its words and structure but not its look. Text hidden by other
  tricks (a zero font size, white on white, off-screen positions) is shown (#29).
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
  (`io.github.qubernetic.inboxcraft`, `account-<store id>-<id>`) until the account is removed in "Change settings";
  a crash or a store that refuses then leaves the entry behind, and `PRIVACY.md` says how to remove one by hand. An
  entry an older build left (`account-<id>`) is never read again and stays until removed by hand (#34, #113).
- Dates in the list are shown in UTC: the `time` crate has no local time zone yet (#30).
- The search field looks at subject, sender and recipients in every folder of the accounts shown, not at body text
  and not only in the open folder; it runs one `mail.search` per field and merges them. The query language comes
  with E5.1 (#30).
- The arrow keys step through the list only while no control has the keyboard focus; Tab moves the focus through
  every folder and row, and Enter or Space selects (#30, #33).
- A link in a message is never opened: the window has no browser hand-off (egui's `links` feature is off, and
  turning it on is the project owner's call), so a click copies the address and the float over the reader shows
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
- Bold text in a message is set in Figtree SemiBold; italic text is slanted by egui, as Figtree's italics are not
  bundled (#83). Tables are laid out as a grid without borders, four deep at most (#33).
- `mail.thread` reads every message of every account to group the thread; fast enough for the sample and small
  mailboxes, an index comes with the store on disk (#33).
- The next-message rule applies when the open message leaves the list after a refresh; archive and delete, which
  will use it most, come with E2. Bodies that arrive while a message is open show the next time it is opened (#33).
- The reader cannot be tested on a real double click in `egui_kittest`, so the list has no double-click shortcut to
  full width; the Full width button is the way (#33).
- The window's typeface is Figtree (four weights, #83). Scripts it and egui's fallback fonts lack (CJK, Arabic,
  Hebrew, Devanagari and others) show as boxes, and right-to-left text is laid out left to right (egui has no
  bidi). egui's Ubuntu Light is no longer used but is still compiled in with the `epaint_default_fonts` crate,
  which also brings Hack and the emoji fonts (Ubuntu Font Licence 1.0, not an OSI licence; allowed in `deny.toml`
  for that crate only, the project owner's to confirm); leaving it out means replacing that crate's other fonts too.
- Text size and typeface (#83, #86, #87): the text scale (90 % to 130 %) and the typeface (Figtree or Inter) are
  set on the appearance settings page or through `ui.text_scale.set` and `ui.font.set`, and kept on disk. The fit
  test draws the list, the reader, the add-account page and the settings page in both typefaces at 90 %, 100 % and
  130 %, in a window 1200 points wide (and the settings page 820 points wide); a narrower window is not checked.
  Spacing and paddings stay
  on the grid at every scale; only text and the heights that hold it grow. Loading the owner's own font file is an
  idea only: it would be an outside file, needing the hostile-input rules (safe parsing, size caps, a fuzz target)
  and handling of missing glyphs.
- Icons (#84, #86): Phosphor and Lucide, chosen with `ui.icons.set`; the settings control comes with E1.16b. Lucide
  has no solid icons, so its Fill is the line glyph (an active or selected one reads by its colour and the selection
  behind it, less boldly than Phosphor's solid shape) and its Duotone draws a faint disc behind the lines. The set has
  the ids the window and the triage actions need so far; a toolbar or a view that needs another meaning adds an id
  and its glyph (in both sets). Only the flag, the attachment tile and the add-account steps use icons yet; the
  toolbars, the folder pane and the empty states move onto them with E1.17. The three Phosphor fonts are bundled
  whole (about 1.5 MB, every Phosphor icon) rather than cut down to the glyphs in use, so a new id needs no new
  font file. An icon is one colour token (Duotone's back at 20 % of it); a gradient through a glyph (the gradient
  helper on the icon's text) is not done. The unread dot and the account dots and
  stripes stay shapes, as they are colour markers rather than icons. Loading the owner's own icon pack is an idea
  only (an outside file: safe parsing, size caps, a fuzz target, an SVG rasteriser).
- Appearance (#86, #87): the six `ui.*.set` commands, the file that keeps their result and the settings sheet that
  runs them exist; of the sheet's sections only Appearance, Accounts and Shortcuts (#130) are built, the others are faded and say so (#104). The Accounts page lists the accounts and adds one; an account's own page (the check on demand, its folder roles, removing it) and the account colours switch are not built, and a row shows no sign-in state. The page has no command of its own: it opens from the rail's settings button, Ctrl+Comma (#88)
  or `InboxApp::open_settings`. The custom accent is typed as `#rrggbb` or picked with egui's own
  colour picker, which shows its controls in English and with no accessible names of its own beyond the field's
  label. The preview shows invented rows, not the owner's mail. Reduced motion has an on/off switch; going back to
  "follow the system" after a choice is possible only through `ui.motion.set` with `reduce: system`. Loading the
  owner's own font file or icon pack is an idea only and the page offers neither: both are outside files, needing
  the hostile-input rules (safe parsing, size caps, a fuzz target, an SVG rasteriser for icons) and handling of
  missing glyphs. The file sits in the config folder and is written in the sample window too: it
  holds no mail and nothing about an account, so the sample mode writes these files (the look, the keys and the inbox
  view of `triage.json`, never the screener's mode) and nothing else. A custom
  accent is checked only as accent text on the dark surfaces; every other token derived from the four colours is
  made readable by the theme itself, which may move it further than the answer of `ui.accent.set` says. Two windows
  open at once each keep their own last change; the later write wins.
- Message list (#88, #134): the screener card and the grouped inbox show the engine's data (#134); "Done all" on a
  group card marks the whole group done with one Undo, but none for a group that takes more than the 100 acts the
  undo history holds (#135). In the grouped inbox (cards or grouped list) the arrow keys still step through
  the list in date order, not in the order of the groups, a card shows its first three rows while the keys can select
  one hidden under "Show all", and every row is drawn each frame (the simple list draws only the rows in view). The
  group menu is a right click on a row or the reader's More menu; it has no key. The rail
  holds Mail, the calendar and the settings; the other places of the brief (the owner's own avatar) come with
  their features. A row shows the first sender's avatar from initials only.
- Hover actions (#89, #131): Done, read or unread and Delete act on a real account as on the sample, through the
  engine's commands (#131). Done marks the message read and done (`mail.done`, #135); on a done row under "Show
  done" it is "Not done". Flag and Archive have keys (S, A) but no hover button: the design's hover group is Done,
  read, Delete and Snooze, and the hover Snooze stays disabled ("Open the message to snooze it") because the time
  picker opens from the reader only (#135). The leaving row slides out but the rows below jump up rather than
  close the gap smoothly. The keys (D, A, R, S, Delete, Ctrl+Z) can be changed on the Shortcuts page (#130).
- Reader (#90, #131): Done, Archive, Delete and Move to a folder act on the open message, flag and read or unread
  from More (#131); Reply, Reply all and Forward open the composer filled in (#133). Snooze (H) and Pin act on the
  open message (#135). More also holds Full width,
  Close and the next-message rule; Escape leaves full width but does not close the
  reader. The buttons inside a message (Details, Fold, the quote toggles, the remote-content banner, Save) are
  still egui's own buttons in the theme's style, not the design system's text buttons. `ui_shot` cannot open the
  More menu or the Move menu or hover a link, so none of them has a picture; the UI tests cover them. The thread's earlier messages fold
  to one line, but the open message is the selected one, not always the latest of the thread.
- Components and motion (#85): the card, icon button, pill, tooltip, avatar, badge, hover action group, toast and
  popup exist and are shown in the `ui_shot` gallery; the list uses them since #88 and the reader since #90. The keymap knows three keys of its own (Ctrl+F for the search, Ctrl+1 for Mail, Ctrl+Comma for the
  settings); the arrow keys and Escape are still read directly; the other keys are changed on the Shortcuts page
  (#130). Reduced motion is set with
  `ui.motion.set` (on, off or follow the system) and kept on disk; its switch comes with the settings page (E1.16b).
  The desktop cannot read the system's wish for less motion (GNOME and KDE keep it differently and the portal has
  none), so there motion is on; the web build has no window yet, so nothing reads `prefers-reduced-motion` so far.
  Reduced motion stops egui's fades and its scrolling to a place, but not its smoothing of a mouse wheel's notches
  (about 0.1 s, fixed inside egui 0.37 with no setting), so a wheel still scrolls with a short glide.
  The row slide-out token waits for the Done of E1.17. A menu row shows its key as text, which a screen reader does
  not read with the row's name. Avatars show initials only; pictures of people (from a vCard or a sender's own
  picture) are not read.
- The mode and the accent preset can be changed only from code (`InboxApp::set_mode`, `set_preset`) and the
  `ui_shot` scripts: the commands, the settings page and keeping the choice on disk come with E1.16, so the window
  follows the system's mode in Electric. The error colours are fixed per mode, not derived from
  the preset. The app icon keeps its own teal tile and orange dot, the same in every preset (#81).
- Account colours are the preset's three stops, its accent and two mixes, moved in lightness to 3:1; with Aurora in
  light mode the accent becomes a dark cyan, and in a preset two of them can look alike at a glance. The stripe
  repeats what the folder pane says in words (#81).
- `ui_shot` fills egui's triangles on the CPU and blends in gamma space, close to but not pixel-identical with the
  GPU window. The window itself was never opened in the agent sandbox (no display); the owner's first launch is
  E1.13c (#30).
- Glass and floats (#82): nothing in the window floats yet except egui's menus and tooltips (`float over text`);
  the 80 % float and the text that fades before it are built and tested but first used by the hover actions
  (E1.17). Menus and tooltips cast no shadow (egui's own shadow is a gradient outside the helper, so it is off).
  The gradient's full strength shows nowhere yet (the navigation rail comes with E1.17); the selection gradient is
  the only one in the list. A yellow field over a near-black background reads olive rather than amber (Citrus in
  dark), because the fields are mixed in sRGB. Layer shadows reach under the neighbouring layer's edge by a few
  points. A resize still rasterises the colour fields again each time the window crosses a step of 128 pixels,
  and the selection gradient whenever the selected row changes width; layer shadows are size-free nine-slices, but
  their edges repeat a 32-pixel strip, dither noise included. A float over sharp text casts no shadow, so a menu
  over the reader stands out by its colour and edge alone.
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
  - The add-account form asks for IMAP only: `account.add` takes a submission server (#119), but the form does not
    ask for one yet, so an account added in the window cannot send; nor does it ask for a CalDAV address (#145).
    Both `account.add` and `calendar.server` are `owner` commands, which the CLI and the control channel refuse,
    so nothing the owner can reach sets them for an account yet.
    Detect fills in its first IMAP proposal; the others are not offered to choose from.
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
  at each edge and corner. In the top-row frame a drag moves the window only in the band of the panes' first rows
  where no control takes the press; the strip moves it by any drag. The desktop's own snapping and its window menu
  (right click on a title bar) are not offered. Pictures and the web build draw no
  frame, so the screens in `docs/images` show the window's content only.
- **The rail** (#104) offers Start and Pins before those views exist: they are disabled and say "Not available
  yet". Calendar shows the calendars, or their empty state while there is none (#130, #147). The owner's initials at its foot come from the first account's address, as no account holds a
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
- The palette and the Shortcuts page list the window's own actions (`keymap::actions`), each running the engine's
  commands it needs; the engine's commands that need a message, an account or other parameters are not listed one
  by one. Changing a key is not an engine command yet, so the control channel and MCP cannot change one. A key
  given to an action may also be one the window reads directly (the arrows, Escape, Enter); both then run. egui
  matches a key with Shift or Alt held to the key without them when no other action has that one. The calendar
  screen shows its empty state while no account has a calendar (#130, #147).
- Acting on mail (#131): one message at a time, no selection of several. The toast's Undo takes back every act made
  after its own, so a message opened (and marked read) after it is unread again. The Move menu lists every folder of
  the account that holds mail but the message's own; it has no search and no picture (`ui_shot` cannot open it). The
  window counts the acts it put into the engine's history from the commands' answers; when the engine holds fewer (it
  keeps 100), Ctrl+Z stops quietly. A failed write the server refuses is taken back by the engine, and its toast still
  shows until it goes.
- Composer (#132): one message at a time; a draft closed or left from an earlier run cannot be opened again in the
  window yet, unless the outbox hands it back (#136); it stays in the store (`compose.list`) and in the account's
  Drafts folder. Plain text only: no formatting, no attachments. Suggestions come from the senders and recipients of the mail kept here (`mail.search`), not from an
  address book; a name that would move the address (`<`, `"`, `,`, `@`) is taken as the address alone. The engine
  takes a domain without a dot (`ansel@example`) as an address, so only a recipient with no `@`, two of them or an
  empty part is marked as none. The composer has no window of its own.
- Reply, reply all and forward have no keys yet, only the reader's buttons and the engine's commands; templates are
  written and removed only through the engine's `template.save` and `template.delete` (`owner`), not in the window:
  the settings sheet has no page for them yet (#133).
- A blank is asked for at the foot of the composer, one at a time; its mark in the text is tinted but cannot be
  clicked to fill it in (#133).
- The Let in card's group pills are forgotten when another message opens; a sender's group is chosen on the card or
  later in the group menu, not when the sender is blocked. New senders lists the held mail of the accounts shown, at
  most 1000 messages of the screener folders and 1000 of the inboxes (#134).
- The time picker opens from the reader only: the hover Snooze on a row stays disabled, and Pin and Park have no keys.
  The picker's alert switch is not shown, because notifying of a returned snooze is one choice for all mail
  (`notify.set`), not one per message. "Show done" is forgotten at a restart and is not offered while searching or
  in New senders. The rail's Later opens on the pins; it shows the snoozed, pinned and parked mail of the accounts
  shown, at most 1000 of each (#135).
- The countdown toast follows the last send the window made: a second send takes its place, and the first one is
  counted down in the outbox only. Undo send in the toast and in the outbox works only in the hold; a scheduled
  send's Edit takes it back, and it is scheduled again from the composer. The Outbox opens from the folder pane only,
  with no button on the rail and no key. A hold set to another value than 5, 10, 30 or 60 seconds through `send.set`
  shows with none of them chosen (#136).
- The bounce banner gives the report's reason only: it has no "Edit and resend" (no command makes a new draft from a
  sent message yet) and does not open the delivery report. The note that a send went late is a toast of six seconds,
  shown once (#136).
- No tray outside Linux yet, and outside Unix no socket: on Windows a later start says that InboxCraft is already
  running instead of bringing its window forward, and closing the window ends InboxCraft, since nothing could bring
  it back. Start at login is XDG autostart and the Background portal only; Windows and macOS have none
  yet. A desktop without a StatusNotifierWatcher (GNOME without the AppIndicator extension) shows no tray icon:
  InboxCraft then runs on without a window all the same, and a start from the menu brings it back (#137).
- Without a window InboxCraft looks for the workers' events every half second rather than at once. A window opened
  again starts with egui's state fresh (scroll positions, open popups); the reader keeps the textures of the
  pictures it showed in the window before, whose ids mean nothing (or another picture) in the new one, so such a
  picture may be drawn wrong or not at all until `ui-egui` forgets them when a new window opens (#165). On Wayland
  a later start may only mark the running window as wanting attention rather than raise it, since no activation
  token is passed on (#137).
- The tray's tooltip counts the unread mail of every account's inbox; the icon itself carries no count yet, and its
  menu has no entries per account (#137).
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
- `account.add` and the add-account form do not ask for a CalDAV address: an account gets one through
  `calendar.server` (`owner`), and until then its calendar jobs and the CalDAV step of the check do not run (#145,
  #146).
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
- The calendar's views have no keys of their own (steps, views, today), and an event picked by the keyboard is
  reached by Tab only (#147).
- Hiding a calendar lasts until the window closes; it is not kept between runs (#147).
- The event editor takes times typed as `2026-11-06 13:00` at the computer's present offset; it has no date or time
  picker yet, and the place, the description and the event's own zone are not edited in it (#148).
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
- The Notifications page has no switch for calendar alerts yet: `notify.set` with `alerts` turns them off (#150).
- An alert notification has no snooze or dismiss button; mail (`EMAIL`) alarms and alarms more than a year from their
  event are left out; the default alert of an all-day event goes off at midnight the day before (#150).

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
