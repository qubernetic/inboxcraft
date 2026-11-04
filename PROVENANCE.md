# Provenance of reused code

InboxCraft reuses MIT/Apache code from other projects. Every reused component has a row here, and the matching
paragraph in [NOTICE](NOTICE). GPL/LGPL/AGPL code is never reused.

| Component (here) | Source repository | Commit (rev) | How | Changed? | Notes |
|---|---|---|---|---|---|
| `crates/html` | UCS-Care-Group/mailcraft `crates/html` | `<40-char sha>` | git dependency / vendored | <yes: …/no> | |
| `crates/mime/src/header.rs`, `params.rs`, `transfer.rs`, `part.rs` (header splitting and unfolding, RFC 2231 parameters, base64 and quoted-printable decoding, multipart splitting) | UCS-Care-Group/mailcraft `crates/mime/src/header.rs`, `transfer.rs`, `parse.rs` | `e92228cf05362ac76df40d103b929a8506257395` | adapted | yes: errors instead of silent skipping (field count and length, boundaries, nesting, part count), strict base64, our own types and `charset` crate, no writer | Each adapted file carries a second `SPDX-FileCopyrightText: MailCraft contributors` line (#18) |
| `crates/net` (IMAP sync parts) | ZIDAN44/mailcraft `crates/net` | `<sha>` | vendored | <…> | |
| `xtask/src/layers.rs` | storytold/wordcraft | `<sha>` | adapted | yes | table for InboxCraft crates |

## Revision history
| Date | Change | Summary |
|---|---|---|
| <date> | major | First version |
| 2026-11-04 | minor | The message reader adapted from UCS-Care-Group/mailcraft `mime` (#18) |
