# InboxCraft design

> **Last reviewed:** <date> · **Last updated:** 2026-11-05 · **Change:** minor

InboxCraft's own look: the colours, distances and type sizes of the front end, and the rules the screens follow.
Everything here is original. Nothing is copied, traced or recreated from another mail program (see `AGENTS.md`,
"Assets"). The values live in `crates/ui-egui/src/theme.rs`; this page explains them, and the tests in
`crates/ui-egui/src/theme_tests.rs` hold them to the contrast rules below.

## Principles
- **Calm and readable.** Warm paper and harbour teal in light, slate and sea glass in dark. One accent colour; the
  rest is text on quiet surfaces.
- **Contrast is checked, not hoped for.** Every text colour meets WCAG 2 AA (4.5:1) on each surface it sits on, and
  every marker, focus ring and field edge meets 3:1. A test fails when a colour change breaks this.
- **Nothing depends on colour alone.** Unread rows have a dot and a coloured sender; flagged rows have a flag; the
  account colour stripe repeats what the folder pane says in words. A screen reader hears "unread" and "flagged".
- **Every control has an accessible name.** Rows are named by sender, subject, date and state; folders by name,
  account and unread count. The UI tests refuse a control without one.
- **Message text is data.** Subjects, names and previews are shown on one line: line breaks, tabs and NUL become
  spaces, bidi controls that could reorder what is shown are removed, and each is cut to 300 characters.

## Colours

| Role | Light | Dark | Used for |
|---|---|---|---|
| background | `#F6F5F1` | `#15181B` | behind everything, hovered rows |
| panel | `#ECEAE4` | `#1C2024` | the folder pane, buttons, row separators |
| surface | `#FBFAF7` | `#191C1F` | the message list and its header |
| field | `#FFFFFF` | `#101315` | inside text fields |
| field border | `#80868D` | `#6E757D` | the edge of fields and windows |
| text | `#1C2024` | `#E7E5DF` | text |
| muted | `#565C63` | `#A5ABB2` | previews, dates, addresses, section headings |
| selection | `#D5E6E3` | `#24413F` | the selected row or folder |
| accent | `#1D6A64` | `#6CC4BA` | links, the sender of an unread message |
| on accent | `#FFFFFF` | `#10201E` | text on the accent |
| unread | `#1D6A64` | `#6CC4BA` | the unread dot |
| flag | `#B5541A` | `#F0A06A` | the flag of a flagged message |
| focus | `#0F5C8C` | `#7FB8F0` | the keyboard focus ring |
| badge / on badge | `#1D6A64` / `#FFFFFF` | `#6CC4BA` / `#10201E` | unread counts |
| error / on error | `#B3261E` / `#FFFFFF` | `#F2B8B5` / `#3B0A08` | the error banner |

Account colours, in account order and repeating after the sixth: teal, plum, rust, blue, olive, raspberry
(light `#1D6A64 #8A4B9C #B5541A #2F5FA8 #6B6B1F #A23B5B`, dark `#6CC4BA #C99BD8 #F0A06A #8DB4F0 #C9C66A #EE8FAA`).
They mark an account's heading in the folder pane and, in the unified list, a stripe at the left of each row.

The window follows the system's light or dark choice.

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
- **Attachments**: tiles with a page mark drawn in code (a page with a folded corner, in the accent colour), the name,
  the size and Save.
- **Status line** (bottom): where the link under the pointer goes; a link whose text names another place shows
  "Careful: ..." in the error colour. Otherwise the last notice (a save, a copied link, an error) with Dismiss.

## Pictures
`just shot` (or `cargo run -p inboxcraft-ui-egui --example ui_shot -- crates/ui-egui/shots.jsonl`) draws the
sample window without a display and writes the PNGs under `docs/images/`: the unified list, one account and a
search, and the reader (a reply with its quote, a newsletter before and after its pictures load, attachments full
width), each in light and dark. Every UI change shows them in its PR.

## Revision history
| Date | Change | Summary |
|---|---|---|
| 2026-11-05 | major | First version: colours, distances, type and the mail list window (#30) |
| 2026-11-05 | minor | The reader (#33) |
