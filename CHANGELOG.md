# Changelog

All notable changes to InboxCraft are listed here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project uses
[Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added
- Repository structure, project documents and contribution rules.
- Development container and justfile (#1)
- Cargo workspace with every crate of the architecture as an empty skeleton, and the first `cargo xtask` gates:
  `ci`, `spdx [--fix]` and `test-files` (#5)
- `cargo xtask` gates `layers` (the crate layering table in `xtask/src/layers.rs`), `wasm`, `assets` and `stats`,
  all part of `cargo xtask ci` (#6)
- `cargo xtask cov [--check]`: line coverage per crate under `cargo llvm-cov nextest`, checked against the
  floors in `.config/coverage.toml` (95 % workspace, 98 % core crates, 80 % `net` and `ui-egui`); part of
  `cargo xtask ci`, and `just cov` runs it (#7)
- `inboxcraft-testkit`, the dev-only fake server frame: a scripted line-protocol server on a loopback port over
  plain TCP or implicit TLS, a fault script (drop, delay, garbage), a test CA generated at test time, the
  certificate faults (unknown CA, expired, wrong host name, client clock far off) and a test client that reports
  each fault as its own error kind; 98 % coverage floor (#8)
- Opt-in live tests on two throwaway test accounts, run by the owner alone with `just it-live`: the
  `inboxcraft-testkit` `live` module reads `INBOXCRAFT_LIVE_*` and skips with its reason printed without them
  (always in the agent sandbox and in CI), enforces the recipient allow-list of the two run-time addresses (plain
  dot-atom addresses only), names the accounts `live-imap` and `live-google` in output, redacts secrets and
  creates and removes the per-run folder `InboxCraft-Test/<run id>` and no other; nextest profile `live`;
  `just it-live` builds the live test archive first, reads the credentials from the OS credential store for the
  run from that archive only, passes on test name filters but no options, and fails when a secret, an address or a
  login name appears in the test output (#10)
- `inboxcraft-time`: the injectable `Clock` with `SystemClock` (the browser's clock on wasm) and `FakeClock`, the
  only clock tests use; `Timestamp` (years 0 to 10000), civil dates, weekdays and fixed offsets; `MailDate` reads
  and writes RFC 5322 dates, the obsolete forms of section 4.3 included (comments, two- and three-digit years,
  zone names, military zones and unknown names as `-0000`), refusing input over 998 bytes or comments nested
  deeper than 16; property tests and the first fuzz target, `fuzz/fuzz_targets/rfc5322_date.rs` (#15)
- `inboxcraft-charset`: `Charset` and `decode` turn text in any WHATWG charset into UTF-8 through `encoding_rs`
  (labels without case, `iso-8859-1` and `us-ascii` as `windows-1252`); an unknown label (`utf-7`, `x-unknown`) or
  one WHATWG cannot decode (`iso-2022-kr`) is a `CharsetError`, broken bytes become U+FFFD and are reported.
  RFC 2047 encoded words: `decode_text` reads them in header text (white space between words dropped, a character
  split across words of one charset joined, the RFC 2231 language ignored, a broken word left as it stands and
  reported), `decode_word` reads one strictly, `encode_text` writes the parts that need it as UTF-8 words of at
  most 75 characters, Q or B by length, without CR, LF and NUL; property tests and the fuzz targets
  `charset_decode` and `rfc2047_words` (#16)
- `inboxcraft-model`, the mailbox model, serialisable with `serde` and checked again when read back: our own ids
  and the server's UIDs, UIDVALIDITY, modification sequences and UID sets kept as ranges; accounts whose servers
  use TLS or STARTTLS (no unencrypted choice) and hold no secret; folders with their SPECIAL-USE attributes and
  roles found by SPECIAL-USE first, then by English and Hungarian names (`Elküldött`, `Piszkozatok`, `Kuka`,
  `Levélszemét`, `Archívum`, ...), then by asking the owner once per role, Inbox, Sent and Drafts fixed once set;
  addresses compared without case, and two contacts with the same display name shown with their addresses;
  flags and keywords; messages with their envelope, size and preview; threads by `Message-ID`, `In-Reply-To` and
  `References`, across folders and accounts; the read path jobs (sync folder list, sync folder, fetch body, idle)
  and events (folders, messages added, changed and expunged, UIDVALIDITY reset, body fetched, folder synced,
  connection state, one error kind per cause). Names, subjects and previews are cleaned of control characters and
  invisible direction marks; property tests and the fuzz target `model_values` (#17)
- `inboxcraft-mime` reads messages (RFC 5322, RFC 2045 to 2049): `parse` gives a tree of parts that borrow the
  message's bytes; header blocks with CRLF or bare LF, folded fields unfolded, raw 8-bit values read as UTF-8 or
  windows-1252, a leading mbox `From ` line skipped; `Headers::text` decodes RFC 2047 encoded words, turns
  control characters into spaces and drops invisible direction marks, in file names too; `Content-Type` and `Content-Disposition` with RFC 2231 parameters (character
  sets and continuations) and encoded words in file names; base64 (strict, missing padding forgiven) and
  quoted-printable; multiparts, `multipart/digest` and attached `message/rfc822` and `message/global` messages;
  the text and HTML body and the attachments a reader shows (an `inline` text part is a body even with a file
  name); the delivery report of a bounce, read from the report's own parts only (`multipart/report` with `message/delivery-status` or `message/global-delivery-status`, RFC 3464 and 6533) with
  its action and RFC 3463 status per recipient. Hostile input is an error, never a panic: messages over 128 MiB,
  more than 2000 header fields or a field over 256 KiB, nesting deeper than 20, more than 1000 parts, missing,
  bad or absent boundaries, broken base64, unknown transfer encodings and charsets; property tests and the fuzz
  targets `mime_message`, `mime_headers` and `mime_dsn`. The header, parameter, transfer encoding and multipart
  reading is adapted from UCS-Care-Group/mailcraft (MIT OR Apache-2.0) (#18)
- `inboxcraft-mime` builds messages: `MessageBuilder` writes `Date`, `From`, `Sender`, `Reply-To`, `To`, `Cc`,
  `Subject`, `Message-ID`, `In-Reply-To`, `References` and extra fields, folded within 78 characters where the
  value allows, with RFC 2047 encoded words for text and display names that need them; a text body, an HTML body
  or both as `multipart/alternative`, and attachments in `multipart/mixed` (base64, file names in RFC 2231 form when
  they are not plain ASCII); text in `7bit` or quoted-printable with CRLF line ends; boundaries that are in none of
  the parts. A header injection guard: text for people loses CR, LF and NUL, and addresses, message ids, field
  names and media types that do not fit their grammar are refused (`MimeError::BadHeader`), as are media type names
  over 127 characters and attached `message` types other than `message/global`. What the reader could
  not read back (a field over 256 KiB, a message over 128 MiB) is refused too; build-then-parse property tests (#19)
- `inboxcraft-proto` has the sans-I/O IMAP client core, `imap::Client`: bytes from the server go in, bytes for the
  server and events come out, the same on native and wasm. It reads the greeting (`OK`, `PREAUTH` over TLS only,
  `BYE`), CAPABILITY, STARTTLS, LOGIN and AUTHENTICATE PLAIN (RFC 4616, with SASL-IR when offered), LIST with
  `RETURN (SPECIAL-USE)` when the server has LIST-EXTENDED, names in modified UTF-7 and SPECIAL-USE attributes as
  the model's `SpecialUse`, SELECT and EXAMINE with what the mailbox reports, UID FETCH of flags, arrival time,
  size and the whole message, its header or its text, UID SEARCH with dates, flags, UID sets, texts (UTF-8 ones as
  literals) and `NOT`, `OR` and `AND`, and LOGOUT. LOGIN and AUTHENTICATE are refused before TLS and under
  `LOGINDISABLED`; the capabilities are forgotten after STARTTLS and read again after it and after a sign-in;
  bytes after the STARTTLS go-ahead end the session; the password is hidden in server text as it was given, as
  LOGIN quotes it, without its edge white space and as the PLAIN response. Server text loses control characters
  and the invisible direction and zero-width marks (the model's `is_invisible_mark`, now public); before TLS no
  server words are passed on at all (`[ALERT]` is dropped, BYE and refusal texts are empty); a response code is
  kept only as an atom of at most 128 characters that holds no password. EXISTS, EXPUNGE and FETCH responses when
  no mailbox is open or being opened end the session. A CAPABILITY the server refuses after a sign-in leaves the
  capabilities unknown. A literal is moved out of the read buffer, not copied, so a response holds it once. Lines
  over 1 MiB, literals over 64 MiB, more than 64 literals in a response, lists nested deeper than 32, LIST answers
  over 10000 mailboxes and flag lists over 64 are capped or refused. Golden transcripts in
  `crates/proto/tests/transcripts/`, property tests and the fuzz target `imap_client`. The response values, status
  lines, modified UTF-7, the quoting of commands and the caps are adapted from UCS-Care-Group/mailcraft
  (MIT OR Apache-2.0) (#20)
- `inboxcraft-proto` speaks the IMAP sync extensions. ENABLE (RFC 5161) switches CONDSTORE or QRESYNC on, and only
  what the server confirms counts. CONDSTORE (RFC 7162): SELECT and EXAMINE with `(CONDSTORE)`, `MODSEQ` in UID
  FETCH and in FETCH responses, `CHANGEDSINCE`. QRESYNC: SELECT and EXAMINE with what the last session knew
  (UIDVALIDITY, modification sequence, the UIDs held), VANISHED and VANISHED (EARLIER) as their own event, UID
  FETCH with `(CHANGEDSINCE ... VANISHED)`, and the responses before `[CLOSED]` passed over when one mailbox is
  opened after another. A UIDVALIDITY that is not the one known, at SELECT or while the mailbox is open, is reported
  as `Event::UidValidityChanged`. IDLE (RFC 2177) with DONE, also asked for before the server's go-ahead. NOTIFY
  (RFC 5465) for the open mailbox and for other mailboxes (personal, inboxes, subscribed, a subtree or a list of
  names), with STATUS responses and mailboxes made, renamed or removed as events, NOTIFY NONE and
  `[NOTIFICATIONOVERFLOW]`. Each needs the server to offer it; a server that lacks it gets nothing that needs it
  (`ProtoError::NotOffered`, or `NotEnabled` for VANISHED before ENABLE QRESYNC), and a VANISHED it sends anyway
  ends the session. Transcripts for each extension and for a server without it; the property test and the fuzz
  target `imap_client` drive the new commands and responses (#21)
- `inboxcraft-proto` moves and copies messages and reads previews. UID COPY, and UID MOVE (RFC 6851); a server
  without MOVE gets UID COPY, UID STORE of `\Deleted` and, with UIDPLUS, UID EXPUNGE of only those messages, under
  one tag, and the answer says how far that got (moved, expunged, marked deleted, or kept because a step was
  refused). Without UIDPLUS the messages are copied and marked, but not expunged: EXPUNGE would remove every message
  marked `\Deleted`. UIDPLUS (RFC 4315): UID EXPUNGE, COPYUID from COPY and from MOVE (also in parts), kept only
  when its UIDs were asked for, none twice and as many on both sides, and UIDNOTSTICKY at SELECT. IMAP4rev2 counts
  as offering MOVE and UIDPLUS. PREVIEW (RFC 8970), also lazy, made one clean line of at most 256 characters.
  COMPRESS=DEFLATE (RFC 4978) is negotiated: after the OK the client hands back the bytes that came with it, still
  compressed, for `net` to inflate. A move or UID EXPUNGE in a read-only mailbox is refused before anything is sent
  (`ProtoError::ReadOnly`), a second COMPRESS too (`AlreadyCompressed`). Transcripts with and without each
  capability; the property test and the fuzz target `imap_client` drive the new commands (#22)
- `inboxcraft-testkit` fake IMAP server, `testkit::imap::FakeImap`, with its own command reader (tags, atoms,
  quoted strings, synchronising and LITERAL+ literals, nested lists, all capped), never `proto`'s parser:
  CAPABILITY, NOOP, LOGOUT, STARTTLS (with the certificate faults), LOGIN and AUTHENTICATE PLAIN with SASL-IR, refused
  before TLS while LOGINDISABLED is on; LIST with SPECIAL-USE, SELECT, EXAMINE, STATUS, CLOSE, UID FETCH (flags,
  arrival time, size, whole message, header, text, MODSEQ, PREVIEW), UID SEARCH; ENABLE, CONDSTORE (CHANGEDSINCE,
  UNCHANGEDSINCE), QRESYNC (SELECT with VANISHED (EARLIER), `[CLOSED]`, UID FETCH VANISHED); IDLE; NOTIFY with its
  filters and STATUS; UID STORE, UID COPY and UID MOVE with COPYUID, UID EXPUNGE and EXPUNGE; COMPRESS=DEFLATE.
  Every capability can be switched off per test (`FakeImap::without`). `FakeImap::sample()` serves invented
  `example.org` mailboxes; `Mailbox` and `Message` build others. `FakeImap::on` scripts what happens at a greeting,
  a command, an answer or an IDLE: drop, delay and garbage faults, lines sent as they are, BYE, new mail, removed
  mail, changed flags and a new UIDVALIDITY. `Listen::start_imap` serves it over plain TCP, STARTTLS or implicit
  TLS; `FakeImap::answer` plays it without a socket. `Transcript::in_clear` lists what a client sent before TLS;
  `TestClient` gains `start_tls`, `command` and `compress`. New dependency `flate2` (pure Rust deflate, no
  network) for COMPRESS. Property tests on hostile input and the fuzz target `testkit_imap` (#23)
- `cargo xtask layers` refuses any dependency of `testkit` on `proto`, so a fake server never uses the parser it
  tests (#23)
