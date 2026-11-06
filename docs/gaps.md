# Where InboxCraft falls short

> **Last reviewed:** <date> · **Last updated:** 2026-11-06 · **Change:** minor

## Ranked
| # | Gap | B | Evidence | Impact | Hours | Doc |
|---|---|---|---|---|---|---|
| 1 | <gap> | ✓ | <how we know> | <who it hurts> | <h> | |

B = blocks the next stage.

## Known rough edges (not ranked)
- `inboxcraft-cli` and `inboxcraft-web` answer `--version` and refuse everything else (#5).
- The app crates have no coverage floor of their own; their `main` counts only for its crate, which has none (#7).
- `cargo xtask ci` runs the tests only in the coverage-instrumented build (`cov` gate); a bug that shows only in
  an uninstrumented build goes unseen in CI (#66).
- `testkit` serves one client per fake server. A line script speaks implicit TLS only and matches whole lines
  exactly (#8); the fake IMAP server also speaks STARTTLS (#23).
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
- `time` knows fixed offsets only: no IANA zones and no lookup of the computer's own zone yet, so a date shows in
  the sender's offset or UTC. ISO 8601 and iCalendar text come with `ical` (#15).
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
  server with `SMTPUTF8`; nothing checks that yet (#19).
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
- `net`'s back-off has no random spread; a job that was running when its connection ended is reported failed and
  not run again (the engine decides). The offline queue is in memory: what is queued when the app ends is gone. A
  certificate fault, a refused sign-in, a sign-in the server never answered (one more try after a dropped or
  timed-out one) or a server without a sign-in method stops the tries until the engine starts a new worker. A
  connection that drops during the sign-in twice for an ordinary network reason halts too, and needs a new worker; a missing password fails the queued jobs and ends the watch, and the next job asks again
  (#26).
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
  cannot run it. The SMTP sign-in and the CalDAV discovery report "not built yet" until E2.2 and E4.2 (#28, #34).
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
  names are hints, never checked until the owner asks. `/.well-known/caldav` and `carddav` with
  `current-user-principal`, and the CalDAV SRV records (RFC 6764), come with CalDAV (E4.2) (#28).
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
- The window keeps nothing on disk yet: `inboxcraft` alone shows an empty mailbox in memory, `--sample` an invented
  one that is forgotten on exit (#30). An account added in the window is forgotten on exit too, and so is its
  password: when the window closes, it removes the credential store entry (`io.github.qubernetic.inboxcraft`,
  `account-<id>`) of every account added in it, trying again one the store refused to remove in "Change settings".
  A crash, a killed process or a store that refuses at exit leaves the entry behind; the next account with that id
  in a later session overwrites it, and `PRIVACY.md` says how to remove one by hand (#34).
- Dates in the list are shown in UTC: the `time` crate has no local time zone yet (#30).
- The search field looks at subject, sender and recipients in every folder of the accounts shown, not at body text
  and not only in the open folder; it runs one `mail.search` per field and merges them. The query language comes
  with E5.1 (#30).
- The arrow keys step through the list only while no control has the keyboard focus; Tab moves the focus through
  every folder and row, and Enter or Space selects (#30, #33).
- A link in a message is never opened: the window has no browser hand-off (egui's `links` feature is off, and
  turning it on is the project owner's call), so a click copies the address and the line under the reader shows
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
- The reader shows no bold type (the bundled font has one weight); bold text is drawn in the full text colour.
  Tables are laid out as a grid without borders, four deep at most (#33).
- `mail.thread` reads every message of every account to group the thread; fast enough for the sample and small
  mailboxes, an index comes with the store on disk (#33).
- The next-message rule applies when the open message leaves the list after a refresh; archive and delete, which
  will use it most, come with E2. Bodies that arrive while a message is open show the next time it is opened (#33).
- The reader cannot be tested on a real double click in `egui_kittest`, so the list has no double-click shortcut to
  full width; the Full width button is the way (#33).
- The window uses egui's bundled fonts: Ubuntu Light (Ubuntu Font Licence 1.0, which is not an OSI licence; allowed
  in `deny.toml` for that crate only, the project owner's to confirm) with no bold weight, so unread rows are marked
  by a dot and the accent colour. Scripts the font lacks (CJK and others) show as boxes, and right-to-left text is
  laid out left to right (egui has no bidi). InboxCraft's own choice of typeface is open (#30).
- The mode and the accent preset can be changed only from code (`InboxApp::set_mode`, `set_preset`) and the
  `ui_shot` scripts: the commands, the settings page and keeping the choice on disk come with E1.16, so the window
  follows the system's mode in Electric. The glass, float and colour-field tokens exist and are checked, but nothing
  draws them yet (E1.15b); the gradient shows nowhere yet. The error colours are fixed per mode, not derived from
  the preset. The app icon keeps its own teal tile and orange dot, the same in every preset (#81).
- Account colours are the preset's three stops, its accent and two mixes, moved in lightness to 3:1; with Aurora in
  light mode the accent becomes a dark cyan, and in a preset two of them can look alike at a glance. The stripe
  repeats what the folder pane says in words (#81).
- `ui_shot` fills egui's triangles on the CPU and blends in gamma space, close to but not pixel-identical with the
  GPU window. The window itself was never opened in the agent sandbox (no display); the owner's first launch is
  E1.13c (#30).
- `eframe` without its default features links `libwayland-client` at build time instead of loading it at run time
  (#30).
- The Flatpak is never built in the agent sandbox, which has no `flatpak`; the `flatpak` CI job builds the bundle on
  every full run (#32). Nobody has launched it yet: the owner's first launch is #35 (#31).
- The Flatpak builds with the Rust of the `rust-stable` SDK extension, not the exact `rust-toolchain.toml` version;
  it needs at least the workspace's `rust-version` (1.95). The runtime is pinned to freedesktop 25.08 (#31).
- The metainfo has no releases and no screenshots, which Flathub would ask for; `just flatpak` shows the linter's
  findings and builds anyway. The notification and Background portal permissions are there before the app uses
  them; the Secret Service one is used for the account passwords since #34 (#31).

- Onboarding (#34):
  - The add-account form asks for IMAP only: the SMTP server, CalDAV and the sending identity come with E2.2 and
    E4.2. Detect fills in its first IMAP proposal; the others are not offered to choose from.
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
