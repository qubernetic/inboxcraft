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
- `inboxcraft-store`, the local-first cache: the `Storage` trait over accounts, folders, messages, flags, bodies and
  folder sync state (UIDVALIDITY, UIDNEXT, HIGHESTMODSEQ), written as a `Batch` of changes that applies whole or not
  at all. Two backends: `Memory` (always built, the one on wasm) and `Sqlite` (native only, feature `sqlite`, on by
  default): SQLite in WAL mode with `synchronous=FULL`, in defensive mode and with no trusted schema, a schema version
  with migrations, an FTS5 index of the body text, and the message sources as files beside the database, written
  and synced before their row commits and swept when a kill left one behind; on unix the directories are `0700` and the files `0600`, wider ones narrowed at
  open. Bodies are kept for opened messages and for the last 30 days (`StoreConfig::body_days`); removed mail leaves
  no words in the database, its index or its write-ahead log. `Search` matches from, to and Cc, subject and date,
  and body words as a phrase, newest first. New dependency `rusqlite` with the bundled SQLite, native builds only
  (#24)
- `inboxcraft-net` connects, signs in and runs the first sync of an account. TLS through `rustls` (`ring`, TLS 1.2
  and 1.3) with the operating system's verifier (`rustls-platform-verifier`) or, for tests, given roots, checked at
  the time of an injected clock. Implicit TLS, or STARTTLS before anything else is sent: a STARTTLS account whose
  server does not offer it, or refuses it, gets nothing at all, and a password goes out only once the connection is
  encrypted, also to a server that offers a sign-in in clear. AUTHENTICATE PLAIN where offered, LOGIN otherwise.
  Every cause has its own error kind: host not found, connection refused, timed out, dropped, unknown issuer,
  expired, not yet valid, wrong host, clock far off, other TLS, STARTTLS missing, login refused, no login method,
  no password stored, command refused, protocol, too large, folder gone, message gone. `Worker` runs one thread per
  account: it takes the model's jobs in order on one signed-in connection and reports the model's events, the
  folder list, the first sync of a folder opened read-only (summaries with flags, size, arrival time, PREVIEW and
  the envelope read from the header block, in batches; then the bodies of the messages of the last 30 days,
  `SyncConfig::body_days`), the new messages of a known folder, a UIDVALIDITY reset, and the body of one message.
  A refused password is not sent again by the same worker. `envelope` reads address lists (groups, quoted names,
  comments, encoded words) and message ids; fuzz target `net_envelope`. New dependency
  `rustls-platform-verifier`, native builds only (#25)
- `net` keeps an account up to date by itself. A known folder is resynced: with QRESYNC (switched on with ENABLE
  where offered) EXAMINE brings the flags changed and the UIDs removed since the last sync; without it the flags of
  the known messages are fetched (only those changed are reported where the server keeps modification sequences)
  and the UIDs that did not come back are reported removed; then the new messages. A changed UIDVALIDITY reloads
  the folder. An `Idle` job names the folder to watch between jobs: IDLE where offered (renewed every 25 minutes,
  ended by a job within a quarter of a second), a resync every 5 minutes otherwise, and a resync after each change
  the server tells of (`WatchConfig`). Without a connection, jobs wait in an offline queue (at most 1000,
  `WorkerSettings::max_queued`); a failure the network or the server may get over is retried after a back-off that
  doubles from 2 seconds to at most 5 minutes (`Backoff`), reported as `BackingOff`; a connection lost while
  watching is reconnected after it and the folder resynced. A certificate fault, a refused sign-in or a server
  without a sign-in method stops the tries until the worker is started again. So does a sign-in the server never
  answered (new `ErrorKind::LoginUnanswered`): at once after a `BYE` or something that is not IMAP, and on the second
  dropped or timed-out sign-in before one succeeds, so the password is not sent over and over. The connection is compressed with
  COMPRESS=DEFLATE after the sign-in where offered (`WorkerSettings::compress`), inflated in small steps. A resync
  keeps what the server tells bounded: QRESYNC's changes once per message and at most `SyncConfig::max_told` of them
  (past it the flags are fetched instead), the known messages still there as ranges, at most `SyncConfig::max_ranges`
  (past it the sync fails with `TooLarge`). The back-off wait and the IDLE renewal are measured on the monotonic
  clock, so a wall clock set back cannot stretch them. Live test
  `live_imap_round_trip` on `live-imap`: sign-in, folder list, run folder, APPEND of an invented message, first
  sync, IDLE, resync, folder removed; it skips with its reason printed without credentials, in the agent sandbox and
  in CI (#26)
- `inboxcraft-testkit`: the fake IMAP server answers CREATE, DELETE and APPEND (APPENDUID with UIDPLUS), and
  `Listen::start_imap_many` serves several connections one after the other with the mailboxes and scripted points
  carried over, for clients that reconnect; `LiveConfig::invented_message` gives a live run's invented messages
  (#26)
- `inboxcraft-engine`: the `Session` and the command registry. Every command has an id, an agent class (`safe`,
  `owner`, `never`) and runs through `Session::run`; a command without a class is refused when registered, so it
  never runs, and an agent runs `safe` commands only. A command only reads and returns its JSON answer with the
  store changes and jobs it asks for, made after it returned, so a failed command changes nothing and a command
  that panics becomes an error with the session as it was. Jobs out and events in: the folder list brings the
  folder syncs (the inbox first) and the watch on the inbox, events become store changes, and events with UIDs of
  an older UIDVALIDITY are dropped. The read commands, all `safe`: `account.list`, `folder.list` (roles, unified
  views, roles to ask about), `mail.list` (one folder or one role across every account, merged by date, narrowed
  to one account), `mail.read` (plain text and attachments; opening a message changes nothing on the server, and a
  body not kept yet is fetched once; a body too large to keep says so with `body_error`) and `mail.search` (from, to, subject, ISO 8601 dates, body words).
  `hostile_params_never_panic` runs every command with junk; fuzz target `engine_params` (#27)
- `account.check` (`safe`) checks an account step by step and names the cause of a failure: DNS, TCP, TLS, the
  certificate (with its issuer) and the IMAP sign-in; the SMTP sign-in and the CalDAV discovery are listed as not
  built yet. Each step reports its status, host and port as it ends, and `account.list` shows the last check. A port
  that times out is looked at again: when port 443 of the same address connects and the other mail ports time out,
  the network blocks mail (new `ErrorKind::MailPortsBlocked`); when another mail port answers, the port is likely
  wrong (new `ErrorKind::PortTimedOut`). A certificate outside its validity with this computer's clock far from it is
  `ClockFarOff`, also where the platform's verifier gives no dates. The issuer drops control characters and
  invisible direction and joiner marks. The password goes out only after the certificate verified. While a check of
  an account runs, another one starts nothing. After a refused or unanswered sign-in, a check an agent started stops
  before the sign-in (new `StepStatus::NeedsOwner`, `needs_owner`); only the owner's check sends the password again,
  and one that signs in ends the worker's halt. The account's worker runs it at once on a connection of its own, also
  while it is offline or has stopped trying. Live test
  `live_account_check_passes_every_imap_step` on `live-imap`; fuzz target `net_certificate` (#28)
- Detect in `net` proposes settings from an address: the MX records, the autoconfig file at the domain over HTTPS
  (plain HTTP only as a redirect to HTTPS on the plain ASCII MX domain, its finds marked `AutoconfigRedirect`),
  Mozilla's ISPDB only when switched on (off by default), SRV records (RFC 6186, RFC 8314), and common host names as
  a hint; the proposal lists every DNS query and URL asked. Nothing
  detected gets a password before the owner has seen it and TLS has verified it. DNS goes through `hickory-proto`
  messages over our own UDP and TCP exchange with the system's resolver configuration, HTTPS through `ureq` with
  `rustls`, sending nothing but the URL. Fuzz targets `net_autoconfig` and `net_dns`. New dependencies `ureq` and
  `hickory-proto` (#28)
- Live test `live_both_accounts_sign_in_and_list_folders` in `crates/net/tests/live.rs`: `live-imap` and
  `live-imap2` each sign in over IMAP and list their folders, named only by those names in the output; it skips
  cleanly without `INBOXCRAFT_LIVE_*`, in the sandbox and in CI (#57)
- `inboxcraft-html`: `display` turns hostile message HTML into display blocks (paragraphs, headings, list items
  with their markers, quotes, preformatted text, rules, pictures and tables) and runs nothing: scripts, style
  sheets, frames, objects, SVG, the head and event attributes are dropped, forms keep their visible text and
  lose everything that could submit, and elements marked `hidden`, `display: none` or `visibility: hidden` are
  not shown. Remote pictures are blocked until the owner's consent flag (`RemoteContent::Allowed`) and counted;
  `cid:` pictures point at the message's parts. Every link exposes its target; only `http`, `https` and
  `mailto` are kept, and a link whose text names another host or address is flagged (`Link::mismatch`), with
  invisible format characters ignored and full-stop look-alikes read as dots in the comparison; a web address
  with a user name, in the target or the text, is always flagged. Bidi overrides and invisible format
  characters other than the joiners are removed from all text. Input, nesting, tables, blocks, links and pictures are capped. The
  tokenizer and the character references are adapted from UCS-Care-Group/mailcraft `html` (MIT OR Apache-2.0).
  A hostile fixture set and the fuzz target `html_display` (#29)
- The first window, `inboxcraft --sample` (#30): the invented sample mailbox (two accounts of one invented person on
  `example.org`, 15 folders, 23 messages dated back from now; nothing connects anywhere) in a folder pane with the
  unified views and each account's folders with their roles and unread badges, an account picker that narrows to one
  account (its address in the header), the message list newest first (unread dot, sender, date, subject, preview,
  flag, account stripe) and a search field over subject, sender and recipients (`mail.search`), Ctrl+F to focus it
  and Escape to empty it. Every control has an accessible name. `inboxcraft` alone opens an empty window; `--help`
  lists the arguments and anything unknown is refused with exit code 2. `inboxcraft-ui-egui`: InboxCraft's own theme
  (`theme.rs`: colours, distances and type sizes for light and dark, WCAG AA contrast checked by tests, documented in
  `docs/design.md`), the `tr!` macro every UI text goes through (English message ids, named placeholders), the list
  state over the read commands, and `ui_shot` (`just shot`): headless pictures of the sample window filled on the
  CPU and written as PNG under `docs/images/`. New dependencies: `egui` and `eframe` 0.37 (OpenGL through glow,
  Wayland and X11, no wgpu, no `links`), `egui_kittest` for the UI tests, `png`.
- The dev Flatpak (#31): `contrib/flatpak/` builds InboxCraft as `io.github.qubernetic.inboxcraft` from the checkout
  without a network (the freedesktop 25.08 runtime, Rust from the SDK extension), with a menu entry whose second
  action opens the sample mailbox, AppStream metainfo and the app icon. `just flatpak` on the host builds and
  installs it for the user (and lints the manifest when `flatpak-builder-lint` is there). The permissions are the
  network, Wayland with an X11 fallback, the GPU, the Secret Service and notifications, each with its reason; the
  app's data stays under `~/.var/app/`. `cargo xtask flatpak-sources` (`just flatpak-sources`) writes the crates of
  `Cargo.lock` into `contrib/flatpak/cargo-sources.json` (crates.io only), and the new `flatpak-sources-check` gate
  of `cargo xtask ci` fails when they differ.
- The app icon, drawn in code (`crates/ui-egui/src/icons.rs`): a letter in an inbox tray with an unread dot,
  exported as `assets/app-icon/inboxcraft.svg` by `just icon` and used as the window icon (#31)
- CI: the `flatpak` job builds the dev Flatpak into `inboxcraft.flatpak` on every full run and keeps it as the
  `flatpak` artifact for 7 days; the `ci` gate needs it (#32)
- The reader (#33): selecting a row opens the message beside the list with its thread oldest first (the older
  messages folded to one line, unfolded on a click), a short header (sender, date, recipients) and the full one on
  Details, plain text with its quotes folded behind "Show quoted text", HTML as `html`'s safe display blocks, and
  attachment tiles with the name, the size and Save (into the downloads folder, under a cleaned name, never over a
  file; the Flatpak opens the Downloads folder for it, `--filesystem=xdg-download`). Remote pictures stay blocked
  behind a banner until the owner loads them once or always for the sender; the request then goes through `net` and tells the server nothing but the URL. The line under the reader shows where
  the link under the pointer goes and warns when its text names another place; a click copies the address. "Full
  width" lets the reader fill the window and Escape leaves it; the arrow keys step through the list; the
  next-message rule (next, previous or back to the list) picks what shows when the open message leaves the list.
  The sample mailbox has bodies, two reply threads, pictures drawn in code, a newsletter and a junk message with
  remote pictures. New commands: `mail.thread` and `mail.attachment` (safe), `remote.allow` and `remote.forget`
  (owner), `remote.list` (safe); `mail.read` also answers the HTML part, the inline parts by Content-ID, numbered
  attachments and `remote_allowed`. `net::picture`: HTTPS only, a fixed `User-Agent: InboxCraft`, no cookies,
  referrer, `Accept` or proxy, at most 3 redirects (each HTTPS), `image/*` answers of at most 10 MiB, and no host
  that resolves only to this computer or the local network (an IPv4 address inside an IPv6 one, mapped, compatible,
  NAT64 or 6to4, is judged as IPv4). `ui-egui` decodes PNG and JPEG (`png`, `zune-jpeg`, already in the dependency
  graph) with the size checked before anything is allocated. Fuzz target `ui_reader`.

### Changed
- `cargo xtask layers`: `flate2` moves from the testkit-only crates to the new net-only crates, for the
  COMPRESS=DEFLATE stream of `net`; `just it-live` archives the live tests of `net` too (#26)
- `rustls-platform-verifier` 0.6 to 0.7, the version `ureq` uses. `cargo xtask layers` counts `hickory-proto` as a
  network crate, so it stays in `net` (#28)
- The second live test account is `live-imap2`, a throwaway account on the reference provider like `live-imap`,
  read from `INBOXCRAFT_LIVE_IMAP2_ADDRESS`, `_HOST`, `_USER` and `_SECRET`; the recipient allow-list and the
  leak scan of `just it-live` cover its values. No Google account takes part in the live tests (#57)
- `deny.toml` allows the font licences of egui's bundled fonts (OFL-1.1 and the Ubuntu Font Licence 1.0) for the
  `epaint_default_fonts` crate only, listed in `NOTICE`, and ignores three "unmaintained" notices (RUSTSEC-2026-0370,
  0371 and 0372: zbus 5 parts merged into zbus 6) that come with the screen reader bridge `accesskit_unix`; the
  `ui-egui` drawing code (`crates/ui-egui/src/draw/`)
  counts for its crate's 80 % floor and no longer for the workspace total (#30)

### Fixed
- `just it-live` makes `target/nextest` before it builds the live test archive; on a fresh checkout
  `nextest archive` exited with 103 because the folder was missing (#57)
