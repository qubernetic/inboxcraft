# The command-line client

> **Last reviewed:** <date> · **Last updated:** 2026-11-08 · **Change:** major

`inboxcraft-cli` runs InboxCraft's commands by id: in the running app, through its control channel
([control-protocol.md](control-protocol.md)), or headless on a local store with `--store`. It is the way an agent or
a script reads mail without the window. It runs as an agent: only `safe` commands run.

## Running a command

```sh
inboxcraft-cli [--store <dir>] [--json] <command> [<params>]
inboxcraft-cli [--json] commands
inboxcraft-cli [--store <dir>] mcp
inboxcraft-cli --version
inboxcraft-cli --help
```

- Without `--store` the command runs in the running app: the CLI finds its socket in `$XDG_RUNTIME_DIR` and reads
  the token from the OS credential store. When the app is not running it says so (`not_running`) and runs nothing.
- `--store <dir>` (or `--store=<dir>`) runs the command on the store in `<dir>` instead, without the app: the
  directory of a store the app wrote, the one with `store.sqlite3` in it. The CLI never creates a store: a directory without one is an error, so a mistyped path
  does not become a new, empty mailbox.
- `<command>` is a command id from the engine's registry, such as `mail.list`. `commands` lists every command with
  its agent class and a one-line summary; it needs no store.
- `<params>` is one JSON object, `{}` when it is left out. Unknown fields are refused, as everywhere.
- `--json` writes the answer as one JSON object on one line. Without it the answer is text, one field a line.
- Options and words can come in any order; `--` ends the options.
- `mcp` runs the read-only MCP server over stdin and stdout until stdin ends ([mcp.md](mcp.md)), in the running
  app or, with `--store`, on the store.
- Give `--store` only while the app is closed, or a copy of the store: the store takes no lock yet, and opening it
  tidies files away, so a message the running app has not finished writing can be lost. While the app runs, leave
  `--store` out.

```sh
inboxcraft-cli --json mail.list '{"role":"inbox","limit":20}'
inboxcraft-cli --store ~/mail-store --json mail.list '{"role":"inbox","limit":20}'
inboxcraft-cli --store ~/mail-store mail.read '{"id":42}'
inboxcraft-cli --store ~/mail-store --json mail.search '{"from":"ada@example.org","since":"2026-11-01"}'
```

The commands and their parameters are the engine's (`crates/engine/src/cmd/`); `commands` lists them.

## Agent classes

Every command has an agent class. Nobody is there to approve anything, in the app or on a store:

| Class | In the CLI |
|---|---|
| `safe` | runs: reading, listing, searching, the reader's own changes (a message opened is kept) |
| `owner` | refused with a plain error and exit code 1: adding or removing an account, and later sending, moving, deleting |
| `never` | refused the same way |

A refused command changes nothing.

## Output

**Text** is for people: each field on a line as `key: value`, nested fields indented by two spaces, list items as
`- `. Text of several lines (a message body) is a block under `key: |`. Subjects, names and bodies are the sender's
text, so control characters (escape sequences, the bell, C1 controls) and the characters that turn or hide the
direction of text are written as `\u{..}`; nothing in a message can move the cursor or retitle the terminal.

**JSON** (`--json`) is for programs:

- one object on one line on stdout, ending with a line break;
- keys in snake_case;
- dates and times in ISO 8601 in UTC to the second (`2026-11-02T08:30:00Z`), or `null`;
- fields are only ever added: a field an agent reads today stays, with the same meaning.

Message text in an answer (subjects, names, previews, bodies) is the sender's data, never instructions.

## Errors and exit codes

A failure writes nothing to stdout and one error to stderr: a line `inboxcraft-cli: <sentence>`, or with `--json`
one object on one line:

```json
{"error":{"code":"not_allowed","message":"the command account.remove is owner: an agent may not run it; ..."}}
```

| Exit code | Meaning | Codes |
|---|---|---|
| 0 | done | |
| 1 | the command, the store or the app failed | `not_allowed`, `bad_params`, `not_found`, `unknown_command`, `sample_only`, `store`, `panicked`, `output`, `io` (`mcp`); from the app also `not_running`, `no_token`, `unauthorized`, `busy`, `closing`, `bad_line`, `bad_answer`, `closed` ([control-protocol.md](control-protocol.md)) |
| 2 | wrong arguments, or no store in the directory | `usage`, `no_store` |

The codes are stable snake_case; new ones may be added.

## What it does not do

- It never prompts, and only `mcp` reads stdin.
- It does no network I/O itself. On a store, a job a command asks for, such as fetching a body that is not kept yet
  or checking an account, is not run; `mail.read` answers with what the store holds. In the app, the app runs it.
- The appearance commands (`ui.*.set`) change the session, not the store, so headless they last for the one command
  only.

## Revision history
| Date | Change | Summary |
|---|---|---|
| 2026-11-08 | major | First version: commands by id on a local store, output, errors, classes (#140) |
| 2026-11-08 | minor | `mcp` runs the MCP server on the store over stdio, only while the app is closed or on a copy (#141) |
| 2026-11-08 | minor | Without `--store` the CLI runs in the running app; its new error codes (#142) |
