# InboxCraft design file

> **Last reviewed:** <date> · **Last updated:** 2026-11-06 · **Change:** minor · **Status:** draft

`inboxcraft.figcraft` is the design file of the whole app: the design system, the screens and the main flows, made
in [FigCraft](https://getartcraft.com/apps/figcraft). It is built from the same sources as the code:
[`docs/design.md`](../design.md) and the tokens of `crates/ui-egui/src/theme.rs`. Where the file and those two
disagree, `theme.rs` and `docs/design.md` win and the file is fixed.

Everything in it is original. The typeface is Figtree and the icons are Phosphor, the same files as in `assets/`.
Every person, address and message is invented (`example.org`).

## Opening it
Open `inboxcraft.figcraft` in FigCraft (File ▸ Open). For the text to show in Figtree and the icons to show at all,
install the fonts first: copy `assets/fonts/figtree/*.ttf` and `assets/icons/phosphor/*.ttf` into
`~/.local/share/fonts/` (or the system's font folder). Without them FigCraft falls back to another typeface and the
icons show as empty boxes.

## Pages
| Page | What it holds | Picture |
|---|---|---|
| Cover | name, sources, status | [cover.png](cover.png) |
| Foundations | the accent presets; every colour token per preset and mode; avatar colours; the type tokens; distances and radii; glass and floats; motion; the icons in Regular, Fill and Duotone | [foundations.png](foundations.png) |
| Components | avatar, key chip, badge, button, icon button, pill, tooltip, menu row, toast, rail item, folder row, message row, group header, text field, switch, checkbox, radio, recipient chip, banner, check step, event; each with its variants and properties, and a gallery of all of them in light and dark | [components.png](components.png) |
| Screens · Mail | grouped inbox with the reader, simple list with two accounts, full-width reader with blocked pictures, search with operators, first start, empty and error states, start screen, command palette, notification and tray | [screens-mail.png](screens-mail.png) |
| Screens · Triage | screener review, moving a sender to another group, the time picker (snooze), snoozed, pins and parked, follow-up reminder | [screens-triage.png](screens-triage.png) |
| Screens · Writing | the composer in place of the reader (the default) and in a window of its own, an impossible address, send later, undo send and the outbox, a bounce, a template with a blank | [screens-writing.png](screens-writing.png) |
| Screens · Calendar | week, month, agenda, day with the event editor, an invitation in a message | [screens-calendar.png](screens-calendar.png) |
| Screens · Setup and settings | add account, the account check under way, failed and passed, settings for appearance, accounts, notifications and shortcuts | [screens-setup.png](screens-setup.png) |
| Mood | the main window in Electric, Aurora and Citrus, light and dark | [mood.png](mood.png) |
| Scenarios | five flows as numbered storyboards, in light and in dark side by side: first start, a morning of triage, write and send, an invitation, make it yours | [scenarios.png](scenarios.png) |

Each screens page has a "Dark" column on the right with its main screens in Electric dark.

## How it is organised
- **Colours are variables.** The Colour collection has one mode per preset and mode (`Electric · Light`, `Electric ·
  Dark`, `Aurora · Light` …) and one variable per token of `docs/design.md`, with the values `theme.rs` derives.
  Every fill, stroke, gradient stop and shadow in the file is bound to a variable, so switching a frame's Colour mode
  shows it in another preset or mode. Distance and Motion hold the distances and the motion durations.
- **Type is text styles.** `Type/<token>` for every font token (caption, body, strong, button, sender, sender
  unread, subject, subject unread, preview, heading, title, mono).
- **Screens use instances.** Rows, buttons, fields and the rest are instances of the Components page, set through
  their properties (a message row's sender, subject, preview, date, flag, attachment, account stripe and avatar).
- **Pages are transparent** and default to Electric dark, so the labels read on a dark canvas; each screen sets
  its own mode. The PNGs are drawn on a dark base.
- **Text on the gradient** sits on a text gradient: the preset's stops darkened in small steps until white reads at
  4.5:1 on every stop, as long as that takes no more than 30 % (Electric 14 %, Aurora 30 %); otherwise the full
  gradient with the deep ink (Citrus). Icons on the rail keep the full gradient. This is a proposal:
  `docs/design.md` still says white or the deep ink, whichever has more contrast, which gives the deep ink on
  Electric.
- **The dark accent is the middle stop, lightened.** Links, accent text and active icons in dark mode use the
  preset's middle stop mixed 30 % with white, made readable at 4.5:1 (Electric a soft violet), instead of the
  preset's separate accent (Electric's yellow), which stood apart from everything else. The separate accent stays one
  of the account colours. This is a proposal: `docs/design.md` and `theme.rs` still use the separate accent.
- **Group icons are outlines.** A group's header shows its Regular (outline) icon in the muted colour, without a
  background; a solid icon on the gradient next to every group drew the eye away from the mail. This is a proposal
  too: `docs/design.md` still describes the group badge as a solid icon on the gradient.
- **Glass is faked, as in the app.** Colour fields behind the window, translucent layers with a light top edge and a
  soft shadow; no background blur, because the app has none.
- **Size:** screens are 1440 × 900 points; the scenario storyboards are copies at 30 %.

## Limits
- The token values come from a port of `Palette::derive`; they may differ from the app by one colour step where egui
  premultiplies colours.
- Screens for features not built yet (triage, writing, calendar, settings) show the intended behaviour from the plan
  and `docs/design.md`; their exact layout is a proposal until the code lands.
- Each flow is also a prototype flow on the full-size screens (a click goes to the next step). Steps on different
  pages may not play through in every viewer; the storyboards on the Scenarios page always show the whole flow.

## Revision history
| Date | Change | Summary |
|---|---|---|
| 2026-11-06 | major | First version: foundations, components, screens of the whole app, mood, scenarios (#96) |
| 2026-11-06 | minor | Transparent pages; white text on a darkened text gradient where it reads at 4.5:1 (#96) |
| 2026-11-06 | minor | Group headers show an outline icon without a background (#96) |
| 2026-11-06 | minor | The composer fills the reader's place or opens in a window of its own (#96) |
| 2026-11-06 | minor | The scenarios also in Electric dark (#96) |
| 2026-11-06 | minor | Dark mode's accent: the middle stop lightened instead of the separate accent (#96) |
