# InboxCraft design file

> **Last reviewed:** <date> · **Last updated:** 2026-11-17 · **Change:** major · **Status:** draft

`inboxcraft.figcraft` is the design file of the whole app, made in
[FigCraft](https://getartcraft.com/apps/figcraft): InboxCraft in the form language of the Qubernetic brand. **It is
the source of truth for the look.** [`docs/design.md`](../design.md) puts its rules and tokens into words. A change
of look starts in the file.

**The app does not follow this file yet.** It replaced the earlier glass and gradient design file on 2026-11-17;
the code and the screenshots in `docs/images/` still show that earlier look until the new one is implemented.

Every person, address and message in the file is invented (`example.org`).

<img src="cover.png" width="820" alt="The cover of the design file: the InboxCraft mark and wordmark, how to read the file, and the list of its 21 pages">

`cover.png` is the one picture of a page kept here. Rendered pages are tens of megabytes and change with every
build, so open the file in FigCraft to see the rest.

## Opening it
Open `inboxcraft.figcraft` in FigCraft (File ▸ Open). The file names IBM Plex Mono, IBM Plex Sans and Audiowide (all
under the SIL Open Font Licence) and embeds no font file. FigCraft shows only installed typefaces, so install the
three first; without them it falls back to another typeface.

## Pages
| Page | Holds |
|---|---|
| Cover | The title page, how to read the file, the list of pages |
| Foundations | Colour in dark and light with the contrast of every pair, six account colours, type, spacing, shape, icons, focus and keys, the wording rules, motion in three levels, the character field, the "coming soon" rule |
| Components | 64 components as FigCraft components with variants, every state in dark and light |
| App icon | The InboxCraft mark: its three drawings by size, the size ladder, the mark in the command line, the tray and the app icon, the wordmark and lockups, clear space and minimum sizes |
| Frame | The window: the command line with its result lists, the index with one to six accounts and forty folders, the status line with every state and notice, the own and the system window frame |
| Home | The brief of the day with counts, agenda and drafts; nothing to do, overflowing counts, the first run, everything failing at once |
| Mail | The inbox in its three views (Grouped, Smart, List) with empty, no match, show done and a row leaving; the screener card, New senders in the three screener modes, Later, Outbox, search, Sent, a folder, Drafts, the welcome screen; long text and counts |
| Reader | The reader with its facts, why line and content bar; the states when the text is not there, the five picture states, link status, a conversation, a held sender, full width, the reader in its own window |
| Messages | Eight kinds of mail in the fitted and the plain text view: mailing list, marketing, receipt, forge, pictures, newsletter, invitation, bounce; the original opened in the browser from a cleaned copy |
| Composer | The composer docked, as a floating panel and in its own window, ten states each; long text, Cc and Bcc |
| Calendar | Week, day, month and agenda with overlapping events, the all-day lane, the now line and zones; the side panel, event details, the empty calendar, the sync control |
| Popups and menus | Every float: the More and Move menus, the time picker for snooze, send later and reminders, the event editor, the account picker, suggestions, templates, the day's events in the month, the question before an attachment opens |
| Settings | The nine sections, Appearance to About, as a second column beside the index with the section's page |
| Accounts | Adding an account, what detection asked and where, the check line by line as it runs, its failures in plain words, changing and removing an account |
| Outside the window | The tray icon and its menu, desktop notifications, the app icon on the desktop |
| Planned | Features of the roadmap, each tagged PLANNED: search in the text, bulk select, attachments on a draft, editing a series, OAuth, machine text and an agent's request, the log drawer, setting up a phone, smart folders, and one example of a plugin building in through standard places |
| Mobile · sketch | Nine phone screens as a direction to look at, not a specification |
| Motion | Fourteen movements, each at full, reduced and off, and two flows as sequences of whole windows |
| Sizes | The same screens at 1100 and 800 wide and at 130 % text, with the breakpoints written as rules |
| Layout · why | The reasons of the layout, each with its decision |
| Coverage | What the file holds, its checks, and what it leaves out |

## How it is organised
- **Colours are variables.** The Colour collection has a Dark and a Light mode and one variable per token; fills,
  strokes and text colours are bound to them, so a frame shows in the other mode by switching its Colour mode.
  Distance, Motion and Text size are collections of their own; Text size has the mode `130 %`.
- **Type is text styles**, bound to the Text size collection.
- **Screens use instances.** Rows, buttons, fields and the rest are instances of the Components page, set through
  their properties.
- **Every state is drawn twice,** dark at the left and its light twin at the right. A state is a frame named
  `<kind>/<id>/<state>`.
- **Tags.** MISSING UI: the app has the command and no screen for it yet. PLANNED: a feature of the roadmap, drawn
  now. Both are notes of the file. The dashed "coming soon" mark is part of the app: it stands on a surface that is
  shown and not implemented yet.
- **Size:** windows are 1440 × 900 points; the phone sketches are 390 × 844.

The file is checked when it is built: every listed state is present (1 902 named frames), every text and edge
colour pair reaches its WCAG contrast (142 pairs), no edge misses the line it belongs to by a pixel, and the sample
mailbox says the same on every page.

## The marks
The InboxCraft mark and wordmark and the Qubernetic mark shown in the file and in `cover.png` are trademarks of
Qubernetic. They are not under the MIT or Apache licence of the rest of this repository; see `NOTICE`.

## Not in the code yet
Everything. The app is built with the earlier look; the code follows no page of this file until the new look is
implemented. This list will name what is still missing once that work has begun.

## Limits
- The Planned page shows direction: where a feature stands and how it looks. Numbers and steps in it are invented,
  and each feature's behaviour is decided when it is designed. The plugin example and the mobile sketch are ideas
  for the future, not plans with a date.
- Motion is drawn as still key frames with their times; the file plays nothing.
- The picture of the phone code on the Planned and Mobile pages is a placeholder and decodes to nothing.
- Google and Microsoft 365 accounts are left out on purpose.

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
| 2026-11-08 | minor | Translucent cards, one even selection, white on custom colours, segmented and select controls; the app's new screens and flows; a custom colour example; every float solid (#203) |
| 2026-11-08 | minor | The Planned and Later pages: every feature still to come, the rule-based smart features, with eight more flows (#203) |
| 2026-11-08 | minor | Cards, selection, white on custom colours and floats are in the code; the event tokens are, their painting is not (#205) |
| 2026-11-08 | minor | Controls and events in the calendar are in the code (#206) |
| 2026-11-08 | minor | The account form, an account's settings and the check after a change (#216) |
| 2026-11-17 | minor | The Qubernetic design file, first part, beside the present one (#222) |
| 2026-11-17 | minor | The Qubernetic design file, second part: the frame and the main screens (#224) |
| 2026-11-17 | major | The Qubernetic design file takes the place of the earlier one (#226) |
