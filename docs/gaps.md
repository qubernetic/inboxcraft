# Where InboxCraft falls short

> **Last reviewed:** <date> · **Last updated:** 2026-11-04 · **Change:** minor

## Ranked
| # | Gap | B | Evidence | Impact | Hours | Doc |
|---|---|---|---|---|---|---|
| 1 | <gap> | ✓ | <how we know> | <who it hurts> | <h> | |

B = blocks the next stage.

## Known rough edges (not ranked)
- Every crate is an empty skeleton; the apps answer `--version` and refuse everything else (#5).
- The `ui-egui` drawing code is not left out of the workspace coverage total yet: there is none. Its files join
  `workspace-exclude` in `.config/coverage.toml` when it lands (#7).
- The app crates have no coverage floor of their own; their `main` counts only for its crate, which has none (#7).
- `cargo xtask ci` runs the tests twice, once under nextest and once under `cargo llvm-cov nextest` for the
  coverage gate (#7).
- `testkit` serves one client per fake server and speaks implicit TLS only; STARTTLS comes with the fake SMTP
  server. A script matches whole lines exactly (#8).
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
  LITERAL+). It has no IDLE, ENABLE, CONDSTORE, QRESYNC, NOTIFY, PREVIEW, COMPRESS, UTF8=ACCEPT, NOOP, CLOSE,
  UNSELECT, ESEARCH, XOAUTH2 or OAUTHBEARER yet; they come with the rest of E1.4 and with OAuth (#20).
- `proto` FETCH asks for UID, FLAGS, INTERNALDATE, RFC822.SIZE and one of `BODY.PEEK[]`, `[HEADER]` and `[TEXT]`;
  no ENVELOPE, BODYSTRUCTURE, partial bodies or MODSEQ. A FETCH response comes as an event whether a command asked
  for it or the server told of a change; the caller tells them apart by UID (#20).
- `proto` LIST always lists `""` `"*"`; names are decoded from modified UTF-7 only, and a name over 1024 bytes or
  with a control character is left out. A CAPABILITY that the server answers with OK and no list keeps the list
  from before; `RECENT` is only kept during SELECT and EXAMINE (#20).
- `proto` refuses PREAUTH on a connection that is not encrypted, which rules out a server that greets that way
  over a local tunnel (#20).
- `proto` ends the session on an EXISTS, EXPUNGE or FETCH response when no mailbox is open or being opened (RFC 9051
  section 7.4), also before TLS, rather than passing it over: a server that sends one after a refused SELECT is cut
  off. `[ALERT]` text that comes before TLS is dropped, not shown later (#20).

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
