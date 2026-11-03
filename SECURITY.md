# Security policy

> **Last reviewed:** <date> · **Last updated:** <date> · **Change:** <kind>

## Reporting a vulnerability
Please **do not open a public issue.** Report privately through GitHub:
[Report a vulnerability](https://github.com/qubernetic/inboxcraft/security/advisories/new), or by mail to [info@qubernetic.com](mailto:info@qubernetic.com).

Include the InboxCraft version or commit, your platform, the kind of account and server, and the steps to reproduce.
You will get an acknowledgement within 3 working days; we agree a disclosure date with you, and credit you unless you
prefer not.

## Supported versions
Fixes go into the latest release only.

## What counts
- Anything that runs or fetches content from a message without the owner asking.
- Credentials leaving the OS credential store, or appearing in files, logs or crash reports.
- A TLS downgrade, a missing certificate check, or AUTH before encryption.
- Header or protocol injection from data that came from outside.
- A crash or hang from hostile input (a message, a server reply, a file, control/MCP params).
- An agent or script being able to send mail or change settings without the owner's permission.

## Known limits (not vulnerabilities)
- The local mailbox cache is not encrypted at rest; it relies on the operating system's account protection.
- S/MIME and OpenPGP are not supported yet.
- Someone who already controls your user account can read your mail.

## Revision history
| Date | Change | Summary |
|---|---|---|
| <date> | major | First version |
