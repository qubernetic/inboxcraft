# Privacy

> **Last reviewed:** <date> · **Last updated:** 2026-11-07 · **Change:** minor

## What the InboxCraft project collects
Nothing. InboxCraft has no telemetry, no analytics and no crash reporting, and there is no InboxCraft server.

## Where your data is
Your mail, calendars and settings are kept on your computer, in <settings/data paths per OS>. Passwords and tokens
are kept by the operating system's credential store (Secret Service, Keychain or Credential Manager), never in
InboxCraft's files. Deleting InboxCraft's data folder removes everything it stored there, but not what is in the
credential store.

What InboxCraft keeps is in one folder, `inboxcraft/store` in your data folder: `%LOCALAPPDATA%` on Windows, else
`$XDG_DATA_HOME` (in the Flatpak `~/.var/app/io.github.qubernetic.inboxcraft/data`), else `~/.local/share` on Linux
and `~/Library/Application Support` on macOS. Only your own user account can read it. It holds:
- your accounts' settings (with your templates), their folders and messages, the message bodies it keeps, and the
  work it still has to do on the server. A body is kept once you open the message, and the body of a delivery
  report from a mail system (`MAILER-DAEMON` or `postmaster`) as soon as it arrives in a folder other than the
  trash, junk or sent mail, to see which message it is about; a report from any other sender waits for the screener
  and for you to open it. A report may return your whole message (`message/rfc822` or `message/global`): then the
  kept body holds a full copy of what you sent, attachments included. The search reads only the report's own text,
  not the message it returns;
- your drafts, with everything you typed into them: the recipients (Bcc included), the subject and the text, and
  where each one stands in the outbox, with the server's reply if it refused one;
- the schedule: what is to happen at a set time (a draft to send later, a snoozed message to bring back, a
  follow-up reminder, a calendar alert);
- what InboxCraft knows of each sender address, across your accounts: that you know it, the screener's decision
  (accepted or blocked) and its group;
- each message's own state: done, parked, pinned, snoozed or waiting for a reply, and for a message you sent that
  came back, which delivery report said so and why (the To and Cc addresses it failed for and the remote server's
  reply). That mark stays with the Sent copy after the report is deleted and cannot be cleared on its own; it goes
  when the Sent message or its account is removed from InboxCraft;
- your calendars, with their events as the server sent them (iCalendar text) and their alerts.

A draft also has a copy on your mail server, in the account's own Drafts folder, once it has been still for ten
seconds:
- the copy carries the From line with your display name, To and Cc as typed, the subject, the text with the
  signature, and the time it was saved as its `Date`; it leaves out Bcc and the recipients that are not addresses;
- a reply's copy names the message it answers (`In-Reply-To`, `References`), as the reply itself will, and a
  forward's copy carries the forwarded message's attachments;
- where the server names the copy (UIDPLUS), each new copy replaces the one before and discarding the draft takes it
  back; where it does not, the draft's first copy stays there, no later one goes up, and you remove it with your
  mail program or the server's web mail.

A snoozed message is kept on your mail server too, in the account's own `InboxCraft/Snoozed` folder, so another
InboxCraft on the same account brings it back at the same time:
- InboxCraft writes a new copy of the message there, the same message with one line on top,
  `InboxCraft-Snoozed-Until`, saying when it comes back (in UTC); once the server has the copy, the old one is
  deleted. A line an earlier snooze left is replaced, so the copy says one time only;
- the line stays in the copy for good: when the message comes back at its time, when you bring it back early, and
  when you undo the snooze. Another mail program can see that you snoozed it and until when, and so can anyone you
  later send the whole message to (forwarding it as an attachment, for example);
- the server records the copy as received when it was snoozed, so a mail program that sorts by arrival lists it as
  new at that time;
- a message whose text InboxCraft has not downloaded yet is moved to the folder as it is, without the line. So is
  every message on a server that cannot delete one chosen message (no UIDPLUS), and any message snoozed before
  InboxCraft has signed in to the account since it started: there a new copy could not take the old one's place.

Beside the store, `inboxcraft/instance.lock` and `inboxcraft/instance.sock` keep a second start of InboxCraft from
opening the same mail: the first is empty, the second is a socket through which a later start asks the running one to show its window or to quit. Only your user account can open either of them or the `inboxcraft` folder they are
in. Neither holds anything of yours. Starting at login
(`inboxcraft --autostart`) writes `autostart/io.github.qubernetic.inboxcraft.desktop` in your config folder, which
names the program and nothing else; in the Flatpak the desktop's Background portal keeps the real entry.

When new mail comes in, InboxCraft tells you with a notification on your desktop. It hands your desktop's
notification service (in the Flatpak, its notification portal), which runs on your computer, the following:
- for new mail and for snoozed mail that comes back: the sender's name, or their address when the mail gives no name,
  and the subject;
- for a follow-up reminder: the name or address of the first person you wrote to, and the subject;
- for more than three new messages of one account at once: the number of new messages and the account's address.

Your desktop may keep notifications in its list until you clear it, and its own settings for InboxCraft decide
whether they show on the lock screen. You can choose per account to be told of all new mail, of the people you know,
or of nothing, and whether snoozed mail that comes back and follow-up reminders tell you (`notify.set`, the
Notifications page of the settings). The window keeps these choices in `notify.json` beside `appearance.json` in your
config folder; the file names each account by its number only, never by its address. The sample window keeps none.
Whether closing the window leaves InboxCraft running is kept in `background.json` beside them, and nothing else is.

Removing an account ("Change settings") removes its folders, messages, drafts, calendars, schedule entries and the
work still queued for it; the draft copies already in the account's Drafts folder on the server stay. What
InboxCraft knows of senders is kept, because it is yours and not an account's: it stays until you delete the data
folder, which removes everything in the store above.

An account's password is one entry in the credential store: service `io.github.qubernetic.inboxcraft`, user name
`account-<store id>-<number>`, where the store id is a random number the store got when it was made, so two copies
of InboxCraft (the Flatpak and another build, say) never share or overwrite each other's passwords. InboxCraft keeps
your accounts from one start to the next, so the entry stays when you close the window; it removes it when you
remove the account ("Change settings"). If InboxCraft crashes, is killed, or the credential store refuses at that
moment, an entry can stay behind. You can see and remove it with your system's own tool: "Passwords and Keys"
(Seahorse) or `secret-tool clear service io.github.qubernetic.inboxcraft` on Linux, Keychain Access on macOS,
Credential Manager on Windows.

## What InboxCraft connects to, and when
- Your own mail and calendar servers, with the settings you gave (IMAP, SMTP, CalDAV).
- When you use "Try to detect" (see below): your domain's DNS records and autoconfig address, and Mozilla's ISPDB
  only if you switch it on.
- When you check an account: the account's own IMAP server, as for reading mail (see below), its SMTP server and its
  CalDAV server, which get the same user name and password, only after the IMAP server took them.
- When your calendars are found and synced (each time InboxCraft starts, and when you or an agent ask for it): the
  account's own CalDAV server, at the address you gave it, over HTTPS only (an address that is not `https` gets
  nothing). Every request carries your user name and password (HTTP Basic authentication) over the encrypted
  connection, after the server's certificate verified. A redirect to another server is not followed, as the password
  would go with it. The server gets the events you write or remove and is asked for the ones that changed since the
  last sync. An event you delete goes to the server six seconds later, so Undo can still take it back before
  anything leaves; an event you write with guests carries their addresses and yours (as its organiser) in the text the
  server keeps, but InboxCraft does not mail them yet. Every organiser and guest it writes carries
  `SCHEDULE-AGENT=CLIENT`, which tells a server that mails guests on its own (RFC 6638) to leave that to InboxCraft;
  a server that ignores it could still mail them. The address takes no query or fragment, so nothing typed there is
  repeated. When you clear the address or move it to another server, the calendars and events kept from the old one
  are removed from this computer, every write to it still in the queue is dropped, the account's connection starts
  afresh with the new address, and an answer that still comes from the old server is ignored.
- When you send a message: the account's own SMTP server, over TLS only (from the first byte on port 465, or after
  STARTTLS on 587, refused when the server does not offer it). The password goes only over the encrypted connection.
  The server is greeted as `[127.0.0.1]`, which names nothing of your computer. The message goes to every address in
  To, Cc and Bcc; Bcc is never written into the message, so the other recipients do not see it. The message has a
  `Message-ID` made of the time and a number at your address's domain, and a `Date` in your computer's time zone. A
  copy goes to the account's Sent folder.
- Pictures in a message, only when you ask to show them (see "Remote pictures" below).
- <Update checks: GitHub Releases, if switched on.>

## Detecting settings
Setting up an account by hand is the default. "Try to detect" looks your settings up from your address and shows you
what it found; it asks, in this order, and stops at the first answer that names a mail server:

1. Your name servers (from your system's resolver configuration), for the MX records of your address's domain
   (the part after the `@`).
2. Your domain's web server, over HTTPS, for its autoconfig file: `autoconfig.<domain>` and
   `<domain>/.well-known/autoconfig/`. Plain HTTP is asked once, at `autoconfig.<domain>`, and only a redirect to
   HTTPS on your mail provider's domain is followed; what it finds is marked as reached through an unencrypted
   redirect, as anyone on the network could have sent that redirect.
3. Mozilla's ISPDB (`autoconfig.thunderbird.net`), for your domain and your mail provider's domain. **Off unless
   you switch it on**: it tells Mozilla which domain you are setting up.
4. Your name servers again, for SRV records (`_imaps._tcp.<domain>` and the like).
5. Nothing more: common host names such as `imap.<domain>` are offered as a guess for you to confirm.

These requests carry the domain in the name or the URL and nothing else: no address, no user name, no password, no
`User-Agent`, no cookies, and no proxy. The proposal lists every DNS query and URL that was asked; each URL also
means a name lookup of its host by your system. Nothing detected gets your
password before you have seen the settings and the server's certificate has verified.

## Checking an account
The account check connects to the account's IMAP server with the saved settings, step by step, to tell you why a
connection fails: the name, the connection, TLS, the certificate, the sign-in. When the mail port does not answer,
it also tries to open (and closes at once, sending nothing) port 443 of the same address and, when that connects, the
other mail ports (993, 465, 587), to tell a network that blocks mail from a server that is down. The password goes out only after the
certificate verified, as when reading mail. After the server refused the password or never answered the sign-in, a
check an agent asks for stops before the sign-in and says it needs you; only a check you start sends the password
again, so an agent cannot get the account locked at your provider by checking over and over. The SMTP server is held
the same way on its own, after it refused the password in a check or when you sent a message, and so is the CalDAV
server, after it refused the password: nothing more goes to it until a check you start finds your calendars again.

## Remote pictures
A message can name pictures on someone else's server. InboxCraft shows none of them until you ask: the reader shows
a banner saying how many are blocked, with "Load once" (this message, this time) and "Always for <address>".

- **When you allow them**, each picture is asked for at that moment, so its server learns that the message was
  opened and when, and from which network address. A sender can make each picture address unique to you.
- **What the request carries**: the picture's address and nothing else of yours. It goes over HTTPS only, with the
  same `User-Agent: InboxCraft` for everyone and no version, and with no cookies, no `Referer`, no `Accept` and no
  proxy. Every picture has a new connection and a full TLS handshake, so the server cannot tie two pictures to one
  another by the connection.
- **Redirects**: up to 3 are followed, over HTTPS only. A redirect can lead to another host, which then learns the
  same as the first one.
- **Name lookups**: the picture's host name is looked up by your system's resolver, so your DNS server sees it too.
  A host that resolves only to this computer or your local network is not asked.
- **"Always for <address>"** is kept for this session only, until settings are kept on disk. It goes by the `From`
  address, which a forged message can carry: its pictures then load without asking.
- **Pictures inside the message** (`cid:`) come from the message itself and never from the network.

Saving an attachment writes it into your Downloads folder, under a new name if a file of that name is there; it
never overwrites a file.

## Scripts and agents
The control channel, through which the command-line client and the MCP server reach the running app, is a socket in
your runtime folder that only your own account can open, and every request needs a token InboxCraft keeps in your
system's credential store, never in a file. `inboxcraft --revoke-control` replaces the token, and every client that
kept the old one is refused. Over the channel only the commands marked `safe` run (`inboxcraft-cli commands` lists
them): reading; marking read and unread, flags and undo; drafts, including discarding one, which deletes it on the
server too; checking an account, which connects to its servers; the look of the window; and, once they are there, snooze, park and pin.
Sending, moving, archiving, deleting, accepting or blocking senders and the other settings are refused. The client
shows the token only to a socket in a runtime folder that is yours alone. The MCP server sees what you allow; sending mail needs your
explicit permission.

## Changes and contact
Changes to this page are listed below. Questions: open an issue, or write to [info@qubernetic.com](mailto:info@qubernetic.com).

## Revision history
| Date | Change | Summary |
|---|---|---|
| <date> | major | First version |
| 2026-11-05 | minor | What Detect asks and the ISPDB switch (off by default); what the account check connects to (#28) |
| 2026-11-05 | minor | After a refused or unanswered sign-in, only the owner's check sends the password again (#28) |
| 2026-11-05 | minor | The name lookups behind each URL, the redirect mark, which ports the check tries (#28) |
| 2026-11-05 | minor | Remote pictures: when they are asked for, what a request carries, "always" for a sender, Save (#33) |
| 2026-11-05 | minor | Where a password is kept, when it goes, and how to remove one by hand (#34) |
