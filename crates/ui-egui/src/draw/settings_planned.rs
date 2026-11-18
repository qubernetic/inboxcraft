// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The settings pages that are designed and not built yet, drawn as coming soon: Calendar (the alerts switch and each
//! account's calendars with "read again"; the week's first day is a fact), Privacy (the senders whose remote pictures
//! always load), and the three sections that are planned whole, with the coming-soon mark on their head: About (the
//! program, its licences, its makers and contributors), AI (where a model runs, and the consent to send mail to one
//! elsewhere) and Plugins. Nothing on them can be pressed or focused.

use egui::Ui;

use super::{Asked, body, group, head};
use crate::component::button::{LinkSize, link_sized};
use crate::component::segmented::segmented_for;
use crate::component::settings_row::{settings_row, settings_row_soon, settings_row_soon_with};
use crate::component::soon::soon;
use crate::component::switch::{SwitchState, switch_in};
use crate::keymap::Keymap;
use crate::mailbox::Mailbox;
use crate::settings::Section;
use crate::theme::Theme;
use crate::tr;

/// The Calendar page: alerts, each account's calendars, the week.
pub(super) fn calendar(ui: &mut Ui, theme: &Theme, keymap: &Keymap, mailbox: &Mailbox) -> Option<Asked> {
    head(ui, theme, Section::Calendar, &[], |_| {});
    body(ui, theme, "calendar", |ui| {
        group(ui, theme, "A", tr!("Alerts"), None);
        settings_row_soon(ui, theme, tr!("Calendar alerts"), tr!("An event's alert shows as a desktop notification at its time."));
        let note = tr!("A calendar comes from its account's CalDAV address. Which of them the calendar shows is chosen in its side panel.");
        group(ui, theme, "B", tr!("Calendars"), Some(note));
        for account in mailbox.accounts() {
            let help = tr!("The calendars found at the CalDAV address of {address}.", address = account.address);
            settings_row_soon_with(ui, theme, &account.label, &help, |ui| {
                link_sized(ui, theme, keymap, tr!("read again"), LinkSize::Small);
            });
        }
        group(ui, theme, "C", tr!("Week"), None);
        settings_row(ui, theme, tr!("Week starts on"), Some(tr!("Fixed: it cannot be changed yet.")), |ui| {
            crate::component::label(ui, theme.fonts.mono, tr!("Monday"), theme.palette.muted);
        });
    });
    None
}

/// The Privacy page: what a remote picture tells, and the senders whose pictures always load.
pub(super) fn privacy(ui: &mut Ui, theme: &Theme) -> Option<Asked> {
    let intro = [
        tr!(
            "A remote picture tells its server that the message was opened, when and from where. InboxCraft asks every time before it loads one, unless you chose “always for this sender” in the reader."
        ),
        tr!("Nothing in a message runs, and a picture's request carries nothing but its address."),
    ];
    head(ui, theme, Section::Privacy, &intro, |_| {});
    body(ui, theme, "privacy", |ui| {
        let note = tr!("The senders whose remote pictures load without asking. “ask again” takes one off the list: its pictures are held again.");
        group(ui, theme, "A", tr!("Pictures always load"), Some(note));
        settings_row_soon(
            ui,
            theme,
            tr!("Senders whose pictures always load"),
            tr!("The list, with “ask again” beside each sender, comes with this page."),
        );
    });
    None
}

/// The About page, planned whole: the program, Qubernetic, the contributors.
pub(super) fn about(ui: &mut Ui, theme: &Theme) -> Option<Asked> {
    head(ui, theme, Section::About, &[], |_| {});
    body(ui, theme, "about", |ui| {
        soon(ui, theme, |ui| {
            group(ui, theme, "A", tr!("This program"), None);
            let version = tr!(
                "{version}. A mail and calendar program for IMAP, SMTP and CalDAV that keeps your mail on this computer.",
                version = env!("CARGO_PKG_VERSION")
            );
            settings_row(ui, theme, tr!("Version"), Some(&version), |_| {});
            settings_row(
                ui,
                theme,
                tr!("Licence"),
                Some(tr!("MIT or Apache-2.0, at your choice. The parts it is built from keep their own licences.")),
                |_| {},
            );
            settings_row(
                ui,
                theme,
                tr!("Type, icons"),
                Some(tr!("IBM Plex Mono, IBM Plex Sans and Audiowide under the SIL Open Font Licence.")),
                |_| {},
            );
            group(ui, theme, "B", tr!("Qubernetic"), None);
            settings_row(ui, theme, tr!("Made by"), Some(tr!("Qubernetic and the InboxCraft contributors.")), |_| {});
            group(ui, theme, "C", tr!("Contributors"), None);
            settings_row(ui, theme, tr!("With"), Some(tr!("Everyone who asked to be named, in the order they joined.")), |_| {});
        });
    });
    None
}

/// The AI page, planned whole: where a model runs, and the consent to send mail to one elsewhere.
pub(super) fn ai(ui: &mut Ui, theme: &Theme) -> Option<Asked> {
    let intro = [tr!(
        "InboxCraft has no AI of its own. A model you bring can summarise a message and draft a reply for you to change and send. It never sends, moves or deletes anything, and what it writes is marked."
    )];
    head(ui, theme, Section::Ai, &intro, |_| {});
    body(ui, theme, "ai", |ui| {
        soon(ui, theme, |ui| {
            group(ui, theme, "A", tr!("Model"), None);
            let f = theme.fonts.button;
            let places = [(0_u8, tr!("None"), f), (1, tr!("On this computer"), f), (2, tr!("Elsewhere"), f)];
            settings_row(ui, theme, tr!("Where the model runs"), Some(tr!("No model: Summarise and Draft a reply are not offered.")), |ui| {
                segmented_for(ui, theme, &places, 0, Some(tr!("Where the model runs")));
            });
            let consent = tr!("A model elsewhere sees the messages you ask it about. It is asked once, before anything leaves this computer.");
            settings_row(ui, theme, tr!("Send mail to a model elsewhere"), Some(consent), |ui| {
                switch_in(ui, theme, tr!("Send mail to a model elsewhere"), SwitchState::Unavailable);
            });
        });
    });
    None
}

/// The Plugins page, planned whole.
pub(super) fn plugins(ui: &mut Ui, theme: &Theme) -> Option<Asked> {
    let intro = [tr!(
        "A plugin is code from someone else that adds to InboxCraft through the app's own commands. It asks before it is added and does only what you allowed."
    )];
    head(ui, theme, Section::Plugins, &intro, |_| {});
    body(ui, theme, "plugins", |ui| {
        soon(ui, theme, |ui| {
            group(ui, theme, "A", tr!("Plugins"), None);
            settings_row(
                ui,
                theme,
                tr!("Installed plugins"),
                Some(tr!("None. Here you will see each plugin, what it may do, and take it back.")),
                |_| {},
            );
        });
    });
    None
}
