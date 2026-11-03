# Provenance of reused code

InboxCraft reuses MIT/Apache code from other projects. Every reused component has a row here, and the matching
paragraph in [NOTICE](NOTICE). GPL/LGPL/AGPL code is never reused.

| Component (here) | Source repository | Commit (rev) | How | Changed? | Notes |
|---|---|---|---|---|---|
| `crates/html` | UCS-Care-Group/mailcraft `crates/html` | `<40-char sha>` | git dependency / vendored | <yes: …/no> | |
| `crates/net` (IMAP sync parts) | ZIDAN44/mailcraft `crates/net` | `<sha>` | vendored | <…> | |
| `xtask/src/layers.rs` | storytold/wordcraft | `<sha>` | adapted | yes | table for InboxCraft crates |

## Revision history
| Date | Change | Summary |
|---|---|---|
| <date> | major | First version |
