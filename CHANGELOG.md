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
  names and media types that do not fit their grammar are refused (`MimeError::BadHeader`). What the reader could
  not read back (a field over 256 KiB, a message over 128 MiB) is refused too; build-then-parse property tests (#19)
