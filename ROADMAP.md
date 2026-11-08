# InboxCraft roadmap

**Stage: pre-alpha** · next: beta, after the three blocking gaps of [docs/gaps.md](docs/gaps.md) (about 14 hours)
and the owner's daily use on both mailboxes (E3.11)

> **Last reviewed:** <date> · **Last updated:** 2026-11-08 · **Change:** minor

The detail is in [docs/gaps.md](docs/gaps.md) (what is missing, ranked) and [docs/architecture.md](docs/architecture.md).

## Headline numbers
| Number | Value | Kind |
|---|---|---|
| Commands in the registry | 81 | measured (`inboxcraft-cli commands`) |
| Tests | 3266 | measured (`cargo xtask ci`, nextest) |
| Fuzz targets | 29 | measured (`fuzz/Cargo.toml`) |
| Lines of Rust | 69846 code, 55671 test, in 667 files | measured (`cargo xtask stats`) |
| Hours to beta | 14 Opus 5.5 agent-hours | estimated (the blocking gaps of docs/gaps.md; the owner's daily use not counted) |

## By area
| Area | Ready | Hours left | Doc |
|---|---|---|---|
| IMAP sync (IDLE, CONDSTORE/QRESYNC, offline queue) | 85 % | 10 | [architecture](docs/architecture.md) |
| SMTP sending | 75 % | 6 | |
| CalDAV calendars | 75 % | 14 | |
| Account autodetect | 70 % | 6 | |
| Tray and background mode | 60 % | 30 | |
| Screener / grouped inbox / snooze / send later | 90 % | 6 | |
| Reading mail safely (HTML) | 90 % | 4 | |
| Search | 40 % | 12 | |

Ready and hours left are estimates from the gaps in [docs/gaps.md](docs/gaps.md). SMTP and CalDAV count the add-account
form that does not ask for their servers; tray and background count Windows and macOS, which have none yet.

## Upcoming
1. The add-account form asks for the SMTP server and the CalDAV address (4)
2. The computer's time zone handed to the engine, for presets, snooze, send later and the calendar (4)
3. Drafts opened again in the window, and attachments in the composer (6)
4. Beta hardening on the owner's two mailboxes (E3.11, the owner's hands-on use)

## Later ideas
Not planned for now; the model keeps room for them.

| Area | Note |
|---|---|
| Google Calendar | a vendor account through CalDAV with OAuth (PKCE); out of scope for now (2026-11-05) |

## Progress log
| Date | What landed |
|---|---|
| 2026-11-04 | Workspace skeleton: every crate and app as an empty crate, `cargo xtask ci`, `spdx`, `test-files` (#5) |
| 2026-11-04 | The `layers`, `wasm`, `assets` and `stats` gates, all in `cargo xtask ci` (#6) |
| 2026-11-04 | The coverage gate `cargo xtask cov --check`, floors in `.config/coverage.toml`, in `cargo xtask ci` (#7) |
| 2026-11-04 | `testkit`: the fake server frame with the drop, delay, garbage and certificate faults (#8) |
| 2026-11-04 | Opt-in live test harness: `just it-live`, nextest profile `live`, `testkit::live` (#10) |
| 2026-11-04 | `time`: injectable clock (system and fake), RFC 5322 dates with the obsolete forms, first fuzz target (#15) |
| 2026-11-04 | `charset`: WHATWG charsets to UTF-8, RFC 2047 encoded words read and written, two fuzz targets (#16) |
| 2026-11-04 | `model`: accounts, folders with roles (SPECIAL-USE, English and Hungarian names, ask once), messages, threads, the read path jobs and events (#17) |
| 2026-11-04 | `mime`: messages read into a tree of parts (headers, RFC 2231 parameters, base64 and quoted-printable, multiparts, attached messages, attachments, delivery reports), capped, three fuzz targets (#18) |
| 2026-11-04 | `mime`: messages built (fields, text and HTML, attachments) behind a header injection guard, build-then-parse property tests (#19) |
| 2026-11-04 | `proto`: the sans-I/O IMAP client core (greeting, CAPABILITY, STARTTLS, LOGIN and AUTHENTICATE PLAIN over TLS only, LIST with SPECIAL-USE, SELECT, EXAMINE, UID FETCH, UID SEARCH, LOGOUT), golden transcripts, fuzz target (#20) |
| 2026-11-04 | `proto`: ENABLE, CONDSTORE, QRESYNC with VANISHED and the UIDVALIDITY check, IDLE, NOTIFY with STATUS and mailbox changes, transcripts with and without each (#21) |
| 2026-11-04 | `proto`: UID COPY, UID MOVE with the copy and expunge fallback, UIDPLUS (UID EXPUNGE, COPYUID, UIDNOTSTICKY), PREVIEW, COMPRESS=DEFLATE negotiation (#22) |
| 2026-11-04 | `testkit`: the fake IMAP server with its own command reader, invented mailboxes, every E1.4 extension switchable per test, scripted mail changes and faults, STARTTLS with the certificate faults, fuzz target (#23) |
| 2026-11-04 | `store`: the local-first cache behind the `Storage` trait, a memory backend (wasm, tests) and a native SQLite backend with FTS5, sources as files, migrations, body retention and the simple search; a kill test in a child process (#24) |
| 2026-11-05 | `net`: TLS with the platform verifier, implicit TLS and STARTTLS with no password before TLS, the IMAP sign-in, one worker thread per account, the first sync (folder list, headers with PREVIEW, bodies of the last 30 days, a body on demand), one error kind per cause, fuzz target (#25) |
| 2026-11-05 | `net`: live updates with IDLE, resync of flags and removed messages (QRESYNC where offered, with what it keeps capped), a changed UIDVALIDITY reloads the folder, back-off with a cap, the offline queue, the COMPRESS=DEFLATE stream; the `live-imap` live test; the fake IMAP server reconnects and answers CREATE, DELETE and APPEND (#26) |
| 2026-11-05 | `engine`: the `Session` and the command registry with agent classes (unclassified refused, panics caught), jobs out and events in, the unified folder views merged by date with the account filter, the read commands `account.list`, `folder.list`, `mail.list`, `mail.read` and `mail.search`, `hostile_params_never_panic`, fuzz target (#27) |
| 2026-11-05 | `account.check` step by step (DNS, TCP with the blocked-mail-ports test, TLS, the certificate with its issuer and the clock check, the IMAP sign-in; SMTP and CalDAV not built yet), Detect (MX, autoconfig over HTTPS, the ISPDB off by default, SRV, host name hints) with no password to a detected server before the owner saw it, three fuzz targets, a live test (#28) |
| 2026-11-05 | `html`: hostile HTML to display blocks with nothing that runs or submits, remote pictures blocked until consent, `cid:` pictures, link targets exposed and text/target mismatches flagged, capped input, nesting and tables, a hostile fixture set, fuzz target (#29) |
| 2026-11-05 | Live tests on two throwaway accounts of the reference provider, `live-imap` and `live-imap2`; both sign in and list their folders; `just it-live` works on a fresh checkout (#57) |
| 2026-11-05 | The first window: `inboxcraft --sample` with the unified list, per-account narrowing, the folder pane with roles and unread badges and the search field, in InboxCraft's own light and dark theme; `ui_shot` pictures (#30) |
| 2026-11-05 | The dev Flatpak: `git pull && just flatpak` builds and installs InboxCraft with a menu entry (and the sample mailbox as its second action), offline from the checkout with the crates checked against `Cargo.lock` in `cargo xtask ci`; the app icon drawn in code (#31) |
| 2026-11-05 | The reader: thread oldest first, short and full header, folded quotes, safe HTML, attachment tiles with Save, remote pictures only after consent (once or always for the sender) through `net` leaking only the URL, link targets on hover, the next-message rule, the full-width reader that Escape leaves (#33) |
| 2026-11-05 | CI runs the tests once: the `cov` gate's run under coverage is the test run, no separate nextest gate (#66) |
| 2026-11-05 | Adding an account: the manual form with Detect's proposal, the password into the OS credential store only, `account.add` and `account.remove` (owner), the check's steps live with host, port and issuer; one `net` worker per account in the app (#34) |
| 2026-11-06 | Theme tokens from the mode (System, Light, Dark) and an accent preset (Electric, Aurora, Citrus), WCAG AA checked for every preset in both modes; teal and coral gone; `cargo xtask side-by-side` puts each picture beside its mood sketch (#81) |
| 2026-11-06 | Glass without blur: colour fields behind the window, the panes as glass layers, floats that go more opaque over sharp text, one dithered gradient helper; no repaint while idle (#82) |
| 2026-11-06 | Figtree in place of Ubuntu Light, every text style a font token, unread rows by weight, a text scale of 90 % to 130 % with rows and the reader's toolbar following the font, checked for overflow, clipping and overlap (#83) |
| 2026-11-06 | An icon painter: views ask for an `IconId` in Regular, Fill or Duotone, the set maps every id in an exhaustive match; Phosphor 2.1.1 bundled; the flag, the file tile and the step marks moved onto it (#84) |
| 2026-11-06 | The design system's components (card, icon button, pill, tooltip with key chip, avatar, badge, hover actions, toast, popup) with a `ui_shot` gallery; motion tokens and one reduced-motion flag every animation reads; a keymap the tooltips read their keys from; avatars in a vivid palette picked per address, in the reader's header (#85) |
| 2026-11-06 | The appearance commands `ui.theme.set`, `ui.accent.set` (preset or four custom colours, nudged to WCAG AA), `ui.font.set`, `ui.icons.set`, `ui.text_scale.set` and `ui.motion.set`, all `safe`, kept across a restart in the config folder; Inter and Lucide bundled as the second typeface and icon set (#86) |
| 2026-11-06 | The appearance settings page: mode, accent preset, custom accent, typeface, icon set, text size and a "Reduce motion" switch, each running its `ui.*.set` command, with a live preview; checked for overflow in both typefaces at 90 % and 130 % (#87) |
| 2026-11-06 | The message list on the design system: a navigation rail in the preset's gradient, the rows on a card with avatars, the screener and grouped inbox cards and the hover actions built and shown in `ui_shot` with invented data, hidden in the app until their data and commands exist (#88) |
| 2026-11-06 | The hover actions act on the sample mailbox: Done, read or unread and Delete, from the row or its keys (D, R, Delete), through the sample-only `sample.mail.set` in memory; the row slides out, a toast with Undo (Ctrl+Z) puts it back; reduced motion makes both land at once; hidden without `--sample` (#89) |
| 2026-11-06 | The reader on the design system: a toolbar of mail actions over the subject, the thread as cards with Reply, Reply all and Forward on the open message, full width and the next-message rule in a More menu, the link status only while a link is hovered; actions without a command disabled with a tooltip, Done and Delete acting on the sample mailbox (#90) |
| 2026-11-06 | The design file in FigCraft: foundations, components, every screen in light and dark, mood, five flows (#96) |
| 2026-11-06 | UI polish toward the design file, waves 1 and 2: primary and secondary buttons on the text gradient, the search pill, the dark accent from the middle stop, inset rows and outline group icons; the rail with the app mark and its views, the folder pane on the design, the app's own window frame (`ui.frame.set`) and the brand mark as the app icon (#104) |
| 2026-11-06 | UI polish wave 3: the settings as a sheet with sections and setting rows, the add-account form and the check on the design, the reader's large subject with its group chip, the list's Show all link, stripes and hover actions; the own window frame fixed on Wayland (resizing, moving, hover after a move) with round corners (#104) |
| 2026-11-06 | Accounts move to the settings sheet: the Accounts page lists them and adds one (form, Detect, the check), the empty window is one card, the folder pane drops Add account (#104) |
| 2026-11-07 | Tests for the weakest files of the read mail epic: the account workers and the credential store without one, the owner's config folder, IDLE ended or refused by the server, the pill's hover and focus (#108) |
| 2026-11-07 | The IDLE tests wait at the fake server for the IDLE they count (`FakeServer::wait_received`) instead of racing the worker's stop (#164) |
| 2026-11-08 | IMAP: the base64 forms of the credentials are hidden like the password (#64) |
| 2026-11-08 | Change fragments in `changes/`, checked in `cargo xtask ci` and folded by `cargo xtask changes --fold`; one command list per engine module, with stubs for the daily-use areas (#111) |
| 2026-11-08 | The desktop app keeps its store on disk; schema version 4 holds the job queue, drafts, outbox, schedule, senders, message state and calendars; kept jobs survive a restart (#113) |
| 2026-11-08 | The engine acts on mail: flag, read and unread, open, move, archive and delete, each undoable, with the write jobs for the server (#114) |
| 2026-11-08 | The IMAP write path: flags, move, delete for good, APPEND and CREATE on the server, the offline queue run after the reconnect, a message gone meanwhile told as an event (#115) |
| 2026-11-08 | Drafts that are never lost: kept at every change, copied to the Drafts folder, with the compose commands, a signature per account and checked recipients (#116) |
| 2026-11-08 | SMTP: the sans-I/O submission client in `proto` (EHLO, STARTTLS, AUTH PLAIN and LOGIN, MAIL, RCPT, DATA, SIZE, 8BITMIME, SMTPUTF8, enhanced status codes), golden transcripts and a fuzz target (#117) |
| 2026-11-08 | `testkit`: the fake SMTP submission server with its own command reader, scripted refusals with enhanced codes, captured messages, a bounce script into the fake IMAP mailbox, STARTTLS and implicit TLS with the certificate faults, fuzz target (#118) |
| 2026-11-08 | SMTP submission in `net` and `mail.send`: implicit TLS and STARTTLS, AUTH only after TLS, Bcc in the envelope only, the Sent copy, the SMTP step of the account check, and a live test (#119) |
| 2026-11-08 | Reply, reply all, forward and templates in the engine: threading headers, standard prefixes, a quote line of our own, reply all without the owner's addresses, forwards with their attachments, and template blanks that block Send (#120) |
| 2026-11-08 | Outbox, send errors and bounces in the engine: refusals with the server's reply and enhanced code, retry by command, delivery reports linked to the sent message by `Message-ID` and let past the screener, tested against the fake SMTP server's refusal and DSN scripts (#121) |
| 2026-11-08 | The scheduler with its time presets in the computer's wall-clock time, tested across summer-time changes (#122) |
| 2026-11-08 | Undo send and send later in the engine: every send held 5 to 60 seconds and undoable with its draft intact, send later through the scheduler, a time missed while the app was off sent at the next start with a notice, and the window ticking the engine, tested on a fake clock (#123) |
| 2026-11-08 | The screener with the known-sender index: before the inbox, inside it or off, accept (with a group, open or done) and block, both undoable (#124) |
| 2026-11-08 | The grouped inbox: our classifier with its table test, People, Updates, Newsletters and Invitations, a per-sender choice with undo, grouping per account (#125) |
| 2026-11-08 | Done, park and pin: done files in the archive and "show done" lists it in its old place, the park pile, the pins view, a pin through done by setting, each undoable (#126) |
| 2026-11-08 | Snooze: a copy with the time in our own header field in `InboxCraft/Snoozed`, back unread at the top at the time, read by a second InboxCraft, undoable (#127) |
| 2026-11-08 | Follow-up reminders: "remind me if no reply by" on a sent message, cancelled by a synced reply, a notice that opens the sent message at the time, undoable (#128) |
| 2026-11-08 | The shared time picker for snooze, send later and reminders, with typed times, in the component gallery (#129) |
| 2026-11-08 | The command palette (Ctrl+K), keys for every action changed on the Shortcuts page and kept between runs, and the calendar screen's place behind the rail with its empty state (#130) |
| 2026-11-08 | The window acts on real mail: read and unread, flag, done, archive, delete and move from the hover actions, the toolbar and the keys, opening marks read, and Undo and Ctrl+Z through the engine's history (#131) |
| 2026-11-08 | Writing mail: the composer in the reader's place with recipient chips, suggestions, the account's signature, silent draft saving and Send, a floating panel while the reader has the window, and the Sending page (#132) |
| 2026-11-08 | Reply, reply all, forward and templates in the window: the reader's buttons open the composer filled in, a template menu, and the blanks asked for one at a time before Send is free (#133) |
| 2026-11-08 | The screener and the grouped inbox in the window: New senders with a preview, accept (with a group, or open) and block; cards, grouped list or simple list from the engine's groups; a sender's group changed with undo; the Triage page with the screener's mode kept between runs (#134) |
| 2026-11-08 | Done, park, pin, snooze and follow-up reminders in the window: the time picker, Later on the rail with its piles, "Show done", the returned mark, "Done all" and the pin setting (#135) |
| 2026-11-08 | The outbox, undo send, send later and the bounce mark in the window, and the undo-send seconds on the Sending page (#136) |
| 2026-11-08 | InboxCraft runs on with its window closed: one instance on the owner's mailbox, `--background`, `--quit`, the Linux tray with the unread count, and start at login through XDG autostart or the Background portal (#137) |
| 2026-11-08 | Desktop notifications for new mail, returned snoozes and follow-up reminders, decided by the engine per account (all, people, none) and shown through the notification portal or `org.freedesktop.Notifications` (#138) |
| 2026-11-08 | The Notifications page: each account's notification level, returned snoozes and reminders, running in the background and starting with the computer, kept between runs (#139) |
| 2026-11-08 | E2.7 part 1: the headless CLI runs every command by id on a local store, `--json`, agent classes enforced (#140) |
| 2026-11-08 | E2.8: the read-only MCP server over stdio on a local store, message text marked untrusted (#141) |
| 2026-11-08 | E2.7 part 2: the local control channel with a token in the credential store, revocable; the CLI and MCP talk to the running app when no `--store` is given (#142) |
| 2026-11-08 | E4.1: `ical` reads and writes iCalendar, time zones from `VTIMEZONE`, recurrence expanded with property tests, two fuzz targets (#143) |
| 2026-11-08 | E4.2 part 1: `dav` finds the calendars, syncs them with sync tokens or entity tags and writes and removes events with entity tags; the fake CalDAV server in `testkit` with Basic auth over TLS only; three fuzz targets (#144) |
| 2026-11-08 | E4.2 part 2: the `net` worker finds, syncs, writes and removes CalDAV calendars and events with sync tokens and entity tags, conflicts reported and never overwritten, Basic auth over verified TLS only; the CalDAV step of the account check; a live test on the `InboxCraft-Test` calendar (#145) |
| 2026-11-08 | E4.4 and E4.5 in the engine: the calendars and their events listed with recurrence expanded and the event's own zone, events made, changed and deleted with repeat presets, all-day, busy or free, private and guests, a delete held for the undo window, undo, the CalDAV reports applied (#146) |
| 2026-11-08 | E4.4 in the window: the day, week, month and agenda views with overlaps side by side and blocks over several days, the mini month, the calendars' boxes and the details with the event's own zone (#147) |
| 2026-11-08 | E4.5 in the window: the event editor with repeat presets, all-day, busy or free, private, the guests and the question whether to tell them, and Undo on its toast (#148) |
| 2026-11-08 | E4.6 invitations: an invitation in mail becomes a card in the reader, answered at once with Going, Maybe or No and changed later; updates and cancellations follow the same event by `UID` and `SEQUENCE`, with the request, reply, update and cancel chain tested on invented events (#149) |
| 2026-11-08 | E4.7 reminders: calendar alerts become desktop notifications at the alert time, on a fake clock in tests and across a change of the clock, and a click opens the event (#150) |
| 2026-11-08 | The daily-use epic's change fragments folded into the docs; the headline numbers, the by-area table, the ranked gaps and the README's "What works today" filled (#151) |

## Revision history
| Date | Change | Summary |
|---|---|---|
| <date> | major | First version |
| 2026-11-04 | minor | Progress log: workspace skeleton and first gates (#5) |
| 2026-11-04 | minor | Progress log: the remaining E0 gates (#6) |
| 2026-11-04 | minor | Progress log: the coverage gate (#7) |
| 2026-11-04 | minor | Progress log: the fake server frame (#8) |
| 2026-11-04 | minor | Progress log: the live test harness (#10) |
| 2026-11-04 | minor | Progress log: the clock and RFC 5322 dates (#15) |
| 2026-11-04 | minor | Progress log: charsets and encoded words (#16) |
| 2026-11-04 | minor | Progress log: the mailbox model (#17) |
| 2026-11-04 | minor | Progress log: the message reader (#18) |
| 2026-11-04 | minor | Progress log: the message builder (#19) |
| 2026-11-04 | minor | Progress log: the IMAP client core (#20) |
| 2026-11-04 | minor | Progress log: the IMAP sync extensions (#21) |
| 2026-11-04 | minor | Progress log: IMAP move, UIDPLUS, preview and compression (#22) |
| 2026-11-04 | minor | Progress log: the fake IMAP server (#23) |
| 2026-11-04 | minor | Progress log: the local-first store (#24) |
| 2026-11-05 | minor | Progress log: TLS, the IMAP sign-in and the first sync (#25) |
| 2026-11-05 | minor | Progress log: live updates, resync and back-off (#26) |
| 2026-11-05 | minor | Progress log: the resync caps what it keeps (#26) |
| 2026-11-05 | minor | Progress log: the engine read path and the command registry (#27) |
| 2026-11-05 | minor | Progress log: the account check and Detect (#28) |
| 2026-11-05 | minor | Google Calendar moves to the later ideas; progress log: the second live account (#57) |
| 2026-11-05 | minor | Progress log: safe HTML display (#29) |
| 2026-11-05 | minor | Progress log: the first window with the sample mailbox (#30) |
| 2026-11-05 | minor | Progress log: the dev Flatpak (#31) |
| 2026-11-05 | minor | Progress log: the reader (#33) |
| 2026-11-05 | minor | Progress log: the tests run once in CI (#66) |
| 2026-11-05 | minor | Progress log: adding an account (#34) |
| 2026-11-06 | minor | Progress log: theme tokens from mode and accent preset (#81) |
| 2026-11-06 | minor | Progress log: the side-by-side pictures (#81) |
| 2026-11-06 | minor | Progress log: glass and floats (#82) |
| 2026-11-06 | minor | Progress log: Figtree, font tokens and the text scale (#83) |
| 2026-11-06 | minor | Progress log: the icon painter and Phosphor (#84) |
| 2026-11-06 | minor | Progress log: the components and motion tokens (#85) |
| 2026-11-06 | minor | Progress log: the appearance commands, Inter and Lucide (#86) |
| 2026-11-06 | minor | Progress log: the appearance settings page (#87) |
| 2026-11-06 | minor | Progress log: the message list on the design system (#88) |
| 2026-11-06 | minor | Progress log: the hover actions on the sample mailbox (#89) |
| 2026-11-06 | minor | Progress log: the reader on the design system (#90) |
| 2026-11-06 | minor | Progress log: the design file (#96) |
| 2026-11-06 | minor | Progress log: UI polish waves 1 and 2 (#104) |
| 2026-11-06 | minor | Progress log: UI polish wave 3 (#104) |
| 2026-11-06 | minor | Progress log: accounts on the settings sheet (#104) |
| 2026-11-07 | minor | Progress log: tests for the weakest files of the read mail epic (#108) |
| 2026-11-07 | minor | Progress log: the IDLE tests wait for the IDLE they count (#164) |
| 2026-11-08 | minor | Progress log: base64 forms of the credentials hidden (#64) |
| 2026-11-08 | minor | Progress log: change fragments and per-module command lists (#111) |
| 2026-11-08 | minor | Progress log: the store on disk with the local tables (#113) |
| 2026-11-08 | minor | Progress log: the acts on mail with undo and the write jobs (#114) |
| 2026-11-08 | minor | Progress log: the IMAP write path and the offline queue run after the reconnect (#115) |
| 2026-11-08 | minor | Progress log: drafts and the compose commands (#116) |
| 2026-11-08 | minor | Progress log: the sans-I/O SMTP client (#117) |
| 2026-11-08 | minor | Progress log: the fake SMTP server (#118) |
| 2026-11-08 | minor | Progress log: SMTP submission and `mail.send` (#119) |
| 2026-11-08 | minor | Progress log: reply, reply all, forward and templates in the engine (#120) |
| 2026-11-08 | minor | Progress log: outbox, send errors and bounces in the engine (#121) |
| 2026-11-08 | minor | Progress log: the scheduler with time presets (#122) |
| 2026-11-08 | minor | Progress log: undo send and send later in the engine (#123) |
| 2026-11-08 | minor | Progress log: the screener and the known-sender index (#124) |
| 2026-11-08 | minor | Progress log: the grouped inbox (#125) |
| 2026-11-08 | minor | Progress log: done, park and pin (#126) |
| 2026-11-08 | minor | Progress log: snooze (#127) |
| 2026-11-08 | minor | Progress log: follow-up reminders (#128) |
| 2026-11-08 | minor | Progress log: the shared time picker (#129) |
| 2026-11-08 | minor | Progress log: the command palette, the Shortcuts page and the calendar screen (#130) |
| 2026-11-08 | minor | Progress log: acting on real mail with undo (#131) |
| 2026-11-08 | minor | Progress log: the composer with drafts (#132) |
| 2026-11-08 | minor | Progress log: reply, forward and templates in the window (#133) |
| 2026-11-08 | minor | Progress log: the screener and the grouped inbox in the window (#134) |
| 2026-11-08 | minor | Progress log: done, park, pin, snooze and follow-up in the window (#135) |
| 2026-11-08 | minor | Progress log: the outbox, undo send and send later in the window (#136) |
| 2026-11-08 | minor | Progress log: the app runs on with its window closed, with the Linux tray (#137) |
| 2026-11-08 | minor | Progress log: desktop notifications (#138) |
| 2026-11-08 | minor | Progress log: the Notifications page (#139) |
| 2026-11-08 | minor | Progress log: the headless CLI (#140) |
| 2026-11-08 | minor | Progress log: the read-only MCP server (#141) |
| 2026-11-08 | minor | Progress log: the local control channel (#142) |
| 2026-11-08 | minor | Progress log: `ical` (#143) |
| 2026-11-08 | minor | Progress log: `dav` and the fake CalDAV server (#144) |
| 2026-11-08 | minor | Progress log: CalDAV sync in `net` (#145) |
| 2026-11-08 | minor | Progress log: the calendar commands in the engine (#146) |
| 2026-11-08 | minor | Progress log: the calendar's views in the window (#147) |
| 2026-11-08 | minor | Progress log: the event editor in the window (#148) |
| 2026-11-08 | minor | Progress log: invitations answered from mail (#149) |
| 2026-11-08 | minor | Progress log: event reminders (#150) |
| 2026-11-08 | minor | Stage, headline numbers, by-area table and upcoming filled for the daily-use epic (#151) |
