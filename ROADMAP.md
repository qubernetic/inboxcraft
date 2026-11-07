# InboxCraft roadmap

**Stage: <pre-alpha|alpha|beta>** · next: <stage>, <what is left, in points or hours>

> **Last reviewed:** <date> · **Last updated:** 2026-11-07 · **Change:** minor

The detail is in [docs/gaps.md](docs/gaps.md) (what is missing, ranked) and [docs/architecture.md](docs/architecture.md).

## Headline numbers
| Number | Value | Kind |
|---|---|---|
| Commands in the registry | <n> | measured (`cargo xtask stats`) |
| Tests | <n> | measured |
| Fuzz targets | <n> | measured |
| Hours to beta | <n> Opus 5.5 agent-hours | estimated (<how>) |

## By area
| Area | Ready | Hours left | Doc |
|---|---|---|---|
| IMAP sync (IDLE, CONDSTORE/QRESYNC, offline queue) | <%> | <h> | |
| SMTP sending | <%> | <h> | |
| CalDAV calendars | <%> | <h> | |
| Account autodetect | <%> | <h> | |
| Tray and background mode | <%> | <h> | |
| Screener / grouped inbox / snooze / send later | <%> | <h> | |
| Reading mail safely (HTML) | <%> | <h> | |
| Search | <%> | <h> | |

## Upcoming
1. <item> (<hours>)

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
