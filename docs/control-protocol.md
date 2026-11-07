# The control channel

> **Last reviewed:** <date> · **Last updated:** 2026-11-07 · **Change:** major

The running app takes commands from the CLI and the MCP server through a local socket: `inboxcraft-cli mail.list`
without `--store` runs in the app ([cli.md](cli.md)), and so do the tools of `inboxcraft-cli mcp` ([mcp.md](mcp.md)).
Every request carries a token from the OS credential store. Only `safe` commands run; an `owner` or `never` command is
refused, as on a store. The channel is the crate `inboxcraft-control-key` (the token and the lines, shared by both
ends), `apps/inboxcraft/src/control.rs` (the server) and `apps/inboxcraft-cli/src/remote.rs` (the client).

## The socket

The app on the owner's mailbox listens on a Unix socket in the user's runtime folder (`$XDG_RUNTIME_DIR`):

| App | Socket |
|---|---|
| a build of its own | `$XDG_RUNTIME_DIR/inboxcraft/control.sock` |
| the Flatpak | `$XDG_RUNTIME_DIR/app/io.github.qubernetic.inboxcraft/control.sock` (the folder Flatpak shares with the host) |

A client tries them in this order and takes the first that answers. The folder is made `0700` (narrowed when it is
wider) and the socket `0600`, whatever the umask: only the owner's own processes reach it. A socket a run that ended
left behind is replaced; one another app still answers on is not, and that app goes on without a channel. The socket
goes when the app ends. `--sample` has no channel. Without `$XDG_RUNTIME_DIR`, or without a credential store, the app
runs without one and says why on stderr.

## The token

32 bytes from the operating system's random source (`/dev/urandom`), written as 64 lowercase hex digits. The app keeps
it in the OS credential store (Secret Service, Keychain, Credential Manager) under the service
`io.github.qubernetic.inboxcraft` and the user name `control-token`, beside the account passwords; it makes one at
its start when there is none. A client reads it from there. It is never written to a file, a log or an answer, and
`Debug` output shows `Token(..)`. The app compares it in constant time.

```sh
inboxcraft --revoke-control
```

puts a new token in the place of the old one. The app reads the credential store for every line, so a client that
kept the old token is refused from its next line on, on a connection that is already open too. The CLI reads the
token afresh each time it starts, so it goes on working.

## The lines

One JSON object a line each way, UTF-8, the line ended by `\n` (`\r\n` is read too). A client writes:

```json
{"token":"<64 hex digits>","command":"mail.list","params":{"limit":5}}
```

`params` is one JSON object and may be left out. The app answers each line with one line:

```json
{"ok":{"messages":[...],"total":2}}
{"error":{"code":"not_allowed","message":"the command mail.archive is owner: an agent may not run it; ..."}}
```

`ok` holds the command's answer, as `inboxcraft-cli --json` writes it. A connection carries any number of lines, one
answer each, in order; the CLI opens one for each command.

| Code | When | Then |
|---|---|---|
| `unauthorized` | the token is missing, not 64 hex digits, wrong or revoked, or the credential store holds none | the connection is closed |
| `bad_line` | the line is not UTF-8, not one JSON object, names no command (or one over 128 bytes), or its `params` are not an object | the connection goes on |
| `too_long` | the line is over 1 MiB | the connection is closed |
| `busy` | eight clients are connected already, or the window did not answer within 30 seconds | closed for the ninth client; goes on after a late answer, and the late call may still run, so a client that tries again can run it twice |
| `closing` | the app is ending | the connection is closed |
| an engine code | the command was refused or failed: `not_allowed`, `bad_params`, `not_found`, `unknown_command`, ... | the connection goes on |

A client that says nothing for five minutes is let go. A refusal never repeats the line it refuses. The CLI adds its
own codes: `not_running` (no socket answers, or no `$XDG_RUNTIME_DIR`), `no_token` (none in the credential store),
`bad_answer` (a line that is no answer) and `closed` (the connection failed); it waits a minute for an answer.

## How the app runs a request

The socket has a thread of its own and one thread a client. A request with the right token goes to the window as a
call; the window runs it before its next frame, or, while InboxCraft runs without a window, at its next look for the
workers' events, at most half a second later. It runs as an agent: only a `safe` command runs, and its jobs (fetching
a body, checking an account) go to the workers as the window's own do. The list is read again afterwards. An act of the
agent's (marking read, flagging) ends the window's Undo for the acts before it, so the window never takes the agent's
act back in place of the owner's.

Everything a client sends is hostile. `cargo +nightly fuzz run control_lines` runs the fuzz target on both lines.

## What it does not do

- It is built on Linux and the other Unix systems only. On Windows the app runs without a channel and the CLI finds
  no app to talk to; `--store` works there.
- It does not check the client's process: any process of the owner's that can read the credential store can use it.
- It has no approvals: an `owner` command (sending, moving, settings) cannot be asked for over the channel.

## Revision history
| Date | Change | Summary |
|---|---|---|
