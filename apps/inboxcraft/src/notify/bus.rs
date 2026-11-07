// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Notifications over the session bus: [`Notifications`] (`org.freedesktop.Notifications`, which every Linux desktop
//! serves) and [`Portal`] (the notification portal, which the Flatpak goes through). Each listens on a thread of its
//! own for the clicks on what it showed and hands them on as [`Click`]s; a click on something it did not show, or
//! one it cannot read, is dropped.

use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex};

use zbus::blocking::{Connection, Proxy};
use zbus::zvariant::{OwnedValue, Value};

use super::{Click, Deliver, OPEN, Shown, escape};
use crate::tray::APP_ID;

/// The notification server's bus name, object and interface.
pub const NOTIFICATIONS: &str = "org.freedesktop.Notifications";

/// The notification server's object.
pub const NOTIFICATIONS_PATH: &str = "/org/freedesktop/Notifications";

/// The portal's bus name.
pub const PORTAL: &str = "org.freedesktop.portal.Desktop";

/// The portal's object.
pub const PORTAL_PATH: &str = "/org/freedesktop/portal/desktop";

/// The notification portal's interface.
pub const PORTAL_INTERFACE: &str = "org.freedesktop.portal.Notification";

/// The most notifications whose clicks are remembered; the oldest is forgotten first.
pub const MAX_SHOWN: usize = 200;

/// Where a click goes.
pub type Clicked = Arc<dyn Fn(Click) + Send + Sync>;

/// The notifications on show, by the server's id and by key.
#[derive(Debug, Default)]
struct Ids {
    keys: BTreeMap<u32, String>,
    ids: BTreeMap<String, u32>,
}

impl Ids {
    fn insert(&mut self, id: u32, key: &str) {
        if let Some(old) = self.ids.insert(key.to_owned(), id) {
            self.keys.remove(&old);
        }
        self.keys.insert(id, key.to_owned());
        while self.keys.len() > MAX_SHOWN {
            let Some((_, key)) = self.keys.pop_first() else { break };
            self.ids.remove(&key);
        }
    }

    fn remove(&mut self, id: u32) {
        if let Some(key) = self.keys.remove(&id) {
            self.ids.remove(&key);
        }
    }
}

/// Notifications through `org.freedesktop.Notifications`: buttons only where the server shows actions, and the body
/// escaped where the server reads it as markup.
pub struct Notifications {
    proxy: Proxy<'static>,
    actions: bool,
    markup: bool,
    ids: Arc<Mutex<Ids>>,
}

impl std::fmt::Debug for Notifications {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Notifications").field("actions", &self.actions).field("markup", &self.markup).finish_non_exhaustive()
    }
}

impl Notifications {
    /// Asks the server on `bus` what it shows, and listens for clicks, handed to `clicked`.
    ///
    /// # Errors
    ///
    /// No notification server on the bus.
    pub fn connect(bus: &Connection, clicked: Clicked) -> Result<Self, String> {
        let failed = |error: zbus::Error| format!("notifications cannot be shown: {error}");
        let proxy = Proxy::new(bus, NOTIFICATIONS, NOTIFICATIONS_PATH, NOTIFICATIONS).map_err(failed)?;
        let capabilities: Vec<String> = proxy.call("GetCapabilities", &()).map_err(failed)?;
        let has = |name: &str| capabilities.iter().any(|capability| capability == name);
        let ids = Arc::new(Mutex::new(Ids::default()));
        let signals = proxy.receive_all_signals().map_err(failed)?;
        let heard = Arc::clone(&ids);
        std::thread::Builder::new()
            .name("inboxcraft-notifications".to_owned())
            .spawn(move || {
                for message in signals {
                    let header = message.header();
                    let body = message.body();
                    match header.member().map(|member| member.as_str()) {
                        Some("ActionInvoked") => {
                            let Ok((id, action)) = body.deserialize::<(u32, String)>() else { continue };
                            let key = heard.lock().ok().and_then(|ids| ids.keys.get(&id).cloned());
                            if let Some(click) = key.and_then(|key| super::clicked(&key, &action)) {
                                clicked(click);
                            }
                        }
                        Some("NotificationClosed") => {
                            if let (Ok((id, _reason)), Ok(mut ids)) = (body.deserialize::<(u32, u32)>(), heard.lock()) {
                                ids.remove(id);
                            }
                        }
                        _ => {}
                    }
                }
            })
            .map_err(|error| format!("notifications cannot be shown: {error}"))?;
        Ok(Self { proxy, actions: has("actions"), markup: has("body-markup"), ids })
    }
}

impl Deliver for Notifications {
    fn deliver(&mut self, shown: &Shown) -> Result<(), String> {
        let replaces = self.ids.lock().ok().and_then(|ids| ids.ids.get(&shown.key).copied()).unwrap_or(0);
        let mut actions: Vec<String> = Vec::new();
        if self.actions {
            actions.extend(["default".to_owned(), inboxcraft_ui_egui::tr!("Open").to_owned()]);
            for button in &shown.buttons {
                actions.extend([button.action().to_owned(), button.label()]);
            }
        }
        let mut hints: HashMap<&str, Value<'_>> = HashMap::from([("desktop-entry", Value::from(APP_ID))]);
        if shown.mail {
            hints.insert("category", Value::from("email.arrived"));
        }
        let body = if self.markup { escape(&shown.body) } else { shown.body.clone() };
        let id: u32 = self
            .proxy
            .call("Notify", &("InboxCraft", replaces, APP_ID, shown.title.as_str(), body.as_str(), actions, hints, -1_i32))
            .map_err(|error| format!("the notification was not shown: {error}"))?;
        if let Ok(mut ids) = self.ids.lock() {
            ids.insert(id, &shown.key);
        }
        Ok(())
    }
}

/// Notifications through the notification portal, as the Flatpak shows them: the body is plain text, and the
/// buttons are the portal's to show.
pub struct Portal {
    proxy: Proxy<'static>,
}

impl std::fmt::Debug for Portal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Portal").finish_non_exhaustive()
    }
}

impl Portal {
    /// Finds the portal on `bus` and listens for clicks, handed to `clicked`.
    ///
    /// # Errors
    ///
    /// No notification portal on the bus.
    pub fn connect(bus: &Connection, clicked: Clicked) -> Result<Self, String> {
        let failed = |error: zbus::Error| format!("notifications cannot be shown: {error}");
        let proxy = Proxy::new(bus, PORTAL, PORTAL_PATH, PORTAL_INTERFACE).map_err(failed)?;
        let _: u32 = proxy.get_property("version").map_err(|error| format!("notifications cannot be shown: {error}"))?;
        let signals = proxy.receive_signal("ActionInvoked").map_err(failed)?;
        std::thread::Builder::new()
            .name("inboxcraft-notifications".to_owned())
            .spawn(move || {
                for message in signals {
                    let Ok((key, action, _parameter)) = message.body().deserialize::<(String, String, Vec<OwnedValue>)>() else { continue };
                    if let Some(click) = super::clicked(&key, &action) {
                        clicked(click);
                    }
                }
            })
            .map_err(|error| format!("notifications cannot be shown: {error}"))?;
        Ok(Self { proxy })
    }
}

impl Deliver for Portal {
    fn deliver(&mut self, shown: &Shown) -> Result<(), String> {
        let buttons: Vec<HashMap<&str, Value<'_>>> = shown
            .buttons
            .iter()
            .map(|button| HashMap::from([("label", Value::from(button.label())), ("action", Value::from(button.action()))]))
            .collect();
        let mut notification: HashMap<&str, Value<'_>> = HashMap::from([
            ("title", Value::from(shown.title.as_str())),
            ("body", Value::from(shown.body.as_str())),
            ("default-action", Value::from(OPEN)),
            ("priority", Value::from("normal")),
        ]);
        if !buttons.is_empty() {
            notification.insert("buttons", Value::from(buttons));
        }
        self.proxy
            .call::<_, _, ()>("AddNotification", &(shown.key.as_str(), notification))
            .map_err(|error| format!("the notification was not shown: {error}"))
    }
}

/// Notifications on `bus`: through the portal in the Flatpak (`sandboxed`), else through the notification server.
///
/// # Errors
///
/// Neither is on the bus.
pub fn connect(bus: &Connection, sandboxed: bool, clicked: Clicked) -> Result<Box<dyn Deliver>, String> {
    if sandboxed { Ok(Box::new(Portal::connect(bus, clicked)?)) } else { Ok(Box::new(Notifications::connect(bus, clicked)?)) }
}

#[cfg(test)]
#[path = "bus_tests.rs"]
mod tests;
