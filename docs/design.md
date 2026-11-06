# InboxCraft design

> **Last reviewed:** <date> · **Last updated:** 2026-11-06 · **Change:** major

InboxCraft's own look: the colours, distances and type sizes of the front end, and the rules the screens follow.
Everything here is original. Nothing is copied, traced or recreated from another mail program (see `AGENTS.md`,
"Assets"). The values live in `crates/ui-egui/src/theme.rs`; this page explains them, and the tests in
`crates/ui-egui/src/theme_tests.rs` hold them to the contrast rules below.

## Principles
- **Two choices make every colour.** The mode (System, Light or Dark) and the accent preset (three gradient stops
  and an accent) give every colour token; a view names tokens, never colours. `theme.rs` is the only file that names
  a colour, and a test refuses a colour constant anywhere else in `crates/ui-egui`.
- **Vivid colour in a few places, quiet surfaces elsewhere.** The gradient runs at full strength in one place (the
  navigation rail, to come) and as accents elsewhere: the selected row, the unread marker, the badge, links. The
  surfaces are near-neutral with a trace of the preset's middle stop.
- **Contrast is checked, not hoped for.** Every text token is moved in lightness until it meets WCAG 2 AA (4.5:1) on
  each surface it sits on, and every marker, focus ring and field edge until it meets 3:1. The tests run this for
  every preset in both modes, and fail when a preset or a rule breaks it.
- **Nothing depends on colour alone.** Unread rows have a dot and a coloured sender; flagged rows have a flag; the
  account colour stripe repeats what the folder pane says in words. A screen reader hears "unread" and "flagged".
- **Every control has an accessible name.** Rows are named by sender, subject, date and state; folders by name,
  account and unread count. The UI tests refuse a control without one.
- **Message text is data.** Subjects, names and previews are shown on one line: line breaks, tabs and NUL become
  spaces, bidi controls that could reorder what is shown are removed, and each is cut to 300 characters.

## Mode
System (the default) follows the computer's light or dark choice; Light and Dark keep one. egui switches between the
two installed styles, so a change shows from the next frame.

## Accent presets
Each preset is one entry of `theme::PRESETS`; adding a preset is adding one entry, and the tests then check it in
both modes.

| Preset | Gradient (first, middle, last) | Accent |
|---|---|---|
| Electric (default) | `#2F6BFF` `#8A3FFC` `#FF2D95` | `#FFD60A` |
| Aurora | `#9B5CFF` `#FF2BD6` `#FF8A00` | `#00E5FF` |
| Citrus | `#FF2E63` `#FF8F00` `#FFD000` | `#3D5AFE` |

## Colour tokens
Each mode starts from neutral values of our own; the preset then tints them and gives the colours. "Readable" means
moved towards black (light) or white (dark) in small steps until the ratio holds on every surface named.

| Token | Light | Dark | Used for |
|---|---|---|---|
| background | `#F3F5F9` with 3 % of the middle stop | `#0D1016` with 5 % | behind everything, the reader, hovered rows |
| panel | `#EBEEF3` with 3.6 % | `#141821` with 6 % | the folder pane, buttons, bars, row separators |
| surface | `#FBFCFD` with 1.8 % | `#10131A` with 3 % | the message list and its header |
| field | `#FFFFFF` with 0.9 % | `#090B10` with 1.5 % | inside text fields |
| selection | surface with 20 % of the first stop | with 32 % | the selected row or folder; the start of the selection gradient |
| selection end | surface with 12 % of the last stop | with 20 % | the end of the selection gradient |
| text | `#161A22` tinted, readable at 4.5:1 | `#EEF1F6` tinted, readable at 4.5:1 | text |
| muted | `#5D6676` tinted, readable at 4.5:1 | `#9AA3B2` tinted, readable at 4.5:1 | previews, dates, addresses, section headings |
| accent | the middle stop mixed 62/38 with black, readable at 4.5:1 | the preset's accent, readable at 4.5:1 | links, the sender of an unread message, accent text |
| on accent | white or deep ink, by contrast | the same | text on the accent |
| unread | the middle stop, readable at 3:1 | the same | the unread dot |
| flag | the last stop, readable at 4.5:1 | the same | the flag; text that asks for a second look |
| focus | the first stop, readable at 3:1 | the same | the keyboard focus ring, the hovered control's edge |
| field border | muted towards the surface, readable at 3:1 | the same | the edge of fields and windows |
| badge / on badge | panel with 16 % of the middle stop / accent, readable on it | 30 % / the same | unread counts |
| error / on error | `#B3261E` readable / white or deep ink | `#F2B8B5` readable / white or deep ink | the error banner, error text |
| gradient | the three stops at full strength | the same | the rail, the primary button, group badges (to come) |
| ink | deep ink `#05060A` | the same | text on the gradient where white does not read |
| glass / edge / highlight | near-white at 72 % / text-tone at 8 % / white at 90 % | near-white at 5.5 % / 9 % / 7 % | glass layers over the colour fields (E1.15b) |
| shadow | text-tone at 10 % | black at 35 % | under glass and floats |
| float | near-white at 80 % | `#1E2330` tinted at 80 % | tooltips, menus, hover actions, toasts |
| line / hover | text-tone at 7 % / 4.5 % | near-white at 6 % / 6 % | row separators and hovered rows on glass |
| colour fields | each stop at 30 % | each stop at 42 % | the soft fields painted once behind the window (E1.15b) |

**Text on the gradient** picks white or the deep ink, whichever has more contrast with the colour under it; with
that ink every point of every preset's gradient reads at 4.5:1 (white alone would not on Citrus's yellow end).

**Account colours**, in account order and repeating after the sixth: the first, middle and last stop, the accent,
the mix of the first and middle stop and the mix of the middle and last, each readable at 3:1 on the surfaces.
They mark an account's heading in the folder pane and, in the unified list, a stripe at the left of each row.

## Distances

| Name | Points | Used for |
|---|---|---|
| xs | 4 | between lines that belong together; item spacing |
| s | 8 | between items; button padding |
| m | 12 | inside a row; the folder pane's margin |
| l | 16 | around a pane; the list's left and right margin |
| xl | 24 | between groups |
| row height | 52 | a message row: two lines and air |
| folder pane | 240 | the folder pane's default width (it can be resized); the search field's width |
| corner | 6 | the corner radius of fields, selections and buttons |

## Type

egui's bundled proportional font (Ubuntu Light) for now; InboxCraft's own choice of typeface is open (`docs/gaps.md`).

| Name | Points | Used for |
|---|---|---|
| small | 12 | dates, badges, addresses, section headings |
| body | 14 | rows, folders, fields, buttons |
| heading | 15 | account names in the folder pane, empty-state titles |
| title | 20 | the view's name above the list; the app name |

## The mail list window
- **Folder pane** (left): the app name, the account picker (all accounts or one), "Mailboxes" with the unified views
  (Inbox, Drafts, Sent, Archive, Junk, Trash: those some account has), then each account shown with its colour,
  label and address and its folders, the ones with a role first and the rest by path, nested folders indented. Every
  view and folder with unread mail carries a badge with the count.
- **Header** (top of the list): the view's or folder's name, whose mail it is (the narrowed account's address, or
  "All accounts") and how many messages; the search field on the right, with a Clear button while it holds text.
  Ctrl+F (Cmd+F on macOS) focuses it and Escape empties it.
- **List**: newest first, one row per message: unread dot, sender, date (time today, "Yesterday", the weekday in
  the last week, then the day and month, with the year for earlier years), subject and the preview in muted text, a
  flag for flagged mail, and the account stripe when more than one account is shown.
- **Search**: the text is looked for in subject, sender and recipients in every folder of the accounts shown, and
  the header says "Search". Opening a view or folder ends it.
- **Empty states** say what is going on: no accounts yet (with the `--sample` hint), an empty folder, no match.
- **Errors** show in a banner on top until the next command succeeds or the person dismisses them.

## The app icon
A white letter stands in a light teal inbox tray, with an orange unread dot at its corner, on a dark teal rounded
square. The icon keeps these colours in every mode and preset (`theme::app_icon`), so the desktop shows one icon. It is drawn in code from rectangles, convex polygons and circles on a 256-unit
square (`crates/ui-egui/src/icons.rs`); the same shapes give the SVG the desktop shows
(`assets/app-icon/inboxcraft.svg`, written by `just icon`) and the window icon. The square keeps a 16-unit margin, so
the icon sits among others at the same size.

## The reader
- **Where**: selecting a row opens the message to the right of the list, which narrows to a column. "Full width"
  hides the folder pane and the list; Escape, "Leave full width" or Ctrl+F (for the search) bring them back. Close
  ends the selection. The arrow keys step through the list while no field has the focus.
- **Toolbar**: the subject, the next-message rule ("When a message leaves the list": show the next, show the
  previous, go back to the list), Full width and Close.
- **Thread**: oldest first. The open message is unfolded; the others are one line (sender, the first words, the
  date) on the panel colour and unfold on a click; an unfolded older message has Fold.
- **Header**: the sender's name large with the address muted, the date on the right, "to ..." under it; Details
  shows From, Reply to, To, Copy and the date in full.
- **Remote content**: while remote pictures are blocked, a banner on the panel colour says how many and that loading
  them tells the sender the message was opened, with "Load once" and "Always for <address>". A blocked picture is a
  muted box with its alternative text, so the layout shows where it was.
- **Body**: plain text in the body size; a quote is a "Show quoted text (n lines)" button, and once shown it is
  muted text beside a bar, its attribution line small above it. HTML blocks: headings in the title or heading size,
  list items with their bullet or number, links in the accent colour and underlined, quoted blocks folded the same
  way.
- **Attachments**: tiles with a page mark drawn in code beside the app icon (`icons.rs`: a page with a folded corner,
  in the accent colour), the name, the size and Save.
- **Status line** (bottom): where the link under the pointer goes; a link whose text names another place shows
  "Careful: ..." in the error colour. Otherwise the last notice (a save, a copied link, an error) with Dismiss.

## Adding an account
- **Where**: "Add account" sits under the app's name in the folder pane, and in the middle of the empty window. The
  page takes the place of the list and the reader; the folder pane stays.
- **Form**: one column of labelled fields at most 560 points wide: the address with Detect beside it, the box for
  Mozilla's list of providers (off), what Detect found in small type (a redirect over plain HTTP or a guessed name in
  the flag colour, so it gets a second look) with "What Detect asked (n)" folded, then the name for the account,
  the name on sent mail, the user name, the password (dots), the IMAP server, the port and TLS or STARTTLS. A line
  says the password goes into the system's credential store. Errors show above the form in the error colour.
- **Check**: "Checking <address>", a one-line summary (muted while it runs, the accent colour when signed in, the
  error colour when a step failed), then one line per step with a mark drawn in code: a ring in the accent colour
  while waiting, a disc with a tick in the accent colour when passed, a disc with a cross in the error colour when
  failed, a muted ring with a dash when skipped, held back or not built yet. Under each step the host and port it
  tried and the certificate's issuer, muted, and the cause of a failure in the error colour. Done when signed in;
  Check again, Change settings and Close after a failure; Close while it runs.

## Pictures
`just shot` (or `cargo run -p inboxcraft-ui-egui --example ui_shot -- crates/ui-egui/shots.jsonl`) draws the
sample window without a display and writes the PNGs under `docs/images/`: the unified list, one account and a
search, the reader (a reply with its quote, a newsletter before and after its pictures load, attachments full
width), and adding an account (the form, after Detect, the check under way, passed, failed), each in light and
dark in Electric; and the list, the thread and the failed check in Aurora and Citrus (`<shot>-<preset>-<mode>.png`).
A script line picks its preset with `"preset"`. Every UI change shows them in its PR.

## Revision history
| Date | Change | Summary |
|---|---|---|
| 2026-11-05 | major | First version: colours, distances, type and the mail list window (#30) |
| 2026-11-05 | minor | The app icon (#31) |
| 2026-11-05 | minor | The reader (#33) |
| 2026-11-05 | minor | The page mark joins the app icon in `icons.rs` (#33) |
| 2026-11-05 | minor | Adding an account (#34) |
| 2026-11-06 | major | Colour tokens from the mode and an accent preset; Electric, Aurora and Citrus; teal and coral gone (#81) |
