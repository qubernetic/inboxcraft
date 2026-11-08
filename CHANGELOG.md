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

### Removed
- The `draft` CI job: a draft PR now starts no job at all (#68)
- The dev Flatpak's permission to talk to `org.freedesktop.Notifications`: in the Flatpak, notifications go through
  the notification portal, which needs none (#138)

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
