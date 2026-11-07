// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The tray icon on Linux: a StatusNotifierItem over the session bus (`ksni`), which KDE Plasma, most other panels and
//! GNOME with the AppIndicator extension show. Its tooltip says how much is unread; a click brings the window forward,
//! and its menu opens the window or ends InboxCraft. Each of those is a [`Request`] to the window, which is woken to
//! answer it. Other systems have no tray yet.

use std::sync::mpsc::Sender;

use inboxcraft_ui_egui::fetch::Wake;
use inboxcraft_ui_egui::tr;
use ksni::blocking::{Handle, TrayMethods};
use ksni::menu::{MenuItem, StandardItem};
use ksni::{Icon, Status, ToolTip, Tray};

use crate::desktop::Badge;
use crate::instance::Request;

/// The app id, which names the tray and its themed icon.
pub const APP_ID: &str = "io.github.qubernetic.inboxcraft";

/// The tray's state: the unread count, and the way to the window.
pub struct InboxTray {
    unread: u64,
    send: Sender<Request>,
    wake: Wake,
    icon: Vec<Icon>,
}

impl std::fmt::Debug for InboxTray {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InboxTray").field("unread", &self.unread).finish_non_exhaustive()
    }
}

impl InboxTray {
    /// A tray that hands its requests to `send` and calls `wake` so the window answers them.
    #[must_use]
    pub fn new(send: Sender<Request>, wake: Wake) -> Self {
        let icon = inboxcraft_ui_egui::icons::window_icon()
            .ok()
            .and_then(|icon| Some(Icon { width: i32::try_from(icon.width).ok()?, height: i32::try_from(icon.height).ok()?, data: argb(&icon.rgba) }))
            .into_iter()
            .collect();
        Self { unread: 0, send, wake, icon }
    }

    fn ask(&self, request: Request) {
        // A window that is gone has nothing left to show or end.
        if self.send.send(request).is_ok() {
            (self.wake)();
        }
    }
}

impl Badge for InboxTray {
    fn show(&mut self, unread: u64) {
        self.unread = unread;
    }
}

impl Tray for InboxTray {
    fn id(&self) -> String {
        APP_ID.to_owned()
    }

    fn title(&self) -> String {
        "InboxCraft".to_owned()
    }

    fn status(&self) -> Status {
        Status::Active
    }

    fn icon_name(&self) -> String {
        APP_ID.to_owned()
    }

    fn icon_pixmap(&self) -> Vec<Icon> {
        self.icon.clone()
    }

    fn tool_tip(&self) -> ToolTip {
        let description = if self.unread == 0 { tr!("No unread mail").to_owned() } else { tr!("{count} unread", count = self.unread) };
        ToolTip { title: "InboxCraft".to_owned(), description, ..ToolTip::default() }
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        self.ask(Request::Show);
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        vec![
            StandardItem {
                label: tr!("Open InboxCraft").to_owned(),
                activate: Box::new(|tray: &mut Self| tray.ask(Request::Show)),
                ..StandardItem::default()
            }
            .into(),
            MenuItem::Separator,
            StandardItem {
                label: tr!("Quit InboxCraft").to_owned(),
                activate: Box::new(|tray: &mut Self| tray.ask(Request::Quit)),
                ..StandardItem::default()
            }
            .into(),
        ]
    }
}

/// The running tray, which shows the unread count.
pub struct TrayBadge(Handle<InboxTray>);

impl Badge for TrayBadge {
    fn show(&mut self, unread: u64) {
        // A tray whose service ended shows nothing more.
        let _ = self.0.update(|tray| tray.unread = unread);
    }
}

/// Puts `tray` on the session bus. In the Flatpak (`sandboxed`) it goes by its connection's name, as the sandbox lets
/// it own no name of its own.
///
/// # Errors
///
/// No session bus, or no panel that shows tray icons.
pub fn start(tray: InboxTray, sandboxed: bool) -> Result<TrayBadge, String> {
    tray.disable_dbus_name(sandboxed).spawn().map(TrayBadge).map_err(|error| format!("the tray could not be shown: {error}"))
}

/// RGBA pixels as the ARGB the StatusNotifierItem takes; a part pixel at the end is dropped.
#[must_use]
pub fn argb(rgba: &[u8]) -> Vec<u8> {
    rgba.as_chunks::<4>().0.iter().flat_map(|&[r, g, b, a]| [a, r, g, b]).collect()
}

#[cfg(test)]
#[path = "tray_tests.rs"]
mod tests;
