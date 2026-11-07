// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Delivery against a fake notification server and a fake notification portal on a private bus.

use std::collections::HashMap;
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use zbus::blocking::Connection;
use zbus::zvariant::{OwnedValue, Value};

use super::{Clicked, MAX_SHOWN, NOTIFICATIONS, NOTIFICATIONS_PATH, Notifications, PORTAL, PORTAL_INTERFACE, PORTAL_PATH, Portal, connect};
use crate::bus_tests::PrivateBus;
use crate::notify::{Button, Click, Deliver, Shown};

/// A private bus with a fake on it, the fake's connection and the calls it took; `None` without `dbus-daemon`.
type Served<T> = Option<(PrivateBus, Connection, Arc<Mutex<Vec<T>>>)>;

/// One `Notify` call: replaces id, icon, title, body, actions and hints.
type Notified = (u32, String, String, String, Vec<String>, HashMap<String, OwnedValue>);

/// `org.freedesktop.Notifications` as a desktop serves it: it writes down what it was asked and gives ids from 1 on.
struct FakeServer {
    capabilities: Vec<String>,
    notified: Arc<Mutex<Vec<Notified>>>,
}

#[zbus::interface(name = "org.freedesktop.Notifications")]
impl FakeServer {
    fn get_capabilities(&self) -> Vec<String> {
        self.capabilities.clone()
    }

    #[allow(clippy::too_many_arguments)]
    fn notify(
        &self,
        _app_name: &str,
        replaces_id: u32,
        app_icon: &str,
        summary: &str,
        body: &str,
        actions: Vec<String>,
        hints: HashMap<String, OwnedValue>,
        _expire_timeout: i32,
    ) -> u32 {
        let mut notified = self.notified.lock().unwrap();
        notified.push((replaces_id, app_icon.to_owned(), summary.to_owned(), body.to_owned(), actions, hints));
        if replaces_id == 0 { u32::try_from(notified.len()).unwrap() } else { replaces_id }
    }
}

fn mail(key: &str, body: &str) -> Shown {
    Shown { key: key.to_owned(), title: "Anna".to_owned(), body: body.to_owned(), buttons: vec![Button::Done, Button::Read], mail: true }
}

/// A private bus with the fake server on it.
fn served(capabilities: &[&str]) -> Served<Notified> {
    let bus = PrivateBus::start()?;
    let notified = Arc::new(Mutex::new(Vec::new()));
    let fake = FakeServer { capabilities: capabilities.iter().map(|name| (*name).to_owned()).collect(), notified: Arc::clone(&notified) };
    let server = zbus::blocking::connection::Builder::address(bus.address())
        .unwrap()
        .name(NOTIFICATIONS)
        .unwrap()
        .serve_at(NOTIFICATIONS_PATH, fake)
        .unwrap()
        .build()
        .unwrap();
    Some((bus, server, notified))
}

/// Where the clicks go, and what came.
fn clicks() -> (Clicked, Receiver<Click>) {
    let (send, heard) = mpsc::channel();
    let send = Mutex::new(send);
    (Arc::new(move |click| send.lock().unwrap().send(click).unwrap()), heard)
}

fn hint(hints: &HashMap<String, OwnedValue>, key: &str) -> Option<String> {
    hints.get(key).map(|value| String::try_from(Value::from(value.try_clone().unwrap())).unwrap())
}

#[test]
fn a_notification_goes_to_the_server_with_its_buttons_and_the_body_as_markup_escapes() {
    let Some((bus, _server, notified)) = served(&["actions", "body", "body-markup"]) else { return };
    let (clicked, _heard) = clicks();
    let mut notifications = Notifications::connect(&bus.connect(), clicked).unwrap();
    notifications.deliver(&mail("mail-7", "<b>Q&A</b> <a href=\"https://example.org\">x</a>")).unwrap();
    let notified = notified.lock().unwrap();
    let (replaces, icon, title, body, actions, hints) = &notified[0];
    assert_eq!((*replaces, icon.as_str(), title.as_str()), (0, "io.github.qubernetic.inboxcraft", "Anna"));
    assert_eq!(body, "&lt;b&gt;Q&amp;A&lt;/b&gt; &lt;a href=\"https://example.org\"&gt;x&lt;/a&gt;");
    assert_eq!(actions, &["default", "Open", "done", "Done", "read", "Mark read"]);
    assert_eq!(hint(hints, "desktop-entry").as_deref(), Some("io.github.qubernetic.inboxcraft"));
    assert_eq!(hint(hints, "category").as_deref(), Some("email.arrived"));
}

#[test]
fn a_server_without_actions_or_markup_gets_neither() {
    let Some((bus, _server, notified)) = served(&["body"]) else { return };
    let (clicked, _heard) = clicks();
    let mut notifications = Notifications::connect(&bus.connect(), clicked).unwrap();
    let mut shown = mail("returned-7", "Anna: <Lunch> & more");
    shown.mail = false;
    notifications.deliver(&shown).unwrap();
    let notified = notified.lock().unwrap();
    let (_, _, _, body, actions, hints) = &notified[0];
    assert_eq!(body, "Anna: <Lunch> & more", "plain text stays as it is");
    assert!(actions.is_empty());
    assert_eq!(hint(hints, "category"), None);
}

#[test]
fn a_later_notification_with_the_same_key_takes_the_place_of_the_first() {
    let Some((bus, _server, notified)) = served(&["actions"]) else { return };
    let (clicked, _heard) = clicks();
    let mut notifications = Notifications::connect(&bus.connect(), clicked).unwrap();
    notifications.deliver(&mail("mails-1", "4")).unwrap();
    notifications.deliver(&mail("mail-2", "x")).unwrap();
    notifications.deliver(&mail("mails-1", "6")).unwrap();
    let replaced: Vec<u32> = notified.lock().unwrap().iter().map(|call| call.0).collect();
    assert_eq!(replaced, [0, 0, 1]);
}

#[test]
fn a_click_on_a_notification_it_showed_comes_back_as_what_it_stands_for() {
    let Some((bus, server, _notified)) = served(&["actions"]) else { return };
    let (clicked, heard) = clicks();
    let mut notifications = Notifications::connect(&bus.connect(), clicked).unwrap();
    notifications.deliver(&mail("mail-7", "x")).unwrap();
    notifications.deliver(&mail("mails-1", "x")).unwrap();
    let emit = |id: u32, action: &str| server.emit_signal(None::<&str>, NOTIFICATIONS_PATH, NOTIFICATIONS, "ActionInvoked", &(id, action)).unwrap();
    emit(99, "default");
    emit(1, "launch-missiles");
    emit(1, "default");
    emit(1, "done");
    emit(1, "read");
    emit(2, "default");
    // Closed: a click that comes after it is not ours any more.
    server.emit_signal(None::<&str>, NOTIFICATIONS_PATH, NOTIFICATIONS, "NotificationClosed", &(1_u32, 2_u32)).unwrap();
    emit(1, "default");
    emit(2, "done");
    let mut got = Vec::new();
    while let Ok(click) = heard.recv_timeout(Duration::from_millis(500)) {
        got.push(click);
    }
    assert_eq!(got, [Click::Open(7), Click::Done(7), Click::Read(7), Click::Show]);
}

#[test]
fn without_a_notification_server_there_are_no_notifications() {
    let Some(bus) = PrivateBus::start() else { return };
    let (clicked, _heard) = clicks();
    let error = Notifications::connect(&bus.connect(), clicked).unwrap_err();
    assert!(error.contains("notifications cannot be shown"), "{error}");
}

#[test]
fn a_server_that_went_away_is_an_error_on_the_next_notification() {
    let Some((bus, server, _notified)) = served(&[]) else { return };
    let (clicked, _heard) = clicks();
    let mut notifications = Notifications::connect(&bus.connect(), clicked).unwrap();
    drop(server);
    let error = notifications.deliver(&mail("mail-1", "x")).unwrap_err();
    assert!(error.contains("not shown"), "{error}");
}

#[test]
fn only_the_newest_notifications_are_remembered() {
    let mut ids = super::Ids::default();
    for id in 1..=u32::try_from(MAX_SHOWN).unwrap() + 5 {
        ids.insert(id, &format!("mail-{id}"));
    }
    assert_eq!(ids.keys.len(), MAX_SHOWN);
    assert_eq!(ids.ids.len(), MAX_SHOWN);
    assert!(!ids.keys.contains_key(&5) && ids.keys.contains_key(&6));
    ids.remove(6);
    ids.remove(6);
    assert!(!ids.ids.contains_key("mail-6"));
}

/// One `AddNotification` call: the id and the notification.
type Added = (String, HashMap<String, OwnedValue>);

/// The notification portal as xdg-desktop-portal serves it.
struct FakePortal {
    added: Arc<Mutex<Vec<Added>>>,
}

#[zbus::interface(name = "org.freedesktop.portal.Notification")]
impl FakePortal {
    fn add_notification(&self, id: &str, notification: HashMap<String, OwnedValue>) {
        self.added.lock().unwrap().push((id.to_owned(), notification));
    }

    #[zbus(property, name = "version")]
    fn version(&self) -> u32 {
        2
    }
}

fn portal_served() -> Served<Added> {
    let bus = PrivateBus::start()?;
    let added = Arc::new(Mutex::new(Vec::new()));
    let server = zbus::blocking::connection::Builder::address(bus.address())
        .unwrap()
        .name(PORTAL)
        .unwrap()
        .serve_at(PORTAL_PATH, FakePortal { added: Arc::clone(&added) })
        .unwrap()
        .build()
        .unwrap();
    Some((bus, server, added))
}

#[test]
fn in_the_flatpak_a_notification_goes_to_the_portal_with_its_buttons() {
    let Some((bus, _server, added)) = portal_served() else { return };
    let (clicked, _heard) = clicks();
    let mut portal = connect(&bus.connect(), true, clicked).unwrap();
    portal.deliver(&mail("mail-7", "<b>plain</b>")).unwrap();
    let mut no_buttons = mail("reminder-8", "x");
    no_buttons.buttons.clear();
    portal.deliver(&no_buttons).unwrap();
    let added = added.lock().unwrap();
    let (id, notification) = &added[0];
    assert_eq!(id, "mail-7");
    let text = |key: &str| String::try_from(Value::from(notification[key].try_clone().unwrap())).unwrap();
    assert_eq!((text("title"), text("body"), text("default-action")), ("Anna".to_owned(), "<b>plain</b>".to_owned(), "open".to_owned()));
    let buttons: Vec<HashMap<String, OwnedValue>> = Value::from(notification["buttons"].try_clone().unwrap()).try_into().unwrap();
    let pairs: Vec<(String, String)> = buttons
        .iter()
        .map(|button| {
            let get = |key: &str| String::try_from(Value::from(button[key].try_clone().unwrap())).unwrap();
            (get("label"), get("action"))
        })
        .collect();
    assert_eq!(pairs, [("Done".to_owned(), "done".to_owned()), ("Mark read".to_owned(), "read".to_owned())]);
    assert!(!added[1].1.contains_key("buttons"));
}

#[test]
fn a_click_in_the_portal_comes_back_as_what_it_stands_for() {
    let Some((bus, server, _added)) = portal_served() else { return };
    let (clicked, heard) = clicks();
    let _portal = Portal::connect(&bus.connect(), clicked).unwrap();
    let emit = |id: &str, action: &str| {
        server.emit_signal(None::<&str>, PORTAL_PATH, PORTAL_INTERFACE, "ActionInvoked", &(id, action, Vec::<Value<'_>>::new())).unwrap();
    };
    emit("mail-7", "open");
    emit("../../etc", "open");
    emit("mail-7", "rm");
    emit("returned-8", "done");
    server.emit_signal(None::<&str>, PORTAL_PATH, PORTAL_INTERFACE, "ActionInvoked", &("mail-9", 3_u32)).unwrap();
    emit("reminder-9", "open");
    let mut got = Vec::new();
    while let Ok(click) = heard.recv_timeout(Duration::from_millis(500)) {
        got.push(click);
    }
    assert_eq!(got, [Click::Open(7), Click::Done(8), Click::Open(9)]);
}

#[test]
fn outside_the_flatpak_the_notification_server_is_used_and_without_a_portal_there_is_none() {
    let Some((bus, _server, notified)) = served(&[]) else { return };
    let (clicked, _heard) = clicks();
    connect(&bus.connect(), false, Arc::clone(&clicked)).unwrap().deliver(&mail("mail-1", "x")).unwrap();
    assert_eq!(notified.lock().unwrap().len(), 1);
    let error = connect(&bus.connect(), true, clicked).err().unwrap();
    assert!(error.contains("notifications cannot be shown"), "{error}");
}

#[test]
fn a_portal_that_went_away_is_an_error_on_the_next_notification() {
    let Some((bus, server, _added)) = portal_served() else { return };
    let (clicked, _heard) = clicks();
    let mut portal = Portal::connect(&bus.connect(), clicked).unwrap();
    drop(server);
    let error = portal.deliver(&mail("mail-1", "x")).unwrap_err();
    assert!(error.contains("not shown"), "{error}");
}
