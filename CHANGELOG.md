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
  referrer, `Accept` or proxy, a new connection and a full TLS handshake for every picture (nothing kept to tie two
  fetches together), at most 3 redirects (each HTTPS), `image/*` answers of at most 10 MiB, and no host
  that resolves only to this computer or the local network (an IPv4 address inside an IPv6 one, mapped, compatible,
  NAT64 or 6to4, is judged as IPv4). `ui-egui` decodes PNG and JPEG (`png`, `zune-jpeg`, already in the dependency
  graph) with the size checked before anything is allocated. One message asks for at most 32 inline and 64 remote
  pictures and holds at most 64 MiB of them decoded; all messages together at most 256 MiB. Fuzz target `ui_reader`.
- Adding an account (#34): "Add account" in the folder pane and in the empty window opens a form, manual by
  default (address, names, user name, password, IMAP server, port, TLS or STARTTLS). Detect fills in its best
  proposal and says where it came from (a redirect over plain HTTP and a guessed host name are marked for a second
  look), what it asked and what failed; the ISPDB is asked only when the box is ticked. "Add and check" runs the new
  `account.add` command (owner) and puts the password into the OS credential store (`keyring`: Secret Service,
  Keychain or Credential Manager) under the account's id, and nowhere else: not in a command's parameters, an
  answer, an error, the log or the store. When the credential store refuses it, the account is removed again. The
  account check's steps then show as the worker reports them, each with the host and port it tried and the
  certificate's issuer; after a failure the owner checks again or changes the settings, which removes the account
  and its password and opens the form again. Accounts are not kept on disk yet, so when the window closes it removes
  the passwords of the accounts added in it from the credential store again. The password field takes at most 256
  characters into one buffer that is overwritten when cleared and keeps no undo history, so undo never brings a
  cleared password back. The form says what Detect sends before it is pressed. A new account starts syncing once
  its first check signs in. New commands: `account.add` and `account.remove` (owner); a complaint about
  `account.add`'s parameters ends before the first quote and repeats nothing that was sent. The desktop app runs one
  `net` worker per account and applies their events each frame; `ui_shot` draws the page in each state (`onboard`)
- `cargo xtask side-by-side` (`just side-by-side`): every `ui_shot` picture of the shot script next to the mood
  sketch of its preset and mode, scaled to the same height, one PNG per picture in `plan/side-by-side/`. The
  sketches are read from `plan/mood/` (gitignored, never committed); `--script`, `--mood` and `--out` name other
  places (#81)
- The design system's components (#85, `ui-egui::component`): a card that opens and closes, an icon button
  (active, disabled) with its tooltip, a pill, a tooltip bubble with the action's name and its key as a chip, an
  avatar, a count badge and a group badge (a solid icon, white wherever white reaches 3:1 on every stop, otherwise
  the deep ink) on the preset's gradient, a hover action group that fades in over a row,
  a toast that rises with Undo and its key, and a popup with menu rows. `ui_shot` draws them all on one page with
  `"components": true`, in every preset and mode.
- Motion tokens (#85, `ui-egui::motion`), all at most 0.25 s: hover fade, tooltip, row slide-out, card open and
  close, toast rise. Every animation runs through the motion module and reads one reduced-motion flag
  (`InboxApp::set_reduce_motion`); with it on, everything lands at once and egui's fades and its scrolling to a place stop too;
  egui's smoothing of mouse wheel notches (about 0.1 s) is fixed inside egui and keeps running. Its
  default follows the system where the system can be read, otherwise motion is on.
- A keymap (#85, `ui-egui::keymap`): which key runs which action, by id; tooltips, menu rows and a toast's Undo read
  their key from it when they are drawn, so a changed key shows at once. The search's Ctrl+F is its first key.
- Avatars (#85): a person's initials on one of ten vivid avatar colours (no teal or coral), picked from the address
  by a stable hash, with white or dark initials by contrast; the reader shows the sender's avatar in the message
  header, the sample mail's people included.
- The appearance commands (#86), all `safe`: `ui.theme.set` (system, light, dark), `ui.accent.set` (a preset, or
  four colours of your own: three gradient stops and the accent; an accent under WCAG AA as text on dark surfaces is
  made lighter, hue kept, and the answer says so with the colour asked for and the contrast reached),
  `ui.font.set` (Figtree, Inter), `ui.icons.set` (Phosphor, Lucide), `ui.text_scale.set` (90 to 130 per cent) and
  `ui.motion.set` (reduce motion on, off, or follow the system). Each answers the whole appearance. The window's
  own setters run them too.
- The appearance is kept across a restart (#86) in `inboxcraft/appearance.json` in the config folder
  (`XDG_CONFIG_HOME`, `.config`, `%APPDATA%` or `Library/Application Support`), read as outside data choice by
  choice and written through a temporary file; a change that cannot be kept shows in the error banner.
- Inter v4.1 (OFL) as the second typeface and Lucide 1.70.0 (ISC, its Feather icons MIT) as the second icon set,
  bundled with their licence files (#86). Every `IconId` has a Lucide glyph; Lucide has lines only, so its active
  (Fill) and empty (Duotone) states draw a filled disc behind the lines. `ui_shot` takes `font` and `icon_set`.
- The appearance settings page (#87): the settings button in the folder pane opens it in place of the list and the
  reader. One control each for the mode, the accent preset, the custom accent (four colour fields with a picker,
  `#rrggbb` only, and "Use these colours"), the typeface, the icon set and the text size, and a "Reduce motion"
  switch; each runs its `ui.*.set` command at once. A preview card shows the look, the typed custom colours
  included before they are used; when the engine makes a custom accent lighter, the page says which colour is used
  instead. Close or Escape leave the page. The overflow test covers it in both typefaces at 90 % and 130 %;
  `ui_shot` takes `settings`.
- The message list on the design system (#88): a navigation rail down the window's left edge in the preset's
  gradient at full strength, with Mail (Ctrl+1) and the appearance settings (Ctrl+Comma; the button moved there
  from the folder pane); the rows on a card, each with the sender's avatar, the selected row in the selection
  gradient, unread by dot and weight. The screener card, the grouped inbox's cards (badge, name, unread count,
  "Done all", "Show all (n)") and the hover actions (Done, mark read or unread, Delete), which a row selected by
  keyboard shows as a hovered one does with its text fading out before them, are built but have no data or command
  yet: the app draws none of them, and `ui_shot` shows them with invented data (`showcase`, `step`). A quiet and a
  primary text button with a tooltip join the components; every button of the list and the rail has a tooltip.
- The hover actions act on the sample mailbox (#89): Done (marks the message read and moves it to its account's
  Archive), mark read or unread, and Delete (to Trash), from a hovered row or the row selected by keyboard, and
  their keys D, R and Delete. Done and Delete let the row slide out first; a toast says what was done, with Undo
  and its key chip (Ctrl+Z) for six seconds, and Undo puts the row back as it was, selected again when it was.
  Reduced motion makes the slide and the toast land at once. They run through `sample.mail.set`, a `safe` command
  that runs only in a `Session::sample` (others refuse it with `sample_only`), changes the in-memory sample only and
  queues no job; without `--sample` the actions are hidden and the keys do nothing. `ui_shot` takes `act` and
  `slide`; `docs/development.md` says what `--sample` lets the owner try.
- The reader on the design system (#90): a toolbar of mail actions (Done, Archive, Delete, Snooze, Pin, Move to a
  folder, More) as icon buttons with tooltips over the subject, the largest text of the window, which wraps rather
  than being cut, and who wrote in the thread with how many messages. The thread shows as cards: earlier messages
  folded to one line with the sender's avatar, the open message in full with Reply (primary), Reply all and
  Forward. On the sample mailbox Done and Delete act on the open message as the hover actions do; every action
  without a command (and Done and Delete outside the sample) is disabled and its tooltip says "Not available yet".
  Full width, Close and the next-message rule moved into the More menu. Attachment tiles wrap side by side. The
  components gain `button::unavailable`, `IconButton::unavailable`, a note line in the tooltip, and the popup's
  `entry` and `choice` rows; `ui_shot` draws the full-width reader in every preset.
- The design file `docs/design/inboxcraft.figcraft` (FigCraft): colour variables per preset and mode, type styles,
  the components with their variants, every screen of the app in light and dark, the mood in each preset and five
  flows as storyboards; a PNG per page and `docs/design/README.md` (#96)
- `ui.frame.set` (`safe`): who draws the window frame, the desktop's title bar, the window controls in the top row
  or a slim strip of the app's own with the title (the default). A native window opens without the desktop's
  decorations unless the title bar is chosen, moves by its strip, maximises on a double click and resizes at its
  edges; pictures and the web build draw no frame. The choice is a row on the Appearance page (#104)
- The rail shows the app's mark, Start, Mail, Calendar, Pins, Sent mail, Search, the settings and the owner's
  initials; the views not built yet are disabled and say so (#104)
- Text buttons in three tones (quiet, secondary outlined, primary on the text gradient) with an optional icon; the
  search field is a pill with a magnifier (#104)
- Change fragments: a change records its CHANGELOG lines, its ROADMAP progress line, its new `docs/gaps.md` lines
  and its revision rows in `changes/<issue>.md`. `cargo xtask changes` (`just changes`) checks every fragment and
  is part of `cargo xtask ci`; `cargo xtask changes --fold` writes them into `CHANGELOG.md` `[Unreleased]`,
  `ROADMAP.md`, `docs/gaps.md` and the revision tables, dated today in UTC, and deletes them (#111)
- The store's schema version 4 with every local table the daily-use features need: the job queue, drafts, the
  outbox, the schedule, senders (known, the screener's decision, the group), each message's own state (done,
  parked, pinned, snoozed, follow-up) and the calendars with their sync token, events (ETag and iCalendar text) and
  alerts; their records in `model` (`Draft`, `Outgoing`, `Scheduled`, `Sender`, `MessageState`, `Calendar`,
  `CalendarEvent`, `Alert`) and their `Change` variants in both backends. A schedule entry leaves with the draft,
  message or alert it names (#113)
- Kept jobs: `Outcome::with_kept_job` writes a job into the store's queue in the same batch as the command's
  changes; it leaves the queue with its last event, and `Session::start` hands the jobs an earlier run left over
  again. Only folder lists, folder syncs and body fetches are kept, never a check or a watch (#113)
- `Sqlite::id`: every store has a random id of its own, made with schema version 4 (#113)
- The acts on messages: `mail.flag`, `mail.seen` (read and unread) and `mail.open` (read and marked read), `safe`;
  `mail.move`, `mail.archive` and `mail.delete` (to the Trash, for good only from the Trash), `owner`. Each shows
  at once and goes to the server as a kept job; a moved message keeps its id and waits under a UID of ours until
  the server names its new one (#114)
- The undo history: each act but a delete for good stores its inverse, and `history.undo` takes the newest back,
  again and again, up to 100 acts; an agent undoes only what an agent may do (#114)
- The write jobs in `model` (`StoreFlags`, `Move`, `Expunge`, `Append`, `CreateFolder`) with their answers
  (`FlagsStored`, `Moved` with the new UIDs, `Expunged`, `Appended`, `FolderCreated`); a failed write is taken
  back (#114)
- The arriving-mail pipeline (`engine::arrive`): each new message passes each stage in order, one file per
  stage (#114)
- `proto` writes: UID STORE (`FlagStore`, with UNCHANGEDSINCE and the MODIFIED UIDs back), APPEND with APPENDUID
  (`AppendUid`; the message always goes as a literal after the go-ahead), CREATE, CLOSE and UNSELECT, with golden
  transcripts and the `imap_client` fuzz target driving them (#115)
- The IMAP write path in `net`: flags, move, delete for good (`\Deleted` and UID EXPUNGE of the job's messages
  only), APPEND and CREATE run on the server. A changed UIDVALIDITY writes nothing and is told; a message gone
  meanwhile is told as expunged after a store and named by no new UID after a move, not as a failure; CREATE of a
  folder that exists counts as made (#115)
- A write job that a dropped or timed-out connection cut off runs again after the reconnect, three times at most
  (`net::WRITE_TRIES`), except a move on a server without MOVE, whose fallback may have copied the messages already:
  it fails, and the engine takes it back; writes queued offline run in order once the worker is online (#115)
- A live test of the write path in the run folder: APPEND, a flag, CREATE inside the run folder and a move there
  (#115)
- Drafts in the engine: `compose.new` (from the account that last sent, with its signature), `compose.update`,
  `compose.show`, `compose.list` and `compose.discard`, all `safe`; a draft is kept in the store at every change, so
  text typed before a kill is there after a restart (#116)
- Recipients are checked as typed: an impossible address is marked with what is wrong and blocks Send, like a draft
  without a recipient (#116)
- `compose.signature` (`owner`): a signature per account, swapped when a draft moves to another account (#116)
- A draft that has been still for ten seconds goes to the account's Drafts folder from `Session::tick`; where the
  server names the copy (UIDPLUS), each new copy replaces the one before it and a discarded draft's copy goes too,
  and where it does not, a draft leaves at most one copy there. `mail.thread` lists the drafts that answer a message
  of the thread (#116)
- `proto::smtp`: a sans-I/O SMTP submission client (RFC 5321, RFC 6409) driven like the IMAP one (bytes in, bytes
  out, events): the greeting, EHLO, STARTTLS (sent EHLO again after it, bytes before the handshake refused), AUTH
  PLAIN and AUTH LOGIN, MAIL FROM with SIZE, `BODY=8BITMIME` and SMTPUTF8 each said only to a server that offers
  it, RCPT TO, DATA with every line ended in CRLF and dot-stuffed, RSET and QUIT; refusals carry their enhanced
  status code (RFC 3463) from a server that offers ENHANCEDSTATUSCODES. Reply lines, lines per reply and the
  extension list are capped; golden transcripts in `crates/proto/tests/transcripts/smtp/` and the fuzz target
  `smtp_client` (#117)
- `inboxcraft-testkit` fake SMTP submission server, `testkit::smtp::FakeSmtp`, with its own command reader (never
  `proto`'s): EHLO and HELO, STARTTLS, AUTH PLAIN and AUTH LOGIN only over an encrypted connection (unless
  `auth_in_clear` makes it careless on purpose), MAIL FROM with SIZE, `BODY=8BITMIME` and SMTPUTF8, RCPT TO, DATA
  with dot-stuffing, RSET, NOOP and QUIT, enhanced status codes; extensions switchable per test; scripted refusals
  with their enhanced codes at the greeting, a command, one recipient or the message, and the faults of a script;
  every accepted message captured in its `Outbox`; a bounce script that drops an RFC 3464 delivery report into the
  fake IMAP server's mailbox through a `testkit::imap::Postbox`, which every later session of that fake reads,
  each `FakeImap::answer` call among them; implicit TLS and STARTTLS with the certificate
  faults, `Listen::start_smtp` and `Listen::start_smtp_many`; the fuzz target `testkit_smtp` (#118)
- Sending: `mail.send` (`owner`) sends a draft through its account's submission server with its own `Message-ID`
  and a `Date` in the computer's offset, to every To, Cc and Bcc address, Bcc in the envelope alone; the draft and
  its Drafts copy go once the server took it and the message is appended to the Sent folder, and a refusal keeps the
  draft with the server's reply in its outbox item (#119)
- `net::SmtpConnection`: submission over implicit TLS (465) and STARTTLS (587) with AUTH PLAIN or AUTH LOGIN,
  `BODY=8BITMIME` and SMTPUTF8 where the message needs them, all recipients or none; the worker runs a `Send` job
  at once on a connection of its own and answers `Sent` or `SendFailed` (#119)
- `account.check` signs in to the submission server after the IMAP sign-in passed, with the same password, as its
  SMTP step; an account without one reports it as not set. `account.add` takes a `submission` server (#119)
- A live test that sends one invented message from `live-imap` to `live-imap2` and removes it again (#119)
- `compose.reply`, `compose.reply_all` and `compose.forward` (`safe`): a draft made from a message, from the account
  that holds it. A reply goes to the `Reply-To`, else the sender (the recipients of the owner's own message); reply
  all adds the other recipients without the address or login of any of the owner's accounts, and a message's Bcc
  is never read. `Re: ` and `Fwd: ` are added unless a
  prefix is there already, and the message is quoted below a line of our own (#120)
- A reply carries `In-Reply-To` and `References`, so it and its Sent copy join the thread of the message it answers;
  only ids the message builder can write are kept, and of a long thread the first and the newest (#120)
- A forward puts the message inline below its sender, date, subject and recipients and goes out with the original's
  attachments, read from the kept body; a forward needs the body kept, and one whose body has gone since shows the
  block `forward_body` and `mail.send` refuses it (#120)
- Templates per account: `template.list` (`safe`), `template.save` and `template.delete` (`owner`), kept in the
  account record. `compose.template` (`safe`) puts one into a draft with `{name}`, `{first_name}` and `{my_name}`
  filled in; a blank (`{?prompt}`) and a name that is not known become a `[[prompt]]` mark, and while one is left the
  draft shows the block `blank` and cannot be sent (#120)
- `inboxcraft_mime::is_message_id` and `is_media_type`: whether the builder writes a value (#120)
- `outbox.list` (`safe`): each outbox item with where it stands (`waiting`, `sending`, `refused`), a refusal with
  the server's reply, its enhanced status code and whether it is permanent; `outbox.retry` (`owner`) sends a
  refused draft again, kept as it was (#121)
- Bounces: a delivery report (`multipart/report` with `report-type=delivery-status` or `global-delivery-status`)
  from `MAILER-DAEMON` or `postmaster` passes the screener through the new bounce stage before it, its body is
  fetched, and the message of the `Message-ID` it returns in the account's Sent folder is marked `bounced` with the
  report and the reason (each failed recipient the message had in To or Cc, with the remote server's reply or the
  status code), shown by `mail.list` and `mail.read` (#121)
- `inboxcraft_model::Signs::report`, `inboxcraft_model::Bounce` and `MessageState::bounced`;
  `inboxcraft_mime::Part::reported_message_id`, the `Message-ID` of the message a report is about (#121)
- The scheduler: `Session::tick` fires every schedule entry whose time has come on the session's clock, in time
  order, takes it off the schedule in the same batch as what it changes (a returned snooze or a fired follow-up
  clears its time in the message's state) and hands it back as a `Fired` event, marked missed when it was due
  before `Session::start` or is handled more than a minute late; `Session::next_due` says when to tick next and
  `Context::next_schedule_id` gives a new entry its id (#122)
- `schedule.presets` (`safe`): later today (three hours on, rounded to the nearest half hour, while still today),
  this evening (20:00) and tomorrow morning (08:00), those already behind left out, in the computer's wall-clock
  time across a summer-time change; `schedule.list` (`safe`) and `schedule.cancel` (`owner`) (#122)
- `inboxcraft_time::Offsets`, the computer's offset from UTC over time (a fixed `Offset` or an `OffsetTable` of
  changes), with `Offsets::resolve` for a wall-clock time skipped or shown twice at a change, and
  `Session::set_offsets` (#122)
- Undo send: every `mail.send` waits in the outbox for the owner's hold (5 to 60 seconds, 5 by default;
  `send.set`, `owner`, and `send.settings`, `safe`; `Session::send_hold` and `Session::set_send_hold` for the
  front end to keep it) before it goes, and `mail.unsend` (`owner`) takes it back until then, the draft as it was
  (#123)
- Send later: `mail.send_later` (`owner`) sends a draft at a time or a preset of `schedule.presets`, saying that
  nothing is sent while the computer is off or InboxCraft is closed; `schedule.cancel` on its entry takes the send
  back, and `outbox.list` shows when a waiting item goes (#123)
- A send whose time passed while InboxCraft was not running goes at the next start, and the owner is told with the
  new `Notice::SentLate` ("Sent late" on the desktop) (#123)
- `Session::tick_if_due` and `Session::due_at`: the window now ticks the engine as it pumps and asks for a frame
  when something is next due, also while it is idle or hidden, so held sends, send later, snoozes, follow-up
  reminders and draft copies happen in the app (#123)
- The known-sender index: one record per address across the accounts, known once the owner received mail from it
  (the first sync of a folder, mail let in while the screener is off), wrote to it (every recipient in the sent
  folder) or accepted it; the owner's own addresses and delivery reports (`MAILER-DAEMON`, `postmaster`) are never
  screened (#124)
- The screener, the arriving-mail pipeline's first stage: later mail from an unknown sender is held in our own
  `InboxCraft/Screener` folder (made on the server the first time), marked inside the inbox (`new_sender` in
  `mail.list`) or let in, by `screener.mode` (`owner`: `before`, `inside`, `off`; off until the owner turns it on,
  which learns every sender the kept mail names); `Session::screener` and `Session::set_screener` for the front end
  to keep it (#124)
- `screener.list` (`safe`): the senders waiting, each with the mail held; `mail.read` is the preview and decides
  nothing (#124)
- `screener.accept` (`owner`, with a group, `then`: `open` marks the newest read for the front end to open, `done`
  files the held mail in the archive as done) moves the held mail into the inbox and lets the future mail through;
  `screener.block` (`owner`) moves the held mail and the future mail, in every mode, to `InboxCraft/Blocked`; both
  undoable with `history.undo` (#124)
- `mail.read` never fetches a blocked sender's message: it shows the kept body or `not_downloaded`
  (`body_error`), says `blocked`, and its remote content never loads (#124)
- The grouped inbox: each message of a grouped account is in People, Updates, Newsletters or Invitations, by our own
  classifier of its header and body (a `text/calendar` part, `Auto-Submitted` other than `no`, list fields,
  `Precedence: list`, `bulk` or `junk`, no-reply and delivery-report senders) or by the owner's choice for its
  sender; `net` reads the signs of a message's kind from its header block (`model::Signs`), and a kept body with a
  calendar part makes its message an invitation (#125)
- `group.set` (`owner`): a sender's mail, kept and future, goes to the group the owner chose, or back to the
  classifier with `auto`; `history.undo` takes it back (#125)
- `group.account` (`owner`): an account's grouping on or off, kept with the account (on until turned off) (#125)
- `group.list` (`safe`): the inbox's mail and its unread mail counted per group, and the mail of the accounts not
  grouped (#125)
- `mail.list` says each row's `group` and lists one group (`group`, or `none` for the mail no group holds) (#125)
- `mail.done` (`owner`): a message is filed in its account's archive as done, and its old folder is kept;
  done `false` brings it back there; `history.undo` takes either back (#126)
- "Show done": `mail.list` with `done` lists the done mail in the place it was done from, among the mail there (#126)
- The park pile: `mail.park` (`safe`) puts a message on it or takes it off, `mail.list` leaves the pile out and
  counts it in `parked` (`parked` lists it too), and `park.list` (`safe`) lists it across the accounts (#126)
- Pins: `mail.pin` (`safe`) pins a message or takes its pin off, and `pin.list` (`safe`) is the pins view across the
  accounts; done takes a pin off unless `pin.set` (`owner`) keeps it (`Session::pins_outlast_done`) (#126)
- `mail.list`, `park.list` and `pin.list` say each row's `done`, `parked` and `pinned` (#126)
- Snooze: `mail.snooze` (`safe`, `until` or a preset of `schedule.presets`) puts a message in the account's own
  `InboxCraft/Snoozed` (made on the server the first time) until a time, in a new copy whose header carries the time
  in InboxCraft's own `InboxCraft-Snoozed-Until` field (replacing one an earlier snooze left); the old copy is deleted
  only once the server has the new one, and the copy is written only on a server that deletes one chosen message
  (UIDPLUS or IMAP4rev2, told by `net` after each sign-in), elsewhere the message is moved without the field (#127)
- At the time, `Session::tick` brings a snoozed message back to the inbox unread and marked as returned, listed as if
  it arrived then and told of as a notification; `mail.unsnooze` (`safe`) brings it back at once, and `history.undo`
  takes a snooze back (#127)
- A message that arrives in the snoozed folder with the field as its first header field, as another InboxCraft
  snoozed it, is snoozed until that time here too; the field further down counts for nothing (#127)
- `mail.list` says each row's `snoozed_until` and `returned` (#127)
- Follow-up reminders: `mail.follow_up` (`safe`, `until` or a preset of `schedule.presets`) on a message the owner
  sent reminds them at that time if no reply came; a reply that arrives before then (its `In-Reply-To` or
  `References` names the sent message, and it is not from one of the owner's own addresses) cancels it, and at the
  time `Session::tick` tells the owner with a notice that opens the sent message (#128)
- `mail.cancel_follow_up` (`safe`) takes a reminder off, and `history.undo` takes either back (#128)
- `mail.list` says each row's `follow_up` (#128)
- The time picker, one component for snooze, send later and reminders: the presets still ahead with their time, a
  field whose text counts as soon as it reads as a time (`thu 9`, `12 nov 17:00`, `tomorrow`, `in 3 days`), without
  Enter, the time read shown under it and used by the confirm button, and an alert switch that keeps its last
  value; in the component gallery, and the typed time has the fuzz target `ui_typed_time` (#129)
- The command palette: Ctrl+K lists every action of the window with its icon and key; typing narrows the list, the
  arrows move the pick, Enter or a press runs it, Escape or a press beside it closes it, and an action that cannot
  run here is listed faded (#130)
- The Shortcuts page of the settings sheet: every action with its key, a new key from the next key pressed (a key
  another action had moves, and the page says so), Remove, Reset all keys and a field that narrows the list; the
  keys are kept between runs in `shortcuts.json` beside the appearance file, and the keys' file has the fuzz target
  `ui_keymap` (#130)
- The calendar screen behind the rail's Calendar button (Ctrl+2), with its empty state until calendar sync (#130)
- The window acts on real mail: the hover actions, the reader's toolbar and their keys mark read or unread, flag,
  mark done, archive, delete and move a message on a real account as on the sample, through the engine's commands;
  the change shows at once and goes to the server after (#131)
- Move to a folder in the reader's toolbar opens a menu of the folders of the message's account; More holds Flag or
  Take the flag off and Mark as read or unread for the open message; A archives and S flags the selected message
  (#131)
- Opening a message marks it read (`mail.open`), as an act Ctrl+Z can take back (#131)
- The composer in the reader's place: New message in the folder pane (C, or the command palette) opens it with From
  (a menu of the accounts), To, Cc and Bcc as chips, Subject and the text, Discard and Send (Ctrl+Enter); a message
  opened in the list, or the calendar, sends it to a floating panel at the window's bottom right, and a press on
  that brings it back whole (#132)
- Recipient chips: Enter, a comma, a semicolon or leaving the line makes a chip of what was typed, Backspace in the
  empty field removes the last one; the people in the mail kept here whose name or address holds the typed text are
  offered under the field, and Enter takes the highlighted one; a recipient that is no address is marked at once and
  blocks Send, which says why (#132)
- The draft is kept silently at every change (`compose.update`): Close keeps it, with what was typed in a recipient
  line as a chip, and Discard removes it here and on the server (#132)
- The Sending page of the settings sheet: each account's signature, saved with `compose.signature`; `account.list`
  answers each account's `signature` (#132)
- Reply, Reply all and Forward in the reader open the composer in the reader's place over a draft the engine filled
  in (`compose.reply`, `compose.reply_all`, `compose.forward`): the text has the focus, or To for a forward; a draft
  being written is closed and kept first (#133)
- Templates in the composer: Templates at its foot is a menu of the templates of the account in From, and the one
  chosen goes in front of the text (`compose.template`). Each blank left is marked on the accent's tint in the text,
  a line over the toolbar asks for it by its prompt, and Send says "Fill in "<prompt>" to send" until it is filled
  in (#133)
- Send says why a forward whose kept message has gone cannot be sent (#133)
- The screener in the window: the screener card on top of the unified inbox shows the senders waiting
  (`screener.list`), and Review opens New senders, the mail held, newest first. A held message opens as a preview
  (`mail.read`, it stays unread) under a card that asks "Let <sender> in?", with the groups to let them into, Accept,
  Accept and open (the message then shows in the inbox) and Block; each is an act with a toast and Undo (#134)
- A row from a new sender says "New sender" after the sender, and a screen reader hears it (#134)
- The grouped inbox from the engine's data: People, Updates, Newsletters and Invitations as cards (the first three
  rows and "Show all"), as one grouped list under headings, or the simple list; the rows no group holds come
  last (#134)
- The group menu, a right click on a row of a grouped account or the reader's More menu: a sender's mail, kept and
  future, goes to another group or back to InboxCraft's sorting (`group.set`), with a toast and Undo (#134)
- The Triage page of the settings: the screener's mode (before the inbox, inside and marked, off), the inbox view
  (cards, grouped list, simple list) and a switch per account for its grouping (`group.account`); the mode and the
  view are kept in `triage.json` beside `appearance.json`, and the sample window never writes its mode there (#134)
- `account.list` says whether each account is grouped (`grouped`) (#134)
- The sample mailbox holds two new senders in its screener folder with the screener on, and its inbox is grouped
  in People and Updates; `ui_shot` draws New senders, the toast after Accept, the cards after a group change, the
  grouped list and the Triage page (`"open": {"role": "senders"}`, `"view"`, `"triage"`, the acts `accept`,
  `accept_open`, `block` and `newsletters`) (#134)
- Done, park, pin, snooze and follow-up reminders in the window. Snooze in the reader (or H) opens the time picker
  over the presets still ahead (`schedule.presets`) and a typed time (`mail.snooze`); on a message in a sent folder
  the same button is "Remind me if no reply" (`mail.follow_up`). Pin pins the open message or takes its pin off
  (`mail.pin`); More parks it or takes it off the park pile (`mail.park`), brings a snoozed message back now
  (`mail.unsnooze`) and takes a reminder off (`mail.cancel_follow_up`). Each is an act with a toast and Undo,
  but bringing back a snooze, which the engine keeps out of its history (#135)
- "Later" on the rail (the Pin icon, `ui.later.show`): the snoozed mail by the time it comes back, the pins
  (`pin.list`) and the park pile (`park.list`), each behind a pill with its count (#135)
- "Show done" under the header of a folder or a view lists the done mail in its old place, dimmed, with "Not done"
  on its hover actions and on D; "Parked (n)" beside it opens the park pile (#135)
- A pinned row shows the pin; a row back from a snooze says "Returned" while it is unread; the reader's chips say
  until when a message is snoozed and when its reminder is due (#135)
- "Done all" on a group's card marks the whole group done, with one Undo; a group that takes more acts than the
  undo history holds (100) says so in its toast and offers no Undo (#135)
- The reader's Done reads "Not done" on a done message and brings it back (#135)
- "Keep pins on done mail" on the Triage page (`pin.set`), kept in `triage.json` (#135)
- `ui_shot` draws Later, "Show done" and the time picker (`"put_away"`, `"later"`, `"show_done"`, `"picker"`) (#135)
- The outbox in the window: while anything waits to be sent, "Outbox" shows in the folder pane with how many, and
  lists each send with whom it goes to, its subject and where it stands ("Sending in 4 s", "Scheduled · Fri 08:00",
  "Sending…", or "Not sent" with the server's reply). Undo and Edit take a waiting send back and open its draft
  (`mail.unsend`); a refused one has Edit and Send again (`outbox.retry`). Once empty it gives way to the inbox
  (#136)
- Undo send in the window: Send closes the composer and a toast counts the hold down ("Sending "subject" in 4 s");
  its Undo, or Ctrl+Z, takes the send back and opens the draft again as it was (#136)
- Send later in the window: the caret beside Send opens the time picker ("Send later"); with a time picked Send
  becomes "Schedule · Fri 08:00" and sends then (`mail.send_later`), and "Send now instead" takes the time away. A
  send that went late because InboxCraft was not running at its time says so in a toast once (#136)
- A sent message a delivery report says did not get through is marked "Not delivered" in its row, and its reader
  says why in a banner (#136)
- "Undo send" on the Sending page: every send is held 5, 10, 30 or 60 seconds (`send.set`), kept in `triage.json`
  (#136)
- `ui_shot` draws the outbox, the countdown, send later and a bounce (`"outbox"`); `Scripted::refusing` answers
  every send with a refusal (#136)
- One InboxCraft at a time on the owner's mailbox: the first start holds `instance.lock` beside the store and
  listens on `instance.sock` (Unix, the owner's alone); a later start brings the running window forward and leaves,
  so two runs never open one store (#137)
- Closing the window leaves InboxCraft running without one: sync, rules and schedules go on, and a later start, the
  tray or the menu entry opens a new window on the same mailbox. `inboxcraft --background` starts without a window,
  `inboxcraft --quit` (and the menu entry's Quit action) ends the running one (#137)
- The tray on Linux: a StatusNotifierItem (`ksni`) with the unread count of every account's inbox in its tooltip;
  a click shows the window, its menu opens the window or quits (#137)
- Starting at login: `inboxcraft --autostart` and `--no-autostart` write or remove an XDG autostart entry that
  starts InboxCraft with `--background`; in the Flatpak they ask the Background portal, which each start also asks
  to let InboxCraft run without a window (#137)
- New mail tells the owner with a desktop notification: the sender's name and the subject, with Done and Mark read
  buttons where the desktop shows them; a click opens the message in the full-width reader, also when the window
  was closed. Only unread mail new in an inbox after its first sync, not held or blocked by the screener, not from
  the owner's own addresses, and not mail the owner's own move brought back (accepting a sender lets its held mail in
  without a burst); more than three new messages of one account at once are one notification that counts them (#138)
- `notify.settings` (`safe`) and `notify.set` (`owner`): per account all new mail, people only (senders the owner
  knows or accepted) or none, and whether returned snoozes and follow-up reminders notify (#138)
- On Linux the notifications go through the notification portal in the Flatpak and `org.freedesktop.Notifications`
  (over `zbus`) elsewhere; the body is escaped where the desktop reads it as markup (#138)
- The Notifications page in the settings: per account all new mail, people only or none, and switches for returned
  snoozes and follow-up reminders (`notify.set`), kept in `notify.json` beside `appearance.json`; the sample window
  neither takes nor keeps them (#139)
- "Run in the background" and "Start with the computer" on the Notifications page: off, closing the window ends
  InboxCraft, and the choice is kept in `background.json`; on, InboxCraft starts in the background at login through
  the XDG autostart entry, or through the Background portal in the Flatpak, asked on a thread of its own so the window
  never waits for it. Turning running in the background off also stops starting with the computer (#139)
- `ui_egui::desk::Desk`, the app's side of running without a window, handed in with `InboxApp::set_desk`; `ui_shot`
  draws the Notifications page (`"notifications"`) (#139)
- `inboxcraft-cli --store <dir> [--json] <command> [<params>]` runs every command by id, headless, on a local
  store: text output or one JSON object a line (snake_case, ISO 8601 dates in UTC, fields only ever added), one
  error on stderr (a JSON object with `--json`) and exit code 1 or 2 on failure, never a prompt; only `safe`
  commands run, `owner` and `never` ones are refused with a plain error; `commands` lists every command with its
  class; message text never reaches the terminal as a control character (`docs/cli.md`) (#140)
- `inboxcraft_testkit::store::fixture`: a small invented mailbox written to disk, for tests that open a store the
  way the apps do (#140)
- `inboxcraft-cli --store <dir> mcp`: the read-only MCP server (`inboxcraft-mcp`) over stdio, headless on a local
  store, with the tools `mail_list`, `mail_read`, `mail_search`, `thread_read` and `account_check`, each a `safe`
  command run as an agent; compact text results (a status line, one line a message, quoted parts folded, bodies cut
  after 8000 characters, long lists of people and attachments cut with a count, quoted values cut by bytes too, a list
  holding the messages that fit in 64 KiB and counting only those, a result cut at 64 KiB), and every
  value a message brings written as a quoted JSON string, its invisible and direction characters escaped, under a
  line that marks it untrusted; the fuzz target `mcp_lines` (`docs/mcp.md`) (#141)
- The local control channel: the app on the owner's mailbox listens on a Unix socket in `$XDG_RUNTIME_DIR`
  (`inboxcraft/control.sock`, or the Flatpak's shared `app/<app id>/` folder), folder `0700` and socket `0600`, only
  in a runtime folder of the owner's that nobody else may write and in a socket folder that is no link, one
  JSON object a line each way (`docs/control-protocol.md`). Every line carries a token of 32 random bytes the app
  keeps in the OS credential store; a wrong, missing or revoked token is refused (`unauthorized`) and the connection
  closed, and a line without one before the credential store is read. Only `safe` commands run, for an agent, in the
  window's session; at most eight clients, a line at most 1 MiB and within five minutes, an answer taken within 30
  seconds (#142)
- `inboxcraft --revoke-control` puts a new token in the place of the old one; the app reads the credential store for
  every line, so the old token is refused from the next line on (#142)
- `inboxcraft-cli` without `--store` runs the command in the running app, and `inboxcraft-cli mcp` without `--store`
  runs its tools there, so the app runs the jobs they ask for. It shows the token only to a socket of the user's in
  folders that are the user's alone (`0700`); new error codes `not_running`, `no_token`,
  `unauthorized`, `busy`, `closing`, `bad_line`, `bad_answer` and `closed` (#142)
- `inboxcraft-control-key`: the token (constant-time comparison, never in `Debug` output), the request and answer
  lines with their limits, the socket paths and the check that a folder is the user's alone, shared by the app and the CLI; the fuzz target `control_lines` (#142)
- `inboxcraft_mcp::Commands`: the MCP server runs its tools on an engine session or on the running app (#142)
- `Mailbox::run_for_agent` in `ui-egui`: runs a command for an agent and reads the list again; an agent's act ends
  the window's Undo for the acts before it (#142)
- `inboxcraft-ical`, iCalendar (RFC 5545): calendar objects read into a tree of components, properties and
  parameters (lines unfolded on the bytes, RFC 6868 parameter escapes) and written back with CRLF, lines folded at
  75 octets and CR, LF and NUL left out of values; typed values (dates, the three forms of date and time, durations,
  UTC offsets, TEXT escapes); recurrence rules read, written and expanded on the wall clock with every rule part
  (`BYSETPOS`, `BYWEEKNO`, `WKST`, ordinal weekdays, days a month does not have skipped); `VTIMEZONE`s as `time`'s
  `Offsets`; events with their people, answers, alarms, `RDATE`, `EXDATE` and changed instances
  (`RECURRENCE-ID`), and the occurrences of a calendar in a span of time, summer time followed. Capped: 4 MiB,
  50 000 properties and components, nesting 8 deep, 10 000 occurrences held and given per span and 2 000 000
  steps of work for a whole calendar; property tests for recurrence,
  round trips and spans; the fuzz targets `ical_calendar` and `ical_rule` (#143)
- `inboxcraft-dav`, CalDAV (RFC 4791, RFC 6764, RFC 6578) without I/O: `Discovery` from the well-known path or a
  given one to the principal, the calendar home set and its calendars (name, colour, ctag, sync token, components);
  `Sync` of one calendar from its last sync token, started again without one when the server forgot it
  (`DAV:valid-sync-token`), followed over answers cut short (`507`), or by a listing of entity tags where the server
  has no sync report, then what changed fetched with `calendar-multiget`, 50 at a time; `put` and `delete` with
  `If-Match` or `If-None-Match: *`, a change made elsewhere coming back as a conflict (`412`), never overwritten.
  HTTP/1.1 requests written and responses read (length, chunked or until close) from bytes, with a small XML reader
  of its own. Capped: response heads 64 KiB and 128 fields, bodies 16 MiB, XML 64 deep, 200 000 nodes and 64
  attributes an element, 100 000 resources a calendar, 5 redirects in a row, 16 sync reports a sync, 4 MiB a PUT;
  the fuzz targets `dav_xml` and `dav_client` (#144)
- The fake CalDAV server in `testkit` (`inboxcraft_testkit::dav`): the well-known redirect, PROPFIND on the
  principal, the calendar home set, calendars and resources, the reports `sync-collection` (with `page` for answers
  cut short), `calendar-multiget` and `calendar-query`, GET, PUT and DELETE with entity tags, on a kept-alive
  connection; its own HTTP and XML readers; shared `Calendars` a test changes "elsewhere" and reads back,
  `forget_tokens`, `without_sync`, `without_well_known` and scripted answers and faults (`FakeDav::on`); the fuzz
  target `testkit_dav` (#144)
- `TestClient::read_some` reads what a fake server sent as it came, for clients that frame answers themselves (#144)
- Calendar sync in `net`: the worker runs a `JobKind::Calendar` job (`model::CalendarJob`) at once on an HTTPS
  connection of its own, kept alive for the job's requests: `Discover` finds the calendar home set and its calendars
  from the account's CalDAV address (the well-known path, the principal, the home set), `Sync` brings one calendar in
  step from its sync token or by entity tags, `Put` writes an event new (`If-None-Match: *`) or over the copy seen
  (`If-Match`), `Delete` removes one if unchanged. Each answers with one `EventKind::Calendar`
  (`model::CalendarReport`: `Found`, `Synced`, `Written`, `Deleted`, `Conflict` for a change made elsewhere (412),
  or `Failed` with its cause), in the shape of the store's calendar tables, every place an absolute `https` URL (#145)
- `Account::caldav`: the account's CalDAV address, `https` only, with the host and port rules the CalDAV client
  parses it by, so an address that is saved is one it can use; `account.check` finds the calendars there as its
  CalDAV step, with the password the IMAP sign-in used, and reports it as not set without one (#145)
- New error kinds `NotEncrypted`, `OtherServer` and `NoCalendarServer` (#145)
- A live test that finds and syncs the `InboxCraft-Test` calendar of `live-imap`; the live account check expects the
  CalDAV step to pass (#145)
- The calendar commands: `calendar.list` (`safe`) lists the calendars of every account (or one) with their events
  counted, `event.list` (`safe`) the occurrences of their events in a span of up to 400 days, repeating events
  expanded, each with its moments in UTC and on the computer's wall clock, its first and last day, all-day, busy or
  free, private, its repeat preset (or `custom`), its guests with their answers, and the event's own zone with its
  wall-clock times when that clock shows another time than the computer's; an event whose text cannot be read is
  counted, not shown (#146)
- `event.create`, `event.update` and `event.delete` (`owner`): an event with a summary, a place, a description, a
  repeat preset (`event.repeats`: none, daily, weekdays, weekly, monthly, yearly), all-day or timed, busy or free,
  private and guests, kept at once and written to the server (`Put` new, or over the copy seen); an edit keeps the
  event's own zone, and tells the guests (`SEQUENCE` counted on) only with `notify` (the default is not to). A delete
  takes the event away at once and goes to the server once the six-second undo window is over; `history.undo` takes
  a delete back within the window, and a new event or an edit back at any time (#146)
- `calendar.sync` (`safe`) finds the calendars at each account's CalDAV address and brings them in step, as the start
  does; `calendar.server` (`owner`) sets or clears an account's CalDAV address, `https` only (#146)
- Writes to one event go to the server one at a time, each with the entity tag the one before got, so an edit, an
  undo or a delete right after another write to the same event does not conflict with it (#146)
- The calendar screen shows the calendars of every account in four views, Day, Week, Month and Agenda, picked by
  pills over the view, with steps back and on and Today. Day and week lay the hours out with the timed events by
  their minutes, side by side where they overlap, under a lane of blocks over whole days, and a line at the time
  now; the month runs blocks over several days across its weeks, with "+N more" for what does not fit; the agenda
  lists the 30 days from today, day by day (#147)
- The calendar's side panel: a mini month that puts the view on a day, a box per calendar that shows or hides its
  events, and the details of a picked event with both dates and its times, the event's own zone labelled with its
  times when its clock differs from the computer's, the place, the description, busy or free, private, the repeat
  and the guests with their answers (#147)
- The sample mailbox has three invented calendars with events around the current week; `ui_shot` draws the week,
  month, agenda and day in light and dark (`"calendar_view"`, `"event"`) (#147)
- The Previous, Next and Globe icons in both icon sets (#147)
- The event editor on the calendar screen: New event in the toolbar, and Edit and Delete under a picked event's
  details. It sets the title, the start and end (or the days of an all-day event), all day, private, a repeat
  preset, busy or free, the calendar of a new event and the guests' addresses; an edit sends only what changed (#148)
- Editing an event with guests asks whether to tell them of the change, Don't tell them by default; an event
  without guests, and a new event, ask nothing (#148)
- A saved event, a saved edit and a delete show a toast with Undo and Ctrl+Z, which take the change back on the
  calendar screen too (#148)
- `ui_shot` draws the event editor on a new event and on an event with guests, and the toast of a deleted event, in
  light and dark (`"edit_event"`, `"delete_event"`) (#148)
- Invitations in mail (iTIP, RFC 5546, over iMIP, RFC 6047): once a message's body is kept its calendar part is read
  and the message is marked with the event's `UID`, `SEQUENCE`, times, title, place and the owner's answer; updates
  and cancellations are linked to the same event by `UID`, and the later version is the higher `SEQUENCE`; an answer
  (the owner's own, synced back from Sent, or a guest's) is no version and dates no invitation (#149)
- `invite.show` (`safe`): a message's invitation as its card shows it, with the version before it and what changed
  (time, repeat, place, title), whether a later one made it out of date, the answer given and the event in the
  owner's calendar; `mail.read` says whether a message carries one (`invitation`) (#149)
- `invite.answer` (`owner`): Going, Maybe or No goes to the organiser at once as a `METHOD:REPLY`, with the checks
  `mail.send` makes but without the undo-send hold, and the event goes into the account's first calendar (or comes out of it for No); a copy the
  calendar holds already is changed in place and keeps its reminders; the organiser's reminders, attachments, `X-`
  properties and other components never go into the calendar, and the answer names nobody beside the organiser and
  the owner; `compose.update` and `compose.template` refuse the draft of an answer; `outbox.list` marks an answer
  (`invite_answer`), the outbox offers no Edit for a refused one, and `outbox.retry` sends it again at once (#149)
- `invite.remove` (`owner`): takes the event a cancellation is about out of the calendar, with the undo window of
  `event.delete` (#149)
- The invitation card in the reader: a date tile, the title, when and where, what changed, Going, Maybe and No, then
  the answer with Change answer; a cancellation's Remove from calendar with a toast and Undo; a guest's answer; a
  strip for the version before and for a version a later one replaced. The calendar part shows as no file (#149)
- `inboxcraft_mime::MessageBuilder::calendar`, a `text/calendar` part with its `method` beside the text;
  `inboxcraft_model::Invitation`, `ItipMethod`, `Rsvp`, `MessageState::invitation` and `Draft::calendar` (#149)
- The icons Changed and Older in both icon sets (#149)
- `ui_shot` draws the invitation card over the sample's invitations to lunch, the update and the first one out of
  date, in light and dark (`"invitations"`); the fuzz target `engine_invite` (#149)
- Event reminders: an event's display and sound alarms, or without any alarm ten minutes before a timed event and a
  day before an all-day one, become notifications at the alert time; a trigger's days follow the event's wall clock
  across a change to or from summer time; a cancelled event and one the owner declined alert nothing (#150)
- The store holds each event's alerts of the next two weeks with their schedule entries, worked out again when the
  event changes (a sync, `event.create`, `event.update`, `event.delete`, `invite.answer`, undo), at the start and once
  a day; an alert missed while InboxCraft was off tells late, unless the event is over (#150)
- `notify.set` and `notify.settings` take and show `alerts`, whether calendar alerts notify, kept in `notify.json`;
  `inboxcraft_engine::notify::Notice::Alert` with `EventAbout` (#150)
- A click on an alert's notification opens the calendar on the occurrence's day with the event picked
  (`InboxApp::open_event`); the notification says when, on the computer's clock, and where (#150)
- `Storage::alert`, one alert by its id; the fuzz target `engine_alerts` (#150)
- The add-account form asks for the outgoing server and the CalDAV address; Detect fills the outgoing server (#214)
- Settings, Accounts: an account opens its settings, with everything but the password, and `account.update` (`owner`)
  changes them; a changed server or user name runs the owner's check again, and the form first says where the
  sign-in goes (#214)
- A new password can be typed in an account's settings: it replaces the stored one in the OS credential store for the
  worker's next sign-in, empty keeps it, and the password is never shown (#214)
- The design file draws the outgoing server and CalDAV address of the add form, an account's settings and the check
  after a change, in light and dark (#216)
- The first part of the Qubernetic design file, beside the present one in `docs/design/qubernetic/`: foundations,
  58 components with every state in dark and light, and the InboxCraft mark (#222)
- The second part of the Qubernetic design file in `docs/design/qubernetic/`: the window frame, Home, the mail lists,
  the reader, eight kinds of message, the composer, the calendar, and the same screens at 1100 and 800 wide and at
  130 % text (#224)
- The third part of the Qubernetic design file: popups and menus, settings, accounts, what shows outside the
  window, the planned features with an example of a plugin, a sketch of the phone screens, motion at three levels,
  the layout's reasons and the coverage page (#226)
- A "coming soon" mark in the design: a surface that is not implemented yet is shown, cannot be pressed and
  carries the mark (#226)
- Asset files pinned by sha256 in `.config/assets.toml`, checked by `cargo xtask assets` (#232)
- The InboxCraft marks (app icon in the hicolor sizes, tray icons, marks, wordmark and lockup in SVG, app icon PNGs)
  and the Qubernetic horizontal lockup under `assets/brand/`, copied from the brand repository's tag v0.3.0 and
  pinned; trademarks of Qubernetic, not under MIT or Apache-2.0. The window icon is `icon-256.png`, the tray the
  dark tray PNGs at 16, 22 and 24 px, and the Flatpak installs the hicolor tree; the rest are not used yet (#232)
- `just bench-ui`, a frame-time bench of four scenes (the inbox at rest in the default view, the grouped list at
  rest and scrolling, the reader on an HTML message) over the sample grown to 10 000 invented messages, unpinned
  and pinned to one core (#233)
- Text in scripts IBM Plex does not hold is drawn with the computer's own fonts when it has them (#233)
- The "coming soon" mark: a surface that is designed and not built yet is shown disabled with one tag (#233)
- A shot script can press the pointer before its picture (`click`), so a menu or a list it opens is drawn (#233)
- A field's caret is the design's 7 × 16 block in the accent, blinking once a second while the field has the
  keyboard (#234)
- The rows of a menu or a picker come in one after the other, 30 ms apart, at full motion; with reduced motion they
  are all there at once (#234)
- Home and End go to the first and the last row of a menu (#234)
- A one-line field without the focus shows a value longer than its box cut with one ellipsis; with the focus it
  scrolls as before (#234)
- A one-line field holds at most 4 096 characters (a password 256, a typed time 64, the composer's subject 998 bytes,
  what a message keeps of one), a field of several lines 64 KiB unless the screen that shows it names more, and the
  composer's message text 1 MiB; typing, Enter, a paste, a
  middle-click paste, an input method's text, an undo or a screen reader's edit that would make the text longer than
  that is refused whole, in whatever order a frame brings them: the text, the caret and the selection stay as they
  were (a refused input method's commit also takes its unfinished word out, as ending it does) and the field says it
  is too long, brought into view; a text already longer (a draft from the store) opens whole, can always be edited
  down and undone back up to the size it opened with, and never grows past that (#234)
- A recipient line never drops or cuts what was typed: addresses past its 500, or one past 998 bytes, stay typed
  exactly as they were written (separators and spaces), the line says how many were not added or that an address
  is too long, and Send waits until they are gone; a draft keeps what is typed in its To, Cc and Bcc lines
  (`compose.update` takes it as `typed`, at most 4 096 characters a line; it is never sent nor in the Drafts copy,
  and a change of it alone uploads no copy), so closing, answering, reopening or quitting loses none of it, and the
  line says again what it refuses; Cc and Bcc show whenever they hold typed text, and Send takes typed text as
  recipients only from the lines it draws, refusing to send while a line it does not draw holds some (#234)
- The Snooze of a row's hover actions opens the time picker under it, by the pointer or by Enter or Space; the time
  chosen snoozes that message, and Cancel or Esc closes the picker and gives the row the keyboard back; the picker
  follows its message's row (mail coming in above moves it down) and closes for good when that row is not drawn in
  sight in a frame (scrolled away, another list or screen, the message gone) (#234)
- `ui_shot` draws the fields, pickers and menus on a page of the gallery of their own (`"fields": true`), and opens
  a menu with `"menu"` (`more`, `move`, `from`, `templates`, `group`) (#234)
- The shared rows and blocks of the design as components: the list row (its hover fill by the ground it stands on,
  the current row's bar, the line under it, the focus edge inside it), the banner in its six kinds, the empty state,
  the progress line with its bar of ten characters, the settings row and the coming-soon one, a step of an account's
  check, and the facts of a message with their details (#235)
- Four icons for the rows' marks: a tick (a done row), a countdown (a held send), a pause (the park pile) and an
  arrow back (a row back from a snooze) (#236)
- The draft row of the design (subject, whom it goes to, when it was edited; "open" and "discard" while hovered),
  drawn on the gallery until the Drafts place comes (#236)
- `ui_shot` draws the page of the mail lists' rows with `"lists": true`: the mail row, the group head, the outbox
  row, the later row and the draft row in every state of their sheets, dark and light (#236)
- The list's keys, actions on the Shortcuts page and rebindable: `J` and `K` (`ui.list.next`, `ui.list.previous`) and
  the arrows (`ui.list.down`, `ui.list.up`) move one row, Page Down and Page Up (`ui.list.page_down`,
  `ui.list.page_up`) by the rows in sight, End and Home (`ui.list.last`, `ui.list.first`) to the last and the first
  row; `Enter` (`ui.list.open`) opens the keyed message and hands the keyboard to the reader, where `Esc` gives it
  back to the list's keyed row (an `Esc` that closes a menu or a picker there closes only that); `Shift F10` and the Menu key (`ui.list.menu`, `ui.list.menu_key`) open the keyed
  row's group menu under it, its first choice focused, and the row has the keyboard again however the menu closes
  (`Esc`, a choice, a press outside); `Esc` from a row's menu the pointer opened gives that row the keyboard and
  selects nothing
  (a menu closing never changes the message the reader shows, in one pane or beside the reader); asked for while the keyed row
  is out of sight, the row comes into sight and the menu opens at it (#236)
- `ui_shot`'s script draws the lists' new scenes, dark and light: Smart, Grouped and List, Later's pins, New
  senders' queue, an empty folder, a list
  that scrolls, the one pane of a window 800 wide, and at 900 the list, the reader with "Back to the list", a held
  message's head with it, and the composer in the one pane (#236)
- The inbox's three views are actions too, `ui.view.grouped`, `ui.view.smart` and `ui.view.list` ("Inbox view: …"):
  in the command palette and on the Shortcuts page, without a key until one is given (#236)
- The reader's sets in the Qubernetic look, each with every state of its sheet on a gallery page of its own
  (`ui_shot` key `reader`): the why line, the content bar, the thread row, the quote fold, the attachment, the
  hairline table and the invitation card (#237)
- The reader's why line under the facts, drawn as its row and coming soon: it says no reason the window would
  make up, since why a message is here is the engine's to say; its `open log →` is drawn and coming soon (#237)
- The reader's content bar names the kind of message (TEXT, HTML, INVITE) and what was held back of it; its choice
  between `fitted` and `plain text` shows an HTML message fitted to the app or as a text-only program shows it, in
  mono (the message's own text part, or the text of its blocks, quotes folded as a text's are); `open in browser ↗`
  is drawn and coming soon (#237)
- A message opening wipes in from the top behind a one-pixel cyan scan line over 300 ms on the design's curve; with
  reduced motion it is there at once, and a key or a press during the wipe ends it (#237)
- The subject's menu (a secondary press, or Enter or the lists' menu key, Shift+F10 or the Menu key as bound, on the
  subject, which is a Tab stop with its focus ring) copies the subject whole (#237)
- "Open the message the rule names" in the command palette: with nothing open after the server took the open message
  away, it opens the next or the previous message, as the next-message rule says (#237)
- An open line of a thread has a menu (a secondary press, or the lists' menu key as bound on the line) that shows or
  hides the message's full header, so an older message's header stays one key away where its line has no room for
  `details ▾` (#237)
- The reader's gallery page shows a fitted HTML message with its quote folded and open, and `ui_shot` shows a
  message as plain text with `"html_view": "plain"` (#237)
- `ui_shot` takes `"act": "removed"`: the selected row taken away as another device would, so nothing walks on; the
  reader's nothing-open scene uses it (#237)
- A file on a draft as its sheet draws it (`draft-file`: default, hover, focus, too large, which it also says in
  words: heard, and in a tooltip), on a gallery page of the screens' own sets (`ui_shot` `"screens": true`), coming
  soon until files can be attached (#238)
- The calendar's arrows, Enter and Esc as keymap actions (`ui.calendar.left`, `right`, `up`, `down`, `open`, `close`),
  on the Shortcuts page; the keyboard's focus and the picked day or event are one thing; closing the editor or a
  day's float gives the keyboard back to what opened it, and when that is gone, to the picked day's number (month)
  or the head of the view's day (week), or to New event (day, agenda) (#238)
- A timed event whose time changed glides its top to its new place (300 ms); an event's ground fades under the
  pointer on CSS's ease (125 ms); both stand still with motion reduced (#238)
- `ui_bench --calendar`: the week view holding 200 events, and the week, month and agenda holding 5 000 (#238)
- Settings: the sections Calendar, Privacy and About of the design, and the planned AI and Plugins, each with its
  page; what is not built yet on them, and on the other pages (open or write a message in a window, Undo send set to
  Off, the templates and the scheduled list, each connection's state, removing an account, setting up a phone), is
  drawn coming soon (#238)
- `ui_shot`'s script takes `steps` that drive the set window as a person would: `key` (a key with its modifiers,
  through the window's input, so the keymap and a rebinding decide), `tab` and `shift_tab` (1 to 64 times),
  `focus` (a control by its accessible name, as a screen reader asks) and `press` (a control by its name, pressed
  once the frame has laid it out, so a hover action or a menu's row can be pressed); `nth` picks one of several
  controls that can be pressed or focused with the name, topmost first. Each step lets the window come to rest (at
  most 240 frames) before the next and before the picture; a picture draws at most 2000 frames in all. A name nothing has, or several have without `nth`, an unknown step or field and a
  count over its cap are errors that name the line or the step; while steps run, the window's commands run for an
  agent, so a step, or a scene's `click`, that reaches a command which is not `safe` fails the picture (#248)
- `ui_shot --out-dir <dir>` writes each picture under the folder instead of at its `out`, which cannot leave it
  (#248)
- `cargo xtask shots`: every scene of `shots.jsonl` drawn by `ui_shot` equals its committed picture byte for byte;
  it fails naming the scenes whose picture differs or is missing, two scenes that write one picture, a picture
  beside them that no scene writes, a run that draws no scene, and an `out` outside `docs/images/` or with a
  symbolic link on its way; `--fix` writes the renders and deletes the strays, only ever under `docs/images/`. Each
  run renders into a folder of its own under the target directory, removed at the end. It is a gate of
  `cargo xtask ci`, after `deny` (#248)
- Pictures of Petra's row with the keyboard focus beside the list's scroll thumb, the account picker's list open and
  a row's Snooze picker opened from its hover action, dark and light (#248)
- The index down the window's left, 232 wide: the places [ 00 ] HOME (coming soon), [ 01 ] MAIL, [ 02 ] CALENDAR,
  [ 03 ] LATER and [ 04 ] SETTINGS; under MAIL the shared mailboxes with their counts (the Outbox while a send
  waits), then each account, which opens to its own folders; at the foot the owner's row, which opens the account
  picker (All accounts or one account). Only the mailboxes scroll. Hovered, a place shows its key (#254)
- `sync.status` (`safe`, no parameters) answers how syncing stands over every account: `state`, the first that
  holds of `failing` (an account stopped until the owner acts: a refused or unanswered sign-in, no password, no
  login method, a certificate or encryption refused), `offline` (no account signed in, connecting counts as not
  yet), `failing` (a signed-in account's connection or a job failed, a body fetch aside, and it has not signed in
  again or been told current since), `syncing` (a round has folders left, or a signed-in account has not ended a
  round yet) and `synced` (always with no account); `done` and `of` (the folder syncs of the rounds under way);
  `last_synced` (the newest time an account was told current, ISO 8601 UTC, or `null`) and `failing_accounts`. It
  runs in the CLI, headless (where no worker runs, so an account reads as `offline`) and over the control channel
  (#255)
- The workers' events `SyncProgress` (folder syncs done of all in the account's round) and `SyncFinished` (the
  account is current); an end more than five minutes ahead of the clock is dropped and one less far ahead is taken
  as the clock, and an event for an account the session does not hold is dropped too. A round's progress ends with
  the round, with its connection going offline and with changed settings (#255)
- Each account's worker counts the folder syncs it holds as a round: it tells the round's progress when it starts,
  right after each folder ends, synced or failed (before the connection goes offline when it drops under the
  folder), and again at once after a new sign-in, and its end when no job
  and no connection failed during it (a body fetch aside). A resync of the watched folder tells an end too when
  nothing failed since the last sign-in, the last clean round or the resync before it, so the resync right after
  a failure tells none. It counts the jobs it has, with no request to the server (#255)
- `ErrorKind::needs_the_owner`: the failures that stop an account until the owner acts, which `sync.status` reads.
  It and the worker's list of the failures it retries name every kind, so a new kind is put on a side before it
  builds, and a kind off the retry list stops the worker, with no second password (#255)
- `mail.why` (`safe`): why a message stands where it does, at most four reasons, most specific first (back from a
  snooze while it is in the inbox, or snoozed until, the grouped inbox's rule, the owner's choices for the sender,
  the screener, the thread, else the folder), each an i18n key and its values, never text of the message (#256)
- `week.settings` (`safe`) and `week.set` (`owner`): the first day of the week (`monday` to `sunday`, Monday by
  default) and the first week of the year (`first_four_day_week`, ISO 8601 with Monday, by default, or
  `week_of_1_january`); `week.settings` also answers the week of today, its first day, number and week-year. A
  kept value that cannot be read falls back to the default (#257)
- Two time presets in `schedule.presets` and for `mail.snooze`, `mail.follow_up` and `mail.send_later`:
  `this_weekend` (the coming Saturday at 09:00; from 09:00 on a Saturday, and on a Sunday, the next one) and
  `next_week` (09:00 on the first day of the next week, by `week.set`), after `tomorrow_morning` (#257)
- Motion has three levels, full, reduced and off, reduced by default: `ui.motion.set` takes `level`, and the
  appearance it answers and the kept `appearance.json` carry `motion_level` and `motion_chosen`. While the owner has
  not chosen a level, the computer's wish for less motion turns it off (#258)
- The add-account form says in one line that the server rows come from an earlier Detect, for another address, while
  the address has changed since Detect answered and no server field was edited by hand, and names the user name too
  while it is still the one Detect filled in; it goes when Detect runs again or a server field is edited (#259)
- A line under a server row says when its encryption is unusual for its port (TLS on 143, 587 or 25; STARTTLS on 993
  or 465): which encryption the port usually goes with. Saving is not held back and nothing is changed; any other
  port says nothing (#259)
- An invitation the organiser did not send says that the answer goes at once to the organiser, not to the sender,
  with the organiser's address; one from the organiser is as before. Where the sentence does not fit its line, a
  shorter one stands there, and where that does not fit either, the address takes a line of its own under it (the
  card one line higher), cut with one `…` only when it alone is too long (#259)
- The time picker of Snooze, Send later and a follow-up reminder lists "This weekend" and "Next week" after
  "Tomorrow morning", in the engine's order and at the engine's times (Next week at 09:00) (#265)
- The calendar's side panel syncs the calendars on the owner's press: "sync now" at its foot runs `calendar.sync`
  and the line shows when the calendars were last read, the sync as it goes (the account read and a bar of the
  calendars read of those found), the time it ended, or what failed in the window's words with "try again". A press while a sync
  runs starts no second one; nothing else (opening the calendar, a hover, the keyboard's focus, time passing) runs
  it. The line is one Tab stop that keeps the keyboard through its states, and it rests under the event editor (#271)
- Two read-only MCP tools: `week_settings` (`week.settings`, no arguments) says how weeks are counted and which
  week today is; `sync_status` (`sync.status`, no arguments) says how syncing stands, and reads `offline` on a
  store, where no account syncs. Both run headless on a store and in the running app. A call of `mail_why` is
  refused as not offered yet (#273)
- The character field and the emblem as a module of the window's crate, not yet behind any screen: a grid of
  8 × 15 point cells, about 5 in 100 holding a `·` in `faint`, the same pattern on every launch; the field's
  cells within 60 points of the pointer flip to code glyphs and cool down within 200 ms (at the reduced level
  only Home's field, within 30 points; at off still dots); an emblem the caller hands in, drawn at its one size
  only where it fits whole, its cells flipping and settling back under the pointer at full and reduced. The
  field takes no click, key or focus and asks for frames only while a cell cools (#275)
- A gallery page of the field at 100 % and 130 % text (`ui_shot` key `character_field`), and `ui_bench --field`:
  a window and a 4K screen of the field with the pointer sweeping (#275)
- The add-account form says when Detect's servers are a guess from the mail provider the domain's MX records point
  to ("check the servers before you add"), in words of its own beside those for the domain's own settings file,
  Mozilla's list, the DNS records and the domain's common host names; an outgoing server guessed that way says so
  too (#279)
- A server Detect proposes without having reached it says so in one line under its row, with why: it did not answer
  from here, it refused the connection, its name was not found, or its certificate does not fit this computer's
  clock (which may be what is off). The host named in the line is cut to 48 characters with one `…`; a screen reader
  hears the line with the host field. The line goes once that row's host, port or encryption is edited, or Detect
  runs again; the fields hold what the domain names, and Check and add still checks the server before any password
  goes out (#279)
- When the owner typed their own server and Detect found another it did not reach, the note that names Detect's
  server and says the owner's stays also says it was not checked and why, in the same words (#279)
- `ui.settings.open` (`safe`) takes an optional `page`, one of the nine pages of the settings: through the control
  channel and `inboxcraft-cli ui.settings.open '{"page":"accounts"}'` the running app shows the settings on that page,
  also while they show; without `page`, or with one that names none, on the page last shown. An agent's call never
  takes the owner off the settings and is passed over while the owner has typed into the account form or gives a
  key; the newest acts once a frame and the owner's own key or press wins. The answer's `window` is `false` headless,
  where nothing is shown (#284)
- The command palette lists the settings' nine pages after the actions, "Settings: Appearance" to "Settings: About",
  found by "settings" or the page's name; each shows the settings on its page, also while they show, and has no key
  of its own. The palette reads the first 64 characters of what is typed (#284)
- The empty calendar has "Open accounts" under its words, a secondary button reached by Tab, which shows the settings
  on Accounts (#284)
- The command line's sets, not yet behind a screen: the command field (the prompt, the scope's mark and name for
  commands, people, mailboxes, one mailbox or all mail, the text with its caret or the hint, and the key chip that
  shows the opening key at rest and Esc once the field is in use; it holds at most 1 000 characters and cuts a
  longer paste where it goes in, and drops control characters and bidi overrides from what is typed, pasted,
  composed or set by a screen reader as it goes in), the key hint (a key chip and the word of its action, information without a Tab
  stop, its click the press) and the window's own controls ("Minimise", "Maximise" or "Restore", "Close the
  window"; each a Tab stop
  pressed by Enter and Space). `ui_shot`'s `command_sets` draws every state of the three on one page (#286)
- The status line's two sets, not yet behind a screen: the status item (a fact in six kinds: plain, ok,
  waiting, offline, error, accent; a button with hover, focus and press where the caller takes the press) and
  the status message (undo and send countdown with undo, the key and the seconds left, notice, error with
  dismiss, done), drawn on a gallery page of their own, `ui_shot`'s `status_sets` key (#287)
- Home's sets, not yet behind a screen: the count tile (a count over its label, in the tone's colour, `muted` at 0,
  capped at `9 999+`, a button that takes its row's height), the block (a hairline box with its section label and a hairline over its content, a
  button when asked, selected on `stage`) and the block line (why a block holds nothing, or the small link to the
  whole list, which returns its press). They are drawn on a gallery page of their own for `ui_shot`
  (`"home_sets": true`) in light and dark (#288)
- Home as a page module: the brief of today (the date, a greeting with the owner's first name, a
  sentence of what waits, four count tiles of one height, the rest of today's agenda with at most three lines, the
  two newest drafts, two buttons), the form before the first account (what InboxCraft is, two buttons and the four
  steps adding an account takes) and a failing banner in the sentence's place when the server refused a send. It is
  built from the mailbox through `safe` commands only and gives back what was pressed without running it. It is
  drawn on a page of its own for `ui_shot` (`"home": "<state>"`) in its five states (#299)
- `compose.list` takes `limit` (1 to 1000; more is cut to 1000, 0 or anything not a whole number is refused) and
  answers only that many of the newest drafts; its answer gains `total`, how many drafts there are in all, with or
  without `limit` (#299)
- Home is a place of the window and the place it starts on once there is an account (with none it offers to add one,
  as before): `[ 00 ] HOME` in the index shows it and is the place shown while it shows, and the command palette
  lists "Show Home"
  (`ui.home.open`, an action of the window like `ui.mail.show`, not an engine command). A count's tile shows its
  mailbox or pile (the inbox, New senders, Later's snoozed mail, the outbox), a draft row opens that draft in the
  composer, keeping the draft being written first, its "discard" hides it at once and says "Draft discarded" with
  Undo (button and Ctrl+Z) for the toast's six seconds, then discards it here and on the server (also when the
  window closes or the app ends; Undo brings the row back where it was), the
  agenda's link shows the calendar's week or today's day, "all drafts" the Drafts mailbox, and the buttons do what
  they say. On Home the mail's keys (the list's, the acts on a row, the views, Back, Snooze) rest as on the
  settings; the composer's Send and fold run wherever a composer is drawn, over Home too, and rest on the settings,
  which draw none; the window's own keys run, and Esc closes an open list or menu and otherwise nothing. Home is built
  again only while it shows: when the mail, the drafts or the events change, when the hour turns and when one of
  today's events left ends (#303)
- The command line across the top of the window: one field for search, commands, people and mailboxes. Plain text
  searches the mail as it is typed while the mail shows; from another place typing changes nothing under the
  field, and Enter shows the mail searched; `>` lists the window's commands, `@` the people the mail knows, `#` the
  mailboxes. `Ctrl K` and `/` open it empty, `Ctrl F` narrowed to the mailbox shown; `/` runs only while no text
  field has the keyboard. The arrows move the pick, Enter or a press runs it; one `Esc` closes the results, the
  next gives the keyboard back to where it was (#305)
- A command that cannot run here is listed with the reason and runs nothing; a plugin's command is drawn coming
  soon (#305)
- The status line at the window's foot says how things stand: the worst state first (an account's failure with the
  count of the others, offline, the sync's progress while it runs, or when the last sync ended), then the accounts,
  the outbox (and when a held send goes) and the next of today's events, with the keys that work now at its right.
  A failing account's "open account" opens the accounts (#309)
- Settings, Sending: "manage templates →" opens the Templates page, a group per account with a row per template
  (its name, its subject and text on one line, edit and delete). New template and edit open a form on the page
  (account, name, subject, text; Save and Cancel) that says when Save replaces another template of the same name;
  a renamed or moved template leaves no copy under its old name. Delete asks in place, says that it cannot be
  undone, and deletes only on the press of "Delete template"; Esc or Cancel keeps it (#317)
- Settings, Sending: "open the list →" opens the Scheduled page, everything that waits for its time, the next first:
  a row per entry with its time, its kind (send, snooze, reminder, alert), the account's bar, its subject and whom it
  names (or how long before its event an alert rings), and cancel. Cancel asks in place, says that it cannot be undone
  and what follows (a send goes back to Drafts, a snoozed message comes back to its inbox now; one moved, archived or
  deleted meanwhile stays where it is and loses only its time), and cancels only on
  the press of the confirmation's button; Esc or "Keep it" keeps it. Entries that cannot be read are left out and
  the page says how many (#317)
- Settings, Privacy: the senders whose remote pictures always load, a row per sender with "ask again", which takes the
  sender off the list at once on its press (its pictures are held again) and says so in the status line; with
  nobody on the list the page says so (#317)
- Settings, Calendar: the "Calendar alerts" switch works: off, no event's alert notifies, and its help says so (#317)
- Settings, Accounts: an account's form has "Remove account…" at the right end of its buttons. It asks in the buttons'
  place, first that it cannot be undone, then what goes from this computer and what stays on the server; only the
  press of the question's "Remove account" removes the account with what this computer keeps of it and takes its
  password out of the system's credential store, and the status line says so. A held Enter or Space never answers
  the question. When a message is being written from that account, the question says it is discarded with it, and
  the removal closes it (#317)
- Settings, Triage: "What follows an act" chooses what Done, Archive, Delete, Snooze or Move on the open message
  leads to: the next message, the previous one, the list, or Stay, which leaves the keyboard on the next row without
  opening it; under Stay the act keys take the row the keyboard is on, so they go on down the list. The reader's More
  menu offers Stay too. The choice is kept between runs; a kept value that cannot be read is Next (#317)
- `after.set` (`owner`): what follows an act on the open message, `next`, `previous`, `list` or `stay` (#317)
- Settings, Calendar: "Week starts on" chooses the week's first day and "First week of the year" whether week 1 is
  the first four-day week (ISO 8601) or the week of 1 January; the calendar's weeks and months follow at once. Until
  the owner chooses, the desktop app takes them from the system's region (`LC_ALL`, `LC_TIME`, `LANG`): Sunday and
  the week of 1 January for a short list of regions (the United States, Canada, Mexico, Brazil, Japan and others),
  Monday and ISO 8601 elsewhere; a kept value that cannot be read is that default too (#317)
- Motion at full: a text button's label resolves from glyphs of the code set, from its left, when the pointer comes
  onto it, 70 ms a character and 300 ms at most; it does not run again until the pointer has left. Home's headline
  resolves the same way over 700 ms each time Home opens; at reduced only once per launch, at off never. Each
  character changes once, and a key or a press during it ends it in that frame (#318)
- A new unread row that a sync brings into the list shown moves in (M4): at full its words decode in from the left
  over 300 ms out of glyphs of the code set, then its unread square goes off once for 125 ms; at reduced it fades in
  over 125 ms, then its square goes off once for 125 ms; at off it is there at once. A row that comes back (an undo)
  does not move, nor do the rows of a list just opened; rows that come together each start 30 ms after the one
  above; a key or a click ends it in that frame (#318)
- Later's snoozed pile stands under three heads: Back today (before midnight, and anything whose time has passed),
  Back this week (before the first day of the next week, by the week's first day chosen on Settings, Calendar, where
  the Next week preset begins) and Later. A head with no row is left out; a search lists by time with no head. The
  heads offer no "done all" (#323)
- A table in a fitted message that is wider than the reader is a Tab stop of its own, before its links, with the
  focus ring inside its box; while it has the focus, Left and Right move it 48 points a press, so the columns
  scrolled off are reached from the keyboard (#323)
- Home draws the Qubernetic emblem in characters on its character field, and under it the Qubernetic and the
  InboxCraft wordmarks and the line "Your mail, on your machine.", in dark and light; where the emblem and the names
  do not fit whole together, neither is drawn (#335)
- The Qubernetic wordmark (light and dark SVG, from the brand's tag v0.3.0) and the emblem's grid of characters
  under `assets/brand/qubernetic/`, and `REUSE.toml` naming the licence of the brand's files, with the MIT and
  Apache-2.0 texts also under `LICENSES/` (#335)
- The Flatpak installs `NOTICE` and the licence texts (MIT, Apache-2.0 and CC BY-ND 4.0) in
  `/app/share/licenses/io.github.qubernetic.inboxcraft/`; the metainfo's project licence stays the code's,
  MIT OR Apache-2.0. A change to the Flatpak manifest, so the PR's `flatpak` job runs (#335)
- New message stands at the top of the index on every place, above the places and as wide as the column: the
  compose glyph, its words and its key `C` at the right edge; in the narrow index its glyph alone in a 32 px square,
  named with its key in its tooltip. A press writes a new message as `C` does, from Home, Mail, Calendar, Later and
  the settings; it is the index's first Tab stop. Without an account it cannot be pressed and its tooltip says why
  (#339)
- Text size keys: Ctrl+Plus (and Ctrl+=, the same key without Shift on most layouts) draws every text a step larger,
  Ctrl+Minus a step smaller, Ctrl+0 at 100 % again, through the text size of the settings (`ui.text_scale.set`, 90 %
  to 130 % in steps of 10). The status line says the new size ("Text size 110 %"), or at an end that the size is the
  largest or the smallest and stays; a newer word on the size takes the place of an older one still waiting. The
  Shortcuts page lists the four actions ("Larger text", "Larger text (=)", "Smaller text", "Text size 100 %") and gives
  them new keys like any other; they run on every place, on the settings and under the event editor too, as the
  mode's keys do. The toolkit's own keyboard zoom stays off (#341)
- `event.create` and `event.update` take an `alert`: `"none"` or the minutes before the start, at most a week, written
  as the event's alarm on the series and on each changed instance of it; an update moves only the first timed display or
  sound alarm and keeps every other alarm, a location alarm too, written as it came. `"none"` takes the timed display
  and sound alarms off, so the local alert no longer rings at the default ten minutes either. A new event without an
  `alert` is written with the default lead as its alarm (ten minutes before, a day before an all-day event), so other
  devices ring too. An update of the alert alone changes only the alarms: the times, `DTSTAMP`, `SEQUENCE` and the
  scheduling parameters stay and the guests are not told, and a changed instance that cannot be read is kept as it came
  while the others take the alert. `event.list` says each event's alert, `"custom"` where it has more than one display
  or sound alarm (#343)
- `event.list` names each event's `organizer` and says whether it is the owner's `own`: it names no organiser, or the
  organiser is the account's own address (#344)
- The event editor shows the event's description and keeps its lines; a press on a link in it with Ctrl (Cmd on a
  Mac) held copies the link, and the status line says so (#344)
- Someone else's event opens read-only in the event editor: every field read-only, the controls disabled, a note
  naming the organiser, and Close instead of Cancel and Save (#344)
- The read-only MCP tool `mail_why` (`mail.why`, `id`): why a message stands where it does, the facts the reader's
  why line shows (a snooze and its time, the grouped inbox's group and rule, the owner's choice for the sender, the
  screener, the thread and whether the owner wrote in it, else the folder), at most four reasons, names and addresses
  quoted as untrusted data. It changes nothing, and runs headless on a store and in the running app (#347)
- Hebrew, Arabic, Devanagari and Thai text and emoji are drawn with fonts that ship with the app (Noto Sans
  Hebrew, Arabic, Devanagari and Thai and the one-colour Noto Emoji, SIL Open Font Licence 1.1, Regular only)
  behind IBM Plex in every text style, so they no longer depend on the computer's fonts; the icons keep Phosphor
  alone. The desktop app grows by about 1.2 MiB (#348)
- Once the window has drawn its first frame, the computer's fonts are searched for 21 scripts the app's fonts lack:
  Chinese, Japanese, Korean and Bengali, Gurmukhi, Gujarati, Oriya, Tamil, Telugu, Kannada, Malayalam, Sinhala, Lao,
  Tibetan, Myanmar, Khmer, Ethiopic, Syriac, Thaana, Cherokee and Bopomofo, keeping at most 24 faces and 96 MiB; the faces found stand behind
  the shipped ones (#348)
- How Home greets you is a setting: Settings › Sending, "Your name", greets by the first word of the sending name
  (the default), the last word, the full name or no name; the first and the last word skip titles such as Dr., Mr.
  and the Hungarian dr., a comma reads as "last, first" (the given name ends at the next comma, and a suffix such as
  Jr. or PhD is not one), quotes, brackets and invisible characters around a name are not read, and a name that is
  an address greets without a name.
  `greeting.set` (`owner`) sets it, and the choice is kept between runs (#349)
- Settings › Calendar › "All-day alerts ring at": 09:00, 12:00 or 18:00 the day before, or 09:00 on the day
  (default 09:00 the day before). A new all-day event's default alert is written as a trigger from the start of its
  day at that hour (`-PT15H` for 09:00 the day before), an all-day event without any alert rings here at it, and the
  choice is kept between runs beside the week rule; events already saved keep their alerts (#353)
- `event.all_day_alert.set` (`owner`, `at`: `day_before_9`, `day_before_12`, `day_before_18` or `on_the_day_9`)
  changes when a new all-day event's alert rings (#353)
- The design file draws every page of Settings in a narrow window, at 800 and at 1000 with the text at 130 %, on
  the Sizes page: one pane under 980 (the list of sections or a section's page, with `← Settings` in its head), a
  setting whose control needs more than half its row puts the control under its name (the settings row's new
  stacked states), Sending's signatures one under the other, About without the emblem (#368)
- Settings › Calendar › Editor › New event opens chooses where the event editor opens, for a new event and for Edit
  alike (`ui.layout.set`'s new `event_editor`: `small`, `full` or `window`, kept between runs): the small panel over
  the calendar; the full editor in the calendar's place, its day's hours 320 wide beside it, following the day its
  Starts names (the default); or a window of its own, 704 × 800, its title the event's. The full editor and the
  window put Starts, Ends and the two ticks on one row and Repeat, Show as and Alert on another; Alert, the event's
  alarm, shows there only. Both give the keyboard to the title when they open. Where no second window can open (the
  web app), the editor opens full and the status line says why, once (#370)
- `ui.layout.set` (`owner`) keeps where the reader shows (`docked` or `window`) and where the composer shows
  (`docked`, `floating` or `window`); both are docked on a fresh install. The appearance kept on disk and answered by
  every `ui.*.set` command has the `layout`; a kept file without one, or with junk or a host a newer version knows,
  reads as docked, part by part, and keeps its other choices (#374)
- A window of its own for a surface (#375): its keys and its `Esc` stay in that window and never act on the main
  window's list, the app's own frame draws its title bar as the main window's command line does, and where no second
  window can open (the web app) it falls back to its place in the main window; the composer and the reader move
  into it next
- Appearance › Layout › Write a message chooses where a message is written (`ui.layout.set`): in the reader's place
  (the default), floating over the window, or in a window of its own, 704 × 720, its title the subject, with when the
  draft was kept, the account it goes from and Send's key in its own status line. Its `Esc`, its Close and the
  desktop's close keep the draft on the disk as the composer's Close does; a draft the store does not keep leaves the
  window open. Send's key sends from it; the countdown and its Undo stay in the main window's status line. Where no
  second window can open (the web app), the composer shows in the reader's place and the status line says why, once
  (#379)
- A message opens in a window of its own where Appearance › Layout › Open a message says Window: a press on its row
  or `Enter` in the list opens it there, and the pane lets go of it. The window keeps its message when the list moves
  on, shows the why line, the cards and the pictures as the pane does, and what is done in it (its buttons, and Done,
  Archive, read or unread, flag and Delete by key) acts on its message, never on the list's row; an act that takes
  the message away closes the window. Several windows can be open, at most eight: one more closes the oldest. A
  message the server takes away leaves its window saying so, with nothing of it kept. Where no window of its own can
  open (the web app) the message opens in the pane and the status line says why, once (#380)
- `mail.row` (`safe`, `id`) reads one message as a list shows it, by its id alone, whatever the store's size (#380)
- The fitted view keeps a little of a message's look: blocks stand left, in the middle or right as the message
  aligns them (`text-align`, `align`, `center`), pictures and tables narrower than their room with them; a coloured
  background stands as a band behind its blocks, their words in a near black or white that reaches 4.5:1 on it (a
  white, grey or black background, or one no ink can be read on, is dropped), and words on the app's own fills
  inside it (a table's head row, preformatted text) in the app's ink; a table or cell that asks for a width
  in pixels or a share keeps it, never wider than the reader; a coloured cell keeps its colour (#383)
- A link styled as a button (`role="button"`, a background with padding) and a `<button>` are drawn as a button 36
  high, in the mail's own colour where it is kept, else as a secondary button: a Tab stop like any link, whose target
  the status float shows; Enter or a press copies its address, nothing opens. A `<button>` of a form, or one with a
  script action, goes nowhere and says so (#383)
- The reader's More menu opens the original of an HTML message in the browser, on Linux: a cleaned copy of what the
  reader shows, with no script, no event handler and no address in it (a remote picture is a box with its words, a
  link its words), written to a file only the owner can read (`0600`, in a `0700` folder in the runtime folder, else
  the cache folder), named from the message's number alone, and handed to the desktop portal, or to `xdg-open`
  outside the Flatpak. Writing and handing it on run beside the window, which goes on working meanwhile; the status
  line says when it was handed on, or why not. The copies go at the next start, at a normal close, after ten
  minutes, and beyond sixteen; the sample mailbox keeps its copies in a folder of its own. Only the owner's own press opens it: it is no command, so no agent, shortcut or palette can;
  the web app does not offer it (#388)
- The status line says since when the window has been offline ("offline since 14:10"), with "try again" after what
  it shows meanwhile, drawn coming soon (#396)
- Each place has its own keys at the status line's right: Home mail, calendar, write and commands; the calendar
  mail, write and commands; the settings mail and commands, and back on a page alone in one pane; the command line's
  results next, previous, run and close; one pane with a message in the list's place list (`Esc`) and commands (#396)
- An error in the status line is dismissed by `Esc` where the key reaches nothing else (no field or list of choices
  that had the keyboard, no surface, not back to the list or the sections, not on the calendar; on the settings the
  same whatever is under them), and the line then shows `Esc dismiss` beside commands, and beside the docked
  composer's send (#396)
- A status line message is a live region: a screen reader says it when it comes, an error at once; a send counting
  down is heard as what is sent and that it can be undone, without its seconds, so it is said once (#396)
- The command line with nothing typed lists GO TO (the inbox with how many messages it holds to read, New senders
  with how many wait, the calendar with the next event of the day) and LAST COMMANDS, the three commands run from
  the line most recently (once they ran), newest first, each with its key or why it cannot run now; the arrows or a
  press run one, and Enter on the empty line still goes nowhere (#397)
- A search typed in the command line lists, from any place, the newest three messages found with how many there
  are ("3 of 1000+" where the search read as many as it lists), the people the mail knows and the mailboxes whose
  path holds the text, each group under its head; the arrows move through all of them, and Enter or a press opens
  the message (in the mail searched for the text, the newest found when the arrows picked none), the person's mail
  or the mailbox. A search that finds nothing says so, one that fails says why, and Enter then shows the mail
  searched. Over the event editor nothing that leaves the calendar is listed: no GO TO row, nothing found (#397)
- `ui.home.open` (`safe`, no parameters) is an engine command: the control channel, the CLI in the running app and
  an agent can show Home. An agent's call is passed over while the owner holds the keys, as an agent's
  `ui.settings.open` now is; the owner's own key or press in the same frame wins, and of an agent's calls of
  `ui.home.open` and `ui.settings.open` only the newest acts in a frame. On a store it shows nothing, and its answer
  says so with `"window": false` (#399)
- `ui_bench` scene `reader-step-why` (alone with `--why`): the reader stepping down the grouped list with J, one
  message a frame, each opened message asking `mail.why` for its why line; a line under it gives the median of
  each part a step runs (`mail.why`, `mail.thread`, `mail.open`, the list read again) (#404)
- The pictures in `docs/images/` are taken on the design file's own sample mailbox: Odile Brannock's Personal and
  Studio accounts on Monday 16 November 2026 at 14:04, synced at 14:02, with the frames' people, subjects, times and
  counts (seven to read, three new senders, two back today, one in the outbox, the design review at 15:00 next), so a
  picture and its frame can be compared. `ui_shot` takes it with `"sample": "frames"`; `--sample` and
  `just run-sample` keep the present sample (#416)
- The reader's component gallery shows the invitation card while the owner changes an answer: Going, Maybe, No and
  Keep my answer (#421)

### Changed
- The README follows the brand of the design file and claims only what the code backs: the current mark, the
  design file's Mood page as the look built towards, the list and the full-width reader in the three accent
  presets, light and dark, the CI, fuzzing, licence, Rust and dev Flatpak badges, and the dev Flatpak as the one
  build (#112)
- The change fragments of the daily-use epic are folded into the docs; the README says what works today, with the
  limits that matter for daily use, and how an agent drives InboxCraft through `inboxcraft-cli` and its MCP server;
  `ROADMAP.md` has its stage, measured numbers and by-area table, and `docs/gaps.md` its ranked table (#151)
- Codecov follows the in-repo coverage gate (#109): `codecov.yml` ignores the files `.config/coverage.toml` counts
  nowhere or leaves out of the workspace total (the apps' `main`, the `ui-egui` drawing code and `xtask`'s `main`),
  lets the project coverage drop by at most 1 point against the base and asks 80 % of a patch; a test in `xtask`
  fails when the ignore list and the gate's excludes differ.
- Tests cover the weakest files of the read mail epic (#108): the account workers' event thread once the
  window is gone, the credential store where none runs, the owner's config folder for the kept appearance, a
  move that fails when the appearance is stored, an IDLE the server ends or refuses, and the pill's hover and focus
- The look follows the design file (#104): dark mode's accent is the preset's middle stop lightened; primary
  buttons sit on the text gradient, the stops darkened until white reads (at most 30 %); list rows sit 6 points
  inside their card; a group's header shows an outline icon in the muted colour; the folder pane has an outlined
  account picker with a chevron, Add account as a link, a MAILBOXES heading, an icon per folder, and each account
  lists only its own folders; the app icon is the InboxCraft mark (two cards entering a notched tray, Electric
  gradient, white tile)
- Accounts are added on the settings sheet (#104): its Accounts section lists every account with its colour,
  avatar, name and address under "Your accounts", with Add account at the top; the form, Detect and the check run on
  the same page, with a link back to the list. The window with no account is one card in the middle whose Add
  account opens that page. The folder pane no longer shows Add account. `ui_shot` takes `accounts` and `first`
- The screens follow the design file (#104): the settings are a sheet over the dimmed window, with the sections on
  the left (those not built yet faded) and each setting a row beside its control: segmented choices, accent and
  window-frame cards, a text-size slider, a switch and a two-row preview; the custom accent's fields show only
  while it is in use or being typed. The add-account form has its labels above the fields, the address beside
  Detect, Detect's result in a box, rows for the names and for server, port and security, and "Check and add"; a
  failed check names the step and its reason under the title and lists the steps on a card. The reader's subject
  is set large with the message's group as a chip. In the list, "Show all" is an accent link under the rows' text,
  the grouped cards show no account stripe, and a row's hover actions are Done, an envelope for read or unread,
  Delete and Snooze (not available yet). Without an account the folder pane is left out, in the empty window and
  while the first account is added; Lucide's Fill draws no disc behind the line glyph
- The reader's link status line shows only while the pointer is on a link, as a float over the reader's bottom
  left corner that takes no pointer; a notice still shows in a line at the bottom until dismissed (#90)
- `inboxcraft` alone opens an empty window that adds accounts, with the window's jobs going to `net` workers;
  `--sample` still connects nowhere and offers no "Add account" (#34)
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
- `cargo xtask ci` (and `just ci`) runs the tests once: the separate `nextest` gate is gone and the `cov` gate's
  `cargo llvm-cov nextest` run is the test run, with its JUnit report still in `target/nextest/ci/junit.xml`; a
  failed run says the tests failed and that no coverage floor was checked (#66)
- CI caches the compiled third-party dependencies and the cargo registry in the `test` job: every full run restores
  the cache, only a push to `develop`, `main` or an epic branch saves it, with this workspace's own crates removed
  from `target/` first; the key rotates weekly and follows `rust-toolchain.toml`, `Cargo.lock` and the
  `[profile.*]` sections; `CARGO_INCREMENTAL=0` in that job; `gh cache delete --all` purges it. The `codecov`,
  `flatpak` and fuzz jobs use no cache (#67)
- CI builds the `flatpak` bundle on every push and manual run, and on a PR only when the PR changes a Flatpak
  input (`contrib/flatpak/`, `Cargo.toml`, `Cargo.lock`, `.cargo/`, `apps/inboxcraft/Cargo.toml`,
  `assets/app-icon/`, `.github/workflows/ci.yml`); a new `changes` job decides with `git diff` against the PR base,
  and the `ci` gate accepts a skipped `flatpak` only when `changes` said so (#68)
- The fuzz workflow builds every target on every run, but a PR fuzzes for 30 s only the targets whose file under
  `fuzz/fuzz_targets/` is new or changed in the PR (`git diff` against the PR base); a push to `develop` now runs it
  too and fuzzes every target for 30 s; the nightly schedule and a manual dispatch are unchanged (#69)
- The CI `test` job builds the third-party dependencies at opt-level 0 instead of 2 (local builds keep 2): a cold
  job builds in 12 minutes instead of 31 and a warm one 15 s faster, the tests take 47 s instead of 26 s; the level
  is part of the dependency cache key (#70)
- CI no longer builds the `flatpak` bundle on a push to an epic branch (10 to 11 billed minutes per push); it still
  builds on a push to `develop` or `main`, on a manual run and on a PR that changes a Flatpak input, so an epic's PR
  into `develop` builds it when the epic changed one (#78)
- The look comes from colour tokens derived from the mode and an accent preset (#81): mode System (the default),
  Light or Dark, and the presets Electric (the default), Aurora and Citrus, each one entry of
  `theme::PRESETS` (three gradient stops and an accent). Surfaces, glass, float, selection, colour fields, badges,
  focus, markers, account colours and text are derived from the two choices; text tokens are moved in lightness
  until they meet WCAG AA (4.5:1) on every surface they sit on, markers and control edges 3:1; light-mode accent
  text is the middle stop darkened; text on the gradient picks white or a deep ink by contrast. The teal and coral
  palette is gone. The list, the reader and the add-account page use the tokens; `theme.rs` is the only file that
  names a colour (a test refuses any other). `InboxApp::set_mode` and `set_preset` change the look from the next
  frame (the commands and the settings page come later); `ui_shot` takes a `preset`
- Glass without blur (#82): soft colour fields of the preset's stops are painted once behind the window, and the
  folder pane, the list and the reader (or the add-account page) are translucent glass layers over them with a
  light top edge and a soft shadow, a gap apart; translucency is never laid over text (a UI test checks every shape
  of the window). Floats are about 80 % opaque over content that fades before them and more opaque over sharp text
  (menus, tooltips and popups), so text under them shows at no more than 1.2:1; text tokens are now also readable on
  glass over the fields and on a float over the fields and the gradient. One gradient helper
  (`ui-egui::gradient`) paints every gradient on shapes, lines and text, dithered against banding; a test refuses a
  gradient painted anywhere else, and egui's own scroll-edge fade is off. The selected row and folder show the
  selection gradient. Nothing asks for a repaint while nothing moves. Layer shadows are nine-slices whose picture
  does not depend on the layer's size, and every frame gives back the pictures it did not paint, so resizing the
  window neither rasterises the shadows again nor keeps their textures; a float over sharp text casts no shadow.
- Figtree replaces egui's Ubuntu Light (#83): the Regular, Medium, SemiBold and Bold weights of Figtree v2.0.3
  (SIL Open Font Licence 1.1) are bundled in `assets/fonts/figtree/` with their licence and compiled in; nothing is
  fetched at run time. Every text style is a font token of `theme::Fonts` (caption, body, strong, button, sender,
  subject, preview, heading, title, mono, read and unread); `theme.rs` is the only file that names a font (a test
  refuses any other). Sender and subject are strong, the preview muted, and an unread row is set in heavier
  weights as well as marked by its dot; the sender is no longer coloured by the accent. Bold text in a message is
  set in SemiBold. A text scale of 90 % to 130 % in steps of 10 % (`InboxApp::set_text_scale`, `ui_shot`
  `text_scale`) applies to every token; the message rows, the folder rows, the folded thread rows and the step
  marks take their height from the font's metrics, and the reader's controls wrap above the subject, which wraps
  rather than being cut. A test draws the list and the reader at 90 % and 130 % and fails on text that sticks out
  of its row or button, is cut at the side of its pane, or lies over other text.
- An icon painter with the Phosphor set (#84): views ask for an `IconId` (`ui-egui::iconset`) in a style, Regular
  for toolbars and rows, Fill for active and selected states and Duotone for empty states; each
  icon set maps every id onto a glyph of its font in an exhaustive `match`, so a missing mapping fails the build,
  and keeps its own size and optical alignment. The Regular, Fill and Duotone fonts of Phosphor 2.1.1 (MIT) are
  bundled in `assets/icons/phosphor/` with their licence and compiled in; nothing is fetched at run time. The
  flagged row's flag, the attachment tile's file and the add-account steps' marks now come from the set instead of
  shapes drawn in code; the app icon stays drawn in code. `ui_shot` draws an icon sheet with `"icons": true`.
- The engine registry takes one command list per `cmd/*.rs` module (`COMMANDS`), and `hostile_params_never_panic`
  takes the parameter names each module lists (`FIELDS`) instead of one fixed-size array, which missed `frame`;
  stub modules for `act`, `compose`, `send`, `outbox`, `triage`, `schedule` and `notify` (#111)
- `inboxcraft` keeps the owner's mailbox on disk, in `inboxcraft/store` in the data folder (`XDG_DATA_HOME`, in
  the Flatpak `~/.var/app/io.github.qubernetic.inboxcraft/data`), starts the session at once (left-over jobs, then
  the folder lists) and no longer removes the accounts' passwords when the window closes; without a data folder,
  or with a store that does not open, it says why and does not open the window (#113)
- An account's credential store entry is named with its store's id as well, `account-<store id>-<id>`, so two
  stores on one computer (the Flatpak and another build) never read, overwrite or remove each other's
  passwords (#113)
- On Windows the store is always in `%LOCALAPPDATA%`; `XDG_DATA_HOME` is taken on other systems only (#113)
- `mail.send` and `outbox.retry` answer `waiting` with the time the send goes; the message, its `Message-ID` and its
  `Date` are made when it goes, not when Send is pressed (#123)
- The `people` notification level tells of a known or accepted sender's mail only when it is in People by the
  owner's group for the sender or else the classifier, so a known sender's newsletter tells nothing (#125)
- The switch of the settings sheet is a component of its own (`component::switch`), shared with the time picker
  (#129)
- Every action of the window has an id, a name and an icon (`keymap::actions`) and runs through
  `InboxApp::run_action`, from its key, the rail or the palette; Sent mail, the reader's full width, the three
  modes and reduced motion are actions too and can have a key (#130)
- Undo goes through the engine's history (`history.undo`): the toast's Undo takes its act back with every act after
  it, and once the toast has gone Ctrl+Z walks back through the session's acts one at a time; a delete for good and
  an act that changed nothing show their toast without Undo (#131)
- The sample's inbox opens as group cards; the simple list is a choice on the Triage page (#134)
- Done in the window files the message as done (`mail.done`) instead of archiving it, so "Show done" and the way
  back know it (#135)
- Send in the composer no longer says "Sending your message": the countdown toast shows instead (#136)
- The dev Flatpak may talk to the panel's StatusNotifierWatcher for the tray icon; its menu entry says there is one
  main window and has a Quit action (#137)
- `inboxcraft-cli` without `--store` no longer stops with a usage error: it looks for the running app (#142)
- `testkit` may not depend on `dav` either (`TESTKIT_NEVER`), so the fake CalDAV server never reads with the parser
  it tests (#144)
- The engine drops calendar reports until the calendar commands take them (#145)
- The engine applies the calendar reports it dropped before: calendars and events keep their ids by their URLs, the
  sync token is kept, a written event takes its new entity tag, and a conflict (412) syncs the calendar afresh (#146)
- `Session::next_due` and `Session::tick` also cover a held delete (#146)
- `ical`: an organiser or attendee keeps the parameters it does not read (`Attendee::params`), so they go back to the
  server as they came (#146)
- The design file: translucent cards, one even selection, white on custom colours, segmented and
  select controls, solid floats, event tints, a custom colour example, and the Planned and Later
  pages (#203)
- The look follows the newest design file: translucent cards, one even selection with a gradient
  edge on selected pills and segments, the rail's own stops, and white on more custom colours
  (#205)
- Everything that pops up over content is on one solid float with an edge and a shadow (#205)
- The calendar follows the design file's controls: the view switch, Show as and the calendar of a
  new event are segmented controls, Repeat a select with a popup, the calendars and the event
  editor's boxes filled check boxes, and the event blocks paint with the event tokens with an
  edge at full strength (#206)
- The reader follows the design: a thread's header is the avatar, the name, "to ..." and the date, with Details
  fading in when the pointer comes near and in More; a message alone in its thread names its sender, address and
  date under the subject (#207)
- The invitation card follows the design: a gradient edge while an answer is wanted, the change in the error colour,
  and the card alone when the text only says what it says (#207)
- The screener's review follows the design: the sender in place of the toolbar and subject, a red Block, and the
  mode pills under the list of new senders (#207)
- The list follows the design file: the view switch over the wide unified inbox, the unread count
  in a grouped inbox's subtitle, the account stripe and the row's spacing as drawn, cards that
  run the pane's full width while nothing overflows (#208)
- The toast's Undo and the time picker's Cancel are accent words without a field; the toast
  stands centred on what is right of the rail (#208)
- The time picker's spacing follows the design file (#208)
- The Qubernetic design file has six account colours, and the accent cyan is no account's colour (#224)
- The Qubernetic design file is now the design file, `docs/design/inboxcraft.figcraft`, and `docs/design.md`
  describes it; the app does not follow it yet (#226)
- One fixed palette in two modes, dark and light, with the values of the design file; a test checks the
  contrast of 142 pairs over the two modes (text 4.5:1, edges and marks with a meaning 3:1) (#231)
- The mode's choice on the appearance settings page reads Auto, Dark, Light; Auto follows the computer, as
  System did (#231)
- An appearance kept by an older version that names an accent, a typeface or an icon set still loads; those
  fields are dropped (#231)
- The app's type is IBM Plex Mono for the app's own words and IBM Plex Sans for what people wrote, Regular and
  SemiBold, in twelve styles with their line heights and tracking (labels in capitals), and Audiowide for the
  product names; the fonts are pinned in `.config/assets.toml`. Labels, buttons, dates, counts and the lines the
  app prints are Plex Mono; senders, subjects, message text and event titles Plex Sans. A row's second line
  (subject and preview) is the caption style, read or unread (#232)
- The fixed heights come from the size tokens at the text scale: mail row 56, button 32, field and select 36, menu
  row 32, group head 32, key chip and tag 20; marks in a row (flag, pin) are 14 px (#232)
- Phosphor Regular is the one icon family and weight: a state never swaps in a filled glyph; icons are 16 px in
  controls (#232)
- The app icon, the window icon and the tray icon are the InboxCraft mark from the brand's files; the Flatpak
  installs its icon theme tree, every size from 16 to 512 px and the SVG (#232)
- The tray's flap is cyan while mail is unread (#232)
- Buttons, links, key chips, tags, section labels and tooltips follow the design in every state; disabled is a
  dashed edge and muted text, nothing fades (#233)
- Keys are written out as words in key chips and tooltips: `Ctrl F`, `Del`, `Esc`, the arrows as signs (#233)
- Check boxes, radio buttons, switches, segmented controls, pills, selects and the text size slider follow the
  design in every state: square, no round shapes, a pressed look by pointer and by Enter or Space, nothing faded
  when disabled; a switch can show a change on its way and a switch the app cannot offer (#233)
- A segmented control is one stop for Tab and the arrow keys move its choice; the text size slider names its five
  sizes and sets the one it is let go on; a select opens its choices under the field, by Enter or Space when the
  key comes up (#233)
- Avatars, badges, icons, the scroll bar and the account bar follow the design; the scroll bar stands 12 px wide
  beside what scrolls, never over a row, its thumb 4 px at rest and 8 px under the pointer (#233)
- The account picker is a select; the error banner and the reader's notice are dismissed by a link, quoted text
  folds and unfolds by a link, the box to ask Mozilla's list is a check box whose words wrap instead of being cut, and
  the dividers in the reader, its menu and the calendar's details are 1 px hairlines (#233)
- The error banner stands on the `panel` with `ink` words, 8 above and below and 12 at the sides, in a `minus` edge
  whose foot is the one line between it and the panes, as the design's error banner does; in the app's own frame it
  stands under the title strip and beside the rail, whose lines are its edges there (#233)
- Text centred in a box stands where the design puts its text box, on a half pixel when the room left is odd: a
  badge's digit, the cross of a check box, an avatar's initials and `?`; the slider's sizes are centred on their
  ticks, a segmented option's words start where the design's text box does, and a cut line (a control's or a
  row's) ends in its last letter and the ellipsis with no space between (#233)
- A link's underline lies and weighs as the design draws it, at both sizes (#233)
- A dashed box with round corners carries its dashes round the corners as the design does, its sides shaded where a
  dash begins or ends inside a pixel (#233)
- Text fields, multiline fields and password fields follow the design in every state: the label over a 36 high box,
  the edge `cyan_edge` with the focus and `minus` with an error, the error under the box; a multiline field grows
  from 96 to 240 high and then scrolls; a stored password shows no dots and says that it is kept in the system's
  credential store (#234)
- Menus, choice rows, the recipient suggestions, the typed time and the time picker follow the design in every state;
  every menu and picker (the reader's More and Move, the composer's From and Templates, a row's group menu, the
  selects, the time pickers) hangs 4 px under what opened it, keeps 3 px from the window's edge and scrolls when the
  window is smaller and keeps Tab inside itself; Esc closes it and gives the keyboard back to what opened it, a menu
  opened from the keyboard gives it back however it closes, and one opened by the pointer leaves it with nothing, so
  the window's keys act; one opened from another closes first (#234)
- A recipient line of the composer is the design's: its name in capitals, chips on `grid` with their cross, a chip
  that is not an address marked with a warning and named once, under the line, "Cc Bcc" at the right end; past four
  rows of chips the line scrolls, and Tab brings every chip's cross into view (#234)
- The add-account form puts the user name beside the password (#234)
- A row's hover actions are an icon and a key chip each, 24 high, on the row's first line, shown at once for a
  hovered row and for the row the keyboard is on, and in the `Tab` order after it; an action that cannot run shows
  its icon `dim` and its key dashed; a press on the row's middle opens the row at every width of the list (#235)
- The error banner is the design's banner: its ERROR label in `minus`, the words beside it and the `dismiss` link
  under them, in its place under the title strip or across the window as before (#235)
- Every focus ring stands on whole pixels at every scale: its line one point rounded to whole pixels (1 px at 125 %, 2 px at
  150 %), 2 points clear of its control; nothing moves at 100 % (#235)
- Text laid out through the components' paint helpers stands on the whole pixel nearest the design's baseline, as
  measured at 100 %: a caption line (Plex Sans 12/18), whose baseline falls half way between two pixels, goes one
  pixel up; each row of wrapped or centred text starts where the design's text box puts it (#235)
- A message row is the design's mail row: no avatar; the sender (`strong` when unread), the account's bar after it
  in every view of several accounts, a tag (New sender, Returned, Not delivered), the marks and the time at the right
  (`cyan_text` when unread); a name cut short keeps its whole room, the bar and the tag 8 after it; the subject and
  the preview on one `caption` line, `ink` when unread; the unread square; the open row's `stage` fill and bar; the
  focus edge inside the row; while hovered or keyed, Done, Snooze, Read and Delete in the design's order (#236)
- Done, Delete, Archive, Move, Park and Snooze act in the frame the key or the press is handled: the count, the
  status and the toast change at once. The row then closes over 180 ms on the design's curve
  (`cubic-bezier(.2, .7, .2, 1)`), its opacity fading over the same 180 ms on CSS's `ease`
  (`cubic-bezier(.25, .1, .25, 1)`), as the design's presentation has it; a snoozed row fades over 125 ms in its
  place, linearly, and is then gone; with reduced motion it is gone at once (#236)
- A done row under "Show done" shows a tick and its sender in `muted` instead of being drawn faint (#236)
- A group's head is the design's group head: `[ A ]`, the name in capitals, the unread count or "nothing to read"
  at the right, and "done all" while the pointer or the keyboard is on it, in place of the card's Done all button; on
  the "Other" head it marks the rows listed under it (#236)
- A send of the outbox is the design's outbox row: its mark, whom it goes to, where it stands in the mark's colour
  ("sending in 4 s", the time it is scheduled for, "not sent"), its secondary buttons (Send again is no longer
  primary), the hold left as a bar along its foot; a refused send's row is 72 high with the server's reply (#236)
- The rows of Later are the design's later row: who, the pile's mark, the subject and when it comes back; a
  snoozed row offers "bring back now" while the pointer or the keyboard is on it (#236)
- Every mail list (the three views of the inbox, a folder, Later, search, New senders, the outbox) is laid out as lines of fixed height and builds only the lines in sight and two above and below: 10 000
  rows build about 20 lines a frame, and the grouped inbox's frame (p95) takes 0.35 ms where it took 0.51 (#236)
- The lists are lines across their pane, no longer cards: the list is 440 wide beside the reader at every text size
  and has no splitter (a width it was once dragged to is not read); the rows, the group heads and the screener card
  run its width, every row keeping the line under it; a line in the panel's colour divides the list's head from its
  rows. When the rows do not all fit, they keep their 440 and the list's scroll bar lies over their right edge, as the
  design draws it, and only its thumb takes the pointer (a press beside the thumb reaches the row under it); where its
  thumb passes the focus edge, the edge is drawn over the thumb; a held thumb keeps the point it was taken by under
  the pointer, past either end and back. Every other scroll bar
  keeps its strip of 12 beside what it scrolls (#236)
- Smart shows each group's first three rows and says on the group's head how many of how many show, with "show all"
  and "show fewer" there, as the design draws them, in place of the "Show all (n)" line under the rows; the rows no
  group holds stand under an "Other" head of their own in Smart as in Grouped (#236)
- The screener card on top of the inbox is the design's line: the people mark, how many new senders wait and that
  they are held back, the first three's avatars and Review; it stands in every view, the List view's too (#236)
- An empty list is the design's bare empty state, centred: its place and why in capitals (`INBOX · EMPTY`,
  `LATER · PINS`, `INBOX · SEARCH`), what the place is for and how mail comes there, the keys it names read from the
  keymap; a search that finds nothing says where it searched and that the text of messages is not searched yet (#236)
- An empty pile's pill in Later loses its number: "Pins", never "Pins (0)" (#236)
- The view switch of the inbox is the design's segmented control (Grouped, Smart, List) on the line of the pills, and
  stays there beside an open message; the search field keeps its place in the head until the head is the design's
  (#236)
- When a group opens or folds above the rows in sight, or mail arrives above them, the first message in sight stays
  where it was; at the top of the list the new mail shows at the top (#236)
- The row the keyboard moves to is scrolled into sight, built or not, when the keyed message changes, never because
  rows came or went above it; a row that gains the focus comes into sight, so `Tab` and `Shift Tab` walk the whole
  list (#236)
- A screen reader is told each row's place in the whole list (a send's in the outbox too) and the list's size,
  though only the rows in sight are in its tree (#236)
- The inbox views are Grouped (every group open), Smart (the first three rows of each group) and List (one list by
  time), kept as `grouped`, `smart` and `list` under the triage file's `inbox_view`; a file kept before reads its
  `cards`, `list` and `simple` as Smart, Grouped and List, so the choice outlives the rename, and the old key is
  still written for an earlier InboxCraft; `ui_shot`'s `view` takes the new ids and the old `cards` and `simple` (#236)
- The grouped inbox has no Invitations group: an invitation stands with its sender's mail, People when a person
  sent it, Updates when a system did (an address no one answers from, by the engine's rule); the group menu and New
  senders' "Into" no longer offer Invitations. The engine's grouping is unchanged (#236)
- The group of the mail systems send is named Notifications, as the design names it (its engine id stays
  `updates`) (#236)
- The list has the keyboard when one of its rows has the focus, or when a row was pressed or keyed last and nothing
  else has the focus; a press in the reader or `Enter` gives the keyboard to the reader, and a text field, the
  palette, the settings and the composer in the reader's place keep their keys. The list's keys act only then, all
  alike: after a press in the reader (its More menu, say) the arrows no longer step through the list until a row is
  pressed or keyed, or `Esc` gives the keyboard back. Focus and selection move together: a key moves the focus with the keyed row (its ring shows and a screen
  reader follows), `Tab` onto a row keys it, and a row with the focus no longer quiets the single-letter keys. A row
  that takes the focus (`Tab`, `Shift Tab`, a screen reader) shows its message in the reader without marking it read;
  `Enter`, a press, the list's moving keys and `R` read it (#236)
- The list's keys move through the rows as the list shows them: in Grouped and Smart group by group, and past the rows
  a folded group hides (#236)
- New senders is the design's queue: "Who gets in?", what the screener does, its mode as a segmented control, and a
  card for each sender who waits (their avatar, name, account and address, their newest message, how many more and
  its first line) with Let in and Keep out; the first card is the current one (its edge in the accent, Let in
  primary, the line that says why it waits). The subject opens the message as a preview in the reader, which still
  offers Accept into a group, Accept and open, and Block (#236)
- A row's words (its spoken name, line two, its time) and the rows no group holds are worked out once for a list of
  rows and kept until the rows are listed again, the place, the search or the clock's offset change or the minute
  turns (at most 512 rows' words); New senders' queue is worked out once for a list of rows, and a frame at rest builds
  the list's order anew only when it changed: the grouped inbox's frame (p95) takes 0.25 ms where it took 0.37 before this (#236)
- The planned parts of the lists are drawn, coming soon: "Select" on the line of the pills where it fits (several
  messages at once, the rows' check boxes and the bulk bar; beside the reader the line keeps to one line and leaves it
  out whole), "in message text" in a search's head under the list's head, which says
  what was searched and where, and Drafts' rows as the design's draft rows (#236)
- A window narrower than 980 holds one pane beside the folders, as the design's breakpoint has it: an open message
  stands in the list's place (`Enter` or a press opens it; `Esc`, or "Back to the list", the design's first tool of
  the reader in one pane, a held message's head included (`ui.reader.back`, offered only where it goes back), goes
  back to the list), so the reader never has no room. With nothing open the list is in front, and it stays in front
  while a person walks it: `Tab`, `Shift Tab`, `J`, `K`, the arrows and the page keys move the keyed row and read
  nothing they do not show; an act on the keyed row (`D`, `Delete`, a hover action, a notification's Done, a change
  from the server) keys the next row and reads nothing either; another account, place or search picked while a
  message stands in front brings the new list back with no row keyed; `Enter` or a press put the message in front and the focus on
  "Back to the list". The composer in the reader's place swaps the same way: `Esc` brings the list back, with all
  the list's keys, and "New message" the composer, with all it holds; behind the list the composer sends nothing, by
  no key and not from the palette, and a message opened from the list over it sends it to its floating panel. The
  keyboard is never lost: back in the list it lands on the row left when that row is in sight, else on the nearest
  row in sight; crossing 980 into one pane it goes to the message in front or stays on its row, and out of one pane,
  from "Back to the list", to the open message's row; a row reached by `Tab` never jumps in front. The window is never narrower than 800,
  where it was 720 (#236)
- A card of New senders answered with Let in or Keep out eases its edge to the answer's colour (`plus`, `minus`) over
  125 ms and keeps it to 160 ms, fades over 125 ms at an even pace, and its place closes over 125 ms on the design's
  curve as the next card steps up and becomes the current one (the design's M11 at the level the app plays, timed as
  the design's presentation plays it); a second answer meanwhile leaves the first card leaving, the last answer's card
  leaves before the queue says nobody waits, a leaving card's subject opens nothing, and a card left mid-way by going
  elsewhere does not play again; with motion off the next card is there at once (#236)
- The reader's head as the design draws it: the message's tags (group, state, account) and its acts (Done, Snooze,
  Pin, Move, Delete, More) on one line, the subject in the title style on two lines at most, the second cut with
  one ellipsis (a screen reader hears it whole), then the facts (FROM, TO, DATE, `details ▾`) of a message alone or
  who wrote and how many messages for a thread, the why line and the content bar (#237)
- Reply, the reader's one primary button, with its arrow, Reply all and Forward stand at the reader's foot under a
  hairline instead of under the message (#237)
- A thread's messages are lines of the thread row: folded, or open with the address and to whom and
  `details ▾`; a click folds or unfolds one; plain text is set in the reading style and its quotes fold as quote
  folds (#237)
- Attachments are boxes as wide as what they hold, the name in mono at most 240 wide, with `save` (and Enter on
  the box); `open` is drawn and coming soon (#237)
- Archive moves from the reader's toolbar into More, its first row with its key; More is 288 wide and Close has its
  icon; Move to is 256 wide, its title says the account and how many folders it offers, each folder with the icon
  of what it is for (#237)
- The owner's own act that takes the open message out of the list (Done, Snooze, Archive, Move, Delete; from the
  list or the reader, by key or by button) walks on to the message the next-message rule names, as the lists do: in
  two panes it opens and is read, and the keyboard stays where it was, on the list's row walked on to when the act came
  from the list, in the reader when it came from there (on the reader's first tool, Done, where a key acted with
  nothing focused); in one pane an act from the list keys the next row behind the list, and one from the reader brings
  the list back in front with the keyboard on that row, its message not read. A Read or a Flag never counts: only the
  act that takes the row out, once (#237)
- When the server takes the open message away (or another device, a sync, a rule), nothing opens by itself and
  nothing is read: the reader shows nothing open, the bare empty state in the middle of its pane, the list selects no
  row and the keys that act on a row act on nothing; the keyboard goes to the row that took the removed one's place
  (the last row if it was the last), and from there the down arrow opens that row, the up arrow the row before the
  gap, Enter that row (where no row stands on that side the arrow opens nothing). The arrows, J and K are the lists'
  keys: they never step from the reader (#237)
- In one pane "Back to the list" is the head's first act, 8 before Done (or More for a held message) (#237)
- The head's tags take its room by need: the state (until when it is snoozed, a reminder, came back) first, then the
  group, then the account; they stand in their order (#237)
- More and Move to hang with their right edge on their button's right edge, as the design draws them, still 3 in from
  the window's edges (#237)
- Tab goes from the reader's head through the message to the answers at its foot (#237)
- In a reader narrower than its six acts (a narrow window at 130 % text) the acts stand on lines of their own against
  the right edge, so Delete and More stay in reach (#237)
- An attachment saves only through `save` (or Enter on the box); a press elsewhere on it saves nothing. In a narrow
  pane its name gives way first, then what its state says, then its links, then its size; nothing paints past the box
  (#237)
- The coming-soon tags leave the words their places: the attachments' tag stands once after the last box only where
  it fits beside it, the content bar's beside `open in browser ↗` where the words stay whole (#237)
- The hairline table is bounded before it is laid out (16 columns, 200 rows, column widths from 1 to 480, every head
  and cell cleaned and capped), says under it what it left out, lays out only the rows in sight beyond their first
  cell, paints nothing past its box and scrolls sideways inside itself when wider than its room (#237)
- In a narrow reader a thread's row gives way as the design's widths rule says, text cut before a control is left
  out: the first words first, then the sender below its 152 until only its ellipsis would stand, and `details ▾`
  last of all; the words never stand beside a cut sender or on an open line without `details ▾`, the date is never
  cut while the row holds the avatar and the date, and nothing paints past the row (#237)
- In a reader narrower than Reply, Reply all and Forward, the answers stand on lines of their own, in order, and
  the foot grows to hold them, so none is cut at the pane's edge (#237)
- The scan line of a message wiping in stays whole until nine tenths of the wipe's time and fades over the last tenth
  (#237)
- The reader's notice is a banner between the head and the message, after the head in Tab order: a note of what
  was done, or an error when something failed, with `dismiss`; it stays in sight while the message scrolls, and a
  warning shows whole while a message wipes in (#237)
- A banner's links that do not fit on one line stand whole on lines of their own, in order, and the banner grows to
  hold them; an address wider than the banner is cut only where it stands alone (#237)
- The reader's banners are the design's: remote pictures (PICTURES, `load once`, `always for` the sender), not
  delivered (NOT DELIVERED and the report's reason); a message whose text is not here yet, too large to keep or
  unreadable says so in one note in the text's place, a cut text ends with a note, and a text being fetched shows the
  progress line (#237)
- An attachment's box says how its save went: `saved to` where it went (its `open folder` coming soon), or `not
  saved:` why, with `try again`; the reader's notice no longer says it (#237)
- An invitation in the reader is the design's invitation card: the day, the title, WHEN, WHERE and WHO (CAL stands
  empty), and by state Going, Maybe and No, what changed, the answer given, the cancellation, the guests' answers
  (the first 20 and how many more) or that it asks for no answer; a version a later one replaced is one line. A card
  that asks for an answer has the `cyan_edge`, a cancelled one `minus`, any other `line`. The newest unfolded card
  of the thread that asks has the pane's one primary button, Going; an older card's Going and Reply at the foot are
  secondary while it asks (#237)
- A picture of a message that is not shown is a box in its place, as large as the size its tag gives: a remote
  picture not loaded in a dashed edge with its text and size, one loading with the progress line, one of a kind the
  app does not draw, one here whose data could not be drawn (which says so), and one that failed with why and
  `try again`, which asks for that picture once more; everything it says stands inside the box, and `try again` is
  left out of a box too narrow or too short for it, which is then no Tab stop (#237)
- A message the screener holds opens with its tags (New sender and the account) and More, then the "Let … in?" card
  in the `cyan_edge` between them and the subject: the sender's address, first mail and messages waiting, the groups
  and where their mail goes, Accept (Accept into the group chosen), Accept and reply, which lets the sender in and
  opens the composer over a reply, and Block; the subject, facts, why line and content bar follow as for any message
  (#237)
- Where the link under the pointer goes stands on a bar over the answers: `LINK`, the address and `a click copies
  it`; a link whose text names another place stands in a CAREFUL box with what the text says and the address whole on
  a line of its own (#237)
- In a reader narrower than their buttons, an invitation card and the let-in card stand their buttons on lines of
  their own, in order, measured once a frame; a card's buttons wider than its info column span the card under the
  day; a let-in button wider than the card's inner room has its label cut with one `…` inside it, its name heard
  whole, so no let-in button runs past its card and no card's button past the pane at 1024 and 1100 px, 100 % and
  130 % (#237)
- An HTML message is set in the app's type and colours: paragraphs and list items in the reading style, a heading
  its writer wrote in Plex Sans SemiBold (22/29, a deeper one 14/22), preformatted text in small mono on the
  terminal's ground, links in the accent and underlined; a link is a Tab stop with its focus ring, and Tab and Shift
  Tab reach every link however far apart, bringing it into sight; Enter on it copies its address as a click does
  (nothing opens); a paragraph is no Tab stop any more (#237)
- A quote in an HTML message that follows the message's own text folds behind a quote fold that says how many lines
  it holds; open, its words are muted and each level has its rule at its left, 12 further in, to depth 4 (#237)
- A table in an HTML message is a hairline table as its sheet draws one: a rule between two rows and none between
  its columns, rows 32 high and a row of head cells 28 high on the panel's ground, the words 12 in and the columns 12
  apart, each as wide as its words ask, on whole pixels and at least 36 wide, scrolling sideways inside itself where
  they do not fit; it draws at most 64 columns and 1 000 rows and says under it what it left out (#237)
- An HTML message is laid out once for its width and kept while it is shown, laid out again only when the font atlas
  is built afresh; the layouts of messages no longer shown are let go, the letters kept together stay under a hard
  cap, and only what is in sight is painted (#237)
- The remote-picture banner leaves the plain view the owner chooses, as the design's plain-text frames do; over a
  message shown plain because it does not fit beside the kept layouts it stays, as the owner chose nothing there (#237)
- The lists' menu key is named for all it opens, on the Shortcuts page and in the palette: the menu of the list's
  row, the reader's line or its subject (#237)
- The composer is drawn as the design's frames draw it: a head with the title, when the draft was last kept and
  Close; the lines From, To (Cc and Bcc once shown) and Subject without boxes, their names in a column of 64 and a
  hairline under each; From shows the sender, the address, the account's bar and label; the text on `term` in Plex
  Sans 16/26 with the fields' blinking caret; Send at the left of the foot with its key `Ctrl Enter`, the caret of
  send later beside it and why Send waits after them (in `muted` while it waits for a recipient or a blank, in
  `minus` with a warning when something is wrong, such as a recipient that is not an address); Templates and Discard
  at the right, attaching a file coming soon before them; a blank to fill in asked for in a panel over the foot;
  Tab comes into the composer at From and leaves it from its head, after Discard (#238)
- The floating panel is the whole composer, 520 × 560 at the bottom right, 16 in from the window's corner as its
  frames put it (15 clear of the middle area), with "Back to the reader's place"; its title, or the action "Fold or open the floating draft" (`ui.compose.fold`, from the palette or a key the
  owner gives it), folds it to its head and opens it again; Ctrl Enter sends it while the
  keys are in it (#238)
- The From menu is 360 wide, a row per account with its tick, bar, name and address, and a note when there is only
  one; the Templates menu is 256 wide over its button with its count in the title; the suggestions are 360 wide
  under the line (#238)
- The composer wipes into the reader's place from the top behind a 1 px accent line over 300 ms, eased as the
  design eases (`cubic-bezier(.2, .7, .2, 1)`), and is there at once with reduced motion; it works whole while it
  wipes, and a key, typing, a press or the wheel ends the wipe at once (#238)
- The calendar takes the design's look: the view at the left with the side panel 252 wide at the right (hidden below
  a 600 point view), events as cells on a whole-pixel grid of hairlines whose edges take the day and hour lines'
  pixels, bands over whole days sharing one line, today, the now line and the picked event in cyan; the month's chips
  and "+N more" in a float of the day's events; the agenda's day heads and rows; the mini month, the calendars by
  account and the picked event's details with tags and the guests' answers, every text whole, the panel scrolling;
  the event editor as a float hanging from New event, which shows pressed while it is open. An event is coloured by
  its account's bar alone: a calendar's own server colour and its dot in the side panel are gone (#238)
- The calendar's agenda rows and hours run the view's whole width under the scroll bar, as the design has them; there
  the bar takes a press only on its thumb, to drag it, and a press elsewhere in its strip picks the row or event
  under it (#238)
- What is typed in the event editor goes only by Cancel, or by an Esc after the warning an Esc raised with nothing
  changed since: a press outside, New event or Edit of another event warn and keep it, and the first Esc after their
  warning warns again; a refused Save shows its reason in the warning's place; Esc meant for the palette or the
  settings sheet over the editor closes only them (#238)
- A press outside the event editor reaches nothing under it, the floating composer included: it only warns, or
  closes an editor with nothing typed; the wheel and the pointer's hover reach nothing under it either, so the hours
  and the side panel do not scroll while it is open. The window's own controls and the toasts stay within reach: a
  toast's Undo, or Ctrl Z outside a text field, can remove the event being edited, and then the typing
  is kept and Save shows why it is refused (#238)
- Esc closes only the surface on top: the settings sheet over the calendar no longer also lets the picked event go
  (#238)
- A keymap key may serve one action on each screen: the calendar's arrows, Enter and Esc and the mail list's keys
  share keys; within a screen, and against the window's own keys (the composer's among them), one key still runs one
  action (#238)
- The palette lists the window's actions and those of the screen it opens over (#238)
- The settings sheet takes the design's layout: a column of sections, 232 wide, with "Settings" and how many there
  are, beside the open section's page in a column 720 wide; a page is where it is (`[ 04 ] SETTINGS · APPEARANCE`),
  its title and a sentence, then its groups, each a section label over settings rows; the preview is gone (#238)
- Appearance holds Look (mode, motion, text size) and Layout (the inbox view, where a message opens and is
  written, the window frame); the inbox view moves there from Triage and its views are named Grouped, Smart and
  List (#238)
- The settings sheet keeps the keyboard: `Tab` goes round its controls only, and when it closes the keyboard goes
  back to the control it opened from, or, when that was in a list or picker the sheet closed, to the control the
  list or picker opened from (#238)
- The appearance keeps the app's own frame last on, the top row or the strip (`own_frame`), so Own comes back as it
  was after the desktop's title bar; the answers of the `ui.*.set` commands and the kept appearance file carry it
  (#238)
- The window with no account is the design's welcome box, 520 wide in the middle, with Add account → and where it
  leads (#238)
- The add-account page in the design's fields and selects (Security: TLS or STARTTLS); one press names every missing
  or wrong field 6 under its field or row, and the first takes the keyboard; what Detect asked stands in a column
  beside the form (#238)
- The account check prints its steps as terminal lines, one by one as they run, the failing step's sentence whole
  over them; a screen reader hears each step's server, port and certificate issuer (#238)
- Detect's answer is taken only by the add form, for the address that asked, while the page is open. It fills a
  server row whose host was left empty or filled by Detect: a port typed there stays with its encryption; otherwise
  the encryption goes with Detect's port, and a line under the row says so when it replaced one the owner picked,
  until the owner picks that row's encryption. A Detect that finds nothing takes out what an earlier one filled, and
  its note says which server the form holds (#238)
- The Accounts page with no account is one box 520 wide in the middle of the page, "No account yet", a line on what
  an account needs and Add account with its arrow in it; the head has no button then and no other group follows
  (#238)
- Beside the list the docked composer keeps the reader's place while the owner walks the list: a row that Tab or
  Shift Tab reaches (in one pane with the list over the composer, the list's keys too), the next row an act walks on
  to, with the keyboard left on that row, and a row an Undo selects again are previews behind it, not read and not
  drawn. In two panes the list's keys rest beside the docked composer. A press on a row, Enter, the list's Open or
  opening the held-back message (`ui.reader.open_held`) floats the composer and shows the message read, in one pane
  or two; in one pane the reader takes the keyboard, as after a press. Opening the held-back message rests while the
  list is hidden, from the palette too, as the list's other commands do (#238)
- `outbox.list` gives each item's `cc` and `bcc` beside `to`, and for a refused one what `changed` since it was
  sent; an item an older build kept counts as changed (`unknown`) (#245)
- The outbox row lists To, Cc and Bcc on its one line, each recipient with its address ("To Odile
  <odile@example.org> · Bcc Béla <bela@example.org>", at most 20 of each listed and the rest counted), and a refused
  send that changed says so before the server's reply ("Changed since it was refused: Bcc, text; Edit to send
  it") (#245)
- The outbox row's buttons answer the owner's own press only: a held Enter or Space repeating does not press them
  again (#245)
- The dev container builds through sccache 0.19.0 (pinned, checked against its sha256), with one cache of 40 GiB
  in `target/sccache` that every target dir shares, so a second worktree takes the compiled dependencies from it
  instead of building them again; native mode and the GitHub workflow build without it (#246)
- The window is laid out in four regions: a command line across the top (it holds the window strip in the app's own
  frame, and New message at its left), the index, the body and a status line across the foot, which holds nothing
  yet. The rail and the folder pane are gone: Later and Settings are places in the index, Sent is one of its
  mailboxes, and the rail's Search button is gone (its key still focuses the search). The full-width reader fills
  the body; the index stays (#254)
- The folder pane's width is no longer dragged: the index is 232 wide, and a width the owner gave the folder pane is
  not kept (#254)
- The error banner, the floating composer and the toasts stand in the body. Beside a composer, docked or floating, a
  toast stands beside it where it fits whole, else just above the composer's foot, its words cut with one "…" to the
  room it has: it never stands over the composer's foot, which holds Send and Discard. A short calendar toolbar shows
  its title short (2 – 8 Nov) and leaves out the week's number; where even that does not fit, the side panel gives
  way (#254)
- `account.list`'s `failure` ends when the account signs in again after being offline or its worker tells that it
  is current, no longer at every turn of a signed-in worker; a body fetch that failed no longer sets it (#255)
- `schedule.presets` answers five presets instead of three; with the default week nothing else answers
  differently (#257)
- A kept `appearance.json` from before the levels loads: `system` is the default level, not chosen; `reduce` is off;
  `full` is full; a value that cannot be read is the default, and the mode, text size and frame are kept (#258)
- Where a mail row holds three of its four hover actions (beside the reader at 130 %, a keyed row beside its time),
  Read gives way and Delete stays: Done, Snooze, Delete; Read stays reachable by its key (R) and the command
  palette. Where all four fit, nothing changes (#259)
- Detect tries more ways, in this order: autoconfig at the domain (and the ISPDB when it is on), SRV records,
  `mail.`, `imap.` and `smtp.` of the domain itself, and last, as a guess about a third party, the provider domains
  of the domain's MX hosts (their autoconfig over HTTPS, then `mail.`, `imap.` and `smtp.` of them). A host whose
  name was not found is not tried again on another port; one of the domain's own common host names whose probe
  timed out is not guessed again on another port, though a server the domain names on it still is; any other host
  that timed out is still tried on its other ports. A host made up from a domain is
  tried only on the registered ports: IMAP 993 with TLS, then 143 with STARTTLS; submission 465 with TLS, then 587
  with STARTTLS; never without encryption and never port 25 (#260)
- Detect probes its candidates: each is connected to, encrypted (TLS, or STARTTLS first), and must offer a sign-in
  the app can do; no user name or password is sent. It proposes the first IMAP and the first submission server that
  answered, and stops once it has both, after 24 probes or at 120 seconds; a probe ends within 12 seconds. A newer
  Detect stops an older one that is still running. What it probed stands in the list of what it asked, and each
  probe that failed with its cause. A server the domain names (its autoconfig, the ISPDB or SRV) that no probe
  reached is still proposed, and the add form says it was not checked and why: it did not answer from here, it
  refused the connection, its name was not found, or its certificate does not fit this computer's clock. While the
  domain names servers of a kind, common host names and the MX provider's hosts are tried only once every server it names
  for that kind has failed for a definite reason (the connection refused, no such name, a certificate that does not
  hold, no STARTTLS, no sign-in the app can do); a named server that only timed out, dropped the connection, or whose
  certificate does not fit this computer's clock (far off, or not valid yet) keeps them away. Common host names are no
  longer proposed unchecked: when the domain names nothing and nothing answers, nothing is proposed (#260)
- The calendar counts weeks by the week rule of `week.set`: the week view runs from the week's first day, the
  month view and the mini month start their columns on it (the mini month's letters follow), and `week <n>` is the
  number the rule gives at the year's turn. With the default rule (Monday, the first four-day week) nothing looks
  different (#265)
- The reader's why line says why the open message is here, in the window's words, from the engine's `mail.why`
  reasons: at most four, most specific first, two spaces, a dot and two spaces between them; what does not fit leaves
  whole from the end and the first, standing alone, is cut with `…`. A screen reader hears every reason as one
  sentence. The reasons are asked once when a message opens (a preview too) and again only when its place changes;
  with nothing to say the line takes no room. Its `open log →` stays coming soon (#267)
- The engine's crash safety net now runs every command on things that exist: each account, folder, calendar,
  message (one in each folder role), draft, outbox item, schedule entry and event of a test world, alone, with junk
  or a meaningful value in each other parameter its module reads, and with a second thing that exists. Before, its
  junk had removed account 1 before any command named a message, so no command ran on mail that was there. It
  found no crash (#270)
- Cancel, Esc, Back or any other way out of the add form while Detect asks stops the run: it starts no further
  lookup or probe, and its late answer is thrown away. Detect and Check and add are ready again when the form is
  back. A new Detect also stops one still running (#279)
- The list of what Detect asked no longer has a line for a server it did not check: the form says it, under the
  server's row or in the note that names a server Detect left out (#279)
- A database failure of the store answers one fixed sentence for its kind, under the same code `store`, in the CLI,
  the control channel and the MCP tools: "the store is busy, try again", "the store is full: there is no room left
  on the disk", "the store is damaged", "the store refused the change", "the store could not be brought up to this
  version" or "the store could not be read or written". Before, it was "the store database failed: " followed by
  SQLite's own message. A client can still match the code `store`; it can now also match the sentence, where before
  only its prefix was fixed. A file that is no database stops the app at open as before; its line on the app's
  standard error keeps SQLite's words after the sentence (#280)
- A command an agent runs in the app (through the CLI or MCP) that meets a store failure answers only the sentence;
  the app writes SQLite's words, cut to 256 characters, to its own standard error, one line, not again while the
  words repeat (#280)
- A constraint a migration step meets says "the store could not be brought up to this version", not "the store
  refused the change" (#280)
- The calendar takes a week rule set through `week.set` at the next frame while it shows, and the window keeps the
  rule in `week.json` beside the notification choices, so it is there again after a restart (not for the sample);
  a kept file that cannot be read gives the default rule. A rule taken back at the start reads no calendar before
  the owner opens it (#284)
- Settings › Calendar's "Week starts on" row shows the first day of the week rule and says that its own rows are
  coming, where it said Monday, fixed (#284)
- A store failure of a command the owner runs in the window still shows only its fixed sentence in the banner; the
  database's own words now go to the app's standard error as one line, not again while they repeat, as they do for
  an agent's call (#284)
- Settings are a place of the window, no longer a sheet over it: the index stays beside them, the sections' column
  and the page stand in the body, and SETTINGS in the index is the place shown. The settings' key (Ctrl+,) or
  SETTINGS pressed again shows the place the owner came from; Esc closes a list or ends a key being waited for and
  otherwise leaves the settings shown; a mailbox or a place pressed in the index, or its key, shows it, and an
  account chosen in the index's picker leaves them for that mail. The keys of the mail (the list's, the acts on a
  row, the views, Back, Snooze, Send) rest while the settings show, and Undo's key takes back only what a toast
  shown names: the last act, or a send during its countdown; the palette comes over them without closing them; New message, its button or its key, leaves them
  for the mail. A press on a control or a screen reader's activate acts there whatever its key; a key without
  Ctrl or Alt runs nothing, so a letter is typed (#284)
- Leaving the settings keeps their page, the text that narrows the Shortcuts page and an add or edit account form
  as typed, without its password, which is typed again; a key being waited for stops waiting and Detect stops. A
  check under way goes on and SETTINGS in the index carries a cyan square while it runs and the Accounts page does
  not show; opening the add form no longer forces the Accounts page. While a key is waited for, a row of the index
  pressed ends the wait and then acts (#284)
- In a narrow window the settings' page keeps its width before its room: the room beside it goes down to 24 and the
  sections' column narrows; a settings row whose words would keep less than 160 beside its control puts the
  control under them; on the Shortcuts page Reset all keys goes under the find field where the row is too narrow,
  and two signatures side by side stand one under the other below 240 a column (#284)
- `compose.list` reads only the drafts it answers: the store counts an account's drafts and reads the newest of
  them through an index on the time each was saved, instead of reading every draft and sorting it; its answer, the
  order and `total` stay as they were. The store's schema goes to version 5; a store of version 4 takes each
  draft's save time from its record when it opens, and a draft whose record cannot be read sorts oldest instead
  of keeping the store from opening (#301)
- Ctrl+Plus, Ctrl+Minus and Ctrl+0 no longer zoom the whole window (the toolkit's own keyboard zoom is off): the
  size of the interface comes from the text size in the settings alone (#303)
- The command palette's window and the list head's search field are gone: both are the command line's now. The
  palette's key (`ui.palette.open`) opens the command line; a new key, `ui.command.open` (`/`), does too (#305)
- The `ui_shot` key `palette` is `command` (#305)
- The window frame is the app's own or the system's: `ui.frame.set` takes `own` or `system`, and the appearance keeps
  no `own_frame`. A kept `top_row` or `strip`, and an older client's `ui.frame.set` with either, read as `own`; the
  answer says `own` (#305)
- In the own frame the command line is the title bar: a drag where neither the field nor a window control stands
  moves the window, a double press maximises or restores it, and the window controls (minimise, maximise or restore,
  close) stand at its right end, reached by Tab, pressed by Enter or Space, named for screen readers, and usable over
  the command line's results, a menu or the event editor. The app's mark stands at the command line's left in every
  frame (#305)
- New message left the command line: it is `>` New message, its key (`C`), and Home's Write (#305)
- The command list's Send sends the draft drawn, a floating one too, on the owner's Enter or press; an agent still
  cannot send. With the floating draft folded, Send is listed disabled, saying "Unfold the draft to send it" (#305)
- A draft that cannot go yet (no recipient, a blank to fill in), sent from `>`, says why in the words its own Send
  shows (#305)
- With the event editor open, `>` lists only the settings, Shortcuts, the look and the command line itself: nothing
  that acts on the mail or the calendar under the editor (#305)
- The `ui_shot` key `frame` takes `own` (the older `strip` and `top_row` read as it) (#305)
- What was a toast is a message in the status line: an act's Undo, the send countdown with its Undo, a discard from
  Home with its Undo, the composer's notices and the command errors. One message shows at a time; notices and
  errors wait their turn (at most eight), and an error stays until dismissed. Undo's key takes back the last act
  whether or not its message shows, as before (#309)
- Undo is offered for five seconds, the discard from Home waits as long, and a notice shows for four (#309)
- The error banner over the panes is gone: a command's error is the status line's (#309)
- Under the command line's results nothing in the status line runs but the Undo of a send counting down or a
  discard from Home: its time runs out there too, so its message lies over the dimmed window and its Undo works by
  press and by its key, which then leaves the command line's text as typed (#309)
- Under a window 1280 wide the index narrows to 56 px of icons (its names come as tooltips) and the command field
  takes the room it leaves; the status line keeps its first three facts and its last three keys, the reader's why
  line leaves out its last reason whole, and the calendar's toolbar drops its note (#312)
- Under 980 the status line keeps two facts and two keys, and a picked event's details take the calendar's one pane
  in the week's place; `Esc` gives the pane back to the week (#312)
- Beside the docked composer the list keeps its 440 at every width from 980 up; the composer takes the list's place
  only in one pane, as the reader does, and no longer hides the list in a wider window (#312)
- Settings, Appearance: Look (mode, motion, text size) and Layout (inbox view, open a message, write a message,
  window frame); motion offers full, reduced and off, full no longer coming soon, and its help says what the level
  shown does. The window moves at the level the page shows: the owner's once chosen, else off while the computer
  asks for less motion (where it can be read), else reduced. Until each level has movements of its own, reduced
  and off both land every movement at once and full runs the motion of before. A window whose owner never chose a
  level now starts with reduced motion, still where it used to move; choosing Full brings the movements back (#313)
- The window frame's help says that the command line is the title bar of InboxCraft's own frame (#313)
- Drafts lists the drafts kept here (`compose.list`), newest first, as draft rows: the subject or "(no subject)",
  whom it goes to or "no recipient yet", and when it was last edited; its head says how many there are (all of them,
  also past the 1 000 listed), and with none it says "No drafts." The rows were the Drafts folder's mail, drawn
  coming soon (#314)
- Under the kept drafts (with none, beside "No drafts."), each account whose Drafts folder on the server holds mail
  has a row naming the folder and its size, "Drafts folder on the server · 3 messages", that opens that folder by a
  press or by Enter when keyed; J, K and the arrows walk onto it. A draft written in another program is seen there
  in one press (#314)
- J, K and the arrows walk the drafts, Enter or a press opens the keyed draft in the composer to go on writing, and
  "discard" (on a hovered or keyed row) hides it behind the toast with Undo; it is discarded when the toast ends.
  A hover, the keyboard passing over a row or a press under the command line's results opens or discards
  nothing (#314)
- A screen reader is told each draft's subject, whom it goes to, when it was edited, whose account it is (over
  more than one) and the start of its text. Draft text from outside is capped and made plain where the list is
  read, so a draft of 10 000 characters with line breaks, NULs or bidi controls is one row (#314)
- `schedule.list` answers, beside each entry's id, time and target, its account, its subject (an alert's event
  title) and who it names (a send's recipients, a snooze's sender, a follow-up's recipient), each one short line
  without control characters; an alert's entry adds its event, its start and how many minutes before it rings. An
  entry whose draft, message or alert cannot be read is left out and counted in `skipped`, instead of refusing the
  whole list (#317)
- A change of place (Home, Mail, Calendar, Later, Settings) is drawn in the frame of its key; before, the window
  showed the old place for one more frame. At full the old body's text goes to glyphs from the right over 125 ms
  over the new place, then the new body's text resolves from the left over 300 ms; at reduced the old body fades
  out over the new one in 125 ms; at off the new place is there at once. Another mailbox, view or range does not
  play it, and a key or a press during it ends it (#318)
- Done, Delete, Archive, Move, Park and Snooze show the list without the row in the frame of their key; before, the
  row stood one frame more. Done (and the others but Snooze): at full the row's words go to dots from the right over
  125 ms, then the row closes over 180 ms; at reduced it closes over 180 ms; at off it is gone at once, and the undo
  line keeps its seconds without the bar. Snooze: at full the row slides 24 px to the right and fades over 300 ms,
  then its place closes over 180 ms; at reduced it fades over 125 ms, then its place closes. A key during it acts at
  once and the row is gone in that frame; a click acts on what it was pressed on, and the row is gone in the next
  frame (#318)
- A screener answer (M11): the answered card's edge takes the answer's colour in the frame of the answer, where it
  eased in over 125 ms and held 160 ms before. At full the card leaves its slot 24 px up (Let in) or down (Keep out)
  and fades over 300 ms, cut by its slot, then the slot closes over 180 ms and the next card steps up; at reduced it
  fades over 125 ms in its place, then the slot closes over 180 ms; at off the next card is there at once. A key
  during it ends it in that frame, a click in the next (#318)
- A message opened, a draft sent from the reader's place and a change between dark and light show in the frame of
  their key; before, the window showed the old one for one more frame. A message opening (M7): at full the old
  content stands under the wipe's line as it comes down over 300 ms; at reduced the old content fades out over the
  message in 125 ms; at off the message is there at once. Send (M10): at full the composer folds onto the status
  line, its top falling to one line over 300 ms, then that line goes down into the status line over 125 ms; at
  reduced it fades out over the reader in 125 ms; at off the status line prints. Dark and light (M14): the old
  colours fade out over the new ones, 300 ms at full and 125 ms at reduced; at off the change is there at once. A
  key or a press during any of them ends it in that frame; a press on the reader's place while M7 or M10 plays,
  or within a double click's time of a send, reaches nothing under it, so a double click on Send opens no reply;
  the floating composer over the place, and a new message written there at once, keep their presses (#318)
- The command line (M8): at full the results of a prefix unfold one line every 30 ms, each line whole in its frame,
  past the twelfth together; at reduced and off they come together; at off the caret stands still, in the command
  line and in every text field. A key during the unfolding acts at once with every line there (#318)
- At motion off a sync in the status line (M9) is one print, `syncing…`, without the bar, the share or the turning
  ring, and an account's check (M13) prints a step's line once the step has ended, not as it starts (#318)
- The list stands 440 wide beside the reader also with nothing open; the reader says what to pick, beside the drafts
  that a draft opens in the composer, beside the outbox that nothing opens from it, beside an empty list that there
  is nothing to open (#325)
- The status line's keys follow what the window shows: next, previous, open beside a list; next, previous beside
  the outbox; next, previous, done, snooze with a message open; write beside an empty list; send beside the
  composer; Ctrl K for commands (#325)
- The command line's head says how many of all the commands show and what Enter does (#325)
- The design file draws what the owner chose on 2026-11-18: an invitation card that asks has one primary button
  (Going, with Reply secondary) and, while an answer is being changed, Keep my answer; Detect and Check and add are
  drawn pressable on the add form, waiting only while Detect asks; an account's Save is drawn disabled until
  something changes; the focus edge beside the scroll thumb keeps both places, one column apart, and the other
  reading is no longer drawn. The file now holds the two PLANNED pages Event drafts and Event editor (23 pages)
  (#327)
- The InboxCraft and Qubernetic marks, wordmarks and emblem under `assets/brand/` are licensed CC BY-ND 4.0 (shared
  unchanged, with credit to Qubernetic) instead of all rights reserved; they stay trademarks of Qubernetic and are
  not under MIT or Apache-2.0 (`NOTICE`, `ATTRIBUTION.md`) (#335)
- The command line's mark is the brand's compact mark drawn from its vector file at the window's scale, and beside
  the narrow index it is centred in the index's column (#335)
- Deleting a template, cancelling a scheduled entry and removing an account are confirmed in a dialog over the
  dimmed window, as the design draws it, instead of in place: it opens on its safe button, Tab stays inside it, Esc
  or a press on the dimmed window keeps, and only the owner's press of its danger button acts; nothing under it
  takes a key or a press while it shows. The dialog stands 12 under the row or button it asks about, or over it where
  there is no room under; in a small window its sentences scroll and its buttons stay in sight; the status line says
  what Esc does (#336)
- The composer's draft reaches the disk on the store's draft keeper thread, not the window's: a keep of a 1 MiB draft
  holds the window about 1 ms instead of about 7 ms (median; p95 about 1.5 ms instead of about 10 ms). Every keep
  still reaches the disk with `synchronous=FULL`, in order, the newest of each draft; a send, a discard or any other
  write waits for it first. A keep that fails drops nothing: its newest text stays in memory, the status line says
  it once, and it is tried again 1 s, 2 s, 4 s … at most 30 s apart; a send or other change of that draft is refused
  until it lands. The composer stays open, saying why, until its draft is on the disk; the window's close waits at
  most 3 s and is taken back when a draft is not on the disk, saying how many more are not, and a second close in a
  row (within 10 s, nothing kept since) goes through; a quit from the tray says it in a notification. `compose.update`'s summary no longer says "at once" (#338)
- The places of the index stand 40 px lower, under New message; the mailboxes' room counts it in (#339)
- Drafts in the index has a glyph of its own, a page with a dashed edge, not New message's pencil (#339)
- The composer's own Discard works as a discard from Home or the drafts place does: the draft is kept as it stands,
  the composer closes and the status line says "Draft discarded" with Undo (Ctrl+Z) for five seconds; Undo brings
  the composer back as it was, in the reader's place or floating, and the draft being written by then is closed and
  kept first. When the five seconds end, the window closes or the app ends, the draft goes here and on the server; a
  second discard carries out the first at once. A new message nothing was written in still goes at once, without an
  Undo. The `ui_shot` key `compose` takes `discarded` (#340)
- The design file draws the owner's answers on the event editor's third round: every event editor carries the
  description and the controls that move it to the full editor or its own window; an invitation's description
  stands inside its card (the invite-card set's new `described` state); Settings, Calendar names its groups
  D · Editor and E · Event drafts (#342)
- An all-day event's alerts ring on the computer's own clock, so the hour holds on the days the clock is put forward
  or back; the event editor names an all-day event's alert a whole number of hours before its day as the hour the
  day before it rings (#353)
- `event.list` names an all-day event's alert at one of the four hours by the hour (`"day_before_9"` …
  `"on_the_day_9"`) in place of its minutes or `"custom"`, and the event editor shows it in the setting's words (#353)
- The design file draws New message at the top of the index, above the places and as wide as the column, on every
  place: a secondary button with the compose glyph, its words and its key `C` (the text-button set's new
  with-icon-key states), its glyph alone in the narrow index (the new icon-only states), disabled while there is
  no account (#354)
- The design file gives Drafts, in Mail and under Calendar, a glyph of its own, a page with a dashed edge, so the
  narrow index no longer shows two pencils (#354)
- The window's Esc is the editor's: with typing it warns, and a second throws the typing away. The desktop's close
  and the window's Close never throw typing away: with nothing typed they close the editor, otherwise the window
  stays and says how to keep or throw away the typing (#370)
- While the event editor is open on the calendar in the main window (small or full), the owner's Undo rests; off the
  calendar, and in the main window while the editor is in its window of its own, it runs and is offered as before.
  An agent's `history.undo` is refused while any host holds the editor (#370)
- A close of the main window while the event editor holds typing, in any host, is taken back once: the editor says
  the event is not saved, in sight (the calendar shown, or its window brought forward), and a second close in a row
  goes through (#370)
- With the event editor in its window holding typing, Delete of that event in the main window is refused with the
  reason in the status line, and New event or Edit there bring the window forward and say why (#370)
- Settings in a narrow window: under 980 points, where Mail shows one pane, the body holds either the list of
  sections or one section's page; the page's head starts with `← Settings`, and that link or Esc goes back to the
  list. From 980 the list stays beside the page, 232 wide, and the page keeps 40 at its left (#373)
- A setting whose control needs more than half its row puts the control on a line of its own, under its name and
  help, at the left; Sending's signatures stand one under the other where each would keep less than 320 points
  beside the other (#373)
- At 130 % text the fixed text columns of the Accounts, Templates and Scheduled rows grow with the type,
  and a Scheduled entry puts whom it names (or how long before) under its subject where the row is too narrow for
  both side by side. A page taller than the window scrolls under a scroll bar at the window's right edge (#373)
- Appearance › Layout's "Open a message" row is no longer coming soon: Pane or Window runs `ui.layout.set` (#380)
- The composer draws a reply's quote as the quote block: a 2 px bar at its left, the line that says who wrote it in
  small mono, the quoted lines in 14/22 in the muted ink, 14 in. The text stays plain `> ` lines, kept and sent
  byte for byte as before: a quoted line's first `>` is still there, drawn invisible in the room left of the quoted
  text, and the caret steps through it. At most 2 000 lines are drawn as a quote, the rest as typed; each line is
  laid out on its own, so a bidi control in a quote (drawn invisible) reaches no other line (#389)
- A command chosen in the command line where it cannot run says in the status line why it did not run and what it
  needs, as well as in its row (#397)
- The command line's results end in the bar of prefixes, 30 high on the panel colour under its hairline (#397)
- An account's row in the index says whether it is syncing, offline or had its sign-in refused, read from
  `sync.status` and `account.list` when the mailbox changes or a second has passed; it was always quiet (#398)
- A folder under another is named in the index by its path's parts with " / " between them ("Clients / Harbour
  Bakery"), as the design file draws it, not by the server's path with its own delimiter (#398)
- Stepping the reader down a list of 10 000 messages with J takes about 1.1 ms a step (p95 1.2 ms with every core
  free, 1.3 ms on one core), down from a median of 59 ms and a p95 of 139 ms: `mail.thread` and `mail.why` find one
  message's thread by its links through an index of the links in both stores, at the cost of the thread (one
  record per message, however many links name it), not of the store; marking one message read in the memory store
  copies that message, not every message; and the list marks the opened message read in its row and its folder's
  and views' unread counts, where it read the list, the folders and the screener again. Their answers are the same
  as before (#406)
- The SQLite store's schema version 6 adds the `message_link` table; a store of version 5 indexes the links of the
  messages it holds as it opens (#406)
- Later's head says how much its three piles hold ("9 put aside"), and the snoozed pile's heads count the messages
  under them, as the design file draws them; they said "nothing to read" (#410)
- The status line finds a failure's kind by the engine's own names (`error_kind_name`), as Home does, so a renamed
  kind can no longer quietly go unknown there (#415)
- The pictures with the undo bar are taken at Reduced motion, so the bar shows where Off wrote "5 s"; the two kept
  drafts in the pictures were edited at times of their own; a picture on the frames' sample is synced unless its
  scene says `"sync": "offline"`; every window picture draws the app's own window frame, as the app does, unless its
  scene says `"frame": "system"` (#416)

### Deprecated
- `ui.motion.set`'s parameter `reduce` (`on` is off, `off` is full, `system` the default level, not chosen) and the
  appearance's field `motion` (`system`, `reduce`, `full`) still answer for one release; use `level` and
  `motion_level` (#258). An older build that changes the appearance rewrites the file without the new fields,
  so a chosen Reduced then comes back as Off (#258)

### Removed
- The `draft` CI job: a draft PR now starts no job at all (#68)
- The dev Flatpak's permission to talk to `org.freedesktop.Notifications`: in the Flatpak, notifications go through
  the notification portal, which needs none (#138)
- The glass and gradient design file and the pictures of its pages; `docs/design/` keeps one picture, the
  cover (#226)
- The accent presets (Electric, Aurora, Citrus) and the owner's custom colours, with their cards and fields on
  the appearance settings page (#231)
- The gradient behind the window and the glass layers over it (#231)
- The typeface and icon-set choices: the window draws with Figtree and Phosphor only, and the settings page
  has no Typeface or Icons row (#231)
- The commands `ui.accent.set`, `ui.font.set` and `ui.icons.set`; the `ui_shot` keys `font` and `icon_set`,
  refused like `preset`, and their pictures (#231)
- Figtree, Inter, Lucide and the Phosphor Fill and Duotone fonts (#232)
- The app icon drawn in code, its exported SVG `assets/app-icon/inboxcraft.svg` and `just icon` (#232)
- The keyboard action `ui.motion.toggle` ("Reduce motion on or off"): motion has three levels on Settings,
  Appearance. A key kept for it binds nothing and the rest of the keymap loads (#313)

### Fixed
- The line egui drew under the list header (and the error and reader notice bars) is gone, and the list keeps the
  scroll bar's width free so the bar no longer lies over the cards (#104)
- The app's own window frame (#104): its edges resize the window on Wayland too (egui knows no window rect there,
  so the edges never acted); its corners are round with a one-pixel outline, square when maximised; the strip is a
  band of glass with a line under it; in the top row the window controls sit at the end of the right pane's first
  row instead of over its search field or Close button, and a drag in the band above and between the panes moves
  the window. The strip's controls sit in its middle; a drag moves the window every time, also after a move whose
  button release the desktop kept to itself; the first pane is as far from the rail as the panes are from each other
  The hover of the window controls shows again right after a move or a resize, without a click; Close turns a
  plain red under the pointer, and each control's hover fill keeps clear of the strip's edges. `ui_shot` takes a
  `pointer` to picture a hover
- The scroll bar is 5 points wide at rest and 9 under the pointer, with round ends; the strip's controls have as much
  room above and below them as Close has to the window's edge: a cards' gap, so Close ends where the cards end (#104)
- `just it-live` makes `target/nextest` before it builds the live test archive; on a fresh checkout
  `nextest archive` exited with 103 because the folder was missing (#57)
- The IDLE tests of `inboxcraft-net` (`crates/net/tests/watch.rs`) no longer stop the worker or sleep before the
  IDLE they count has reached the fake server; under coverage instrumentation the resync test saw one IDLE instead
  of two. `FakeServer::wait_received` in `inboxcraft-testkit` waits, up to a timeout, for the lines a client has
  sent so far (#164)
- A draft on its way (held, waiting for its time, or sending) is no longer copied to the Drafts folder (#123)
- Every way out of the settings sheet (Close, Escape, a press beside it, the rail's places and their keys) forgets
  what the open sheet held, so it opens clean the next time (#130)
- The coverage gate builds incrementally whatever `CARGO_INCREMENTAL` says, so CI counts the lines a local run
  counts: a test binary that links a crate without calling a small function of it no longer makes the function's
  lines read as uncovered (#140)
- The toast's Ctrl+Z works while the calendar screen shows (#148)
- A message laid out with a table keeps its columns readable: each column is sized from what it
  holds, where words used to break letter by letter in a column 40 points wide (#213)
- A row with an empty preview shows no dot after its subject (#213)
- One line, not two, stands between the navigation rail and the pane beside it (#233)
- The error banner is drawn: the window's ground no longer covers it (#233)
- An account picker whose account is gone shows and focuses All accounts in its list as in its field (#233)
- Words drawn off the pixel grid (at 150 %, or in a squeezed control) are no longer cut one letter short (#233)
- A key held on a row of a select's choices no longer opens them again, a select whose choice is gone from its
  options opens on its first row, and a screen reader's click on an open select closes it (#234)
- A group's head no longer stands bare over nothing while its last row leaves and then pops away: it closes with
  the row (#236)
- A snoozed message's row tells a screen reader when it comes back ("back Sat 09:00"), as the row shows it, not the
  date it came (#236)
- A short simple list no longer shows a scroll thumb beside rows that all fit (#236)
- A row's line cut with an ellipsis no longer reaches a fraction of a point past its room: the rows' cut measures a
  line as `cut_within` measures one, by its letters' advances added up (#236)
- One `Esc` with a menu, a picker or a list of choices open closes only that, never also full width, the settings or
  the one pane's reader under it (#236)
- `try again` on a failed picture that two unfolded messages of a thread share asks for it again from either message,
  not only from the one that asked for it last (#237)
- An invitation from a sender the screener holds cannot be answered until the owner lets the sender in: its card
  says so and offers no answer, and `invite.answer` refuses it, since an answer is mail to that sender; let in from
  the reader, the card offers the answers at once (#237)
- An open quote's attribution and words reach a screen reader (#237)
- An open quote keeps its line breaks, and is not copied and laid out again every frame (#237)
- An open quote keeps its edge once its line has scrolled out of sight (#237)
- A key pressed in the frame the server removes the open message acts on no row: the row the next-message rule
  picks is never acted on or drawn selected before anyone has seen it (#237)
- A table that leaves out one column or row says so in the singular, and what a screen reader hears of a huge
  table is built only as far as it is heard (#237)
- Typing in the composer cost about 12 ms a character, the draft kept with every change; while a field of the
  composer has the keys the draft is kept at most once a second, and at once when the field is left, the composer
  closes or sends, the window closes or the app ends, so only a crash can lose the last second typed in the recipient
  lines, the subject or the text (#238, #243)
- Ctrl Enter that came in the same frame as a draft opening again could send a composer nobody had seen; Send now
  acts only on a composer painted whole, its lines not under its wipe, in a frame before the one whose input sends,
  since it opened, moved to another place (the reader's, the floating panel, another screen) or took a template, a
  filled blank or another sender (#238)
- Ctrl Enter sends the draft only while the keys are inside the composer: never from the search field, a field of
  the settings sheet over it or of the calendar's event editor (#238)
- A select's list opened a second time inside a float (the event editor) stood under it and took no press (#238)
- An event's description is read with its line breaks and up to 10 000 characters, no longer cut at 300 (#238)
- An event's title and place are read whole up to 1 000 characters, no longer cut at 300, so an Edit no longer saves
  a title cut short (#238)
- No way out of the settings sheet loses what was typed: a signature typed and not saved stays until it is saved;
  the add form, and each account's form changed and not saved, are kept as typed, all but the password, on Escape,
  Close, a press beside the sheet, another section or page, Back, the rail, the command line, the welcome and the
  other form, and come back with Add account or that account's row; asking for one form never drops another, and
  only the form's Cancel, or removing its account, drops it. The password is typed again after leaving a form. What
  an earlier Detect found and filled stays with the kept add form; a Detect still under way ends with it, its answer
  not taken (#238)
- Esc and a press beside the settings sheet close an open list of choices on it first, not the sheet under it (#238)
- A list, menu or picker open when the settings sheet or the palette opens closes, choosing nothing: it no longer
  stands over the sheet, keeps `Tab` from it or makes Esc and a press beside it do nothing. The event editor and the
  floating composer stay under the sheet as they are, with what is typed in them, and so does the month's float of a
  day's events ("+N more"): quiet under the sheet, taking no key or press, and open again when it closes (#238)
- A pane stands on whole pixels with its edges inside it, so they end at its corners and meet there at every scale,
  125 % and 175 % too (#238)
- A settings page with a row for every account lays out only the rows near sight, and `Tab` scrolls every settings
  page, the account form included, to the control it comes to, out of sight before (#238)
- The settings sheet's outline stands on whole pixels at every scale, one solid line a pixel or two thick with no
  half-tone fringe at 125 %, 150 % and 175 % (#238)
- Under the settings sheet a key runs only the sheet's own actions and the window's (the palette, the mode, motion,
  the ways to a place, which close the sheet first): Snooze, New message and Fold no longer run behind it from a key
  with Ctrl, Alt or Cmd, so no picker opens over the sheet (#238)
- The add form's outgoing server row stands level: where its caption takes two lines, the port and the security come
  down with the host's box (#238)
- On the Shortcuts page the row waiting for a key is a box the whole row wide with a 1 px `cyan_edge` round it, not
  only its cyan chip (#238)
- A failed account check says what failed and what to try in an error banner of InboxCraft's own words for the step
  (the name lookup, the connection, the encryption, the certificate, either sign-in, the calendars) in place of the
  coming-soon box; a sign-in or the calendars name a refused password or no calendars only when the check reported
  that kind of failure, and otherwise say only that the sign-in or the calendars did not work; the server's reply
  stays on the line over the steps, and the server's name in the banner is made fit for one line and cut at 253
  characters (#238)
- A skipped step of the account check names the server it would have gone to on its line, as a step that ran does
  (#238)
- Appearance's Motion row always says, after its level, that until the owner chooses the computer's reduce-motion
  setting gives Off; the row is 84 high, as the design's (#238)
- Reset all keys on the Shortcuts page is a secondary button with its edge, as the design has it (#238)
- The settings sheet's head counts every section its list shows, "11 sections", the planned AI and Plugins with them
  (#238)
- A signature's "nothing changed" stands 12 right of its Save, as the design has it, not 36 (#238)
- In a window narrower than about 830 with the composer in the reader's place, Discard stood over Send and outside the
  composer, and a press on Send could throw the draft away; now each control of its foot is drawn only where it fits
  whole beside Send, and the composer keeps room enough for Send, its caret and Discard (#238)
- The thumb of the bar over the mail list, the agenda and the hours is no longer a Tab stop without a name (#238)
- While the composer's least width squeezes the folder pane, the owner can still drag the pane narrower, and that width
  is the one it keeps when the composer closes (#238)
- The in-memory store (the sample, the browser build) copied every message it holds for each change it kept: a
  keep of the composer's draft while typing cost about 11 to 15 ms over 10 000 messages. It now copies only the
  tables a change writes to, and that keep costs under 1 ms with a text of 1 MiB; `ui_bench --compose` prints each
  scene's slowest frame and holds a typing scene's to the budget too (#243)
- `shots.jsonl` no longer draws two "Later" pins scenes that later lines overwrote (#248)
- When the server takes the open message away, the keyboard goes to the row under it as the list shows it, else the
  row above, and in the grouped and Smart inbox to a row of the same card; it took the next row of the flat order,
  which could be the next card's first row or a row a folded card hides, and the list then opened another row's
  message as a preview by itself, beside a docked composer too. Nothing opens or is read by the removal, also when
  the owner had scrolled the open row out of sight: the keyboard then goes to the nearest row in sight (#254)
- The owner's Done, Delete, Snooze or Move on a message whose row was scrolled out of sight walks on to the next row
  and brings it into sight; it opened the nearest row in sight instead (#254)
- Under the calendar's event editor no key of the window, with a modifier or without, opens or shows anything under
  it: only the palette, the settings, Shortcuts and the window's look (light, dark, system, motion) run there; New
  message opened a composer under the editor and Find left the calendar (#254)
- The window frame's strip and the top row's band, which move the window by the pointer, are no Tab stops; Tab
  landed on them, an unnamed stop that did nothing (#254)
- A screen reader's activate on the index or on New message runs nothing under the palette, the settings sheet or
  the event editor beyond what their keys run there; a mailbox pressed that way changed the mail under the sheet and
  a place left the calendar under the editor. A press on the index leaves the keyboard on the row pressed; from the
  docked composer it went to nothing (#254)
- Undo rests while the calendar's event editor is open, by its key, the palette's command and the toast's key and
  button (drawn unavailable): it took back the event being edited. The toast still shows, and its Undo works again
  for what is left of its time once the editor closes (#254)
- On a new sender's card Tab reaches Let in before Keep out, as they are drawn; Shift Tab walks back the same way
  (#259)
- An address at a domain whose mail a provider hosts, where the domain answers nothing on the mail ports, is found
  through the provider named by the domain's MX records (`mail.<provider>`) (#260)
- An id above `i64::MAX` given to a command (`mail.read`, `mail.why`, through the CLI, the control channel or
  MCP) is answered as the unknown id it is, `not_found` "there is no message …", where it said "the store
  database failed: id … is out of range". Every kind of id the store looks up answers the same way: nothing to
  read, the kind's unknown error for a change and nothing removed, whatever rows the store file holds (a damaged
  file with a row at a negative key too) (#276)
- A record whose own id is above `i64::MAX` is refused by the store with its own error, "id … is past the largest
  the store can keep", not as a database failure (#276)
- A label in capitals with letter spacing (a group's name, a table's column head, a navigation row) is cut with
  one ellipsis at its room; it was laid out up to a tenth wider than its room and cut off by the edge (#292)
- A toast with no Undo ("Draft saved") takes no press: Reply, Reply all or Forward under it is pressed, where the
  press did nothing before (#292)
- A kept settings file (the appearance, the keys, the triage and notification choices, the week rule, running in
  the background) that is a named pipe, a link to one, a folder or anything else but a regular file no longer holds
  up the start: it is not read, and its choices are the defaults, as for a damaged file. A link to a regular file is
  still read. The size caps stay: 16 KiB for the settings, 1 KiB for the background choice (#298)
- A screen reader's activate on the act's toast Undo took the act back under the command line's results; it does
  nothing there now. The send's countdown and a draft's discard keep their Undo under the results (#305)
- A message with Undo no longer covers the reader's Reply, Reply all and Forward, nor takes their presses: the
  status line is a strip of its own (#296, #309)
- Crossing 980 while the settings show no longer leaves the keyboard on nothing: a field of the add form keeps it
  (#312)
- Crossing 980 into one pane with a float of the list open (a row's time picker) gives the keyboard to the reader in
  front, where it was left on nothing (#312)
- In a short window the floating composer is never shorter than its folded head and stays inside the body (#312)
- After Esc from a row's menu opened with the pointer, D, A, R, S and Delete act on the row with the focus ring, as J,
  K and Enter do, not on the message open in the reader, and also when nothing is open; H opens that row's time
  picker. This lasts until the ring moves. An Undo moves the ring only when it selects its row again (#323)
- In one pane with the list in front, H opens the time picker on the row that has the keyboard, as the row's Snooze
  shows; before, it opened the reader's picker, which was not shown (#323)
- The list's head is one line as the design draws it: the place, what it holds ("N to read" in the grouped views)
  and the inbox's view switch at its right, a hairline, then the pills' line; where the room is short, whose mail
  (an account's address or "All accounts") is cut first and the count stays whole (#325)
- New senders: the Screener label stands centred on its switch, and a sender card's Let in and Keep out end on the
  card's padding (#325)
- The screener card's Review stands centred on the card (#325)
- The docked composer stands flush with the body (#325)
- The index no longer lists InboxCraft's own Screener and Snoozed folders as mailboxes (New senders and Later show
  their mail); the Blocked folder keeps its row; New senders shows how many senders wait (#325)
- The account picker has its SHOW MAIL OF head, each account's colour bar and address, and stands on the owner's
  row; with one account it says where another is added (#325)
- In one pane the reader's head starts with Back to the list at its left, the tags 8 after it, as the design draws
  it; the keyboard stays on the act it was on when the window crosses into one pane (#325)
- Menus, popups, tooltips and the floating composer draw their one-point edge on whole pixels at 125, 150 and 175 %
  display scale (1 px at 125 %, 2 px at 150 and 175 %): a solid line, no half-tone fringe (#325)
- Settings: a template's row puts its line 12 after the name and runs it 370 wide, as the design; the Templates and
  Scheduled pages' introductions say what the design says, the Scheduled one's two paragraphs 6 apart (#325)
- Settings, Privacy: a sender's address stands where the design puts it, in the muted colour; on a page too narrow
  for the whole address there, the empty name column gives way first (#325)
- Home's character field keeps the emblem on the same cells as its place moves by parts of a pixel, so a move alone
  builds no cells again (#325)
- Every menu keeps its width at every text size; a menu's head longer than its room is cut with one `…` before
  its count (#328)
- The one-point edges of fields, selects, checkboxes, buttons and the segmented control, and the bar of the place
  shown in the index and on the settings, stand on whole pixels at 125 % and 150 % screens (#328)
- The account picker's All accounts stands where the accounts' names stand, after the room of an account bar (#328)
- The text size slider's names keep at least 4 apart whatever their width (#328)
- The calendars' sync line grows with the text (52 high at 130 %, 70 with a failure); its paddings stay (#328)
- The why line draws no `coming soon` tag beside its reasons, as the design draws it; with no reason the tag stays
  (#328)
- The trash role is named Bin, as the design names it, and a delete says "Moved to Bin"; a server's folder keeps
  its own name (#328)
- The inbox of every account says what is left to read in the List view too ("N to read") (#328)
- New senders' queue stands 56 in from the body, 1 to the left of where it stood (#328)
- The index's Drafts row counts the drafts kept, in every place, and counts a new draft at once; a draft whose
  discard waits behind its Undo is not counted after the drafts place is left (#328)
- With nothing open the reader says "J and K move in the list, Enter opens the message. Ctrl K runs any command.",
  with the keys as the keymap has them (#328)
- The status line says a failing account's reason in a few words ("Studio: sign-in refused"), as the design
  does, instead of the engine's sentence for logs (#328)
- Privacy settings: a sender's row no longer leaves an empty name column; while no name is kept, the address starts
  at the row's left (16) and takes the name's column too, cut on one line with `…` at its end before "ask again"
  (#337)
- Editing a repeating event kept in UTC no longer rewrites its times on the computer's wall clock, which moved the
  series for anyone in another zone; an edit writes a time it does not change as it came (its form, `DURATION`
  too), and a changed time in the form the event already uses: UTC, its zone or the wall clock (#351)
- A new all-day event's default alert no longer rings at midnight the night before, and a timed event made all-day
  no longer keeps its ten-minute default to ring at 23:50 the evening before: it gets the all-day default, while an
  alert the owner picked is kept; an all-day event made timed trades an alert at any of the four hours back for the
  ten-minute default. Only the series switches: a changed instance that was already all-day keeps its alert. The
  editor's Alert shows what saving writes (#353)
- The hour kept from an earlier run rings from the start: the alerts worked out before it was handed back are worked
  out again at the next tick (#353)
- On a clock that skips midnight, an all-day alert rings at its hour, not an hour late (#353)
- A right-to-left subject, sender or label (Hebrew, Arabic) shows its words in the order they are read, numbers and
  Latin words in it where the Unicode bidi algorithm puts them, and a line too long for its room keeps its first
  words and ends in the ellipsis at its logical end; a two-row subject breaks into rows in the written order. Each
  label is a paragraph of its own: an override or isolate in a subject reorders nothing outside it, and the
  direction marks and controls are not drawn. An address or a domain stays left to right, its parts in the written
  order, so a label in a right-to-left script is never shown swapped with the one next to it (#356)
- A label in a tracked style that holds a zero-width joiner, a variation selector or right-to-left letters is no
  longer cut about a letter early per such sign or letter (#356)
- An agent's `history.undo` (the control channel, the CLI in the app) is refused with `editing`, "an event is being
  edited", while the owner has an event open in the window's editor; it runs again once the editor is saved,
  cancelled or left, or the window closes. The owner's own Undo runs as before (#367)
- A window of its own (the event editor's, the composer's) opened again after a list was used in it takes its Esc at
  once (#370)
- After the desktop moved or resized a window of its own, its next press is heard again: the release the desktop
  kept goes to that window, never to the main one (#379)
- A window opened again on the same running app uploads the reader's pictures again instead of drawing them from
  the textures of the window before (#165, #380)
- A link's target with labels in a right-to-left script (`https://שלום.קום.example.org/`) shows its labels in the
  order a click copies them wherever the app draws it: the link status under the reader for a text link, a
  button-link and a picture link, a careful link's host and address, a mail link's address, and `Link copied` in
  the reader and in a reader window, and any link in a notice or error of the status line. The scheme, the path
  and the query stay where they are; what is copied is the link as it was (#390)
- The cleaned page for the browser names a right-to-left host or mail address in a link's tooltip with
  left-to-right marks, so a browser shows its labels in the written order too (#390)
- CI: the `flatpak` job installs the runtime, the SDK and the Rust extension into the user installation, so a newer
  SDK on Flathub than the one in the builder image no longer stops the job before the build (#391)
- With several problems the status line shows the worst first: what needs the owner (a refused sign-in, a
  certificate) before what mends itself (no connection, a server not found), as the engine tells them apart (#396)
- The status line never says "offline since" a time still to come after the clock was set back (#396)
- The status line counts problems, a send the server refused among them, not failing accounts, and says "and 1 more
  problem" for one (#396)
- A link in the status line ("open account", "open log") follows its fact without a `·`, as the frames draw it (#396)
- The mailbox's error that a full status queue dropped no longer stays behind unseen: it goes with it, and the same
  failure told again, after it or after a dismissal, is shown again (#396)
- The command line's results' key hints leave out next, previous and run while nothing is listed (#396)
- The index's rows stand on whole pixels at fractional scales: at 125 % a 34-point row is 42.5 pixels, and its fill,
  bar and focus edge were smeared over half a pixel (#398)
- An agent's `ui.settings.open` (and `ui.home.open`) is passed over while the owner holds the keys: the command
  line's results, a dialog or a key being given stand in front, a menu or a list is open, the composer has the
  keyboard (docked or floating), the event editor is open (also under the settings), or the account form holds what
  the owner typed; it took the owner off the command line's results, a dialog or the composer before (#399)
- Home's failing banner says what fails, from `sync.status` and `account.list`: "Offline since 14:10: what you see is
  what this computer holds." when no account is signed in after a sync, the account that refused the sign-in by its
  name (or its address when its name is blank, cut so that "refused the sign-in" shows) or how many did, with "open
  account", and the sends the server refused, the first by its recipient, with "open outbox" and "Nothing is lost.",
  in the design file's words. A reconnect attempt does not take the offline banner down and put it back (#399)
- Home's failing banner wraps its words onto two lines, as the design file draws it, where it cut them on one; it is
  72 high, and the tiles stand at 280 (#399)
- Home's drafts' block draws three drafts in its frame of its own, as `home-drafts/default` does, and two on the
  page, as the page's frames do; `ui_shot` draws the block alone (`"home": "drafts"` and `"drafts-empty"`) (#399)
- The end of the store writes a draft whose keep had failed when the disk came back, also when the draft
  keeper's thread runs first; it could stop with nothing to write, and the end waited 3 s and left the draft
  on the disk as it was before (#400)
- The mail list stands 440 wide from the index's line, as the design file has it: its head, rows and the line to the
  reader stood 1 pixel right (#410)
- The screener card's line stands under its 64, not inside it (#410)
- After Done on the last row in sight the list scrolls only by what the next row lacks once the left row's room has
  closed; it scrolled up to 234 points, and in the List view it now does not scroll at all (#410)
- A window made shorter right after J or K keeps the keyed row whole; it lay below the list for one frame (#410)
- The window asks the desktop never to make it narrower than 800 points, the least width it is laid out at; a
  narrower window was laid out 800 wide and cut at its right (#412)
- Under the command line's results the window is dimmed from the line's foot, as the frames draw it, not from the
  field's: the title bar under the field is no longer dimmed, and a press there keeps the results open (#412)
- With the app's own frame, the title bar moves the window and a double press maximises it while the small event
  editor shows; the press no longer counts as a press outside the editor, which closed an empty one (#412)
- Home's character field and emblem move while the pointer stands still, as the motion source does: a cell within
  reach flips with a chance of 0.8 × f² (f = 1 at the pointer, 0 at the edge of its reach; at the reduced level
  × 0.4), may flip again after 50 to 190 ms and tries again after 60 ms if it did not; a flipped cell is back 80 to
  200 ms after its flip, the pointer there or not, and takes a new glyph every 70 to 230 ms once out of reach; about
  22 in 100 of the cells within reach are flipped at a time at full, 10 in 100 at reduced. The emblem twinkles by
  itself, about 7.5 cells a second, each for 250 to 750 ms, half of the ink ones in cyan and the cyan ones in cyan;
  at the reduced level only on Home, at off nothing moves. The field asks for a frame when its next change is due,
  not sooner: about 30 a second while the emblem twinkles with the pointer away, about 60 under a still pointer,
  none while Home does not show (#418)
- The reader's More menu draws no line under its "When a message leaves the list" head: its choices stand straight
  under it, as the group menu's do, and the menu is 1 point shorter (#421)
- An account's settings wait with Save, disabled, until something changed, and say "nothing changed" or "changed,
  not saved yet" beside Cancel; once saving would sign in anew (a new password, a server moved), the note saying so
  stands in `warn` where the caption about the credential store stood. The title reads "Account settings · <name>"
  after the account's bar, "← Accounts" over it is the small link of the frames, 16 high and 6 over the title's row,
  on the add form too, and no note under the New password box pushes the rows down any more (#422)
- A removed account's "<name> removed from this computer" is a done message on the status line: the check and the
  top line in `plus`. When the credential store kept the password, the line stays a notice with the reason (#422)
- A save of an account's settings that fails keeps what was typed, a new password too, so Save stays on and the save
  can be tried again from the form (#422)
- The error pictures (`error-banner`, `error-banner-strip`, `status-line-error`) show the error of the folder their
  scene opens again: on the frames' sample, the copies the kept drafts left due were made on the first frame, and
  that frame's refresh cleared the error; they are now made before the scene begins (#426)
- The event editor's pictures show what their names say on the frames' sample: `event-edit` and `event-full-edit`
  the owner's Thursday workshop, editable, and `event-read-only` and `event-full-read-only` Marit's Design review,
  read-only (#426)
- A segmented switch short of room keeps its names whole: the room round each name gives way first, down to 4, and
  only where even the whole names do not fit are they cut. "Before the inbox" and "Inside, marked" read whole in
  New senders beside the reader, where the queue's scroll bar took the room (#427)
- New senders puts the screener's switch under its label where the line has no room for both (#427)
- A held sender's card stays inside the list: a long name or subject ends with an ellipsis and leaves its room to the
  address, how many more and the time (#427)
- New senders keeps even pads round its cards: the queue's scroll bar stands over the right pad, as the mail list's
  does, and the cards keep the pane's room (#427)
- The search line beside the reader leaves "in message text" and its coming-soon mark out where they do not fit
  after the search's words, instead of cutting the mark at the pane's edge (#427)
- Settings › Calendar: the "All-day alerts ring at" choice grows with large text, so "09:00 the day before" reads
  whole at 130 % instead of ending in an ellipsis (#428)

### Security
- The CI dependency cache is pruned and saved more strictly (#79): the prune is `cargo xtask cache-prune`, an allow
  list that keeps only cargo's marker files and the `deps`, `build` and `.fingerprint` folders of each profile
  minus every entry named after a package or target of this workspace, deletes everything else under `target/`
  (`llvm-cov-target/tmp` and unknown folders included), deletes nothing when `cargo metadata` fails or its names
  are empty or lack `xtask`, and fails when a path under `target/` is still named after one afterwards; the step
  runs with `shell: bash`. The prune and the save run only on a `push` to `develop`, `main` or `epic/*` that did
  not create the branch. The cache key carries the runner image (`$ImageOS-$ImageVersion`) and the hash of
  `.cargo/config.toml`. A test in `xtask` reads `ci.yml` and keeps these conditions in place.
- The IMAP client hides the base64 forms of the credentials it sent, after LOGIN as after AUTHENTICATE PLAIN: the
  PLAIN token and the password alone, of the password as sent and trimmed, each with and without its padding, are
  hidden in a server's text, an error and `Debug` output; the proptest seed that found the leak is committed (#64)
- The write-ahead log is emptied also after a sender, an outbox item or a calendar's sync token is removed (#113)
- The SMTP client refuses AUTH and MAIL until the connection is encrypted, hides the password and its base64 forms,
  as sent and trimmed, in a server's text, an error and `Debug` output, shows no server words before TLS, takes for
  EHLO only a domain of labels or a whole address literal, and refuses an address with a line break, a space, a
  control character, an invisible mark or an angle bracket (#117)
- No password goes to a submission server before TLS: a server that lists AUTH before STARTTLS is not taken up on
  it, a server without STARTTLS on a STARTTLS account gets nothing past EHLO, and each certificate fault of the
  submission server has its own error kind. After the IMAP server refused the password, nothing is sent until the
  worker is started anew (#119)
- After the submission server refused the password or left it unanswered, a check an agent starts sends it nothing
  and reports the SMTP step as needing the owner; an owner's check or a send that signs in ends the hold (#119)
- A draft on its way can be neither changed nor discarded, so the message that went keeps its Sent copy (#119)
- A send job printed for a log shows the message's length and the number of recipients, never the message or a Bcc
  address; an append shows the length of its message only (#119)
- `inboxcraft-dav` writes Basic auth into a request to an `https` origin only (`DavError::NotEncrypted` otherwise),
  never follows a redirect to another origin (the password would go with it) and hides the password in `Debug`; the
  fake CalDAV server takes a password over TLS only and, over plain TCP, refuses every request without asking for
  one (#144)
- The CalDAV password goes as Basic auth over TLS whose certificate verified, and nowhere else: an address that is
  not `https` fails before a connection opens, a job URL on another origin than the account's CalDAV address fails
  before the password is asked for, a redirect to another server is not followed, the request bytes that
  carried it are overwritten once written, and each certificate fault of the CalDAV server has its own error kind.
  After the CalDAV server or the IMAP server refused the password, calendar jobs send nothing, and a check an agent
  starts reports the CalDAV step as needing the owner; an owner's check that finds the calendars ends the hold (#145)
- A calendar job printed for a log shows the length of an event's text, never the text (#145)
- `calendar.server` forgets the account's calendars, and drops every calendar write of the account still in the
  queue, when the address is cleared or moves to another server (host or port); a discovery carries the address it is
  for, the app starts an account's worker afresh when its servers change, and a discovery answer from another server
  and an event outside its calendar's collection are ignored, so the password and the events go to no server the
  account left (#146)
- A CalDAV address with a query or a fragment is refused, so a token typed there is not repeated (#146)
- Every organiser and guest InboxCraft writes into an event carries `SCHEDULE-AGENT=CLIENT`, so a CalDAV server that
  schedules on its own (RFC 6638) does not mail the guests behind the owner's back (#146)
- Bidi embeddings, overrides and isolates in a mail's words are left out of what banners, facts, check steps and
  progress lines draw and of every name they give, so a message cannot turn its own words round; the direction marks
  stay, so a right-to-left name keeps its brackets and digits in place (#235)
- Send again on a refused send no longer sends a draft that changed since it was sent, by an agent or anyone:
  `outbox.retry` refuses a refused draft whose recipients, account, subject, text, attachments or invitation
  changed, or that keeps no record of how it went, and the outbox row offers no send for it, only Edit, which opens
  it in the composer to be sent from there. A refused send that did not change keeps its plain Send again (#245)
- A recipient's name that holds an at sign, or shows nothing (only spaces, control, bidi, invisible or
  blank-looking characters: the Hangul fillers, the Braille blank, tag characters and every other default-ignorable
  character), is never shown for them: the composer's chip, the drafts place's row and a message's head show the
  address instead, so "odile@example.org" <eve@example.net> shows as eve@example.net. A chip that shows a name has
  its address in its tooltip and in what a screen reader hears (#245)
- A composer chip that shows an address is cut, when it must be, in the part before the at sign, never in the
  domain: a domain wider than a chip widens the chip up to the line's width, and one wider than the line is cut
  from its left only, so its end shows. A cut chip has the whole address in its tooltip (#245)
- Edit on a refused send that changed since (or keeps no record of how it went) opens it in the composer with
  every chip showing its address, not the name, until the composer is closed (#245)
- The composer's Send (its button, Ctrl+Enter, the palette's Send) does not send while a recipient's chip has not
  been whole in sight since the composer opened (scrolled out of its line, or in a Bcc line not shown): it shows Cc
  and Bcc, lets each line show eight rows of chips instead of four, scrolls each to the first such chip, and says
  "Check every recipient, then send"; each press brings the next chips not yet seen into sight, and the press after
  every chip has been in sight sends. A recipient line is never taller than the room over the composer's foot (one
  row of chips at least, with Subject kept in sight under it) and scrolls inside itself, so in a short window or
  floating panel no chip runs under Send and the owner is never stuck (#245)
- The composer sends from the account its From line shows: an account an agent puts on the draft while the
  composer is open is put back when the composer keeps or sends it (#245)
- A caller of the CLI, the control channel or MCP no longer learns the store's table and column names, or the
  words of a trigger in a crafted store file, from a failed command (#280)
