# The Qubernetic design file (under construction)

> **Last reviewed:** <date> · **Last updated:** 2026-11-16 · **Change:** major · **Status:** draft

`inboxcraft.figcraft` in this folder is the design file of InboxCraft's next look: the form language of the
Qubernetic brand, which replaces the glass and gradient look. It is built in three parts, each approved by the
project owner before the next begins. **Until the third part is approved, the source of truth for the look stays
[`../inboxcraft.figcraft`](../README.md) and [`docs/design.md`](../../design.md)**, and the app does not follow
this file. When the file is complete it takes the place of the present one, and this folder goes.

Made in [FigCraft](https://getartcraft.com/apps/figcraft). Every person, address and message in it is invented
(`example.org`).

## What is in it now

| Page | Holds |
|---|---|
| Cover | The title page |
| Foundations | Colour in dark and light with the contrast of every pair, six account colours, type, spacing, shape, icons, focus and keys, the wording rules, motion in three levels, the character field |
| Components | 63 components as FigCraft components with variants, every state in dark and light |
| App icon | The InboxCraft mark: its three drawings by size, the size ladder, the mark in the command line, the tray and the app icon, the wordmark and lockups, clear space and minimum sizes |
| Frame | The window: the command line with its result lists, the index with one to six accounts and forty folders, the status line with every state and notice, the own and the system window frame |
| Home | The brief of the day with counts, agenda and drafts; nothing to do, overflowing counts, the first run, everything failing at once |
| Mail | The inbox in its three views (Grouped, Smart, List) with empty, no match, show done and a row leaving; the screener card, New senders in the three screener modes, Later, Outbox, search, Sent, a folder, Drafts, the welcome screen; long text and counts |
| Reader | The reader with its facts, why line and content bar; the states when the text is not there, the five picture states, link status, a conversation, a held sender, full width, the reader in its own window |
| Messages | Eight kinds of mail in the fitted and the plain text view: mailing list, marketing, receipt, forge, pictures, newsletter, invitation, bounce; the original opened in the browser from a cleaned copy |
| Composer | The composer docked, as a floating panel and in its own window, ten states each; long text, Cc and Bcc, attachments (planned) |
| Calendar | Week, day, month and agenda with overlapping events, the all-day lane, the now line and zones; the side panel, event details, the empty calendar, the sync control |
| Sizes | The same screens at 1100 and 800 wide and at 130 % text, with the breakpoints written as rules |

Every state is drawn in dark and in light, and the screens are built from instances of the components. The file is
checked when it is built: every listed state is present (1 382 named frames), every text and edge colour pair
reaches its WCAG contrast, and no edge misses the line it belongs to by a pixel.

This folder holds no pictures of the pages: rendered pages are tens of megabytes and change with every build. Open
the file in FigCraft to see them.

Still to come: every popup, setting, surface outside the window, missing and planned feature, and motion.

## Typefaces and the mark

The file names IBM Plex Mono, IBM Plex Sans and Audiowide (all under the SIL Open Font Licence) and embeds no font
file. FigCraft shows only installed typefaces, so install the three before opening it.

The InboxCraft mark and wordmark and the Qubernetic mark shown in the file are trademarks of
Qubernetic. They are not under the MIT or Apache licence of the rest of this repository; see `NOTICE`.

## Revision history
| Date | Change | Summary |
|---|---|---|
