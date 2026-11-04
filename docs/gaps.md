# Where InboxCraft falls short

> **Last reviewed:** <date> · **Last updated:** 2026-11-04 · **Change:** minor

## Ranked
| # | Gap | B | Evidence | Impact | Hours | Doc |
|---|---|---|---|---|---|---|
| 1 | <gap> | ✓ | <how we know> | <who it hurts> | <h> | |

B = blocks the next stage.

## Known rough edges (not ranked)
- Every crate is an empty skeleton; the apps answer `--version` and refuse everything else (#5).
- `cargo xtask ci` does not run the `layers`, `wasm`, `assets` and coverage gates yet, and `just layers` and
  `just wasm` fail until those commands exist (#5).
- `cargo xtask test-files` reads lines, not a syntax tree: a module line inside a multi-line string counts as a
  module (#5).

## Revision history
| Date | Change | Summary |
|---|---|---|
| <date> | major | First version |
| 2026-11-04 | minor | Rough edges of the workspace skeleton (#5) |
