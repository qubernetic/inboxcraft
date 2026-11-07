# Asset attribution

Every non-code asset in this repository (icons, images, fonts, sounds, sample files) is listed here with its author,
source and licence. `cargo xtask assets` (part of `cargo xtask ci`) fails if an asset file has no row.

**Policy (mandatory):** InboxCraft contains **no iconography, images, templates, themes, sounds or fonts from Spark,
Mailspring, Outlook, Gmail or any other vendor.** Every asset is original, CC0, OSI open source or under a Creative
Commons licence that allows redistribution.

Art drawn in code is original and has no file to list: the app icon (`crates/ui-egui/src/icons.rs`) and the sample
mailbox (`crates/ui-egui/src/sample.rs`). A file exported from that code has its row below.

| Asset | Author | Source | Licence | Notes |
|---|---|---|---|---|
| `assets/app-icon/inboxcraft.svg` | InboxCraft contributors | the InboxCraft mark (two cards entering a notched tray, Electric gradient, white tile), drawn in code for InboxCraft (`crates/ui-egui/src/icons.rs`), exported with `just icon` | MIT OR Apache-2.0 | the app icon; installed as `io.github.qubernetic.inboxcraft.svg` by the Flatpak |
| `assets/fonts/figtree/Figtree-<weight>.ttf` | The Figtree Project Authors (Erik Kennedy) | https://github.com/erikdkennedy/figtree, release v2.0.3, `fonts/ttf/` (Regular, Medium, SemiBold, Bold), unchanged | OFL-1.1 | the typeface of every text style (`crates/ui-egui/src/theme.rs`); licence text in `assets/fonts/figtree/OFL.txt` |
| `assets/fonts/figtree/OFL.txt` | The Figtree Project Authors | https://github.com/erikdkennedy/figtree, release v2.0.3, `OFL.txt`, unchanged | OFL-1.1 | the licence of the Figtree files beside it |
| `assets/fonts/inter/Inter-<weight>.ttf` | The Inter Project Authors (Rasmus Andersson) | https://github.com/rsms/inter, release v4.1, `Inter-4.1.zip`, `extras/ttf/` (Regular, Medium, SemiBold, Bold), unchanged | OFL-1.1 | the second typeface the owner can choose (`ui.font.set`, `crates/ui-egui/src/theme.rs`); licence text in `assets/fonts/inter/OFL.txt` |
| `assets/fonts/inter/OFL.txt` | The Inter Project Authors | https://github.com/rsms/inter, release v4.1, `LICENSE.txt`, unchanged | OFL-1.1 | the licence of the Inter files beside it |
| `assets/icons/lucide/Lucide.ttf` | Lucide Icons and Contributors; the icons derived from Feather by Cole Bemis | https://github.com/lucide-icons/lucide, release 1.70.0 (npm `lucide-static`), `font/lucide.ttf`, unchanged but for the file name | ISC; MIT for the icons derived from Feather | the second icon set the owner can choose (`ui.icons.set`, `crates/ui-egui/src/iconset/`); licence text in `assets/icons/lucide/LICENSE` |
| `assets/icons/lucide/LICENSE` | Lucide Icons and Contributors | https://github.com/lucide-icons/lucide, release 1.70.0 (npm `lucide-static`), `LICENSE`, unchanged | ISC; MIT | the licence of the Lucide font beside it, with the Feather icons it lists under MIT |
| `assets/icons/phosphor/<font>.ttf` | Phosphor Icons (Helena Zhang, Tobias Fried) | https://github.com/phosphor-icons/web, release 2.1.1 (npm `@phosphor-icons/web`), `src/regular/Phosphor.ttf`, `src/fill/Phosphor-Fill.ttf`, `src/duotone/Phosphor-Duotone.ttf`, unchanged | MIT | the icon set of the window (`crates/ui-egui/src/iconset/`); licence text in `assets/icons/phosphor/LICENSE` |
| `assets/icons/phosphor/LICENSE` | Phosphor Icons | https://github.com/phosphor-icons/web, release 2.1.1, `LICENSE`, unchanged but for LF line endings | MIT | the licence of the Phosphor files beside it |
| `docs/images/<shot>.png` | InboxCraft contributors | screenshot of InboxCraft itself showing its sample mailbox, made with `ui_shot` | MIT OR Apache-2.0 | |
| `docs/design/inboxcraft.figcraft` | InboxCraft contributors | made for InboxCraft in FigCraft from `docs/design.md` and `crates/ui-egui/src/theme.rs` | MIT OR Apache-2.0 | the design file; it names the Figtree and Phosphor fonts of `assets/` and embeds no font, image or icon file |
| `docs/design/<page>.png` | InboxCraft contributors | a page of `docs/design/inboxcraft.figcraft`, rendered with `figcraft-cli render` | MIT OR Apache-2.0 | |
