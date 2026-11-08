# InboxCraft design

> **Last reviewed:** <date> · **Last updated:** 2026-11-08 · **Change:** minor

InboxCraft's own look: the colours, distances and font tokens of the front end, and the rules the screens follow.
Everything here is original. Nothing is copied, traced or recreated from another mail program (see `AGENTS.md`,
"Assets"). The values live in `crates/ui-egui/src/theme.rs`; this page explains them, and the tests in
`crates/ui-egui/src/theme_tests.rs` hold them to the contrast rules below. The design file
[`docs/design/inboxcraft.figcraft`](design/README.md) is the source of truth for the look: its tokens, components and
every screen of the app, in FigCraft. Where this page or `theme.rs` disagrees with it, they are changed to match;
[its README](design/README.md#not-in-the-code-yet) lists what is not in the code yet.

## Principles
- **Two choices make every colour.** The mode (System, Light or Dark) and the accent preset (three gradient stops
  and an accent) give every colour token; a view names tokens, never colours. `theme.rs` is the only file that names
  a colour, and a test refuses a colour constant anywhere else in `crates/ui-egui`.
- **Vivid colour in a few places, quiet surfaces elsewhere.** The gradient runs at full strength in one place (the
  navigation rail, to come) and as accents elsewhere: the selected row, the unread marker, the badge, links. The
  surfaces are near-neutral with a trace of the preset's middle stop.
- **Glass without blur, and readability beats glass.** egui cannot blur what lies behind a shape, so glass is faked:
  soft colour fields painted once behind the window, and translucent layers over them with a light top edge and a
  soft shadow. Translucency is only ever laid over the colour fields, never over text; everything that pops up over
  content is solid. See "Glass and floats".
- **One gradient helper.** Every colour transition (shapes, lines, text, shadows) is painted by
  `crates/ui-egui/src/gradient.rs`, dithered, so its quality is decided in one place; a test refuses a gradient
  painted anywhere else.
- **Contrast is checked, not hoped for.** Every text token is moved in lightness until it meets WCAG 2 AA (4.5:1) on
  each surface it sits on, and every marker, focus ring and field edge until it meets 3:1. The tests run this for
  every preset in both modes, and fail when a preset or a rule breaks it.
- **Nothing depends on colour alone.** Unread rows have a dot and heavier type; flagged rows have a flag; the
  account colour stripe repeats what the folder pane says in words. A screen reader hears "unread" and "flagged".
- **Every control has an accessible name.** Rows are named by sender, subject, date and state; folders by name,
  account and unread count. The UI tests refuse a control without one.
- **Message text is data.** Subjects, names and previews are shown on one line: line breaks, tabs and NUL become
  spaces, bidi controls that could reorder what is shown are removed, and each is cut to 300 characters.

## Appearance choices
The owner's choices are the engine's `Appearance`, changed only through the `ui.*.set` commands (all `safe`):

| Command | Parameters | Choices |
|---|---|---|
| `ui.theme.set` | `mode` | `system` (default), `light`, `dark` |
| `ui.accent.set` | `preset` or `custom` | `electric` (default), `aurora`, `citrus`; or four `#rrggbb` colours, the gradient's three stops and the accent |
| `ui.font.set` | `font` | `figtree` (default), `inter` |
| `ui.icons.set` | `icons` | `phosphor` (default), `lucide` |
| `ui.text_scale.set` | `percent` | 90, 100 (default), 110, 120, 130 |
| `ui.motion.set` | `reduce` | `on`, `off`, `system` (default) |
| `ui.frame.set` | `frame` | `system`, `top_row`, `strip` (default) |

Each answers the whole appearance. Links and accent text in dark mode take the preset's middle stop mixed 30 % with
white, made readable at 4.5:1; the separate accent is one of the account colours. A custom accent must still
reach WCAG AA (4.5:1) on the lightest dark surface (the dark float, `#1E2330`); one that does not is made lighter in
steps of 1 % lightness, hue and saturation kept, and the answer has `nudged: true`, the colour asked for
(`requested`), the contrast reached and why. Every other token comes from the four colours as from a preset and is
checked by the theme. The window installs a changed appearance in the next frame and keeps it in
`inboxcraft/appearance.json` in the config folder, so it is there after a restart.

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
| background | `#F3F5F9` with 3 % of the middle stop | `#0D1016` with 5 % | the base of the colour fields behind the window |
| panel | `#EBEEF3` with 3.6 % | `#141821` with 6 % | buttons and the remote-content banner |
| surface | `#FBFCFD` with 1.8 % | `#10131A` with 3 % | the base of the selection and the cards; the settings sheet; a surface text is checked on |
| card | surface at 80 % | surface at 62 % | the list's and the reader's cards, without a shadow; the colour fields show through |
| field | `#FFFFFF` with 0.9 % | `#090B10` with 1.5 % | inside text fields |
| selection | surface with 16 % of the middle stop | with 30 % | the selected row, folder, pill or segment: one even tone |
| text | `#161A22` tinted, readable at 4.5:1 | `#EEF1F6` tinted, readable at 4.5:1 | text (readable also on glass and on a card over each field and on the selection, hovered or not, and on the float) |
| muted | `#5D6676` tinted, readable at 4.5:1 | `#9AA3B2` tinted, readable at 4.5:1 | previews, dates, addresses, section headings |
| accent | the middle stop mixed 62/38 with black, readable at 4.5:1 | the preset's accent, readable at 4.5:1 | links, accent text |
| on accent | white or deep ink, by contrast | the same | text on the accent |
| unread | the middle stop, readable at 3:1 | the same | the unread dot |
| flag | the last stop, readable at 4.5:1 | the same | the flag; text that asks for a second look |
| focus | the first stop, readable at 3:1 | the same | the keyboard focus ring, the hovered control's edge |
| field border | muted towards the surface, readable at 3:1 | the same | the edge of fields and windows |
| control edge | text-tone at 18 % | near-white at 18 % | the quiet edge of a pill that is not selected and of a segmented control |
| badge / on badge | panel with 16 % of the middle stop / accent, readable on it | 30 % / the same | unread counts |
| error / on error | `#B3261E` readable / white or deep ink | `#F2B8B5` readable / white or deep ink | the error banner, error text |
| gradient | the three stops at full strength | the same | the edge of a selected pill or segment, the primary button, group badges (to come) |
| rail 1 to 3 / rail ink | each stop darkened only as far as white needs for 3:1, at most 25 %; otherwise the full gradient / white, or the better ink on the full gradient | the same | the navigation rail and its icons |
| ink | deep ink `#05060A` | the same | text on the gradient where white does not read |
| glass / edge / highlight | near-white at 72 % / text-tone at 8 % / white at 90 % | near-white at 5.5 % / 9 % / 7 % | the glass layers of the panes, their edge and light top edge |
| shadow | text-tone at 10 % | black at 35 % | under glass and floats |
| float | near-white, solid | `#1E2330` tinted, solid | everything that pops up over content: menus, tooltips, popups, hover actions, toasts, the command palette, the event editor |
| line / hover | text-tone at 7 % / 4.5 % | near-white at 6 % / 6 % | row separators and hovered rows on glass |
| colour fields | each stop at 30 % | each stop at 34 % | the soft fields painted once behind the window |
| event 1 to 6 | the account's colour at up to 20 %, less while text on it would not read at 4.5:1 over every glass | at up to 30 % | an event's fill, one per account; its edge is the account's colour at full strength |

**Text on the gradient** (a primary button) sits on a text gradient: the preset's stops darkened together in steps of
2 % until white reads at 4.5:1 on every stop, as long as that takes no more than 40 % (Electric 14 %, Aurora 30 %);
otherwise the full gradient with the ink that reads better across it (Citrus). The rail has its own stops, darkened
one by one.

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
| row height | from the font | a message row: `s` above and below, the sender's line, `xs`, the subject's line; each line as high as its tallest font's own line height at the current text scale |
| folder pane | 240 | the folder pane's default width (it can be resized); the search field's width |
| corner | 6 | the corner radius of fields, selections and buttons |
| card | 14 | the corner radius of glass layers and floats |
| shadow | 18 | how far the soft shadow of a glass layer or a float reaches; it falls a third of that downwards |
| gap | 10 | the colour fields between two glass layers and around the window's edge |

## Type

The typeface is **Figtree** (SIL Open Font Licence 1.1), bundled from its upstream release v2.0.3 in
`assets/fonts/figtree/` (Regular, Medium, SemiBold, Bold) and compiled in; nothing is fetched at run time. The
owner can choose **Inter** instead (`ui.font.set`; the same licence, release v4.1 in `assets/fonts/inter/`, the same
four weights): every token keeps its size and weight and only the typeface changes. Both are installed from the first
frame, so a change of typeface is a change of tokens. It takes
the place of egui's Ubuntu Light everywhere; egui's Hack stays the monospaced face (with Figtree behind it for signs
Hack lacks), and egui's emoji and icon fonts stay the last fallback. Each weight is a font family of its own, because
egui has no notion of weight.

Every text style is a **font token** of `theme::Fonts`: a weight and a size. A view names a token, never a family,
a size or an egui text style; `theme.rs` is the only file that names a font, and a test refuses any other.

| Token | Points | Weight | Used for |
|---|---|---|---|
| caption | 12 | Regular | dates, badges, addresses, section headings, the status line |
| body | 14 | Regular | running text, folders, fields, empty-state details |
| strong | 14 | SemiBold | bold text in a message |
| button | 14 | Medium | buttons |
| sender | 14 | Medium | the sender of a read message (list and folded thread rows) |
| sender unread | 14 | Bold | the sender of an unread message |
| subject | 14 | Regular | the subject of a read message, in the text colour |
| subject unread | 14 | SemiBold | the subject of an unread message |
| preview | 14 | Regular | the first words after the subject, in the muted colour |
| heading | 15 | SemiBold | account names, empty-state titles, a message's sender, small headings in a message |
| title | 20 | Bold | the view's name, page titles, the app name |
| display | 26 | Bold | the reader's subject |
| mono | 13 | Hack | preformatted text and code |

Sender and subject are strong and the preview muted; an unread row is set in heavier weights (Bold and SemiBold
against Medium and Regular) as well as marked by its dot, so it reads as unread in grey too.

**Text scale.** 90 %, 100 % (the default), 110 %, 120 % or 130 %, applied to every token
(`ui.text_scale.set`; the settings page comes with E1.16b). Layout follows the font,
never fixed points for text: the message rows, folder rows, folded thread rows and step marks take their height
from the font's own line height at the current scale, and every cut line is measured and ends in an ellipsis. The
reader's mail actions wrap onto a second line when there is no room, and the subject has the line below them to
itself and wraps rather than being cut; attachment tiles are measured first and wrap before they would stick out. `draw/fit_tests.rs` draws the list, a thread, the full header, the full-width
reader and the add-account page at 90 %, 100 % and 130 % and fails on text that sticks out of its row or button, is
cut at the side of its pane, or lies over other text.

## Icons
Views ask for an **`IconId`** (`crates/ui-egui/src/iconset/`), which names a meaning (Inbox, Done, Archive, Delete,
Snooze, Pin, Move, Reply, Reply all, Forward, Send, Search, Compose, Calendar, Attach, Settings, Notify, People,
Flag, More, File, Failed, Waiting, Skipped), never a glyph, and a **style** that carries meaning too:

| Style | Used for |
|---|---|
| Regular | lines: toolbars, rows, the attachment tile, a step that waits or was skipped |
| Fill | active and selected states: a flagged message's flag, a step that passed or failed |
| Duotone | empty states: the lines over the same shape filled at 20 % |

The theme names the icon set (`Theme::icons`). Each set maps every id onto a glyph of its font in an exhaustive
`match`, so a new id without a glyph does not build, and keeps its own size and optical alignment (`Metrics`).
`IconSet::paint` draws an icon into the middle of a square, as large as the square, in one colour token.

The default set is **Phosphor** (MIT), bundled from its upstream release 2.1.1 in `assets/icons/phosphor/`
(the Regular, Fill and Duotone fonts, unchanged) and compiled in; nothing is fetched at run time. A Phosphor glyph
fills its em square with a margin of its own (a ring covers about 81 % of it) and sits in the middle of the line,
so it is drawn at the square's side with no shift; a test paints the ring at 16, 20 and 32 points and fails when it
is more than a pixel off the middle. Like the text fonts, the icon fonts are named only in `theme.rs`
(`IconFont`); each is a font family of its own with nothing behind it, and a test checks that every glyph of every
id and style is in its font.

The second set is **Lucide** (ISC; the icons it took from Feather are MIT), chosen with `ui.icons.set` and bundled
from release 1.70.0 in `assets/icons/lucide/` (its one font, unchanged). Lucide has lines only, so Fill and Duotone
keep the line glyph. Fill draws nothing behind it: a disc there read as a smudge on the selection, and an active
icon already shows in the accent colour. Duotone draws a disc 90 % of the square across at 20 % of the icon's colour. A Lucide glyph fills its em square to the edge and sits a little high, so it is drawn at 92 % of
the square and moved down by 4.5 % of it; the same ring test holds it to a pixel of the middle.

The app icon is not from a set: it stays drawn in code (see "The app icon").

## Glass and floats
- **Colour fields** (`glass::backdrop`): the background with three soft fields of the preset's stops, at 30 %
  (light) or 34 % (dark) at their centres and fading out smoothly: the first stop at the top towards the middle, the
  middle stop at the top right, the last stop from the bottom right; the left, where the panes start, stays calm.
  The fields are one picture, rasterised once per window size (rounded up to 128 pixels, so a resize does not
  rasterise them for every pixel of the drag) and kept as a texture.
- **Glass layers** (`glass::Layers`): the folder pane, the list and the reader (or the card of the window with no account) are each a
  glass layer, `gap` apart and from the window's edge: the glass colour over the fields, a 1-point edge, a light top
  edge that fades in and out at its ends, and a soft shadow around it, clear under the layer itself so it does not
  darken the glass. The shadow is a nine-slice: one small picture of its corners and a 32-pixel stretch of each
  edge, tiled along the layer, so a resize never rasterises it again whatever the layer's size. All layers go into one place reserved right after the colour fields, shadows first, so a layer
  never covers another's content and translucency is never laid over text (a UI test checks every shape of the
  window in every preset and mode).
- **Cards**: the list's and the reader's cards sit on the glass layers in the `card` colour, the surface at 62 % dark
  or 80 % light, with a 1-point line and no shadow, so the colour fields show through. The screener card's gradient
  ring stays opaque inside, or the ring would show through it.
- **Floats** (`glass::float`): everything that pops up over content (menus, tooltips, popups, hover actions, toasts,
  the command palette, the event editor) is on one solid `float`, with a 1-point edge, a light top edge and the soft
  shadow, so text under it never shows through. A float is drawn after the content it covers. A UI test draws a
  menu over the reader's text and hover actions over a row in every preset and mode, and checks the float's text
  reads at 4.5:1 while the content under it does not change the picture at all.
- **Gradients** (`gradient`): a gradient is linear through stops, soft colour fields over a base (a gradient mesh),
  or a layer's shadow. Shapes get it as a picture rasterised once per gradient and size, every pixel dithered with
  a noise below one step of eight-bit colour added before rounding, so a wide gradient over few steps shows no
  bands (a test over 1024 pixels and eight steps of grey); lines and text are coloured at their vertices, dithered
  the same way. At the end of every frame the window gives back the pictures that frame did not paint
  (`gradient::sweep`), so a finished resize leaves no textures behind (a UI test resizes the window at scale 1 and
  2 and checks the kept texture bytes). egui's own fade at the
  ends of a scroll area is off: it was a gradient of its own laid over text.
- **No repaint while nothing moves.** Nothing in the look asks for another frame; a UI test runs the window idle in
  every preset and mode and counts no repaint and no picture rasterised again.

## Components
Every view is built from these (`crates/ui-egui/src/component/`). Each draws only with the tokens above, moves only
through the motion tokens below and shows a key only from the keymap; each has an accessible name.

| Component | What it is |
|---|---|
| Card (`card`) | A rounded card on the surface colour (corner `card`), its header in the heading token as tall as the font asks; a click on the header opens or closes it, and the body unrolls and rolls up over the card motion. For grouped inbox groups, the screener and the messages of a thread. |
| Icon button (`button::IconButton`) | An icon in a square one line of button text and a little room high; its hover fill fades in. Active: the Fill icon in the accent on the selection. Disabled (an action without a command yet): muted, takes no click, still has its tooltip; `unavailable` adds a note under the name that says why ("Not available yet"). On a gradient (the rail) it is drawn in one ink, its active and hover fills that ink at 24 % and 14 %. |
| Text button (`button::TextButton`, `button::text`) | A word, an optional icon before it, in a fully rounded field one control high, with its tooltip: quiet (a hover fill only), secondary (outlined in the field border) or primary (on the text gradient). `button::unavailable` draws one that cannot be pressed yet: faded to 45 %, no click, its tooltip saying why. |
| Pill (`pill`) | A fully rounded chip with a word, for filters and choices. Selected: the selection, a 1.5-point edge in the full gradient and a SemiBold label. Otherwise no fill and a quiet control edge. |
| Tooltip (`tooltip`) | A rounded bubble on the float: the action's name and its key as a chip (`Ctrl+F`), read from the keymap whenever it is drawn, so a changed key shows at once, and for a control that cannot be used yet a muted note under them. Every icon button has one. |
| Avatar (`avatar`) | A person's initials (first and last word of the name, else the address) on a round field of an avatar colour, white or the deep ink by contrast. |
| Badge (`badge`) | An unread count in a pill of the badge colour, or a group's outline (Regular) icon in the muted colour, without a background. |
| Hover action group (`hover`) | Icon buttons on a float at the right end of a row, fading in while the row is hovered or selected by keyboard, drawn over the row's text. |
| Toast (`toast`) | A message on a float that rises from the bottom edge, with Undo and its key chip; the Undo key works while it shows. |
| Popup (`popup`) | A float with menu rows (icon, name, key chip; `entry` without the icon, its name in line with the others) and choice rows (`choice`: one of a few, the chosen one marked with the Done icon in the accent, a screen reader hears which), opening over the card motion. |
| Switch (`switch`) | A round track one body line high with a knob that slides right when it is on, the track then on the selection; a screen reader hears its name and whether it is on. The settings sheet and the time picker use it. |
| Time picker (`time_picker`) | One picker for snooze, send later and reminders, on a float: its title, the presets still ahead (name, and the time short on the right: `17:30` today, `Sat 08:00` this week, `Fri 13 Nov` after), a preset behind hidden; a line, "Or type a time" and a field. Typed text counts as soon as it reads as a time (`typed_time`: `thu 9`, `12 nov 17:00`, `tomorrow`, `5pm`, `in 3 days`, `2h`, a day without a time at 08:00, always ahead of now), without Enter: the time it reads shows under the field with the Done icon in the accent, and is the time the confirm button uses, read at the same now in the same frame; text that reads as no time says so and the confirm button cannot be pressed. The alert switch (when the caller asks about one) keeps its last value from one opening to the next. Cancel, Escape, or the primary confirm button ("Snooze", "Schedule", "Remind me"); Enter confirms a typed time too. |

**Avatar colours.** Ten vivid colours (`theme::AVATARS`, the same in both modes and every preset, no teal or coral):
royal blue, sky blue, violet, fuchsia, raspberry, crimson, orange, amber, green and lime. The colour is picked from
the address alone, case and surrounding spaces aside, by a stable hash (FNV-1a), so one person has one colour on
every screen, in every session and in every version. Tests check that the same address always gets the same
colour, that the initials read at 4.5:1 on every avatar colour in both modes, that the palette is vivid and its
colours far enough apart, and that everyone in the sample mail gets an avatar colour from it. The reader shows the
sender's avatar in the message header.

**Keys.** The keymap (`keymap::Keymap`) names which key runs which action, by id (the engine's command ids once
they have a key; `ui.search.focus`, Ctrl+F, until then). Every action of the window is listed with its name and icon
(`keymap::actions`): the command palette shows them all and the Shortcuts page gives each one a key. One key runs one
action: binding a key takes it from the action that had it. Everything that shows a key (tooltips, menu rows, a
toast's Undo) reads it when it is drawn. A key without Ctrl, Alt or ⌘ runs nothing while a text field has the focus
or the settings sheet is open, so a letter typed is typed. The keys are kept in `shortcuts.json` beside the
appearance file, every action with its key or `null`; an action the file does not name keeps its first key.

## Motion
Short and snappy, nothing decorative. Every duration is a motion token (`motion::MOTION`), none longer than 0.25 s:

| Token | Seconds | Moves |
|---|---|---|
| `hover_fade` | 0.12 | hover actions over a row, a control's hover fill |
| `tooltip` | 0.10 | a tooltip, and egui's own fade of tooltips and menus |
| `row_slide` | 0.18 | a done row sliding out of the list |
| `card` | 0.16 | a card or a popup opening and closing |
| `toast` | 0.20 | a toast rising from the bottom edge |

Everything that moves runs through `motion::toggle` (towards on or off) or `motion::appear` (from the frame
something is first shown), and both read **one reduced-motion flag** (`motion::set_reduced`): with it on, every
motion lands at its end in the frame it changes, and egui's own animations stop too: fading areas
(`animation_time`) and scrolling to a place (`scroll_animation`). egui still smooths a mouse wheel's notches over
about 0.1 s with a factor fixed inside egui that no style changes, so wheel scrolling glides a little with reduced
motion on (see `docs/gaps.md`). A test refuses egui's animation calls anywhere else in the crate, and a test switches every component on with
motion and with reduced motion and finds the first still moving and the second at rest. The flag's default follows
the system where the system can be read (the web build), otherwise motion is on; the window holds it
through `ui.motion.set` (on, off or follow the system) and a switch for it comes with the settings page. While something moves the
window asks for the next frame; once it has landed, nothing does.

## The mail list window
- **Navigation rail** (the window's left edge, full height): the preset's three gradient stops at full strength from
  top to bottom, the one place the palette runs undiluted. Mail and Calendar sit at the top and the appearance
  settings at the foot, as icon buttons in the rail's ink: white wherever white reaches 3:1 on every stop, otherwise
  the deep ink (`pane::rail_ink`, tested for every preset in both modes); the active one carries a fill of the ink at
  24 %. Mail (Ctrl+1) and Calendar (Ctrl+2) leave the settings sheet; the settings button (Ctrl+Comma) opens the
  settings page and closes it again. Later (the Pin icon) opens "Later" on the pins, or on the pile shown last, and
  shows as active there; Mail leaves it for the inbox. The rail is one button and 8 points either side wide, and
  shows on every page.
- **Folder pane** (left of the list): the app name with New message (the Compose icon, C) at its right end, the account picker (all accounts or one), "Mailboxes" with the unified views
  (Inbox, Drafts, Sent, Archive, Junk, Trash: those some account has), then each account shown with its colour,
  label and address and its folders, the ones with a role first and the rest by path, nested folders indented. Every
  view and folder with unread mail carries a badge with the count.
- **Header** (top of the list): the view's or folder's name, whose mail it is (the narrowed account's address, or
  "All accounts") and how many messages; the search field on the right, with a Clear button while it holds text.
  Ctrl+F (Cmd+F on macOS) focuses it and Escape empties it.
- **List**: newest first, the rows on a card (the surface colour, corner `card`, a line edge) as tall as its rows
  or the room, scrolling inside it. One row per message: the account stripe when more than one account is shown,
  the unread dot, the sender's avatar two lines high, the sender and the date (time today, "Yesterday", the weekday
  in the last week, then the day and month, with the year for earlier years), the subject and the preview in muted
  text, and the Flag icon (Fill, in the flag colour, one subject line high) for flagged mail. Unread rows show the
  dot and a heavier sender and subject; the selected row the selection, a hovered one the hover fill, both
  with the `corner` radius inside the card.
- **Hover actions**: Done, "Mark as read" or "Mark as unread", and Delete on a float at the row's right end
  (`component::hover`), shown while the pointer is over the row or the row was selected with the arrow keys. The
  float is solid and drawn after the row, so it covers the date, the flag and the text under it.
  Their keys work on the selected row: D Done, R read or unread, Delete, and A Archive and S flag or not, which have
  no button on the row; while a text field has the focus they are the field's. They act on a real account as on the
  sample, through the engine's commands (`mail.seen`, `mail.flag`, `mail.archive`, `mail.delete`): the change shows
  at once and goes to the server after. Done marks the message read and moves it to its account's Archive, Delete to
  its Trash (in the Trash it deletes for good), read and unread change the dot and the weight.
- **Opening** a message marks it read (`mail.open`), as an act Ctrl+Z can take back; a message an undo brought back
  opens as it was.
- **Slide-out**: a row that Done, Archive, Delete or a move takes out of the list slides to the right and fades over the
  `row_slide` token, clipped to its place; then it goes and the next-message rule picks the selection. With reduced
  motion it goes at once.
- **Toast**: after an act, a toast rises from the bottom edge over the `toast` token (in place at once with reduced
  motion) and says what was done ("Marked as read", "Marked as unread", "Flagged", "Flag taken off", "Marked as
  done", "Moved to Archive", "Moved to Trash", "Moved to <folder>", "Deleted for good"), with Undo and its key chip
  (Ctrl+Z, Cmd+Z on macOS) when the act can be taken back (not a delete for good, not an act that changed nothing).
  It stays six seconds; Undo takes the act back through the engine's history (`history.undo`) with every act after
  it (the next message marked read as it opened), so the row is back where it was, read or unread as it was, and
  selected again when it was. A new act replaces the toast; once it has gone, Ctrl+Z still walks back through the
  session's acts, newest first, one at a time.
- **Screener card** (the unified inbox, on top, while senders wait): the waiting senders' avatars overlapping, "n
  new senders", "Held back until you let them in" and Review, the primary text button on the preset's gradient. The
  card's edge is the preset's gradient, 1.5 points wide. Review is placed first; the two lines are cut before it
  rather than running under it. Review opens **New senders**: the mail held (`screener.list`, `mail.list` of the
  screener folders and of the inbox rows marked as new), newest first. A row from a new sender says "New sender"
  in the accent colour after the sender, and a screen reader hears "new sender".
- **Let in card** (the reader, over a message the screener holds; the design file's Triage page): "Let <sender>
  in?" in the heading font, "Into" and a pill for each group (none chosen at first; a second press clears it),
  Accept (primary), Accept and open, Block, and what they do in the caption font; the card's edge is the focus
  colour. Under it "Preview · nothing is accepted yet": the message is read with `mail.read`, so it stays unread and
  nothing goes into the history, and it shows no group chip. Accept, Accept and open and Block are acts with a toast
  and Undo ("Accepted <sender> into <group>", "Accepted <sender>", "Blocked <sender>"); Accept and open shows the
  message in the inbox.
- **Grouped inbox**, as the Triage page chooses: **Cards** (the default): a card for each group (People, Updates,
  Newsletters with the newspaper icon, Invitations with the calendar), its header with the group's badge, its name
  and unread count, and "Done all"; the first three rows, and "Show all (n)" (then "Show fewer") when the group holds
  more. The rows no group holds (an account not grouped) follow on a card of their own. **Grouped list**: one card,
  each group's rows under a heading with its badge, name and unread count, the rest under "Other". **Simple list**:
  one list by date, as any folder. The account stripe shows in the simple list only.
- **Group menu**: a row of a grouped account has a context menu (a right click), and the reader's More menu the same
  choices: a radio choice for each group, the one the sender is in marked, and "Let InboxCraft sort it", with "Moves
  all mail from <address>, kept and future." under them. The choice is an act with a toast and Undo ("<sender> now
  goes to <group>", "<sender> goes where InboxCraft sorts it").
- `ui_shot` can show invented waiting senders and groups instead of the sample's (`"showcase": true`, and `"step": n`
  to select by keyboard, which also shows the hover actions), `"open": {"role": "senders"}` for New senders and
  `"view"` for the inbox view; `"put_away": true` snoozes, pins, parks and marks done some of the sample's mail,
  `"later"` opens a pile, `"show_done": true` turns "Show done" on and `"picker"` opens the time picker over the
  selected message with that text typed.
- **Pills under the header** (the design file's Triage page): in a folder or a view, "Show done", which lists the done
  mail in its old place (`mail.list` with `done`), dimmed to 55 %, with "Not done" in place of Done on its hover
  actions and on D; with it on, Done leaves the row in place, dimmed, instead of sliding it out. "Parked (n)" follows
  while mail of the place waits on the park pile and opens it. In **Later** a pill for each pile with its count:
  Snoozed (the snoozed folders, the one coming back first on top, each row dated by when it comes back), Pins
  (`pin.list`) and Parked (`park.list`); an empty pile says what it is for. Pills are left out while searching.
- **Marks on a row**: a pinned row shows the Pin (Fill, in the accent) left of the flag on the subject's line; a row
  back from a snooze says "Returned" in the accent after the sender while it is unread. A screen reader hears
  "pinned", "done" and "returned".
- **Done all** on a group's card marks every message of the group read and done, one toast ("Marked n as done") whose
  Undo brings them all back. The engine's history holds the newest 100 acts, and each message takes one (two when
  it was unread): when the group takes more, the toast says "Marked n as done, too many to undo" and has no Undo.
- **Buttons**: every button of the rail and the list has a tooltip with its name and, where its action has a key, the
  key chip. Text buttons (`button::text`) are quiet (a hover fill) or primary (the gradient, its ink white or the
  deep ink, whichever reads at 4.5:1 on every stop).
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
- **Where**: selecting a row opens the message to the right of the list, which narrows to a column. More, Full
  width hides the folder pane and the list; Escape, More, Leave full width or Ctrl+F (for the search) bring them
  back. More, Close ends the selection. The arrow keys step through the list while no field has the focus.
- **Toolbar**: mail actions, not view settings, as icon buttons on a small card of their own: Done, Archive and
  Delete; Snooze, Pin and Move to a folder; More, each group set apart by a thin line, wrapping when there is no
  room. Every action has a tooltip with its key. Done (D), Archive (A) and Delete (Delete) act on the open message
  as the list's hover actions do (the toast with Undo follows); on a done message, open under "Show done", Done reads
  "Not done" and brings it back, as the row does; Move to a folder opens a menu ("Move to") of the
  folders of the message's account that hold mail but its own, each by its full path, and the one chosen moves it
  there; with none it is drawn disabled. Snooze (H) opens the time picker under it ("Snooze until", the presets of
  `schedule.presets` still ahead, a typed time, Snooze); on a message in a sent folder the same button is "Remind me
  if no reply" and its picker ("Remind me if no reply by", Remind me) sets the follow-up reminder. The picker is a
  menu that a click outside, Cancel or Escape closes. Pin pins the open message, and shows as active ("Take the pin
  off") while it is pinned. Each is an act with a toast and Undo ("Snoozed until <time>", "Reminder set for <time>",
  "Pinned", "Pin taken off").
- **More**: a menu over the reader (on the float) with Flag or Take the flag off (S) and Mark as read or
  unread (R) for the open message, Park or Take off the park pile, Bring back now for a snoozed message, Take the
  reminder off for one with a reminder, then Full width or Leave full width, Close, and the next-message rule ("When a
  message leaves the list": show the next, show the previous, go back to the list), the chosen one marked.
- **Subject**: under the toolbar, in the title token, the largest text of the window, wrapped rather than cut;
  under it, for a thread, who wrote (each once, in the order they first wrote) and how many messages there are.
  Chips beside the group's say until when the message is snoozed ("Snoozed until Sat 08:00"), when its reminder is
  due ("Reminder Mon 09:00") and "Returned" while it is unread after a snooze; a screen reader hears each in full.
- **Thread**: cards, oldest first. The open message and the ones the owner unfolded are full cards on the surface
  colour with a line round them (corner `card`); the others are cards of one line (the sender's avatar, the sender
  strong, the first words muted, the date) that unfold on a click; an unfolded older message has Fold. The open
  message ends with Reply (primary, on the preset's gradient), Reply all and Forward; each opens the composer in the
  reader's place over a draft the engine filled in (the recipients, "Re: " or "Fwd: ", the quote), the text with the
  focus, or To for a forward. A draft being written is closed and kept first.
- **Header**: the sender's avatar, the name large with the address muted, the date on the right, "to ..." under
  it; the name and the address are cut with an ellipsis before they reach the date. Details shows From, Reply to,
  To, Copy and the date in full.
- **Remote content**: while remote pictures are blocked, a banner on the panel colour says how many and that loading
  them tells the sender the message was opened, with "Load once" and "Always for <address>". A blocked picture is a
  muted box with its alternative text, so the layout shows where it was.
- **Body**: plain text in the body size; a quote is a "Show quoted text (n lines)" button, and once shown it is
  muted text beside a bar, its attribution line small above it. HTML blocks: headings in the title or heading size,
  list items with their bullet or number, links in the accent colour and underlined, quoted blocks folded the same
  way.
- **Attachments**: tiles side by side, wrapping, each with the File icon (Regular, in the accent colour, one and a
  half body lines high), the name (cut to fit), the size and Save.
- **Link status**: only while the pointer is on a link, a float over the reader's bottom left corner says where it
  goes; a link whose text names another place shows "Careful: ..." in the error colour. The float takes no
  pointer, so the link under it stays hovered. A notice (a save, a copied link, an error) shows in a line at the
  bottom with Dismiss until it is dismissed.

## The composer
- **Where**: New message in the folder pane, C or the command palette writes a new message in the reader's place,
  the list a column beside it, as on the design file's "Composer · in place of the reader" screen. There is one at a
  time: New message brings back the one being written. A message pressed in the list, or the calendar, sends the
  composer to its floating panel at the window's bottom right (the float: the Compose icon, the subject or
  "New message", its key, Close); a press on it brings the composer back whole. The keys that act on a message (D,
  A, R, S, Delete, the arrows) rest while the composer hides it.
- **Layout**: the title ("New message", or the subject once there is one) with Close; then a line each, its name
  muted in a column on the left and a rule under it: From (the sender as "Name <address>" with a chevron, a menu of
  the accounts; the account's name muted at the right end), To (the chips, then the field, "Name or address" while
  it is empty; "Cc Bcc" at the right end shows those two lines), Subject; then the text, scrolling, and at the foot
  Discard (the Delete icon) and Templates (the File icon) on the left and Send (primary, Ctrl+Enter) on the right
  with why it is blocked before it and the caret of Send later after it.
- **Chips**: a fully rounded chip on the panel colour with the recipient's avatar, the name (or the address) and a
  cross that removes it; one that is no address is on the error colour with a warning instead of the avatar, and
  Send is blocked with "<address> is not an address" (or "Add a recipient to send" without any). Enter, a comma, a
  semicolon or leaving the line makes a chip of what was typed; Backspace in the empty field removes the last chip.
  A recipient is there once across To, Cc and Bcc.
- **Suggestions**: typing shows the people in the mail kept here whose name or address holds the text (at most six,
  not the owner's own addresses or a recipient already there) on a float under the field. The arrows move the
  highlight (the Done mark), Enter or a press takes it instead of what was typed, Escape puts the list away.
- **Templates**: Templates opens a menu of the templates of the account in From (disabled, "This account has no
  templates", without any); the one chosen goes in front of the text, its subject where there is none. Each blank it
  leaves is a `[[prompt]]` mark on the accent's tint in the text, as on the design file's "Template with a blank to
  fill" screen; a line on the panel colour over the toolbar asks for the first one (a field saying "Fill in "time""
  and Fill in), Enter or Fill in puts what was typed in every place of that mark, and Send is blocked with "Fill in
  "time" to send" while one is left.
- **Drafts**: the draft is kept at once and after every change (`compose.update`), with nothing to press and
  nothing shown. Close keeps it (the toast says "Draft saved"); Discard removes it here and on the server ("Draft
  discarded"); Send puts it in the outbox for the hold (`mail.send`) and the composer closes. A refusal (an account
  without a submission server) shows in the error banner and leaves the composer open with everything in it.
- **Undo send**: while a send waits for its hold, a toast counts it down ("Sending "Work party on Saturday" in 4 s",
  a new frame as each second ends) with Undo and Ctrl+Z, which take it back (`mail.unsend`) and open its draft again
  in the reader's place as it was; a draft being written is closed and kept first. The countdown toast stands in for
  the act's toast while it shows, and Ctrl+Z is its own. Once the hold is over the send is on its way and the toast
  goes.
- **Send later**: the caret beside Send ("Send later") opens the time picker ("Send later", the presets of
  `schedule.presets` still ahead, a typed time, Schedule) with "Send now instead" while a time is picked, and the
  note that it goes while InboxCraft or its tray runs and that a time missed sends at the next start and says so, as
  on the design file's "Send later" screen. A time picked makes Send "Schedule · Fri 08:00"; pressed, it sends at
  that time (`mail.send_later`) and the toast says "Scheduled for Friday 6 November, 08:00". A time picked that has
  passed blocks it with "The time picked has passed: pick another". A send that went late, its time passed while
  InboxCraft was not running, says so in a toast once ("Sent late, InboxCraft was not running at its time: <subject>"),
  besides the desktop's notification.
- **Outbox**: while anything is in it, "Outbox" (the Snooze clock) shows under the views in the folder pane with how
  many; it opens the outbox in the list's place, as on the design file's "Undo send and the outbox" screen, and gives
  way to the inbox once it is empty. The header says "2 waiting · 1 not sent". One card holds each send: the first
  recipient's avatar, "To <names>", the subject, then on the right where it stands on the panel colour ("Sending in
  4 s" or "Scheduled · Fri 08:00" with the clock, "Sending…" with the Send icon, "Not sent" with the Failed icon in
  the error colour) and its buttons: Undo for a send in its hold and Edit for one scheduled, which take it back and
  open its draft; Edit (open the draft) and Send again (primary, `outbox.retry`, the countdown toast again) for a
  refused one, whose server reply follows under the subject in the error colour. A screen reader hears "To <names>,
  <subject>, <where it stands>", the reply for a refused one, and each button with its subject ("Undo the send of
  "<subject>"").
- **Bounce**: a sent message a delivery report says did not get through says "Not delivered" in the error colour
  after the sender in its row ("not delivered" to a screen reader), and opens under a banner ringed in the error
  colour with the Failed icon: "Not delivered: <the reason the report gave>".
- **Sending page**: the settings sheet's Sending section starts with "Undo send", how long every send is held before
  it goes: segmented 5 s, 10 s, 30 s and 60 s (`send.set`), with the seconds now in its note, kept in `triage.json`.
  Then each account's signature in a field (its name and
  address over it) with Save, disabled until the text differs; an empty field clears it. A new message starts with
  the signature under "-- "; another account in From swaps it while it is still at the end as it was.
- **Triage page**: the settings sheet's Triage section (the Funnel icon; the design file draws only its place in the
  list of sections). Screener: segmented Before the inbox, Inside, marked, and Off (`screener.mode`), with what each
  does. Inbox: segmented Cards, Grouped list and Simple list. "Keep pins on done mail": a switch (`pin.set`). Then
  "Grouped inbox" and a row per account (the At icon, its name, a note with its address) with a switch, "Group the
  inbox of <account>" to a screen reader (`group.account`). The mode, the view and the pin switch are kept in
  `triage.json` beside `appearance.json`; the sample window takes the kept view and pin switch but keeps its own mode
  and never writes it.
- **Notifications page**: the settings sheet's Notifications section (the Notify bell), after the design file's
  "Settings · notifications and triage" screen. Two notes first: what the levels mean, and that a notification shows
  who the mail is from and its subject, which the desktop may keep in its list and on the lock screen. Then a row per
  account, "<account> · <address>", with segmented All, People only and None (`notify.set`); a screen reader hears
  "People only for <account>". Without an account a note says to add one first. "Returned snoozes" and "Follow-up
  reminders" are switches (`notify.set`). "Run in the background" (closing the window leaves InboxCraft running in the
  tray) and "Start with the computer" (as the design file names it: InboxCraft starts in the background at login)
  are switches of the app's desk (`ui_egui::desk::Desk`); where the window has none (the sample) both are faded and
  say "Not available in this window". Starting with the computer is faded while running in the background is off
  ("Turn on Run in the background first: a start at login opens no window"), and while the desktop is asked
  ("Waiting for the desktop's answer…"); a refusal shows under it ("Not changed: <why>.") until the sheet closes.
  Turning running in the background off also stops starting with the computer. The levels and the two notification
  switches are kept in `notify.json` beside `appearance.json`; the sample window neither takes nor keeps them.

## Adding an account
- **Where**: on the settings sheet's Accounts page. "Your accounts" heads it (the At icon in the accent colour, a
  note), with Add account at its right end: secondary, or primary while there is no account; then a row per account
  with its colour's dot, its avatar, its name in SemiBold and its address, muted. Without an account a framed box
  says the accounts show up there. The folder pane has no Add account: most people have one account.
- **First start**: the window with no account shows no folder pane, only one glass card in the middle, at most 640
  points wide: the Inbox icon (Duotone), "No accounts yet", a line on what to do, Add account (primary) and "Opens
  Settings · Accounts" under it. The button opens the sheet on Accounts.
- **Adding**: Add account opens the form on the same page under a link back ("Accounts" after the Back arrow); the
  check runs there too. Cancel, Done or the link lead back to the list; closing the sheet closes the form.
- **Form**: one column of labelled fields at most 560 points wide: the address with Detect beside it, the box for
  Mozilla's list of providers (off), what Detect found in small type (a redirect over plain HTTP or a guessed name in
  the flag colour, so it gets a second look) with "What Detect asked (n)" folded, then the name for the account,
  the name on sent mail, the user name, the password (dots), the IMAP server, the port and TLS or STARTTLS. A line
  says the password goes into the system's credential store. Errors show above the form in the error colour.
- **Check**: "Checking <address>", a one-line summary (muted while it runs, the accent colour when signed in, the
  error colour when a step failed), then one line per step with an icon one body line high: Waiting (Regular) in
  the accent colour while waiting, Done (Fill) in the accent colour when passed, Failed (Fill) in the error colour
  when failed, Skipped (Regular) muted when skipped, held back or not built yet. Under each step the host and port it
  tried and the certificate's issuer, muted, and the cause of a failure in the error colour. Done when signed in;
  Check again and Change settings after a failure; the link above the page leads back while it runs.

## Command palette
- **Where**: Ctrl+K anywhere opens it; it leaves the settings sheet. A float on a solid surface 640 points wide near the
  top of the dimmed window, as on the design file's "Command palette · Ctrl+K" screen.
- **Layout**: the field ("Type a command", with the magnifier), a line, then every action of the window with its icon,
  its name and its key at the right end, eight rows in view and the rest scrolling; under them "↑↓ to move · Enter to
  run · Esc to close" in the caption's muted colour.
- **Behaviour**: typing narrows the list to the actions whose name (or id) holds every word typed, the names that
  start with the text first; the first row is picked, on the hover fill. The arrows move the pick round the list,
  Enter or a press runs it and closes the palette; Escape or a press beside the float closes it and runs nothing. An
  action that cannot run here (Done with no message selected, Undo with nothing to undo) is listed faded and runs
  nothing. While the palette is open its keys are its own: the arrows step nothing in the list.

## Shortcuts
- **Where**: the Shortcuts section of the settings sheet, as on the design file's "Settings · shortcuts" screen.
- **Layout**: a line on the rule (one key runs one action), the field "Find a command" with Reset all keys at the
  right, then a row per action: its name, and at the right end its key as a chip ("No key" when it has none) with
  Remove (the Close icon) before it.
- **Behaviour**: a press on the chip makes it wait: "press a key…" in the accent with the focus ring. The next key
  pressed, with the modifiers held, becomes the action's; a key another action had moves, and a line over the list
  says so ("Ctrl+K was the key of Command palette; it has none now."). Escape stops the wait and keeps the old key;
  the press reaches nothing else. The new key shows at once everywhere it is shown.

## Calendar
- **Where**: the rail's Calendar button (Ctrl+2) shows the calendar screen in the place of the folders, the list and
  the reader; Mail brings them back. The strip's title says "Calendar · InboxCraft".
- **Empty state** (while no account has a calendar): one card in the middle with the Calendar icon in Duotone, "No
  calendars yet" and a line on what will show there. The mail's keys (the arrows, D, R, Delete) rest while it shows.
- **Layout**: the side panel on a card 260 points wide at the left, the view on a card beside it. Over the view a
  toolbar: the view's title in the title font ("2 – 8 November 2026", "November 2026", "Thursday 5 November 2026",
  "From today"), the Previous icon, Today and the Next icon (named after the step: "Previous week", "Next month",
  "Earlier days"), and at the right a pill per view, Day, Week, Month and Agenda, the one shown selected, and the
  primary New event button with the Add icon. The calendar opens on this week, on today; weeks start on Monday.
- **Events**: an event is a block in a tint of its calendar's colour (the server's, or an account colour by the
  calendar's place) with a 3-point bar of the colour at its left, its summary in the caption size, strong, and its
  times under it where there is room; hovered or picked the tint deepens. Each is a button named after its summary,
  its days and its times; a press picks it and the side panel shows its details, a second press or the details'
  Close lets it go.
- **Day and week**: a column per day under its short name and date ("Thu 5"; today on the selection; in the week a
  press opens the day), a lane of blocks over whole days (all-day events and events over more than one day, in
  lanes), then the hours, 44 points each with their labels in a 52-point gutter, scrolled to 07:00 (or the
  earliest event) at first. Timed events sit by their minutes, side by side in equal columns where they overlap;
  an event without length takes a quarter of an hour. A line in the accent marks the time now across today.
- **Month**: six weeks from the Monday on or before the month's first day under the weekdays' short names; each
  day's number is a button that opens the day (today on an accent disc, other months' days muted). Blocks over whole
  days run across the cells of a week in lanes; under them the day's timed events, one line each with the start,
  and "+N more" (a button to the day) when they do not all fit.
- **Agenda**: the 30 days from the focus, each day with events under its name ("Today · Thursday 5 November"), an
  event a row: its start, "All day" or "Until" the end, the summary and the place, and the event's own zone with its
  start in a chip at the right when its clock differs from the computer's. "Nothing in these 30 days." when empty.
- **Side panel**: the mini month (its name, "Month before" and "Month after", a button per day that puts the view
  there; today on an accent disc, the days shown on the selection), "Calendars" with a box per calendar and its
  colour dot (unticked hides its events), and the details of the picked event: the colour dot, the summary in the
  heading font and Close; both dates with the times ("Monday 16 November – Wednesday 18 November", "Sunday 8
  November, 08:00 – 11:00"); the event's own zone after the Globe icon, labelled, only when its clock differs from
  the computer's ("Event zone Atlantic/Azores: 07:00 – 10:00", with the dates when they differ too); the place; the
  description; the calendar, busy or free, private and the repeat ("Personal · busy · private · repeats weekly");
  and up to eight guests with their answers ("Petra Lindqvist · going"); then Edit (the Compose icon) and Delete
  (the Delete icon), secondary buttons.
- **Event editor**: New event and Edit open it as a float 440 points wide over the view's top right, as the design
  file's Calendar page shows it: "New event" or "Edit event" in the heading font; Title; Starts and Ends side by
  side, typed as "2026-11-06 13:00" (a day alone, "2026-11-06", for an all-day event); the boxes All day (which
  turns the times into their days and back) and Private; Repeat with a pill per preset (Never, Daily, Weekdays,
  Weekly, Monthly, Yearly; Custom, selected, for a repeat the presets do not name, kept until a preset is picked);
  Show as with Busy and Free; for a new event Calendar with a pill per calendar; Guests, addresses with commas. Only
  an edit of an event with guests adds, on a tinted box, "Tell the 2 guests about this change?" with the radio
  buttons Don't tell them (the default) and Send them the update. Why the last Save was refused shows over the
  buttons in the flag colour; Cancel (secondary) and Save (primary) end it, Escape is Cancel. Each field is named by
  its label.
- **Undo**: a saved event, a saved edit and a delete show the act's toast at the window's foot ("Event added",
  "Event saved", "Event saved; the guests are told", "Event deleted") with Undo and Ctrl+Z, which take the change
  back and read the calendar again; a deleted event reaches the server only once the toast's six seconds are over.

## Invitation card
A message that carries an invitation (iMIP) shows a card between its header and its text, as the design file's
"Invitation in a message" draws it, and its calendar part is no file tile:
- **Head**: a date tile on the accent's text gradient (the month in three capitals in the caption size, the day of
  the month in the title size, named by the event's time for a screen reader), the event's title in the heading
  font and under it, muted, when and where ("Saturday 7 November · 12:30 – 13:30 · The Gull café").
- **Changed**: on a panel-tinted chip with the Changed icon (arrows in a circle), strong, what a newer version changed
  since the one before: "Changed: time 11:00 → 12:30", "place The Gull café → Harbour Kitchen", "title", "repeat".
- **Answers**: Going (primary, the Done icon), Maybe and No (secondary), and in the caption, muted, "Your answer goes
  to odile@example.org at once." A press sends the answer at once and the reader's notice says "Your answer is on its
  way."; the card then says "You answered Going." with Change answer (secondary), which offers the three again with
  Keep my answer. An invitation that cannot be answered says why in the caption ("This invitation cannot be
  answered: you organise it.").
- **Cancelled**: "Odile Brannock cancelled this event." and Remove from calendar (secondary, the Delete icon) while
  the event is in the calendar, with the toast "Removed from your calendar" and Undo; else "It is not in your
  calendar."
- **A guest's answer**: a line per guest ("Béla is not going."). Any other calendar message: "This calendar message
  asks for no answer."
- **Versions**: under the card a strip on the panel tint at 75 % with the Older icon (a clock turning back) tells of
  the version before: "Out of date: the invitation from Monday showed 11:00. You answered Going." The card of a
  version a later one replaced is only such a strip: "Out of date: a later invitation replaces this one."

## Appearance settings
- **Where**: the settings button (the Settings icon, "Appearance settings") sits at the foot of the navigation rail
  and shows as active while the sheet is open; Ctrl+Comma opens it too, on Appearance, and the rail's Mail button
  leaves it. The sheet lies over the dimmed window; its sections on the left (Appearance, Accounts, Notifications,
  Triage, Sending and Shortcuts are built, the rest faded with "Not available yet") pick the page. Close, Escape or a press beside it leaves it, as do
  the rail's places and their keys; every way out forgets what the open sheet held (the text on the Shortcuts page,
  an action waiting for a key, the notes), so it opens clean the next time.
- **Layout**: the title, a line saying every change shows at once and is kept, then the controls in a column 440
  points wide with the preview beside them; when the card is narrower than the column, the preview and 24 points,
  the preview goes above the controls. The page scrolls.
- **Controls**, each running its command (see "Appearance choices") the moment it is pressed: Mode (pills System,
  Light, Dark); Accent (a pill per preset, each after a round swatch of its gradient with its accent in the middle,
  and Custom, selected while the owner's own colours are in use, which takes the typed colours as the button below
  does); Custom accent (Gradient start, Gradient middle,
  Gradient end and Accent, each a colour picker and a `#rrggbb` field, then "Use these colours"); Typeface (a pill
  per typeface, each name written in its own typeface); Icons (a pill per set, after three of its icons); Text size
  (pills 90 % to 130 %); Motion (the "Reduce motion" switch, with a line saying motion follows the system until the
  owner chooses).
- **Custom accent**: the fields start with the colours in use. Only `#rrggbb` is a colour (the `#` may be left out);
  while a field holds anything else a line says how to write a colour and "Use these colours" is disabled. The
  preview shows the typed colours as soon as all four are colours; the rest of the window keeps the colours in use
  until the button runs `ui.accent.set`. When the engine made the accent lighter, the field shows the colour used
  and a line says which colour was asked for and which is used instead.
- **Preview**: a card in the surface colour: the preset's gradient as a band with the app's name in the ink that
  reads over it, three invented rows as the list draws them (one selected, two unread), Reply and Archive in Regular,
  Pin and Flag in Fill in the accent, a selected and a plain pill, and a line in the link colour. It is drawn in the
  mode, typeface, icon set and text size in use.
- **Switch**: a track one body line high and 1.8 lines long, the selection when on and the field colour
  when off, with a knob that slides over the hover token (at once with reduced motion); a screen reader hears a
  switch, on or off.

## Pictures
`just shot` (or `cargo run -p inboxcraft-ui-egui --example ui_shot -- crates/ui-egui/shots.jsonl`) draws the
sample window without a display and writes the PNGs under `docs/images/`: the unified list, one account and a
search, the reader (a reply with its quote, a newsletter before and after its pictures load, attachments full
width, the full-width reader also in Aurora and Citrus as `reader-full-<preset>-<mode>.png`), and adding an account (the form, after Detect, the check under way, passed, failed), each in light and
dark in Electric; and the list, the thread and the failed check in Aurora and Citrus (`<shot>-<preset>-<mode>.png`).
The command palette whole and narrowed to "mode" (`palette-<mode>.png`, `palette-mode-<mode>.png`, `"palette"`),
the Shortcuts page with one action waiting for a key (`shortcuts-<mode>.png`, `"shortcuts"` and `"capture"`) and
the calendar's empty state (`calendar-empty-<mode>.png`, `"first"` and `"calendar"`) and its views over the
sample's three calendars (`calendar-week-<mode>.png` with the ferry's zone in the details,
`calendar-month-<mode>.png` with a trip over three days picked, `calendar-agenda-<mode>.png`, `calendar-day-<mode>.png`;
`"calendar_view"` and `"event"`) and the event editor (`event-new-<mode>.png`, `event-edit-<mode>.png` with the
question to the guests, `"edit_event"`; `event-deleted-<mode>.png` with the toast, `"delete_event"`) and the
invitation card over the sample's invitations (`"invitations"`: `invite-update-<mode>.png`, the update with what
changed and the answer given before, `invite-outdated-<mode>.png`, the first invitation as the strip) are drawn in
light and dark, and so are the
composer (`"compose"`: `compose-new-<mode>.png`, `compose-written-<mode>.png`, `compose-bad-address-<mode>.png`,
`compose-suggest-<mode>.png`, `compose-floating-<mode>.png` with a message opened over it, `compose-reply-<mode>.png`
and `compose-template-<mode>.png` with a blank left) and the Sending page
(`settings-sending-<mode>.png`, `"sending"`), the first account signed with an invented signature. `"outbox"` draws
the sample with its first account able to send and its sent message bounced: the outbox with a send counting down,
one scheduled and one refused (`outbox-<mode>.png`, `"waiting"`), the countdown toast of a message just sent
(`undo-send-<mode>.png`, `"countdown"`), the composer with a time picked and its picker open
(`send-later-<mode>.png`, `"schedule"`) and the bounced message open (`bounce-<mode>.png`, `"bounce"`). The screener
and the grouped inbox are drawn in light and dark too: New senders with the Let in card
(`screener-senders-<mode>.png`), the toast after Accept into Updates (`screener-accepted-<mode>.png`), the cards
after the rota's sender went to Newsletters (`grouped-cards-<mode>.png`), the grouped list (`grouped-list-<mode>.png`)
and the Triage page (`settings-triage-<mode>.png`).
A script line picks its preset with `"preset"` and its text scale with `"text_scale"` (90 to 130); the list with
the reader is also drawn at 90 % and 130 % (`list-text-<percent>-<mode>.png`). A line with `"icons": true` draws
the icon sheet instead of the window: every icon in Regular (text colour), Fill and Duotone (accent) at 24 points
and in Regular at the body size (muted), on the panel colour (`icons-<preset>-<mode>.png`, `icons-<mode>.png` for
Electric). A line with `"components": true` draws the component gallery instead: cards open and closed, icon
buttons (active and disabled), pills, badges, the sample's people as avatars, a row with its hover actions, a
tooltip, a popup menu and a toast with their key chips, and the time picker for snooze at Friday 14:30 with
`thu 9` typed and a preset behind hidden (`components-<preset>-<mode>.png`, `components-<mode>.png`
for Electric). `"font": "inter"` and `"icon_set": "lucide"` draw a line in the second typeface or icon set: the
list with the reader in Inter (`list-inter-<mode>.png`), and the icon sheet and the gallery in Lucide
(`icons-lucide-<mode>.png`, `components-lucide-<mode>.png`). `"settings": true` opens the appearance settings
page: in Electric, Aurora and Citrus (`settings-<preset>-<mode>.png`, `settings-<mode>.png` for Electric), in Inter
(`settings-inter-<mode>.png`) and with Lucide (`settings-lucide-<mode>.png`). `"act"` does a hover action to the
selected row and shows its toast (`list-toast-<preset>-<mode>.png`, Done; `reader-archive-<mode>.png`, Archive on
the open message, the next one open); with `"slide": true` the row of a Done, Archive or Delete is caught sliding
out instead (`list-slide-<preset>-<mode>.png`); the hover actions themselves show on
the keyboard-selected row of `list-showcase-<preset>-<mode>.png`. Every picture but a slide is drawn with reduced
motion, so it shows where each motion ends. Every UI change shows them in its PR.

Each picture is also shown next to the mood sketch of the same preset and mode, to see at once where the effect
falls short (the aim is not to match numbers). Put the sketches, from the planning issue's attachments, in
`plan/mood/` as `<preset>-<mode>.png` (for example `aurora-dark.png`); `plan/` is gitignored and they are never
committed. `just side-by-side` (`cargo xtask side-by-side`) then reads the shot script, scales each sketch to the
height of its picture and writes the pair, picture on the left, to `plan/side-by-side/<picture name>`. A line
without a preset pairs with Electric; a screen the sketches do not show pairs with the sketch of its preset and
mode all the same. A missing picture or sketch is reported and fails the command. `--script <file>`, `--mood <dir>`
and `--out <dir>` name other places. The pairs go into the PR, not into the repository.

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
