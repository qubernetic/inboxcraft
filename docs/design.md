# InboxCraft design

> **Last reviewed:** <date> · **Last updated:** 2026-11-21 · **Change:** minor

InboxCraft's look in words: the Qubernetic design, its tokens, its rules and its screens. The source of truth is
the FigCraft design file [`docs/design/inboxcraft.figcraft`](design/README.md): 23 pages, 64 component sets, every
state drawn in dark and in light. This page puts that file into words; where the two differ, the file is right and
this page is changed.

**The app follows most of this design.** The code and the screenshots in `docs/images/` show the Qubernetic look;
[the design file's README](design/README.md#not-in-the-code-yet) lists the file's pages and what is not in the
code yet.

Everything here is original (see `AGENTS.md`, "Assets"). The InboxCraft mark and wordmark and the Qubernetic mark
are trademarks of Qubernetic and are not under this repository's licences; see [`NOTICE`](../NOTICE).

## Principles
- **Keyboard first, and every working key is shown.** Everything works from the keyboard and the focus is always
  visible. The status line names the keys that work in the present state; a key that would do nothing is not shown.
- **One way in, one place for news.** A command line at the top searches mail, runs commands, finds people and
  goes to mailboxes. A status line at the foot says what the program is doing, carries undo and prints notices.
  Nothing floats over the content to say so.
- **The program says what it does and what leaves the computer.** The reader says why a message is where it is.
  Adding an account lists what was asked and where. The reader's content bar says what was held back.
- **The safe act is the easy one.** Remote pictures wait until asked. Before a file that can run something opens,
  the program asks, and "Save only" is the primary button. Removing an account says what goes and what stays.
- **The owner never waits for a motion.** A command's result is there at 0 ms; motion plays beside it.
- **Plain black and white, one accent.** Flat surfaces, square corners and 1 px hairlines. No gradients, no glass,
  no blur. One accent colour, cyan, with one job in each of its forms; every other colour is a grey with a job.
- **Dark and light are equals.** Every state is drawn in both, and every colour pair is checked in both.
- **Nothing depends on colour alone.** A signal colour always stands with words; an account's bar always stands
  beside the account's name; hover, selected and checked rows are told apart by a bar, a check box and actions.

## Colour
A view names tokens, never colours. The mode (Auto, Dark or Light) gives every token its value.

| Token | Dark | Light | Use |
|---|---|---|---|
| `bg` | `#0E0F10` | `#F5F5F2` | the window |
| `surface` | `#151617` | `#FFFFFF` | panes that hold content; what floats (menus, dialogs) |
| `panel` | `#101112` | `#F5F5F2` | the index column, the command line's bar, the status line |
| `term` | `#0A0B0C` | `#FFFFFF` | the command field, text fields, code, the composer's body |
| `line` | `#2A2D2F` | `#D6D7D3` | structural hairlines: pane edges, control boxes, tags |
| `panel_line` | `#202224` | `#E2E3DF` | quieter hairlines inside a pane, between rows |
| `grid` | `#17191A` | `#ECECE8` | hover on `bg` and `panel`: index rows, group heads, the status line |
| `stage` | `#212427` | `#DEDFDB` | hover on `surface` and in floats; the current row; the chosen segment; avatars |
| `ink` | `#D4D4D4` | `#191919` | text |
| `muted` | `#8B9196` | `#5A6065` | secondary text, icons, key hints, placeholders, disabled text, edges with a meaning |
| `dim` | `#4A4F53` | `#9EA3A6` | decoration only: separators, inactive index numbers, a disabled control's glyph |
| `faint` | `#2B2E30` | `#CFD1CD` | the background characters |
| `cyan` | `#22A8E0` | `#22A8E0` | the accent as a fill, only under `on_cyan` text: primary button, ticked box, picked day |
| `on_cyan` | `#0E0F10` | `#0E0F10` | text on a cyan fill |
| `cyan_text` | `#22A8E0` | `#075E88` | the accent as text: section numbers, links, counts, the prompt |
| `cyan_edge` | `#22A8E0` | `#0B79AD` | the accent as an edge or small mark: focus, the 2 px bar, unread square, caret, now line |
| `cyan_muted` | `#00354C` | `#D3EEF9` | accent wash: selected text, a badge's ground |
| `minus` | `#C98080` | `#9A3A36` | errors, removed, delete |
| `plus` | `#6AB26B` | `#246628` | passed, added |
| `warn` | `#EEC448` | `#6E5400` | waiting, snoozed |
| `scrim` | `#00000099` | `#19191955` | behind a dialog (the last two digits are the opacity) |
| `acct1` | `#9DAF19` | `#778224` | account bar, lime |
| `acct2` | `#A359B9` | `#B54ED5` | account bar, violet |
| `acct3` | `#C05928` | `#D84F05` | account bar, orange |
| `acct4` | `#547FFA` | `#163E8B` | account bar, blue |
| `acct5` | `#F55BB8` | `#7C1457` | account bar, pink |
| `acct6` | `#64DABF` | `#368977` | account bar, aqua |

- **The accent has three forms.** `cyan` is one value in both modes and is only ever a fill under `on_cyan` text;
  on a light ground it is too pale for text or a 1 px edge. `cyan_text` is the accent wherever it is read.
  `cyan_edge` is every ring, bar, caret and unread square, and the edge of a float that takes the keyboard.
- **Signal colours** (`minus`, `plus`, `warn`) always come with words.
- **`dim` is decoration.** It is never the only carrier of text a person must read. Placeholders, hints and key
  hints are `muted`. An edge that carries meaning (an empty check box, radio or switch) is `muted`, not `line`.
- **Accounts.** An account is a bar 3 px wide and 12 px high in its colour, with the account's name beside it:
  in the index, after the sender in a row of a mixed list, in the composer's From line, on a calendar event. It is
  never a fill, a tinted row, a coloured avatar or coloured text. The n-th account takes colour (n − 1) mod 6 + 1,
  so the six repeat from the seventh account; the name tells accounts apart, the colour only helps. **The accent is
  no account's colour,** and none of the six is a signal colour.
- **Contrast.** Text reaches 4.5:1 (WCAG 2) on every ground it stands on: `ink`, `muted`, `cyan_text`, `minus`,
  `plus` and `warn` on `bg`, `surface`, `panel`, `term`, `grid` and `stage`, and `on_cyan` on `cyan`. Edges and
  marks that carry meaning reach 3:1: `cyan_edge` on five grounds, each account colour on `bg`, `surface`, `panel`
  and `stage`. The file checks 142 pairs over the two modes when it is built, and none fails. Exempt, with their
  ratios shown and not checked: `line`, `panel_line` and `faint`, which are structure and decoration, and `dim`,
  which carries no text a person must read.

## Type
Three typefaces, all under the SIL Open Font Licence. **IBM Plex Mono** says what the app says: labels, headings,
buttons, counts, dates, keys, the command line. **IBM Plex Sans** says what people wrote: sender names, subjects,
previews, message text, and help under a field. **Audiowide** is only for the names `qubernetic` and `InboxCraft`
on Home, the cover and About. Two weights, Regular and SemiBold; no italics in the app's own text.

| Style | Typeface | Size / line | Use |
|---|---|---|---|
| display | Plex Mono Regular | 40 / 48, tracking −1 % | Home's headline |
| title | Plex Mono Regular | 22 / 30 | the subject in the reader; a page's title |
| heading | Plex Mono SemiBold | 15 / 22 | headings |
| label | Plex Mono Regular | 11 / 16, capitals, tracking 8 % | section labels, column heads, the index |
| mono | Plex Mono Regular | 13 / 20 | lines the app prints, counts, dates |
| mono_s | Plex Mono Regular | 11 / 16 | tags, a row's small links |
| key | Plex Mono SemiBold | 11 / 14 | key chips |
| button | Plex Mono Regular | 13 / 16 | button labels |
| body | Plex Sans Regular | 14 / 22 | text people wrote, in lists and forms |
| strong | Plex Sans SemiBold | 14 / 22 | an unread sender; emphasis |
| caption | Plex Sans Regular | 12 / 18 | a row's second line; help under a field |
| read | Plex Sans Regular | 16 / 26 | the text of a message |

The text size setting (90 to 130 %) scales every style together; see "Text at 130 %".

## Distances
Six steps on a 4 px grid. Every size and position is a multiple of 4, except the 1 px hairlines, the 2 px bar of
the current row, the 3 px account bar, the 26 px status line and the index's 34 and 30 px rows. Space separates
and hairlines divide: a hairline needs no extra gap around it.

| Step | px | Use |
|---|---|---|
| 1 | 4 | between an icon or key chip and its word |
| 2 | 8 | between the parts of one control; between a label and its number |
| 3 | 12 | padding inside a control; between a row's columns |
| 4 | 16 | padding of a pane; the left edge of every row |
| 6 | 24 | between sections |
| 10 | 40 | the margin of a page such as Home |

Fixed heights: button and segmented control 32, field and select 36, command field 28, menu row 32, mail row 56,
group head 32, index place 34, mailbox row 30, status line 26, key chip and tag 20, avatar 24, 32 or 40.

## Shape
- **Square.** Corner radius 0 on everything; radius 2 on two things only, key chips and avatar squares. No round
  shapes: the radio is a 16 px square with a solid square inside, the switch a 28 × 16 box with a square knob.
- **Hairlines.** Every edge is 1 px, drawn inside its box, never thicker.
- **One shadow,** under what floats over the window (menus, dialogs, the command line's results, tooltips): 12 px
  down, 32 px blur, black at 40 %. A float that takes the keyboard has a 1 px `cyan_edge`; a tooltip has `line`.
- **State is a fill and an edge, never a lifted card.** Hover is `grid` on `bg` and `panel`, `stage` on `surface`
  and in floats. The current place, the open message and the row Enter will run have the `stage` fill and a 2 px
  `cyan_edge` bar on the left. In the index only the current place is filled.
- **Focus** is a 1 px `cyan_edge` outline, square, 2 px clear of the control. Where there is no room outside (a
  field, a cell) it is drawn inside the box. Focus is not selection: one row can have both. Nothing that can be
  clicked is without a focus state, and the ring is never switched off. A card that asks for an answer (an
  invitation, "Let … in?") carries the same edge.
- **Disabled** is `muted` text and a dashed 1 px edge, never a fade: nothing is drawn at less than full strength.
  A control without a box has `muted` text and a `dim` glyph. A disabled switch keeps its knob; an unavailable
  one has no knob and says why in one sentence beside it.

### Pixel rules
With square boxes and 1 px lines, an edge one pixel off shows as a step or a doubled line. The file is checked for
these when it is built.
- **No doubled lines.** A box whose edge would stand 1 or 2 px from a parallel line either lies on that line or
  stands at least 3 px clear. A calendar event lies on the day and hour lines; the last row of a list that ends at
  its pane's edge draws no line of its own.
- **A float stays 3 px clear of the window's border on every side,** with the window's ground between, and keeps
  all four of its own edges.
- **One focus edge for every list row:** 1 px `cyan_edge`, 2 px inside the row, clear of the pane's borders and of
  the hairlines over and under the row.
- **A scroll thumb never covers a focus edge.** Where the thumb passes one, the edge is drawn over the thumb.
  Neither moves when they meet: in a 440 px list the edge's right line is on column 437 and the resting thumb on
  432 to 435, with one column of the row's ground between. This 1 px is meant.
- **Lines stand on whole pixels.** Equal columns are cut at whole pixels, never at a fraction.

## Icons, numbers and labels
- **Icons** are Phosphor Regular only: one family, one weight, no second set, no emoji. 16 px in buttons, menus
  and the index; 14 px for a mark in a row's line (paperclip, flag, pin). `muted` by default, `ink` on the current
  row or a hovered button. Never in a tile, never tinted with the accent or an account colour. An icon alone has a
  tooltip with its name and its key. Icons do not replace words in the index.
- **Bracketed numbers.** A section label is `[ 01 ] MAIL`: square brackets with a space inside, two digits from
  `00`, then the word in capitals. The number is `cyan_text`, the word `muted`.
- **Tags** are a 1 px box with mono 11 text, 20 high. A signal tag takes its colour for edge and text; the fill
  stays empty.
- **Avatars** are a square with two mono initials on `stage`.
- **The character field.** The ground of Home, the first start and the cover is a grid of 8 × 15 px cells; about
  5 in 100 hold a `·` in `faint`. The pattern is fixed and never moves by itself. It shows at 20 % on the empty
  area of a pane and never takes a click. On Home it lies under the whole page, the brief too, and calms near the
  text: inside a text block its cells flip with 0.2 of their chance and a flipped one shows at 0.2 of its opacity,
  coming back to the whole over 80 px, eased; the dots at rest and the emblem do not change.

## Wording
- Plain words: say what happened, then what to do. No codes without words. "Could not reach mail.example.org.
  Trying again in 30 s."
- Sentence case for everything a person reads. No marketing, no exclamation marks, no jokes.
- Lines the app prints (status line, command results) are short and lower case.
- **Keys** are written out as words, one chip per shortcut, keys pressed together sharing a chip: `Ctrl Enter`,
  `Ctrl K`, `Del`, `Esc`. The arrow keys are the signs ↓ ↑ ← →; every other key is a word. A chip shows the owner's
  present binding; an action without a key shows no chip.
- **Counts** have thin groups (`1 204`) and cap at `9 999+`. A count of 0 is not shown.
- **Times** are on the 24-hour clock; dates are day and short month.
- One character for the ellipsis; it means the app is working or the text is cut.
- **Honest sentences.** The app says only what it knows: a search that does not look through message text says so.
- British spelling. The interface is in English; command ids, the CLI and MCP never carry translated text.

## The window
Everything is designed at 1440 × 900. Four fixed regions:

| Region | Size | Holds |
|---|---|---|
| Command line | 44 high | the mark, one field 28 high, the window's controls |
| Index | 232 wide (56 when narrow) | the numbered places, the shared mailboxes, the accounts |
| Body | the rest (1207 × 828) | the panes of the current place |
| Status line | 26 high | what the app is doing, undo and notices; at the right the keys that work now |

- **Command line.** Plain text searches, `>` runs a command, `@` finds a person, `#` goes to a mailbox. `Ctrl K`
  and `/` open it empty; `Ctrl F` opens the same field narrowed to searching mail in the current mailbox. Results
  hang under the field as a float and the window under the command line is dimmed; the line itself is not. `Esc`
  returns to the list.
- **Index.** At the top stands New message, above the places and as wide as the column: a secondary button with
  the compose glyph, its words and its key, `C`, 32 high with 12 px at its sides and 8 px under it. It is the same
  on every place, and disabled while there is no account to write from. Then five places: `[ 00 ] HOME`,
  `[ 01 ] MAIL`, `[ 02 ] CALENDAR`, `[ 03 ] LATER`, `[ 04 ] SETTINGS`. Under MAIL stand the mailboxes every account
  shares, then one row for each account, which opens to its folders. In a short window only the mailboxes scroll.
  The account picker opens from the owner's row at the index's foot. The narrow index shows New message as its
  glyph alone, in a 32 px square. Drafts, in Mail and under Calendar, has a glyph of its own, a page with a dashed
  edge (`file-dashed`), so it is not taken for New message's pencil.
- **Panes.** Every list is 440 wide; the pane beside it takes the rest (766 for the reader at 1440). Rows are
  lines, not cards. A pane with nothing in it says so in its centre.
- **Status line.** At rest: the sync state, the accounts, the outbox, the next event. A notice is a stretch of
  this line under a 2 px line in its kind's colour, not a box. An undo line stays 5 s, a notice with nothing to
  undo 4 s, an error until it is dismissed.
- **Frame.** With the window's own frame the command line is also the title bar and the drag strip, also while its
  results or the event editor show. The other setting is the system's frame. Both are drawn maximised and without
  focus as well.
- **Keys.** `D` done (mark as read and archive), `A` archive, `H` snooze, `R` read, `S` flag, `C` write, `J` and
  `K` or the arrows move in a list, `Ctrl Z` undo, `Ctrl Enter` send. Single letters work when the focus is not in
  a text field. `Tab` goes through the regions in reading order. Every key can be changed in Settings.

### Widths
The breakpoints are widths of the window in px and do not move with the text size. The file draws the main
screens at 1100 and at 800 as well as at 1440.

| Window | What changes | Why |
|---|---|---|
| under 1280 (drawn at 1100) | the index narrows to 56 px of icons; the command field takes the room | the reader beside the 440 px list would have less than 606 px |
| under 980 (drawn at 800) | the body holds one pane; list and reader swap (`Enter` opens, `Esc` goes back) | the reader would have less than 482 px |
| 800 | the least width; the one pane is 743 px; the window cannot be made narrower | no frame draws it narrower |

The command line and the status line keep their heights and never leave. The list keeps 440 px whenever a second
pane stands beside it. A control is never cut: text cuts on one line with `…`, and a part that does not fit is
left out whole. In one pane the composer and the calendar's side panel swap in the same way as the reader.

Settings follows the same two breakpoints (drawn at 800, and at 1000 with the text at 130 %). From 980 up the list of
sections stays beside the page at 232 px and the page's column narrows (630 px at 1000). Under 980 the body holds
one pane: the list of sections or a section's page, swapping as the list and the reader do; the page's head starts
with `← Settings` where it says where it is, and `Esc` goes back too. A setting whose control needs more than half
its row gives the control a line of its own under the name and its help, at the left (the attachment's rule);
Sending's two signatures stand one under the other; About's emblem leaves whole, as on Home. At 130 % the text
columns of a list row (a name, a time) grow with their type, and the scheduled list puts the person under the
subject.

### Text at 130 %
Every type size and line height is × 1.3, and with them the height of every row and control (status line 26 → 34,
button 32 → 42, field 36 → 46, index place 34 → 44) and every icon's box (16 → 20). Region widths, breakpoints,
paddings, gaps and hairlines stay. Less text fits a line, so it cuts sooner; a row of controls that no longer fits
takes a second line; what grows downwards scrolls. Only 100 % and 130 % are drawn; the sizes between follow the
same rules.

## Components
The 64 sets of the file. A screen is built from instances of these only.

| Set | For | States in brief |
|---|---|---|
| `icon-button` | an act shown as an icon, 32 × 32 | default, hover, focus, pressed, selected, disabled, unavailable |
| `text-button` | an act in words; primary, secondary, danger, plain | default, hover, focus, pressed, disabled, unavailable; with key, arrow or icon; secondary with icon and key, and icon only, each with hover, focus, pressed and disabled (the index's New message) |
| `link-button` | a text link, also the small link of a row | default, hover, focus, pressed, disabled |
| `key-chip` | a key | default, on cyan, capture, none, disabled |
| `key-hint` | a key with what it does, in the status line | default, hover |
| `tag` | a short mark in a box | default, accent, warn, minus, plus, coming soon; the file's PLANNED and MISSING UI |
| `section-label` | a bracketed number and a word | default, active, hover |
| `tooltip` | a name, its key, or a reason | default, with key, unavailable reason, long |
| `checkbox` | a tick | off, on, hover, focus, disabled |
| `radio` | one of several | off, on, hover, focus, disabled |
| `switch` | on or off | off, on, hover, focus, disabled, waiting, unavailable |
| `segmented` | one of a few options in a row | default, hover, focus, pressed, disabled |
| `pill` | a filter or a pile | default, hover, focus, selected, disabled |
| `select` | one of a list | default, hover, focus, open, disabled, unavailable |
| `slider` | the text size | default, hover, focus, dragging, disabled |
| `avatar` | two initials in a square | 24, 32, 40; unknown |
| `badge` | a count | count, overflow, muted, accent |
| `window-controls` | minimise, maximise, close | default, each hovered, maximised, unfocused |
| `icon` | the 45 named icons | |
| `text-field` | one line of text | empty, filled, hover, focus, error, disabled, read-only |
| `multiline-field` | several lines; grows from 96 to 240 px, then scrolls | empty, filled, focus, error, disabled, grown, scrolling |
| `password-field` | a password; never dots for one that is only stored | empty, filled, focus, error, disabled, stored |
| `command-field` | the command line's field | rest, hover, focus, typing, the three prefixes, mail search |
| `recipient-field` | To, Cc, Bcc | empty, focus, typing, with chips, invalid, disabled, wrapped |
| `recipient-chip` | one recipient | valid, invalid, hover, focus, disabled |
| `time-picker` | snooze, reminder, send later | presets, typed valid, typed invalid, with alert, past |
| `typed-time` | a time typed as text | empty, focus, valid, invalid |
| `popup-panel` | the box of a menu | default, scrolling, empty |
| `menu-item` | a row of a menu or of the command line's list | default, hover, focus, chosen, unavailable, with key, danger |
| `menu-choice` | a row that is ticked or not | off, on, hover, unavailable |
| `suggestion-row` | a person offered | default, hover, focus, selected |
| `mail-row` | a message in a list, 440 × 56 | read, unread, selected, hover, focus, done, leaving, checked, flagged, pinned, snoozed, invitation, tagged |
| `group-head` | an inbox group's label and count | default, hover, focus, show all, show fewer, zero |
| `nav-row` | a place of the index | default, hover, focus, active, open, with count; narrow |
| `mailbox-row` | a mailbox of the index | default, hover, focus, active, with count; narrow |
| `account-row` | an account of the index | default, hover, focus, open, syncing, offline, refused; narrow |
| `outbox-row` | a message that has not left | countdown, scheduled, sending, refused |
| `later-row` | a message in Later, two lines | snoozed, pinned, parked, returned |
| `draft-row` | a draft | default, hover, focus, no subject, no recipient |
| `settings-row` | a setting with its control | default, hover, focus, with help, unavailable |
| `check-step` | a line of an account check | waiting, running, passed, failed, skipped, not run |
| `hover-actions` | the acts on a hovered row | default, action hovered, focus, unavailable |
| `status-message` | a notice in the status line | undo, send countdown, notice, error, done |
| `status-item` | a fact in the status line | plain, ok, waiting, offline, error, accent |
| `banner` | a notice above a message's text | remote pictures, bounce, error, failing, notice, careful |
| `why-line` | why a message is here | group, new sender, screener, folder |
| `content-bar` | a message's kind, view and what was held back | text, HTML, list, invitation, pictures held, plain text |
| `attachment` | a file of a message | default, hover, focus, saving, saved, failed |
| `draft-file` | a file on a draft | default, hover, focus, too large |
| `event-block` | a calendar event | timed, band, chip, row; picked, hover, focus |
| `thread-row` | a message of a conversation | folded, open, unread, hover, focus |
| `invite-card` | an invitation above a message | request, described (the event's description inside the card), changed, out of date, answered, changing, cancelled, reply, clash |
| `block` | a titled box of rows, on Home | default, hover, focus, selected |
| `block-line` | the last line of such a box | empty, more |
| `count-tile` | a count on Home | default, accent, warn, minus, zero, overflow |
| `account-bar` | the 3 px bar | the six colours |
| `empty-state` | what an empty pane says | bare and centred; with an action, in a box |
| `progress` | work under way, in characters | running, done, failed |
| `quote-fold` | folded quoted text | folded, open, hover |
| `hairline-table` | a table in a fitted message | default, long cell |
| `mini-month` | the small month of the calendar | default, today, picked, hover |
| `scrollbar` | a scroll thumb, 12 wide | rest, hover, dragging |
| `facts` | From, To, Date of a message | default, details open |

## Screens
### Home
A page of its own: at the left a 600 px brief of today (a headline, four counts, the day's agenda, the drafts to
pick up, two buttons), at the right the Qubernetic emblem drawn in characters on the character field. It is also
drawn with nothing to do, with overflowing counts, before the first account, and with everything failing. The
emblem with the names under it (the two wordmarks and the tagline, 16 below it) stands centred in the free area
right of the brief and under the command line, across and down, at any window height. The character field lies
under the whole page and answers the pointer anywhere on it, calmer near the brief's text and the names. Where the window is too
narrow or too low for the emblem and its names, it leaves whole; it is never cut or made smaller. The headline greets the
owner by the first word of the sending name, titles such as Dr. skipped; Settings › Sending sets the last word, the
full name or no name instead.

### Mail lists and the screener
- **Inbox,** in three views: Grouped (every group open), Smart (the first three rows of each group, with "show
  all") and List (one list by time). Groups are section labels with a count. Done mail is out of the way, not
  gone: "Show done" lists it again.
- **A row** is two lines: the sender, the account bar and the time, then subject and preview. Unread is a cyan
  square and heavier type. A subject is one line everywhere but in the reader.
- **The screener.** First-time senders wait in New senders, a queue of one card a sender with Let in and Keep
  out. A card on top of the inbox says that someone waits. Three modes: Before the inbox; Inside, marked; Off.
- **Later** holds three piles as pills: Snoozed, Pins, Parked. **Outbox** rows say where a message stands and
  what can still be done. Sent, Drafts, a folder and search results are plain lists of the same width.

### The reader and the message kinds
- From the top: tags and the message's actions, the subject (up to two lines), the facts, the why line, the
  content bar, the message, the answers at the foot. Reply is the pane's one primary button.
- **The why line** says in one line why the message is here: its group, who let the sender in, its folder.
- **The content bar** names the kind (text, HTML, list, invitation), switches between `fitted` and `plain text`,
  and says what was held back. Fitted sets the message in the app's type and colours and keeps only a part's
  coloured background. The sender's own look opens in the browser from a cleaned copy, in which a link is text
  with its address and cannot be clicked. Nothing in a message runs.
- **Pictures** have five states; a remote picture is a box of its real size until it is asked for.
- When the text is not there (loading, not kept yet, too large, unreadable, cut), one sentence says so in its
  place. After Done or Snooze the reader shows "nothing open"; the next message does not open by itself.
- **Eight kinds** are drawn: a mailing list (the thread as a tree, folded quotes, a coloured diff), marketing, a
  receipt, a notification from a code forge, pictures inside a message, a newsletter, an invitation (a card above
  the text with the answers and a clash warning) and a message that did not arrive.
- **An invitation that asks** makes the card's Going the pane's one primary button; Reply at the foot steps down to
  secondary. After Change answer the card offers the three answers again and Keep my answer, which goes back to
  the answer given and sends nothing.
- The reader can stand at full width or in a window of its own (960 × 900: the main frame without the index).

### The composer
Docked in the reader's place by default, the list beside it; the other setting is a window of its own (704 × 720,
a plain title bar with the mark and the subject, its own status line). While a message or the calendar is opened
during writing, it moves to a 520 × 560 float at the bottom right, which folds to one line. Lines From, To and
Subject with a rule under each; the text on `term`. Send stands at the left of the foot with its key and, beside
it, the reason when it waits; Templates and Discard stand at the right. After a send the reader returns to the
message that was open.

### The calendar
Week (the opening view), day, month and agenda. The hour grid is hairlines; an event is a cell on the grid, its
edges on the day and hour lines, with its account's bar. Cyan marks today, the now line and the picked event. The
side panel stands at the right, 252 wide, where the reader is in Mail: a mini month, the calendars, the picked
event's details, the sync control. "+N more" in a month cell opens a list of that day.

### Popups and menus
- A menu hangs from the button that opened it (the reader's, 4 px under it), with a 1 px `cyan_edge` and the one
  shadow. It keeps `Tab` inside itself; `Esc` closes it and the focus returns to where it came from.
- **One time picker** serves snooze, the reminder and send later, with presets and a typed time.
- **The event editor** is a float hanging from New event, also when Edit opened it.
- **More** has Archive as its first row; **Move to** lists the folders.
- **One dialog** for every confirmation: 480 wide, over a scrim that stops at the status line.
- **Before an attachment that can run something opens,** the program asks and names the kind.
- Tooltips stand 4 px clear of their control. What used to be a toast is a line in the status line.

### Settings and accounts
- Settings is a place of the window: a second 232 px column with nine sections (Appearance, Accounts,
  Notifications, Triage, Sending, Calendar, Shortcuts, Privacy, About) and the section's page beside it, 720 wide.
  In a narrow window it gives way as "Widths" says: one pane under 980, settings whose control takes a line.
- **Appearance** has two groups. Look: mode (Auto, Dark, Light), motion (full, reduced, off), text size. Layout:
  inbox view, open a message (pane or window), write a message (pane or window), window frame (own or system).
  There are no accent presets, custom colours or typeface and icon choices.
- **Adding an account:** the address with Detect, the sign-in, the servers. What Detect asked, and where, stands
  open beside the form. The check prints its seven steps like a terminal, one by one as they run; a failure shows
  the failing line and plain words under it. No password is sent without encryption, and no setting allows it.
  Detect and Check and add can always be pressed: a press with something missing says what, under its field, and
  gives that field the keyboard. They wait only while Detect is asking.
- **An account's settings:** Save is disabled until something changes, and the line beside it says so.
- **Removing an account** is a danger button; its confirmation says first that it cannot be undone.

### Outside the window
- **Tray:** the InboxCraft mark; its flap turns cyan while there is unread mail. A desktop that recolours tray
  icons shows it in one colour. The menu has two entries, Open InboxCraft and Quit InboxCraft.
- **Notifications:** the desktop owns their look, the app their text. The text is who wrote and the subject, never
  the message's text, and nothing at all for a sender who has not been let in.
- **The marks.** The InboxCraft mark stands in the command line, the tray and the app icon (on a dark tile); the
  Qubernetic emblem only on Home and About.

## Motion
One curve for every movement, `cubic-bezier(.2, .7, .2, 1)`: a quick start and a soft landing. Fades are linear.
Nothing bounces, overshoots or loops, except the caret's blink.

| Token | ms | Use |
|---|---|---|
| `fast` | 125 | fades; a row's text dissolving; the cross-fades of the reduced level |
| `medium` | 300 | the reader's wipe, a row decoding in, the theme cross-fade at full |
| `slow` | 700 | one-off moves that may be watched: Home's headline resolving |
| `scramble_char` | 70 | one character of a hover scramble |
| `field_trail` | 200 | a field cell cooling down after the pointer leaves |
| `row_collapse` | 180 | a row's gap closing |
| `stagger` | 30 | from one line of an unfolding list to the next |
| `caret` | 1 000 | one turn of the caret's blink |
| `undo` | 5 000 | how long the undo line stays in the status line |
| `notice` | 4 000 | how long a line with nothing to undo stays |

**Three levels,** in Settings, Appearance: full, reduced and off. Reduced is the default: it keeps motion that
explains a change of state (where the row went, what the app is doing) and drops motion that only decorates text.
Full is the owner's choice. Off changes everything at once. When the computer asks for reduced motion, the level
is off until the owner chooses another.

**The owner never waits.** A command runs at 0 ms of its key: its result is there in that frame, the status line
prints then, and the counts change with it. Input is never held: a key or a click during a movement acts at once
and the movement jumps to its end. A scrambling character changes once, so nothing flashes.

| | What moves | Full | Reduced | Off | What it tells |
|---|---|---|---|---|---|
| M1 | the character field | cells within 60 px of the pointer flip to code glyphs, the nearest most often, and flip again while the pointer stands still; each is back 80 to 200 ms after its flip, taking new glyphs on the way once the pointer has left it; near Home's text with 0.2 to 1 of the chance and opacity | on Home only, as at full | still dots | where the pointer is; it carries no fact |
| M2 | the emblem on Home | cells under the pointer flip and settle back; by itself one cell at a time twinkles to a code glyph (about 7.5 a second, half of the ink ones in cyan) for 250 to 750 ms | the same | still | whose program this is |
| M3 | scramble | a label resolves from the left on hover, 70 ms a character, at most 300 ms; a headline on load, 700 ms | no hover; Home's headline once per launch | none | that a control can be pressed; that a page was just made |
| M4 | a new row arrives | the row decodes in over 300 ms; the unread square blinks once | fades in, 125 ms; the square blinks once | the row is there | that mail came and which row it is |
| M5 | Done | the words dissolve (125 ms), the row closes (180 ms); undo line with a 5 s bar | the row closes (180 ms); undo line with its bar | the row is gone; undo line without a bar | which row went, and that it can be taken back |
| M6 | Snooze | the row slides 24 px right and fades (300 ms), the gap closes (180 ms) | fades, 125 ms | gone | that it went aside, not away, and until when |
| M7 | a message opens | wipes in from the top behind a 1 px cyan line, 300 ms | fade, 125 ms | at once | that the reader's content changed |
| M8 | the command line | the caret blinks; the list unfolds a line every 30 ms; the run line is cyan for 125 ms | the caret blinks; results appear together | caret still; results appear | where typing goes and which line ran |
| M9 | sync | a bar of ten characters fills as the work is done | the same: it is information | text only | that the app is working, how far, how it ended |
| M10 | send | the composer folds onto the status line (300 ms, then 125 ms) | composer and reader cross-fade, 125 ms | the status line prints | where the draft went and when it has left |
| M11 | a screener answer | the card's edge takes the answer's colour; it leaves up or down 24 px (300 ms); the next steps up (180 ms) | the card fades; the next steps up | the next card is there | which answer was given and who is next |
| M12 | a change of place | the old text scrambles out (125 ms), the new resolves (300 ms) | fade, 125 ms | at once | that the whole body changed, and to which place |
| M13 | an account's check | lines print one by one as their steps run | the same: it is progress | lines appear as they finish | which step runs, which passed, where it failed |
| M14 | dark and light | colours cross-fade, 300 ms | cross-fade, 125 ms | at once | that the change is the app's answer to the setting |

M12 plays for a change of place only, not for another mailbox, view or range. In the file a movement is a row of
still key frames with their times.

## Planned and coming soon
The file carries two tags that are notes of the file and never show in the app. **MISSING UI:** the app has the
command and no screen for it yet (the list of drafts, removing an account, the templates and the scheduled list,
the senders whose pictures always load). **PLANNED:** a feature of the roadmap, drawn on the Planned page.

The app has one mark of its own, **coming soon**. Its rule: everything that is designed is shown in the app. A
surface that is not implemented yet is shown faint (`muted` text, dashed edges, never a fade), cannot be pressed,
takes no keyboard focus, and carries the mark once: on the section's head, the row or the control, not on every
child. The mark is a tag in `muted` with a dashed `muted` edge, with no signal colour. A switch in such a row is
the unavailable one.

The Planned page is direction, not specification: it shows where a feature would stand and how it would look
(search in message text, several messages at once, files on a draft, one event of a series, sign-in at the
provider, machine text and an agent's request, the logs, smart folders). Its numbers and steps are invented, and
each feature's behaviour is decided when it is designed. Two things on it are ideas for the future, not promises:
- **The plugin example** shows a plugin filling standard places with the existing sets (a command, a panel under a
  message, a settings page, lines in a log) and drawing nothing of its own.
- **The mobile sketch** is nine phone screens, 390 × 844, with the command line at the bottom. It is a direction
  to look at. One thing is already known to be wrong in it: the chrome round an open message takes too much of the
  reading area.

## Limits
- **The code follows most of this page.** What it does not follow yet is listed in
  [the design file's README](design/README.md#not-in-the-code-yet).
- The file decides the look, not the behaviour of features that are not built. Where a screen is new against the
  app, the engine work behind it is not decided here.
- The file does not define what counts as a tracker, so the content bar's tracker count has no rule yet.
- The desktop draws the tray, its menu and the notifications; the file shows them on a grey wireframe that copies
  no desktop, so their final look is not decided here.
- Motion is drawn as still frames; durations are tokens, and the file plays nothing.
- Only 1440, 1100 and 800 px widths (and Settings at 1000) and 100 % and 130 % text are drawn. Hover, focus and pressed states are not
  drawn again at 130 %. A row's height in the file at 130 % is 1.3 × its height; the app may come out a few px
  less.
- The interface is drawn in English only. The picture of the phone code is a placeholder and decodes to nothing.

## Revision history
| Date | Change | Summary |
|---|---|---|
| 2026-11-05 | major | First version: colours, distances, type and the mail list window (#30) |
| 2026-11-05 | minor | The app icon (#31) |
| 2026-11-05 | minor | The reader (#33) |
| 2026-11-05 | minor | The page mark joins the app icon in `icons.rs` (#33) |
| 2026-11-05 | minor | Adding an account (#34) |
| 2026-11-06 | major | Colour tokens from the mode and an accent preset; Electric, Aurora and Citrus; teal and coral gone (#81) |
| 2026-11-06 | minor | Pictures: each `ui_shot` picture beside its mood sketch, `just side-by-side` (#81) |
| 2026-11-06 | minor | Glass and floats: colour fields, glass layers, the float surfaces, the one gradient helper (#82) |
| 2026-11-06 | minor | Glass and floats: nine-slice shadows, the sweep of unused pictures, no shadow over sharp text (#82) |
| 2026-11-06 | minor | Type: Figtree, font tokens, unread by weight, the text scale and layout from the font's metrics (#83) |
| 2026-11-06 | minor | Icons: `IconId`, styles, the painter per set, Phosphor; the marks moved onto it; the icon sheet (#84) |
| 2026-11-06 | minor | Components, the avatar colours, the keymap, motion tokens and the reduced-motion flag; the gallery (#85) |
| 2026-11-06 | minor | The group badge's icon: solid, white wherever it reaches 3:1; reduced motion and scrolling (#85) |
| 2026-11-06 | minor | Appearance choices and their commands; Inter; Lucide with its disc for Fill and Duotone (#86) |
| 2026-11-06 | minor | The appearance settings page with its preview, the switch, its pictures (#87) |
| 2026-11-06 | minor | The mail list window: the navigation rail, the list card, avatars, hover actions, screener and group cards, text buttons (#88) |
| 2026-11-06 | minor | The hover actions act on the sample: their keys, the slide-out, the toast with Undo, their pictures (#89) |
| 2026-11-06 | minor | The reader on the design system: mail-action toolbar, More menu, thread cards, Reply row, link status float; unavailable buttons, tooltip notes, menu choices (#90) |
| 2026-11-06 | minor | The design file in FigCraft, `docs/design/` (#96) |
| 2026-11-06 | minor | The FigCraft design file is the source of truth for the look (#96) |
| 2026-11-06 | minor | The file's rules in the code: text gradient, dark accent, outline group icons, inset rows; secondary buttons, `ui.frame.set` (#104) |
| 2026-11-06 | minor | A display font token (26 Bold) for the reader's subject (#104) |
| 2026-11-06 | minor | Lucide's Fill draws no disc (#104) |
| 2026-11-06 | minor | Accounts are added and listed on the settings sheet; the empty window is one card (#104) |
| 2026-11-08 | minor | The switch and the time picker components; the time picker in the gallery picture (#129) |
| 2026-11-08 | minor | The command palette, the Shortcuts page and the calendar's empty state; keys kept between runs (#130) |
| 2026-11-08 | minor | The acts work on a real account; Move to a folder, flag and read in More; opening marks read; undo through the history (#131) |
| 2026-11-08 | minor | The composer, its chips, suggestions and floating panel, and the Sending page (#132) |
| 2026-11-08 | minor | Reply, reply all and forward open the composer; Templates and the blank line (#133) |
| 2026-11-08 | minor | The screener card, New senders, the Let in card, the inbox views, the group menu and the Triage page (#134) |
| 2026-11-08 | minor | Later on the rail, the pills under the header, the marks on a row, Done all, Snooze, Pin and the time picker in the reader, the pin switch, Done all past the undo history, Not done in the reader (#135) |
| 2026-11-08 | minor | Undo send, send later, the outbox, the bounce mark and banner, Undo send on the Sending page, the outbox pictures (#136) |
| 2026-11-08 | minor | The Notifications page; Notifications and Triage built on the sheet (#139) |
| 2026-11-08 | minor | The calendar's views, side panel and details; the calendar pictures (#147) |
| 2026-11-08 | minor | The event editor, Edit and Delete in the details, New event in the toolbar, the toast with Undo (#148) |
| 2026-11-08 | minor | The invitation card in the reader and its pictures (#149) |
| 2026-11-08 | minor | Translucent cards, one even selection, the rail's stops, event tints and one solid float (#205) |
| 2026-11-08 | minor | Segmented controls, the select, the filled check boxes and the event tokens in the calendar (#206) |
| 2026-11-08 | minor | The list's header, stripe and row spacing, the view switch, the toast and the time picker (#208) |
| 2026-11-08 | minor | The add form's outgoing server and CalDAV address; an account's settings (#214) |
| 2026-11-17 | major | Rewritten for the Qubernetic design file (#226) |
| 2026-11-21 | minor | The invitation's one primary and Keep my answer, the add form's buttons, Save until a change, the focus edge beside the thumb; 23 pages (#327) |
| 2026-11-21 | minor | The app follows most of this design; what it does not is in the file's README (#332) |
| 2026-11-21 | minor | The invitation card's described state: the event's description inside the card (#342) |
| 2026-11-21 | minor | Home greets by the word of the sending name the owner chose (#349) |
| 2026-11-21 | minor | New message at the top of the index; the text-button set's icon-and-key and icon-only states (#354) |
| 2026-11-21 | minor | Drafts has a glyph of its own in the index (#354) |
| 2026-11-21 | minor | Settings in a narrow window (#368) |
| 2026-11-21 | minor | The least width cannot be made narrower; the title bar under the results and the editor (#412) |
| 2026-11-21 | minor | M1 and M2: the field flips under a still pointer, the emblem twinkles by itself (#418) |
