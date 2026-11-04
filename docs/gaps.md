# Where InboxCraft falls short

> **Last reviewed:** <date> · **Last updated:** 2026-11-04 · **Change:** minor

## Ranked
| # | Gap | B | Evidence | Impact | Hours | Doc |
|---|---|---|---|---|---|---|
| 1 | <gap> | ✓ | <how we know> | <who it hurts> | <h> | |

B = blocks the next stage.

## Known rough edges (not ranked)
- Every crate is an empty skeleton; the apps answer `--version` and refuse everything else (#5).
- `cargo xtask ci` does not run the coverage gate yet (#5).
- The `justfile` has no `assets` and `stats` recipes; they run as part of `just ci` (#6).
- `cargo xtask layers` checks direct dependencies only, and knows network and UI crates by name from two lists in
  `xtask/src/layers.rs`; a crate missing from them is not caught (#6).
- `cargo xtask stats` counts files and lines; the number of commands in the registry comes once the registry
  exists (#6).
- `cargo xtask test-files` reads lines, not a syntax tree: a module line inside a multi-line string counts as a
  module (#5).

## Revision history
| Date | Change | Summary |
|---|---|---|
| <date> | major | First version |
| 2026-11-04 | minor | Rough edges of the workspace skeleton (#5) |
| 2026-11-04 | minor | Rough edges of the `layers`, `wasm`, `assets` and `stats` gates (#6) |
