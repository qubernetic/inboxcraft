# Asset attribution

Every non-code asset in this repository (icons, images, fonts, sounds, sample files) is listed here with its author,
source and licence. `cargo xtask assets` (part of `cargo xtask ci`) fails if an asset file has no row.

**Policy (mandatory):** InboxCraft contains **no iconography, images, templates, themes, sounds or fonts from Spark,
Mailspring, Outlook, Gmail or any other vendor.** Every asset is original, CC0, OSI open source or under a Creative
Commons licence that allows redistribution. The one exception is InboxCraft's own identity: the InboxCraft mark and
wordmark and the Qubernetic mark (under `assets/brand/` and in the design files) are trademarks of Qubernetic, all
rights reserved, and not under MIT or Apache-2.0 (see `NOTICE`).

Art drawn in code is original and has no file to list: the sample mailbox (`crates/ui-egui/src/sample.rs`). A file
exported from that code has its row below.

| Asset | Author | Source | Licence | Notes |
|---|---|---|---|---|
| `assets/brand/inboxcraft/linux/hicolor/<size>/apps/io.github.qubernetic.inboxcraft.<ext>` | Qubernetic | the Qubernetic brand repository, tag v0.3.0, `products/inboxcraft/linux/hicolor/`, unchanged | trademark of Qubernetic, all rights reserved; not under MIT or Apache-2.0 | the InboxCraft app icon in the freedesktop icon theme sizes (PNG and the scalable SVG); the Flatpak installs the whole tree into `/app/share/icons/hicolor/` |
| `assets/brand/inboxcraft/tray/<file>` | Qubernetic | the Qubernetic brand repository, tag v0.3.0, `products/inboxcraft/tray/`, unchanged | trademark of Qubernetic, all rights reserved; not under MIT or Apache-2.0 | the tray icons (plain and unread, light and dark, symbolic SVG); the unread and plain dark PNGs at 16, 22 and 24 px are the tray icon (`crates/ui-egui/src/icons.rs`), the light ones are built in but not shown yet, the symbolic SVGs are not used |
| `assets/brand/inboxcraft/svg/<mark>.svg` | Qubernetic | the Qubernetic brand repository, tag v0.3.0, `products/inboxcraft/svg/`, unchanged | trademark of Qubernetic, all rights reserved; not under MIT or Apache-2.0 | the InboxCraft mark, compact and micro marks, wordmark and horizontal lockup, light and dark; not yet used by the code |
| `assets/brand/inboxcraft/png/icon-<size>.png` | Qubernetic | the Qubernetic brand repository, tag v0.3.0, `products/inboxcraft/png/`, unchanged | trademark of Qubernetic, all rights reserved; not under MIT or Apache-2.0 | the InboxCraft app icon, 16 to 512 px; `icon-256.png` is the window icon (`crates/ui-egui/src/icons.rs`), the other sizes are not used yet |
| `assets/brand/qubernetic/lockup-horizontal-<mode>-600.png` | Qubernetic | the Qubernetic brand repository, tag v0.3.0, `logo/png/`, unchanged | trademark of Qubernetic, all rights reserved; not under MIT or Apache-2.0 | the Qubernetic horizontal lockup, light and dark, 600 px; not yet used by the code |
| `assets/fonts/audiowide/Audiowide-Regular.ttf` | Astigmatic (Brian J. Bonislawsky) | Google Fonts, taken from the Qubernetic brand repository, tag v0.3.0, `fonts/audiowide/`, unchanged | OFL-1.1 | only for the names `qubernetic` and `InboxCraft` (`crates/ui-egui/src/theme.rs`); licence text in `assets/fonts/audiowide/OFL.txt` |
| `assets/fonts/audiowide/OFL.txt` | Astigmatic | the Qubernetic brand repository, tag v0.3.0, `fonts/audiowide/OFL.txt`, unchanged | OFL-1.1 | the licence of the Audiowide file beside it |
| `assets/fonts/plex-mono/IBMPlexMono-<weight>.ttf` | IBM Corp. | IBM Plex (https://github.com/IBM/plex), taken from the Qubernetic brand repository, tag v0.3.0, `fonts/ibm-plex-mono/` (Regular, SemiBold), unchanged | OFL-1.1 | the app's own words: labels, headings, buttons, counts, dates (`crates/ui-egui/src/theme.rs`); licence text in `assets/fonts/plex-mono/OFL.txt` |
| `assets/fonts/plex-mono/OFL.txt` | IBM Corp. | the Qubernetic brand repository, tag v0.3.0, `fonts/ibm-plex-mono/OFL.txt`, unchanged but for LF line endings | OFL-1.1 | the licence of the Plex Mono files beside it |
| `assets/fonts/plex-sans/IBMPlexSans-<weight>.ttf` | IBM Corp. | https://github.com/IBM/plex, release `@ibm/plex-sans@1.1.0`, `ibm-plex-sans.zip`, `fonts/complete/ttf/` (Regular, SemiBold), unchanged | OFL-1.1 | what people wrote: senders, subjects, message text (`crates/ui-egui/src/theme.rs`); licence text in `assets/fonts/plex-sans/OFL.txt` |
| `assets/fonts/plex-sans/OFL.txt` | IBM Corp. | https://github.com/IBM/plex, release `@ibm/plex-sans@1.1.0`, `ibm-plex-sans.zip`, `LICENSE.txt`, unchanged but for the file name and LF line endings | OFL-1.1 | the licence of the Plex Sans files beside it |
| `assets/icons/phosphor/Phosphor.ttf` | Phosphor Icons (Helena Zhang, Tobias Fried) | https://github.com/phosphor-icons/web, release 2.1.1 (npm `@phosphor-icons/web`), `src/regular/Phosphor.ttf`, unchanged | MIT | the one icon family of the window, Regular weight only (`crates/ui-egui/src/iconset/`); licence text in `assets/icons/phosphor/LICENSE` |
| `assets/icons/phosphor/LICENSE` | Phosphor Icons | https://github.com/phosphor-icons/web, release 2.1.1, `LICENSE`, unchanged but for LF line endings | MIT | the licence of the Phosphor files beside it |
| `docs/images/<shot>.png` | InboxCraft contributors | screenshot of InboxCraft itself showing its sample mailbox, made with `ui_shot` | MIT OR Apache-2.0 | |
| `docs/design/inboxcraft.figcraft` | InboxCraft contributors; the InboxCraft and Qubernetic marks by Qubernetic | made for InboxCraft in FigCraft; the marks and the wordmark are placed from the Qubernetic brand package | MIT OR Apache-2.0, except the marks and the wordmark: trademarks of Qubernetic, all rights reserved | the design file; it names IBM Plex Mono, IBM Plex Sans and Audiowide (SIL OFL 1.1) and embeds no font file |
| `docs/design/cover.png` | InboxCraft contributors; the InboxCraft and Qubernetic marks by Qubernetic | the Cover page of `docs/design/inboxcraft.figcraft`, exported with `figcraft-cli export` | MIT OR Apache-2.0, except the marks and the wordmark: trademarks of Qubernetic, all rights reserved | shown in `README.md` and `docs/design/README.md` |
