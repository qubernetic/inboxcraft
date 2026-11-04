# Provenance of reused code

InboxCraft reuses MIT/Apache code from other projects. Every reused component has a row here, and the matching
paragraph in [NOTICE](NOTICE). GPL/LGPL/AGPL code is never reused.

| Component (here) | Source repository | Commit (rev) | How | Changed? | Notes |
|---|---|---|---|---|---|
| `crates/html` | UCS-Care-Group/mailcraft `crates/html` | `<40-char sha>` | git dependency / vendored | <yes: …/no> | |
| `crates/mime/src/header.rs`, `params.rs`, `transfer.rs`, `part.rs` (header splitting and unfolding, RFC 2231 parameters, base64 and quoted-printable decoding, multipart splitting) | UCS-Care-Group/mailcraft `crates/mime/src/header.rs`, `transfer.rs`, `parse.rs` | `e92228cf05362ac76df40d103b929a8506257395` | adapted | yes: errors instead of silent skipping (field count and length, boundaries, nesting, part count), strict base64, our own types and `charset` crate, no writer | Each adapted file carries a second `SPDX-FileCopyrightText: MailCraft contributors` line (#18) |
| `crates/proto/src/imap/value.rs`, `utf7.rs`, `text.rs`, `command.rs`, `frame.rs`, `list.rs`, `fetch.rs` (response values and literal markers, status lines, modified UTF-7, tidying server text and hiding secrets, quoting command arguments, the line and literal caps, reading LIST and FETCH responses) | UCS-Care-Group/mailcraft `crates/proto/src/imap/parse.rs`, `imap/utf7.rs`, `imap/mod.rs`, `lib.rs`, `line.rs` | `e92228cf05362ac76df40d103b929a8506257395` | adapted | yes: a sans-I/O state machine in place of the blocking `Read + Write` client, framing of bytes handed in, our own error type and `model` types (`Uid`, `UidSet`, `SpecialUse`), literals sent as chunks after each go-ahead, nesting errors typed; `utf7.rs` taken as it is, its tests moved to a file of their own | Each adapted file carries a second `SPDX-FileCopyrightText: MailCraft contributors` line (#20) |
| `crates/net` (IMAP sync parts) | ZIDAN44/mailcraft `crates/net` | `<sha>` | vendored | <…> | |
| `xtask/src/layers.rs` | storytold/wordcraft | `<sha>` | adapted | yes | table for InboxCraft crates |

## Revision history
| Date | Change | Summary |
|---|---|---|
| <date> | major | First version |
| 2026-11-04 | minor | The message reader adapted from UCS-Care-Group/mailcraft `mime` (#18) |
| 2026-11-04 | minor | The IMAP client core adapted from UCS-Care-Group/mailcraft `proto` (#20) |
