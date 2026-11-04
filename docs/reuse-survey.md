# Reuse survey of the three MailCraft repositories

> **Last reviewed:** 2026-11-04 · **Last updated:** 2026-11-04 · **Change:** major

Three open-source Rust mail programs called MailCraft exist. This survey says, for each, what InboxCraft takes,
how, and what it leaves, under the rules of `AGENTS.md` (clean room, MIT or Apache only, no names or assets) and
the owner's decision that reading their source is allowed and that no code is copied by the survey itself. Task
E0.1 of the E0 Foundations epic (qubernetic/inboxcraft#4).

## Summary

| Repository | Pinned commit | Licence verdict | Take | Leave |
|---|---|---|---|---|
| `UCS-Care-Group/mailcraft` | `e92228cf05362ac76df40d103b929a8506257395` | MIT OR Apache-2.0, every source file carries the SPDX line | `html` (with `link`, `names`) as a pinned git dependency; `mime`, `charset`, `time`, `ical`, `vcard`, `proto`, `dav`, `control-key` vendored | `model`, `engine`, `net`, `ews`, `jmap`, `mcp`, `ui-egui`, `i18n`, apps, `xtask` |
| `ZIDAN44/mailcraft` | `1517e3c6dbe5f4c2f39d28381a885ed3101d8171` | MIT OR Apache-2.0 by workspace manifest, no per-file SPDX lines | selected files of `net` (IMAP client and sync, IDLE worker, OAuth, TLS stream, autoconfig, CalDAV, fake servers) vendored; `store/src/atomic.rs` and the offline `Op` replay vendored | the rest of `net` (PST, Exchange, JMAP, POP, Sieve, self-update), `store`, `model`, `mime`, `engine`, `ui-egui`, `jmap`, `keys`, `pgp`, `smime`, `print` |
| `MonkeySaint/mailcraft` | `2ac4db88369abcd3d4e9dc025d5b9816c20b50b6` | MIT OR Apache-2.0 by workspace manifest, no per-file SPDX lines | nothing now; `search` and `mime` noted as fallbacks | everything |

**The three questions (spec section 8):** ZIDAN44's engine supports neither NOTIFY nor PREVIEW; its `store` cannot
serve us as it is; its OAuth2 cannot do Google CalDAV as it is (mail scope only, and its CalDAV client signs in
with Basic only). **Store recommendation (Q-O2):** our own `store`, a trait with an in-memory backend everywhere
and SQLite with FTS5 as the native backend behind a non-wasm feature; redb with our own index is the fallback if
the owner refuses a C library.

## Method

- Date: 2026-11-04. Each repository was cloned at its default branch head and read at that commit; nothing was
  copied. Sizes are `wc -l` over `.rs` files and `du` of the crate directory. Line counts include tests.
- Licence checks: the `LICENSE-*` files, every `Cargo.toml`, a grep of every tracked file for SPDX lines and for
  the words GPL, LGPL, AGPL, MPL, "GNU" and "Copyright (c)" outside the two licence files, and each repository's
  `NOTICE` and `ATTRIBUTION.md`. Dependency licences were read from crates.io on 2026-11-04 (version and licence
  of the newest non-yanked release).
- Build checks with the dev image's Rust 1.98.0 and its `wasm32-unknown-unknown` target: `cargo check --locked
  --target wasm32-unknown-unknown` for the UCS candidate crates and for ZIDAN44's `store` and `model`, and three
  one-dependency probe crates for `redb`, `tantivy` and `rusqlite` (bundled) on the same target.
- Spec facts from Google's CalDAV guide and Calendar API scopes page, read on 2026-11-04.

## Rules applied to everything taken

- A GPL, LGPL or AGPL file anywhere in a candidate disqualifies that part. None was found in any of the three
  repositories (the only occurrences of the word are notes that such licences are excluded, a deny-list test in
  MonkeySaint's `xtask/src/licences.rs`, and MonkeySaint's note that libzstd's BSD or GPLv2 dual licence is taken
  under BSD).
- Vendored files keep the source's copyright line and get our SPDX header (`cargo xtask spdx --fix`), the crate
  root gets our deny header, inline `mod tests` move to files of their own (`cargo xtask test-files`), every
  `unwrap`, `expect` and `panic` in non-test code is replaced, and each parser gets a fuzz target. The counts of
  `unwrap()` and `expect(` below are over non-test files and include their inline test modules, so they are upper
  bounds.
- A pinned git dependency needs its repository in `deny.toml` `[sources] allow-git` and a 40-character `rev` in
  `Cargo.toml` (the placeholder line is already there).
- Names, icons, fonts, sample mail and documentation wording are never taken. Each repository's fonts (Inter,
  Source Sans 3, JetBrains Mono, Liberation, Carlito; SIL OFL 1.1) stay with them.

## 1. UCS-Care-Group/mailcraft

**Commit:** `e92228cf05362ac76df40d103b929a8506257395` on `main`, 2026-11-04 03:10 +0100, 1,252 commits, 1,087
tracked files, 39 MB checkout, workspace version 0.10.0, edition 2024, `rust-version` 1.95.

**Focus:** Microsoft 365, Outlook.com and IMAP; EWS, Graph and JMAP transports; an Outlook-shaped engine and UI.
A browser build exists, so its `xtask wasm` gate (`xtask/src/main.rs:149-181`) checks every L0 to L5 crate plus
`ui-egui` for `wasm32-unknown-unknown`. Layer table in `xtask/src/layers.rs:38-64`: `time` and `link` L0;
`control-key`, `charset`, `names`, `i18n` standalone; `model`, `html` L1; `mime`, `ical`, `vcard` L2; `proto`,
`jmap`, `dav`, `ews`, `net` L3; `engine` L4; `mcp` L5; `ui-egui` L6. This is the same shape as ours.

**Licence:** `LICENSE-MIT` ("Copyright (c) 2026 the MailCraft contributors") and `LICENSE-APACHE`; every crate
has `license.workspace = true` with `MIT OR Apache-2.0`. All 874 `.rs` files carry `SPDX-FileCopyrightText:
MailCraft contributors` and `SPDX-License-Identifier: MIT OR Apache-2.0`; `README.md` and one asset script carry
the identifier too. No other licence text in the tree. `NOTICE` says the workspace tooling, packaging, control
channel and `control-key`, MCP framing, desktop plumbing and the theme and widget scheme started from WordCraft
(storytold/wordcraft, MIT OR Apache-2.0), so anything taken from those parts is attributed to both. `CLA.md`
binds contributors to them, not users of their code. GitHub's licence detector shows only "Apache-2.0" because it
reads one file.

**Discipline:** every crate root has the deny header; 19 fuzz targets in `fuzz/fuzz_targets/` (`charset`, `html`,
`sheet`, `ical`, `imap`, `smtp`, `mime`, `vcard`, `link`, `dav_xml`, `dav_document`, and EWS or JMAP ones we do
not need). Tests are partly inline (`#[cfg(test)]` modules in 8 of `html`'s files and 2 of `mime`'s) and partly in
`*_tests.rs` files.

**wasm check:** `cargo check --locked --target wasm32-unknown-unknown` for `html`, `mime`, `ical`, `vcard`,
`time`, `charset`, `proto`, `dav` and `control-key` finished in 1 m 22 s with two warnings and no error.

### Candidate crates

| Crate | Files / lines / KiB | SPDX | Dependencies (crates.io licence, checked 2026-11-04) | Take | How | Leave |
|---|---|---|---|---|---|---|
| `crates/html` | 24 / 15,583 / 771 | 24 of 24 | `mailcraft-link`, `mailcraft-names`, `serde` | whole: hostile HTML to display blocks, plain text, preview line, quotable copy, inline style and `<style>` whitelist, caps on input, nesting and tables (`src/lib.rs:1-24`); 0 `unwrap`/`expect` | pinned git dependency (`mailcraft-html`, pulling `mailcraft-link` and `mailcraft-names`), wrapped by our `inboxcraft-html` (L1) which owns the API the rest of InboxCraft sees; fuzz through the wrapper as `fuzz/fuzz_targets/html.rs` and `sheet.rs` do there | nothing; the `corpus` module is their test corpus and stays unused |
| `crates/link` | 5 / 1,126 / 43 | 5 of 5 | `mailcraft-names` | comes with `html`: RFC 3986 reader, Punycode, percent-encoding | same git dependency | - |
| `crates/names` | 2 / 606 / 25 | 2 of 2 | none | comes with `html`: Unicode format characters, name normalisation | same git dependency | - |
| `crates/mime` | 18 / 6,130 / 259 | 18 of 18 | `mailcraft-time`, `mailcraft-model`, `mailcraft-html`, `mailcraft-charset`, `thiserror` | RFC 5322 and MIME reader and writer, addresses, header folding, transfer encodings, `mailto:`, mbox, rich body (`src/parse.rs`, `src/write.rs`, `src/header.rs`, `src/address.rs`, `src/transfer.rs`, `src/mailto.rs`, `src/mbox.rs`, `src/rich.rs`); 33 `unwrap`/`expect` | vendor into `crates/mime` (L2); it uses `mailcraft_model::{Message, Body, Address}` and a few constants, which map onto our `model` types | nothing else; `mailcraft-model` itself is not taken (below) |
| `crates/charset` | 3 / 382 / 16 | 3 of 3 | `encoding_rs` 0.8.44, (Apache-2.0 OR MIT) AND BSD-3-Clause | whole: WHATWG charsets to text, base64; 0 `unwrap` | vendor into `crates/charset` (L0 standalone) | - |
| `crates/time` | 4 / 1,860 / 78 | 4 of 4 | `serde`, `jiff` 0.2.39 (Unlicense OR MIT, IANA zones bundled), `js-sys` on wasm | whole: civil calendar maths, RFC 5322, ISO 8601 and iCalendar text, zones, clock; 18 `unwrap`/`expect` | vendor into `crates/time` (L0); `src/zone/windows.rs` restates CLDR `windowsZones` (Unicode-3.0), which needs its own NOTICE line | - |
| `crates/ical` | 10 / 4,905 / 231 | 10 of 10 | `mailcraft-time`, `mailcraft-model` | RFC 5545 reader and writer, recurrence rules, VTIMEZONE (`src/read.rs`, `src/write.rs`, `src/rrule.rs`, `src/zones.rs`); 50 `unwrap`/`expect` | vendor into `crates/ical` (L2); needs our `Event`, `Recurrence` and zoned-time types in `model` | - |
| `crates/vcard` | 6 / 2,129 / 91 | 6 of 6 | `mailcraft-time`, `mailcraft-model`, `mailcraft-charset` | vCard 2.1, 3.0, 4.0 reader and writer; 11 `unwrap`/`expect` | vendor into `crates/vcard` (L2); needs our `Contact` type | - |
| `crates/proto` | 12 / 4,363 / 188 | 12 of 12 | `mailcraft-charset`, `thiserror`, `log` | IMAP4rev1 and SMTP clients over any `Read + Write` (`src/lib.rs:3-11`): CAPABILITY, STARTTLS, LOGIN, XOAUTH2 with injection guard, LIST, SELECT, UID FETCH, UID STORE, UID SEARCH, MOVE, COPY, UIDPLUS, SPECIAL-USE, APPEND, modified UTF-7, line and literal caps (`src/imap/mod.rs`); SMTP per RFC 5321, 3207, 4954 (`src/smtp.rs`); 13 `unwrap`/`expect` | vendor into `crates/proto` (L3) as the base; it has no IDLE, ENABLE, CONDSTORE, QRESYNC, NOTIFY, PREVIEW or COMPRESS, which E1.4 adds (the CONDSTORE, QRESYNC and IDLE logic comes from ZIDAN44, section 2) | - |
| `crates/dav` | 5 / 1,220 / 54 | 5 of 5 | `mailcraft-model`, `quick-xml` 0.42.0 (MIT) | bounded DAV XML reader and document editing without sockets (`src/xml.rs`, `src/document.rs`); 0 `unwrap` | vendor into `crates/dav` (L3) as the XML layer; discovery and sync come from ZIDAN44 | - |
| `crates/control-key` | 3 / 198 / 7 | 3 of 3 | none | control-port key file and proof | vendor into `crates/control-key` (standalone); per their `NOTICE` it started from WordCraft, so attribute both | - |
| `crates/model` | 66 / 25,973 / 1,176 | 66 of 66 | `i18n`, `time`, `link`, `charset`, `serde`, `serde_json`, `thiserror` | only the shapes the vendored crates need: `Message`, `Body`, `Address`, `Event`, `Recurrence`, `Contact`, zoned times (`src/message.rs`, `src/address.rs`, `src/calendar.rs`, `src/contact.rs`, `src/zoned.rs`) | re-written into our `model` while vendoring `mime`, `ical` and `vcard`; not taken as a crate | rules, junk, EWS, JMAP and POP sync, search, auto-replies, remote tasks: Outlook-shaped |
| `crates/i18n` | 7 / 1,705 / 2,799 | 7 of 7 | none (catalogues compiled in) | nothing now | the Lead decides the i18n macros in E1; this gettext reader is a candidate then | all |
| `crates/engine`, `crates/ui-egui`, `crates/net`, `crates/ews`, `crates/jmap`, `crates/mcp`, apps, `xtask` | 183 / 85,805; 223 / 120,830; 82 / 41,590; 33 / 11,923; 48 / 14,893; 12 / 2,205 | all | - | nothing | the widget styling the spec mentions, the MCP framing and `xtask` are WordCraft's per their `NOTICE`; take those from storytold/wordcraft directly, as our `NOTICE` already plans | their structure follows Outlook and Microsoft transports; `net` has no IMAP IDLE (no IDLE command anywhere in `crates/net` or `crates/proto`) |

## 2. ZIDAN44/mailcraft

**Commit:** `1517e3c6dbe5f4c2f39d28381a885ed3101d8171` on `main`, 2026-11-04 06:38 +0600, 1,007 commits, 945
tracked files, 16 MB checkout, workspace version 0.32.0, edition 2024, `rust-version` 1.95.

**Focus:** IMAP, SMTP, JMAP, Exchange, PST and MSG files, S/MIME and PGP; a local-first cache in redb data files;
one worker thread per account. `docs/sync.md` and `docs/oauth.md` describe the sync and sign-in designs.

**Licence:** `LICENSE-MIT` ("Copyright (c) 2026 ArtCraft Team and the MailCraft contributors") and
`LICENSE-APACHE`; every crate has `license.workspace = true` with `MIT OR Apache-2.0`. None of the 684 `.rs`
files carries an SPDX line, so the per-file verdict rests on the manifests and the two licence files: no file
carries any other notice (the only hits were the `LegalCopyright` string in `apps/mailcraft/build.rs` and a
`deny.toml` exception for `option-ext` MPL-2.0, a dependency of `dirs`). Verdict: MIT OR Apache-2.0 for every file;
vendored files get a `SPDX-FileCopyrightText: 2026 ArtCraft Team and the MailCraft contributors` line. The
`[patch.crates-io]` egui and winit forks are storytold's, MIT OR Apache-2.0, and are not part of what we take.

**Discipline:** every crate root has the deny header; 25 fuzz targets in `fuzz/` (an own workspace), including
`imap.rs`, `http.rs`, `ical.rs`, `vcard.rs`, `html.rs` and the `store/` ones. Tests are inline `#[cfg(test)]`
modules in most files (every candidate file below has one), which our `test-files` gate rejects, so vendoring
moves them out. Non-test `unwrap`/`expect` counts: `net` 554 (mostly inside those inline test modules and the fake
servers), `store` 38, `model` 55, `mime` 22, `engine` 46, `jmap` 7.

**wasm check:** `cargo check --locked --target wasm32-unknown-unknown -p mailcraft-store -p mailcraft-model`
finished in 1 m 09 s without warnings: the store's `redb`, `flate2`, `miniz_oxide` and `dirs` are behind
`cfg(not(target_arch = "wasm32"))` (`crates/store/Cargo.toml`) and `Store::memory` serves the browser
(`crates/store/src/lib.rs:17-19`).

### Candidate crates

| Crate | Files / lines / KiB | SPDX | Dependencies (crates.io licence, checked 2026-11-04) | Take | How | Leave |
|---|---|---|---|---|---|---|
| `crates/net` | 170 / 54,947 / 2,610 (native only) | 0 of 170 | `imap-proto` 0.17.0 (MIT OR Apache-2.0), `lettre` 0.11.23 (MIT), `rustls` 0.23 (Apache-2.0 OR ISC OR MIT), `webpki-roots`, `ring`, `quick-xml` 0.42.0 (MIT), `serde`, `serde_json`, `log`, `thiserror`, `iana-time-zone`, `rand_core`, `rustls-native-certs`, `fs4` 1.1.0, `sspi` 0.23.1, `md4`, `md-5`, `hmac`, `outlook-pst-rw`, `cfb`, and the workspace's `model`, `mime`, `store`, `jmap`, `smime`, `pgp`, `keys` | the IMAP and sync core and its test servers, by file: `src/protocols/imap.rs`, `imap/conn.rs`, `imap/text_first.rs` (1,493 lines: `Client<S: Read + Write>`, `imap-proto` parser, literal and line caps, ENABLE, CONDSTORE, QRESYNC `SELECT` with `VANISHED`, IDLE, UIDPLUS, MOVE, SPECIAL-USE, LIST-EXTENDED, QUOTA); `src/sync.rs` (598: RFC 4549 folder sync with CONDSTORE and QRESYNC, windowed first sync, UIDVALIDITY reset); `src/worker.rs` and `src/worker/sync.rs` (785 and 464: per-account job loop, IDLE wait with poll fallback); `src/oauth.rs` (445: RFC 8252 loopback, PKCE S256, refresh token kept in the keychain, XOAUTH2); `src/stream.rs` (285: rustls, timeouts, IPv6 then IPv4); `src/autoconfig.rs` and `src/dns.rs` (313 and 317: Mozilla autoconfig, SRV); `src/http.rs` (699: small HTTP/1.1 client for OAuth, autoconfig and DAV); `src/protocols/dav.rs` and `dav/syncing.rs` (1,363: RFC 6764 discovery, ctag and etag sync, multiget, CardDAV); `src/fakes/fake.rs`, `fake/structure.rs`, `fake/tests/*` (2,770: in-memory IMAP server with UIDPLUS, MOVE, CONDSTORE, QRESYNC `VANISHED`, IDLE, hostile-input tests) and `src/fakes/fake_dav.rs` with `scheduling.rs` (954) | vendor, reshaped: the `Client<S>` and parser side into our sans-I/O `proto` (merged with UCS's `proto`, which lacks exactly these extensions), the worker, stream, OAuth, autoconfig and DAV discovery into `net`, the fakes into `testkit`; the `mailcraft_model` and `mailcraft_store` types they touch (`ServerConfig`, `Folder`, `MessageKey`, `Summary`, `FolderState`) map onto ours | `src/data_files/*` (PST and MSG, about 45 files), `src/exchange/*` (EWS, Graph, NTLM, Kerberos), `protocols/jmap.rs`, `pop.rs`, `sieve.rs`, `web_calendar.rs`, `src/updates/*` (self-update), `worker/pgp`, `worker/smime`, `worker/imports`, `fakes/fake_ews*`, `fake_graph*`, `fake_jmap.rs`, `real_*`; with them the dependencies `outlook-pst-rw`, `cfb`, `sspi`, `md4`, `md-5`, `hmac`, `fs4`, `smime`, `pgp`, `keys`; `protocols/smtp.rs` is a `lettre` wrapper and loses to UCS's sans-I/O SMTP |
| `crates/store` | 55 / 21,211 / 950 | 0 of 55 | `mailcraft-model`, `serde`, `serde_json`, `log`, `thiserror`; native: `redb` =4.4.0 (MIT OR Apache-2.0), `flate2` 1.1.10 (MIT OR Apache-2.0), `miniz_oxide` 0.9.1 (MIT OR Zlib OR Apache-2.0), `dirs` 7.0.0 (MIT OR Apache-2.0, pulls `option-ext` MPL-2.0 which our `deny.toml` does not allow) | `src/atomic.rs` (215: write to a temporary name, fsync, rename), the `FolderState` shape (`src/lib.rs:62-80`: UIDVALIDITY, UIDNEXT, HIGHESTMODSEQ, window) and the `ops.json` offline replay idea (`src/bodies.rs:343-370`) | vendor those pieces into our `store`; the rest is a reference (section 3, question 2) | `src/packed/*` (redb data file, one table from path to bytes, compaction, health checks), `src/search/*` (its own on-disk inverted index, about 6,000 lines), `leases.rs`, `beside.rs`, `location.rs`, `bodies.rs` |
| `crates/model` | 56 / 16,310 / 707 | 0 of 56 | `serde`, `serde_json`, `unicode-segmentation`, `chrono`, `chrono-tz` 0.10.4 (MIT OR Apache-2.0), `sha2` | `src/job/op.rs` (the `Op` enum replayed when online), `src/utf7.rs` if UCS's `proto` one does not fit | vendor those two files | the rest: Outlook-shaped PIM, iCalendar and vCard inside `model` (UCS's separate crates fit our layers better), `chrono` where we use `jiff` |
| `crates/mime` | 38 / 13,342 / 598 | 0 of 38 | `mail-parser` 0.11 (1.0.1 current, Apache-2.0 OR MIT), `mail-builder` 1 (2.0.1 current, Apache-2.0 OR MIT), `html2text` 0.17.4 (MIT), `html5ever` 0.40, `flate2`, `memchr` | nothing | - | built on external parsers, with TNEF, office previews and S/MIME wiring; UCS's self-contained `mime` is the better base, and `mail-parser` stays a known alternative if that fails our bar |
| `crates/engine` (138 / 40,181), `crates/ui-egui` (113 / 36,011), `crates/jmap` (6 / 1,547), `crates/keys`, `crates/pgp`, `crates/smime`, `crates/print`, apps, `xtask` | - | 0 | - | nothing | - | Outlook-shaped; S/MIME and PGP are not in our roadmap before M3 |

## 3. MonkeySaint/mailcraft

**Commit:** `2ac4db88369abcd3d4e9dc025d5b9816c20b50b6` on `master`, 2026-10-30 22:35 -0400, 99 commits between
2026-10-09 and 2026-10-30, 840 tracked files, 30 MB checkout, workspace version 0.5.1, edition 2024, `rust-version`
1.95.

**Focus:** a reimplementation of classic Outlook: a 156,230-line `engine` with drawing, charts, SmartArt and
printing, SQLite storage with zstd pack files, EWS, Graph, JMAP, POP3 and IMAP.

**Licence:** `LICENSE-MIT` ("Copyright (c) 2026 the MailCraft contributors") and `LICENSE-APACHE`; every crate
has `license.workspace = true` with `MIT OR Apache-2.0`; none of the 550 `.rs` files carries an SPDX line. No other
licence notice in any source file. `NOTICE` lists the OFL fonts, egui's fallback fonts, and the two C libraries of
native builds, SQLite (public domain) and Zstandard (BSD-3-Clause), with their texts. Verdict: MIT OR Apache-2.0
for every file; nothing disqualifying.

**Discipline:** no deny header in any crate root (the workspace's clippy lints deny `unwrap_used`, `expect_used`
and `panic` instead, `Cargo.toml:131-141`); no `fuzz/` directory (`crates/engine/tests/fuzz.rs` only); tests
mostly in files of their own.

### What the spec named, and what is there

| Spec candidate | Finding | Take |
|---|---|---|
| "offline queue" | Two things: the Outbox due list and the queue of server changes in `crates/engine/src/sync/native.rs:682-760`, persisted as one JSON row per account in SQLite (`crates/store/src/sql/schema.rs:7-8`), and the "offline window" that evicts old bodies in `crates/engine/src/offline.rs`. Both are woven into the engine and its `Mailbox` type. | nothing; the offline-window setting (how far back bodies stay on disk) is a design idea for E1.6 |
| "zbus notifications" | There are no desktop notifications. `zbus` appears only in `apps/mailcraft/src/capture.rs` (the XDG desktop portal screenshot for Insert, Screenshot) and transitively through `keyring`; `org.freedesktop.Notifications` is not in the tree. | nothing |

### Candidate crates

| Crate | Files / lines / KiB | SPDX | Dependencies | Take | Leave |
|---|---|---|---|---|---|
| `crates/search` | 11 / 4,485 / 152 | 0 of 11 | `mailcraft-model` only; wasm-clean | nothing now. An in-memory inverted index with a query language (`from:`, `received:"last week"`, AND, OR, NOT, phrases), Unicode folding, ranking and highlights (`src/lib.rs:1-16`); no persistence. A fallback for the search part if FTS5 is refused and we do not want ZIDAN44's on-disk index | all, for now |
| `crates/store` | 27 / 11,358 / 452 | 0 of 27 | `rusqlite` 0.41.1 (MIT, bundled SQLite), `zstd` 0.14.3 (BSD-3-Clause), `sha2`, `crc32fast`; native only, in-memory on wasm | nothing; a reference for the SQLite backend: WAL with `synchronous=FULL`, `PRAGMA user_version` migrations, sources in append-only zstd packs de-duplicated by SHA-256, committed length per pack as the crash rule (`src/lib.rs:1-12`, `src/sql/schema.rs`) | all; coupled to its `model` and `mime` |
| `crates/mime` | 12 / 6,182 / 233 | 0 of 12 | `base64`, `encoding_rs`, `idna` 1.1; no model dependency | nothing now; a second source for RFC 5322, MIME, RFC 2047 and RFC 2231 if cutting UCS's `mime` from UCS's `model` proves costly | all, for now |
| `crates/net` | 44 / 21,611 / 880 | 0 of 44 | `rustls`, `rustls-native-certs`, `ring`, `keyring` 4.2.0 | nothing; its IMAP client has CONDSTORE, QRESYNC, IDLE, UIDPLUS and MOVE (`src/imap/client.rs:120`, `:1203`) and no NOTIFY or PREVIEW, and no fuzz target; ZIDAN44's is better tested | all |
| `crates/engine`, `ui-egui`, `html`, `model`, `formats`, `layout`, `render`, `proof`, `fonts`, `geom`, `smime`, `testmail`, `mcp`, apps | - | 0 | - | nothing | all |

## 4. The three questions

### 4.1 Does ZIDAN44's engine support IMAP NOTIFY and PREVIEW?

No to both.

- A grep for `NOTIFY` over `crates/net/src` and `crates/engine/src` finds only the desktop-notification
  preference (`crates/engine/src/session/prefs.rs:55`, `crates/engine/src/cmd/view.rs:108`) and the fake JMAP
  server's `notify()` (`crates/net/src/fakes/fake_jmap.rs:71`). RFC 5465 is not implemented; new mail is noticed
  by IDLE on the Inbox when the server has it (`crates/net/src/worker/sync.rs:13`,
  `crates/net/src/protocols/imap.rs:767`) and by polling otherwise.
- `PREVIEW` (RFC 8970) does not occur anywhere. Message-list previews come from fetching the text part first
  (`crates/net/src/protocols/imap/text_first.rs`).
- The capabilities the client reads are IMAP4rev1, STARTTLS, LOGINDISABLED, SASL-IR, AUTH=PLAIN, ENABLE,
  CONDSTORE, QRESYNC, UIDPLUS, MOVE, SPECIAL-USE, LIST-EXTENDED, QUOTA, UNSELECT (`crates/net/src/protocols/imap.rs`);
  no COMPRESS either. Its fake server announces `IMAP4rev1 UIDPLUS MOVE CONDSTORE ENABLE QUOTA`
  (`crates/net/src/fakes/fake.rs:73`), so our `testkit` fake needs NOTIFY and PREVIEW written new.

Consequence for E1.4: vendor ZIDAN44's CONDSTORE, QRESYNC and IDLE handling and write NOTIFY, PREVIEW and COMPRESS
ourselves from the RFCs.

### 4.2 Can ZIDAN44's `store` serve us, or do we build our own?

Build our own, taking a few pieces. ZIDAN44's `store` (`crates/store`, 21,211 lines) is a cache of IMAP folder
state with a bespoke search index:

- It is coupled to `mailcraft-model`: `AccountConfig`, `Folder`, `FolderRole`, `Flags`, `Op`, `Summary`,
  `search::IndexDoc`, the tokeniser and `TOKENIZER_VERSION` (`crates/store/src/lib.rs:38`, `src/search/*`). Taking
  it means taking that `model`.
- Its layout is per account and per folder (`accounts.json`, `folders.json`, `ops.json`, `state.json`, one `.eml`
  per message, `crates/store/src/lib.rs:6-12`), on disk or inside one redb data file per account with a single
  path-to-bytes table (`src/packed.rs:1-9`). There are no cross-account tables: the screener's per-address state,
  snooze and send-later times and grouped-inbox counts, which InboxCraft needs across accounts, have no home and
  would be bolted on.
- Search is its own on-disk inverted index (`src/search.rs:1-45`, about 6,000 lines with tests) kept in step by
  comparing index and mail; a second thing to maintain and fuzz.
- It does meet the wasm constraint (section 2, wasm check): the engine sits behind `cfg(not(target_arch =
  "wasm32"))` and a memory store serves the browser. That pattern, not the code, is what we keep.

What we take: `src/atomic.rs` (atomic file writes), the `FolderState` fields, and the `ops.json` replay idea.

### 4.3 Can ZIDAN44's OAuth2 do Google CalDAV?

Not as it is; the sign-in half is reusable, the CalDAV half is not.

- `crates/net/src/oauth.rs:44-56` registers Google with the single scope `https://mail.google.com/`; the flow is
  RFC 8252 with a loopback listener and PKCE S256 (`oauth.rs:3-9`, `:128-133`), the refresh token goes to the
  keychain and access tokens are fetched as needed. Microsoft gets Graph calendar scopes (`oauth.rs:111`); Google
  does not.
- The CalDAV client signs in with Basic only: `Dav { auth: format!("Basic ...") }` at
  `crates/net/src/protocols/dav.rs:262`, sent on every request (`dav.rs:285`, `:498`, `:513`). The comment at
  `dav.rs:342` says a refused sign-in while looking "(Google's servers want OAuth) means not here", so Google
  calendars are deliberately not found.
- Google's CalDAV guide (read 2026-11-04) requires HTTPS with OAuth 2.0 of a Google account, and gives the
  principal URL `https://apidata.googleusercontent.com/caldav/v2/<calendar id>/user`; a request to
  `https://apidata.googleusercontent.com/.well-known/caldav` answered 404 on 2026-11-04, so RFC 6764 discovery
  does not apply there. The Calendar API scopes page lists `https://www.googleapis.com/auth/calendar` (and
  read-only and events-only variants).

What E4 adds on top of the vendored code: a calendar scope on the Google provider, a bearer `Authorization` in the
DAV client, the Google principal URL as a provider-known starting point, and a token refresh path shared by IMAP,
SMTP and DAV. The owner's decision stands: Google calendar through CalDAV with OAuth first.

## 5. Store recommendation (Q-O2)

**Recommendation:** our own `crates/store` (L1) as a `Storage` trait with two backends: `Memory` (always built,
used on wasm and in tests) and `Sqlite` (native, behind a `native` feature, `rusqlite` with the bundled SQLite and
FTS5). Message sources stay out of the database, as files or packs, which E1.6 settles. Not ZIDAN44's `store`
(section 4.2), and not redb plus tantivy.

**The wasm constraint, checked:**

| Probe (Rust 1.98.0, `wasm32-unknown-unknown`, 2026-11-04) | Result |
|---|---|
| `redb` =4.4.0 alone | compiles (12.5 s); usable there only with its `InMemoryBackend` |
| `tantivy` =0.27.0, default features off, `getrandom` `wasm_js` backend on | fails: `uuid` 1.29.0 needs a `js` feature, and after that its thread pools would not run on this target |
| `rusqlite` =0.41.1 `bundled` | fails: `libsqlite3-sys` is not built for wasm32 |
| ZIDAN44 `mailcraft-store` and `mailcraft-model` | compile (1 m 09 s), because the engine is behind `cfg(not(target_arch = "wasm32"))` |
| UCS `html`, `mime`, `ical`, `vcard`, `time`, `charset`, `proto`, `dav`, `control-key` | compile (1 m 22 s) |

So neither pair compiles for wasm as a whole; whichever engine we choose sits behind the trait with the memory
backend on wasm, exactly the second form the constraint allows. That makes the choice a native one.

**Why SQLite with FTS5:**

1. One transaction for rows and the full-text index. redb and tantivy commit separately, so a crash between the
   two needs reconciliation code; ZIDAN44 wrote about 6,000 lines of its own index inside the redb file to avoid
   that (section 4.2).
2. FTS5 brings tokenising, prefix and phrase queries, column filters and BM25 ranking; `libsqlite3-sys` builds the
   bundled amalgamation (SQLite 3.54.1 per its `upgrade.sh`) with `-DSQLITE_ENABLE_FTS5` (its `build.rs:177`,
   read 2026-11-04).
3. The triage features are relational: screener decisions per address across accounts, snooze-until and send-later
   times, grouped-inbox counts, follow-up reminders. Indexed columns and small transactions fit; a key-value file
   makes us write the indexes.
4. Migrations with `PRAGMA user_version`, a `sqlite3` CLI for support cases, and a design already proven in
   MonkeySaint's store (WAL, `synchronous=FULL`, per-pack committed length as the crash rule).
5. Licences: `rusqlite` and `libsqlite3-sys` MIT, SQLite public domain, both on our `deny.toml` allow list.

**Cost:** one C library compiled by `cc` in the dev image and CI (no system SQLite needed; the amalgamation ships
in the crate, so Flatpak and offline builds work), and a NOTICE paragraph like MonkeySaint's. This is the one point
the owner should weigh against "Rust-native": the program stays Rust; one dependency is C.

**Fallback if the owner refuses C code:** `redb` (pure Rust, MIT OR Apache-2.0, compiles for wasm) for the rows,
with a full-text index of our own; vendoring ZIDAN44's `crates/store/src/search` (about 4,000 lines plus 2,000 of
tests, same licence) is the quickest route, at the cost of owning an index format. tantivy is not recommended in
either case: a second commit point, a thread pool and a large dependency tree for a mailbox-sized index.

## 6. Draft NOTICE and PROVENANCE.md rows

To be written into `NOTICE` and `PROVENANCE.md` by the slice that brings each part in, never earlier. Commits are
the pins above; "Changed?" is filled when the code lands.

### NOTICE paragraphs (under "Portions adapted from other projects")

```text
- MailCraft (https://github.com/UCS-Care-Group/mailcraft), Copyright (c) 2026 the MailCraft contributors,
  MIT OR Apache-2.0: the `html`, `link` and `names` crates (used as a pinned git dependency), and the base of
  `crates/mime`, `crates/charset`, `crates/time`, `crates/ical`, `crates/vcard`, `crates/proto`, `crates/dav`
  and `crates/control-key` (vendored, commit e92228cf05362ac76df40d103b929a8506257395). `crates/time`'s Windows
  zone names restate the "001" rows of windowsZones in the Unicode CLDR supplemental data, Copyright (c)
  Unicode, Inc., Unicode License v3 (https://www.unicode.org/license.txt).
- MailCraft (https://github.com/ZIDAN44/mailcraft), Copyright (c) 2026 ArtCraft Team and the MailCraft
  contributors, MIT OR Apache-2.0: the IMAP client and folder sync (CONDSTORE, QRESYNC, IDLE), the account
  worker, the OAuth 2.0 sign-in, the TLS stream, account autoconfiguration, CalDAV and CardDAV discovery and
  sync, the in-process IMAP and DAV test servers, atomic file writes and the offline change replay (vendored,
  commit 1517e3c6dbe5f4c2f39d28381a885ed3101d8171).
- WordCraft (https://github.com/storytold/wordcraft), Copyright (c) 2026 ArtCraft Team and the WordCraft
  contributors, MIT OR Apache-2.0: `crates/control-key` started there and reached us through
  UCS-Care-Group/mailcraft.
```

If the SQLite backend is accepted, a "Third-party material" line as well:

```text
- SQLite (public domain) through the rusqlite and libsqlite3-sys crates (MIT), compiled into native builds only;
  the browser build keeps its in-memory store.
```

### PROVENANCE.md rows

| Component (here) | Source repository | Commit (rev) | How | Changed? | Notes |
|---|---|---|---|---|---|
| `inboxcraft-html` (wrapper over `mailcraft-html`, `mailcraft-link`, `mailcraft-names`) | UCS-Care-Group/mailcraft `crates/html`, `crates/link`, `crates/names` | `e92228cf05362ac76df40d103b929a8506257395` | git dependency | no | `deny.toml` allow-git entry; fuzzed through the wrapper |
| `crates/mime` | UCS-Care-Group/mailcraft `crates/mime` | `e92228cf05362ac76df40d103b929a8506257395` | vendored | yes | model types mapped to ours; inline tests moved out; `unwrap` removed |
| `crates/charset` | UCS-Care-Group/mailcraft `crates/charset` | `e92228cf05362ac76df40d103b929a8506257395` | vendored | | |
| `crates/time` | UCS-Care-Group/mailcraft `crates/time` | `e92228cf05362ac76df40d103b929a8506257395` | vendored | | CLDR windowsZones names, see NOTICE |
| `crates/ical` | UCS-Care-Group/mailcraft `crates/ical` | `e92228cf05362ac76df40d103b929a8506257395` | vendored | yes | our `Event` and `Recurrence` types |
| `crates/vcard` | UCS-Care-Group/mailcraft `crates/vcard` | `e92228cf05362ac76df40d103b929a8506257395` | vendored | yes | our `Contact` type |
| `crates/proto` (IMAP4rev1 base, SMTP) | UCS-Care-Group/mailcraft `crates/proto` | `e92228cf05362ac76df40d103b929a8506257395` | vendored | yes | IDLE, ENABLE, CONDSTORE, QRESYNC, NOTIFY, PREVIEW, COMPRESS added |
| `crates/proto` (CONDSTORE, QRESYNC, IDLE handling) | ZIDAN44/mailcraft `crates/net/src/protocols/imap.rs`, `imap/conn.rs`, `imap/text_first.rs` | `1517e3c6dbe5f4c2f39d28381a885ed3101d8171` | vendored, reshaped | yes | made sans-I/O over the `proto` stream |
| `crates/dav` (XML reader, document editing) | UCS-Care-Group/mailcraft `crates/dav` | `e92228cf05362ac76df40d103b929a8506257395` | vendored | | |
| `crates/dav` (discovery, ctag and etag sync) | ZIDAN44/mailcraft `crates/net/src/protocols/dav.rs`, `dav/syncing.rs` | `1517e3c6dbe5f4c2f39d28381a885ed3101d8171` | vendored, reshaped | yes | bearer auth and Google principal URL added |
| `crates/net` (folder sync, worker, IDLE loop) | ZIDAN44/mailcraft `crates/net/src/sync.rs`, `worker.rs`, `worker/sync.rs` | `1517e3c6dbe5f4c2f39d28381a885ed3101d8171` | vendored | yes | |
| `crates/net` (OAuth 2.0 with PKCE) | ZIDAN44/mailcraft `crates/net/src/oauth.rs` | `1517e3c6dbe5f4c2f39d28381a885ed3101d8171` | vendored | yes | calendar scope added |
| `crates/net` (TLS stream, HTTP client, autoconfig, DNS) | ZIDAN44/mailcraft `crates/net/src/stream.rs`, `http.rs`, `autoconfig.rs`, `dns.rs` | `1517e3c6dbe5f4c2f39d28381a885ed3101d8171` | vendored | | |
| `crates/testkit` (fake IMAP and DAV servers) | ZIDAN44/mailcraft `crates/net/src/fakes/fake.rs`, `fake/structure.rs`, `fakes/fake_dav.rs`, `fake_dav/scheduling.rs` | `1517e3c6dbe5f4c2f39d28381a885ed3101d8171` | vendored | yes | NOTIFY, PREVIEW, fault scripts and certificate faults added |
| `crates/store` (atomic writes, `FolderState`, offline replay) | ZIDAN44/mailcraft `crates/store/src/atomic.rs`, `src/lib.rs` (`FolderState`), `crates/model/src/job/op.rs` | `1517e3c6dbe5f4c2f39d28381a885ed3101d8171` | vendored | yes | |
| `crates/control-key` | UCS-Care-Group/mailcraft `crates/control-key` (from storytold/wordcraft) | `e92228cf05362ac76df40d103b929a8506257395` | vendored | | |

The existing placeholder rows in `PROVENANCE.md` (`crates/html` as a git dependency, `crates/net` IMAP sync parts
from ZIDAN44, `xtask/src/layers.rs` from WordCraft) are consistent with this and become the rows above.

## 7. What the slices must do when the code comes in

- `deny.toml`: uncomment `https://github.com/UCS-Care-Group/mailcraft` under `allow-git`; keep MPL-2.0 off the
  allow list (do not take `dirs`).
- `Cargo.toml`: `mailcraft-html = { git = "https://github.com/UCS-Care-Group/mailcraft", rev =
  "e92228cf05362ac76df40d103b929a8506257395" }` and the same for `mailcraft-link` and `mailcraft-names` only if the
  wrapper needs them directly.
- Vendored files: our SPDX header plus the source's copyright line, deny header in the crate root, tests moved to
  their own files, `unwrap`, `expect` and `panic` removed, fuzz targets mirrored (`html`, `sheet`, `mime`,
  `charset`, `imap`, `smtp`, `ical`, `vcard`, `dav_xml`, `dav_document`, `http`), coverage floors met.
- Each PR that brings a part in adds its NOTICE paragraph and PROVENANCE.md row (definition of done, item 6).
- Nothing else from the three repositories: no sample mail, no icons, no theme values, no wording beyond feature
  names.

## Revision history
| Date | Change | Summary |
|---|---|---|
| 2026-11-04 | major | First version: three repositories at pinned commits, licence verdicts, per-crate tables, the three questions, the store recommendation, draft NOTICE and PROVENANCE rows (#4) |
