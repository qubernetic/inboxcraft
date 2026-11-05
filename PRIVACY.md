# Privacy

> **Last reviewed:** <date> · **Last updated:** 2026-11-05 · **Change:** minor

## What the InboxCraft project collects
Nothing. InboxCraft has no telemetry, no analytics and no crash reporting, and there is no InboxCraft server.

## Where your data is
Your mail, calendars and settings are kept on your computer, in <settings/data paths per OS>. Passwords and tokens
are kept by the operating system's credential store. Deleting InboxCraft's data folder removes everything it stored.

## What InboxCraft connects to, and when
- Your own mail and calendar servers, with the settings you gave (IMAP, SMTP, CalDAV).
- When you use "Try to detect" (see below): your domain's DNS records and autoconfig address, and Mozilla's ISPDB
  only if you switch it on.
- When you check an account: the account's own IMAP server, as for reading mail (see below).
- Pictures in a message, only when you ask to show them.
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
`User-Agent`, no cookies, and no proxy. The proposal lists every request that was made. Nothing detected gets your
password before you have seen the settings and the server's certificate has verified.

## Checking an account
The account check connects to the account's IMAP server with the saved settings, step by step, to tell you why a
connection fails: the name, the connection, TLS, the certificate, the sign-in. When the mail port does not answer,
it also tries to open (and closes at once, sending nothing) port 443 and the other mail ports (993, 465, 587) of the
same server, to tell a network that blocks mail from a server that is down. The password goes out only after the
certificate verified, as when reading mail. After the server refused the password or never answered the sign-in, a
check an agent asks for stops before the sign-in and says it needs you; only a check you start sends the password
again, so an agent cannot get the account locked at your provider by checking over and over.

## Scripts and agents
The control channel listens only on 127.0.0.1 and needs a key from a file only you can read. The MCP server sees what
you allow; sending mail needs your explicit permission.

## Changes and contact
Changes to this page are listed below. Questions: open an issue, or write to [info@qubernetic.com](mailto:info@qubernetic.com).

## Revision history
| Date | Change | Summary |
|---|---|---|
| <date> | major | First version |
| 2026-11-05 | minor | What Detect asks and the ISPDB switch (off by default); what the account check connects to (#28) |
| 2026-11-05 | minor | After a refused or unanswered sign-in, only the owner's check sends the password again (#28) |
