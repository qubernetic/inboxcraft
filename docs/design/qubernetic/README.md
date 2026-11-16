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

| Page | Picture | Holds |
|---|---|---|
| Cover | [`cover.png`](cover.png) | The title page |
| Foundations | [`foundations.png`](foundations.png) | Colour in dark and light with the contrast of every pair, type, spacing, shape, icons, focus and keys, the wording rules, motion in three levels, the character field |
| Components | [`components.png`](components.png) | 58 components as FigCraft components with variants, every state in dark and light |
| App icon | [`app-icon.png`](app-icon.png) | The InboxCraft mark: its three drawings by size, the size ladder, the mark in the command line, the tray and the app icon, the wordmark and lockups, clear space and minimum sizes |

The pictures are whole pages, rendered with `figcraft-cli render`. The Components page is larger than the renderer's
limit of 16 384 pixels, so its picture is scaled down; open the file to read it at full size.

Still to come: the frame and the main screens (Home, mail, reader, composer, calendar) at three window widths, then
every popup, setting, surface outside the window, missing and planned feature, and motion.

## Typefaces and the mark

The file names IBM Plex Mono, IBM Plex Sans and Audiowide (all under the SIL Open Font Licence) and embeds no font
file. FigCraft shows only installed typefaces, so install the three before opening it.

The InboxCraft mark and wordmark and the Qubernetic mark shown in the file and in its pictures are trademarks of
Qubernetic. They are not under the MIT or Apache licence of the rest of this repository; see `NOTICE`.

## Revision history
| Date | Change | Summary |
|---|---|---|
