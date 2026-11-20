# The control channel

> **Last reviewed:** <date> · **Last updated:** 2026-11-08 · **Change:** major

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

The app opens the channel only in a runtime folder of the user's that nobody else may write (group or others may not
have `w`). The socket's folder must be a real folder, not a link, and the user's own; it is made `0700` (narrowed
when it is wider) and the socket `0600`, whatever the umask: only the owner's own processes reach it. A socket a run
that ended left behind is replaced; one another app still answers on is not, and that app goes on without a channel. The socket
goes when the app ends. `--sample` has no channel.

A client tries the sockets in this order and takes the first that answers, but only one it can trust with the token:
the runtime folder and the socket's folder are real folders, the user's own, mode `0700` (nobody else may enter);
the socket is a socket and the user's own. Anything else is `not_running`, and the client writes nothing to it, so a
socket another account put in a shared folder (a hand-set `XDG_RUNTIME_DIR=/tmp/...`, say) never sees the token. The
user is the owner of `/proc/self`, so the channel needs Linux. Without `$XDG_RUNTIME_DIR`, or without a credential store, the app
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
| `unauthorized` | the token is missing, not 64 hex digits, wrong or revoked, or the credential store holds none; a line without a token is refused before the credential store is read | the connection is closed |
| `bad_line` | the line is not UTF-8, not one JSON object, names no command (or one over 128 bytes), or its `params` are not an object | the connection goes on |
| `too_long` | the line is over 1 MiB | the connection is closed |
| `busy` | eight clients are connected already, or the window did not answer within 30 seconds | closed for the ninth client; goes on after a late answer, and the late call may still run, so a client that tries again can run it twice |
| `closing` | the app is ending | the connection is closed |
| an engine code | the command was refused or failed: `not_allowed`, `bad_params`, `not_found`, `unknown_command`, ... | the connection goes on |

A client that does not finish a line within five minutes of the last answer (or of connecting) is let go, however
slowly it trickles its bytes; one that does not take its answer within 30 seconds is let go too. When accepting a
connection fails (no file handles left, say), the app waits 50 ms before the next; after 100 failures in a row it
closes the channel until its next start. A refusal never repeats the line it refuses. The CLI adds its
own codes: `not_running` (no socket it trusts answers, or no `$XDG_RUNTIME_DIR`), `no_token` (none in the credential store),
`bad_answer` (a line that is no answer) and `closed` (the connection failed); it waits a minute for an answer.

## How the app runs a request

The socket has a thread of its own and one thread a client. A request with the right token goes to the window as a
call; the window runs it before its next frame, or, while InboxCraft runs without a window, at its next look for the
workers' events, at most half a second later. It runs as an agent: only a `safe` command runs, and its jobs (fetching
a body, checking an account) go to the workers as the window's own do. The list is read again afterwards. An act of the
agent's (marking read, flagging) ends the window's Undo for the acts before it, so the window never takes the agent's
act back in place of the owner's.

Every `safe` command runs over the channel; `inboxcraft-cli commands` lists them with their class. Besides reading,
that is:

- the reader's own changes: read and unread (`mail.open`, `mail.seen`), flags, and `history.undo`, which takes back
  only an act an agent may make (`not_allowed` otherwise) and, while the owner has an event open in the window's
  editor, nothing: it is refused with `editing`, "an event is being edited", until the editor is saved, cancelled or
  left, or the window closes. The owner's own Undo is not held back;
- drafts: `compose.new`, `compose.update`, `compose.reply`, `compose.reply_all`, `compose.forward`,
  `compose.template` and `compose.discard`, which deletes a draft here and on the server;
- `account.check`, which connects to the account's servers and signs in to them;
- the appearance (`ui.*.set`, but not `ui.layout.set`, which is the owner's and refused with `not_allowed`) and, in
  the sample mailbox, `sample.mail.set`; `ui.motion.set` takes `level` (`full`, `reduced` or `off`), and for one
  release still the older `reduce` (`on`, `off` or `system`, which hands the choice back to the computer); its answer
  has `motion_level` and `motion_chosen` beside the older `motion`. Every answer's appearance has `layout`, where the
  reader (`docked` or `window`) and the composer (`docked`, `floating` or `window`) show, and where the event editor
  opens (`event_editor`: `small`, `full`, the default, or `window`);
- `ui.settings.open`, which shows the settings in the window: on the page its optional `page` names (`appearance`,
  `accounts`, `notifications`, `triage`, `sending`, `calendar`, `shortcuts`, `privacy` or `about`), also while they
  show; without `page`, or with one that names none (another case, a planned page, junk), on the page last shown.
  An agent's call opens the settings and never takes the owner off them, and it is passed over while the owner has
  typed into the account form or a key is being given on the Shortcuts page; at most its newest call acts in a frame,
  and the owner's own key or press in that frame wins. The answer is the page asked for (or `null`) and `window`,
  `true` in the app; it is the same whether the window acted or passed over the call;
- snooze, park and pin, as they land.

`sync.status` reads how syncing stands in the app, from what its workers told: the state, the folder syncs done of
all in the rounds under way, the last sync and the number of failing accounts. The state is the first that holds:

- `failing` when an account is stopped until the owner acts: a refused or unanswered sign-in, no password, no login
  method, a certificate or encryption refused, whether or not any account is connected;
- `offline` when no account is signed in (connecting counts as not yet), and the failures are of the network or
  the server: no route, a timeout, a refused or dropped connection;
- `failing` when a signed-in account's connection or a job (not a body fetch) failed, until the account signs in
  again after being offline or its worker tells that it is current: a round that ended with nothing failed in it,
  or a resync of the watched folder with nothing failed since the resync before it;
- `syncing` while a round has folders left, or a signed-in account has not ended a round yet; a round's folders
  stop counting when its connection goes offline, and count again at the next sign-in;
- `synced` otherwise, and always with no account.

Everything a client sends is hostile. `cargo +nightly fuzz run control_lines` runs the fuzz target on both lines.

## What it does not do

- It works on Linux only. On the other Unix systems the user id is not known without a crate the channel does not
  add, so the app runs without a channel and the CLI trusts no socket; on Windows it is not built. `--store` works
  everywhere.
- The client does not check who is listening once it is connected (the standard library cannot read the peer's user
  id yet); the owner and mode of the folders stand in for that check.
- It does not check the client's process: any process of the owner's that can read the credential store can use it.
- It has no approvals: an `owner` command (sending, moving, settings) cannot be asked for over the channel.

## Revision history
| Date | Change | Summary |
|---|---|---|
| 2026-11-08 | patch | `account.check` signs in to the servers (#119) |
| 2026-11-08 | patch | The reply, forward and template commands over the channel (#120) |
| 2026-11-08 | major | First version: the socket, the token and its revocation, the lines and their codes (#142) |
