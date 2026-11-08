# InboxCraft design file

> **Last reviewed:** <date> · **Last updated:** 2026-11-08 · **Change:** minor · **Status:** draft

`inboxcraft.figcraft` is the design file of the whole app: the design system, the screens and the main flows, made
in [FigCraft](https://getartcraft.com/apps/figcraft). **It is the source of truth for the look.**
[`docs/design.md`](../design.md) puts its rules and tokens into words and `crates/ui-egui/src/theme.rs` implements
them; where the app or `docs/design.md` differs from the file, they are changed to match it. A change of look starts
in the file.

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
| Screens · Setup and settings | first start, the settings sheet on Accounts (none yet, adding with the outgoing server and the CalDAV address, the check under way, failed and passed, the list, one account open, the check after a change), appearance, notifications and shortcuts | [screens-setup.png](screens-setup.png) |
| Planned · Mail and triage | what is on the way: a folder asked about once, one account narrowed, the full header, the screener inside the inbox, the grouped list, the parked pile, blocked mail, a refused send, a missed send time, one name for two people, smart folders, keyboard use, the Hungarian interface; why a message is where it is, trackers shown and kept out, a last look before sending, subscriptions, what waits for a reply | [planned-mail.png](planned-mail.png) |
| Planned · Calendar and settings | an invitation answered, cancelled or clashing with your day, deleting with guests, reminders, the calendar, privacy and about settings | [planned-calendar.png](planned-calendar.png) |
| Planned · Your own AI | the panel beside the list (briefing, emptying the inbox, new senders, newsletters, a free hour), a reply into Drafts, the approval card, a message that gives orders, a request above the level, the settings, a key of your own, what goes to the cloud, tokens, the journal | [planned-ai.png](planned-ai.png) |
| Later · Ideas | not planned yet: important people, tags, rules on the server, an always-on companion, contacts, folder clean-up, a calendar by its address, the browser version, setting up a phone from the desktop, signs of a fake sender, a rule tried before it runs, export, a booking as a card, a date in the text, a one-time code, every attachment, notifications in batches, muting a conversation, duplicates | [later-ideas.png](later-ideas.png) |
| Mood | the main window in Electric, Aurora, Citrus and the custom example, light and dark | [mood.png](mood.png) |
| Custom colours | an example of four colours of your own (`#A46CD6`, `#3A7ADA`, `#06C4D2`, accent `#428CE6`): the inbox, the week, the event editor and the appearance page, dark and light | [custom-colours.png](custom-colours.png) |
| Scenarios | fourteen flows as numbered storyboards, in light and in dark side by side: first start, a morning of triage, write and send, an invitation, a week in the calendar, when a send goes wrong, an invitation to the end, find it again, nothing hidden, nothing slips, bring your own AI, a morning with your AI, a phone from the desktop, make it yours | [scenarios.png](scenarios.png) |

The screens follow the app as built: the inbox over the whole width and with Show done, the snoozed and pinned piles, the composer's reply, suggestions and folded float, the calendar's views with the details in the side panel, the event editor, and the Triage and Sending settings.

Each screens page has a "Dark" column on the right with its main screens in Electric dark.

## How it is organised
- **Colours are variables.** The Colour collection has one mode per preset and mode (`Electric · Light`, `Electric ·
  Dark`, `Aurora · Light` … and the example `Custom · Light`, `Custom · Dark`) and one variable per token of `docs/design.md`, with the values `theme.rs` derives.
  Every fill, stroke, gradient stop and shadow in the file is bound to a variable, so switching a frame's Colour mode
  shows it in another preset or mode. Distance and Motion hold the distances and the motion durations.
- **Type is text styles.** `Type/<token>` for every font token (caption, body, strong, button, sender, sender
  unread, subject, subject unread, preview, heading, title, mono).
- **Screens use instances.** Rows, buttons, fields and the rest are instances of the Components page, set through
  their properties (a message row's sender, subject, preview, date, flag, attachment, account stripe and avatar).
- **Pages are transparent** and default to Electric dark, so the labels read on a dark canvas; each screen sets
  its own mode. The PNGs are drawn on a dark base.
- **Glass is faked, as in the app.** Colour fields behind the window, translucent layers with a light top edge and a
  soft shadow; no background blur, because the app has none. What pops up over content is not glass: it is solid.
- **Size:** screens are 1440 × 900 points; the scenario storyboards are copies at 30 %.

## Not in the code yet

## Limits
- The token values come from a port of `Palette::derive`; they may differ from the app by one colour step where egui
  premultiplies colours.
- Screens for features not built yet show the intended behaviour and layout. The "Planned" pages hold what is on the
  way, "Later · Ideas" what is only an idea and may never be built. Google and Microsoft 365 accounts are left out on
  purpose. The picture of the phone code is decorative and decodes to nothing. The composer's place is still being designed: the file shows it in place of the
  reader and in a window of its own.
- The add form leaves out the app's line that accounts are not kept yet, because it goes when accounts are. The older
  account-check frames name the steps shorter than the app does; "Account check · after a change" uses the app's words.
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
| 2026-11-06 | major | The file is the source of truth for the look; its changes not yet in the code are listed (#96) |
| 2026-11-06 | minor | The window frame page and the new app icon on the cover; every listed change is in the code (#104) |
| 2026-11-06 | minor | Accounts are added and listed on the settings sheet; the folder pane has no Add account (#104) |
| 2026-11-08 | minor | The notifications screen's other rows live on the Triage and Sending pages (#139) |
| 2026-11-08 | minor | The calendar's views are built; the event editor and invitations are not yet (#147) |
| 2026-11-08 | minor | The event editor is built (#148) |
| 2026-11-08 | minor | The invitation screen is built (#149) |
