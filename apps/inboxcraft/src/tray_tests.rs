// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};

use ksni::menu::MenuItem;
use ksni::{Status, Tray};

use super::{APP_ID, InboxTray, argb, start};
use crate::bus_tests::PrivateBus;
use crate::desktop::Badge;
use crate::instance::Request;

/// A tray, what it asks for, and how often it woke the window.
fn tray() -> (InboxTray, Receiver<Request>, Arc<AtomicUsize>) {
    let (send, heard) = mpsc::channel();
    let woken = Arc::new(AtomicUsize::new(0));
    let counted = Arc::clone(&woken);
    let tray = InboxTray::new(
        send,
        Arc::new(move || {
            counted.fetch_add(1, Ordering::Relaxed);
        }),
    );
    (tray, heard, woken)
}

#[test]
fn the_tray_icon_is_inboxcraft_s() {
    let (tray, _heard, _woken) = tray();
    assert_eq!(tray.id(), APP_ID);
    assert_eq!(tray.title(), "InboxCraft");
    assert_eq!(tray.icon_name(), APP_ID);
    assert_eq!(tray.status(), Status::Active);
    let pixmap = tray.icon_pixmap();
    let icon = pixmap.first().unwrap();
    assert!(icon.width > 0 && icon.height > 0);
    assert_eq!(icon.data.len(), usize::try_from(icon.width * icon.height * 4).unwrap());
}

#[test]
fn the_tooltip_says_how_much_is_unread() {
    let (mut tray, _heard, _woken) = tray();
    assert_eq!(tray.tool_tip().title, "InboxCraft");
    assert_eq!(tray.tool_tip().description, "No unread mail");
    tray.show(1);
    assert_eq!(tray.tool_tip().description, "1 unread");
    tray.show(42);
    assert_eq!(tray.tool_tip().description, "42 unread");
}

#[test]
fn a_click_on_the_icon_brings_the_window_forward() {
    let (mut tray, heard, woken) = tray();
    tray.activate(0, 0);
    assert_eq!(heard.try_recv(), Ok(Request::Show));
    assert_eq!(woken.load(Ordering::Relaxed), 1, "the window is woken to answer");
}

#[test]
fn the_menu_opens_the_window_or_ends_inboxcraft() {
    let (mut tray, heard, woken) = tray();
    let menu = tray.menu();
    let items: Vec<_> = menu
        .iter()
        .filter_map(|item| match item {
            MenuItem::Standard(item) => Some(item),
            _ => None,
        })
        .collect();
    let labels: Vec<&str> = items.iter().map(|item| item.label.as_str()).collect();
    assert_eq!(labels, ["Open InboxCraft", "Quit InboxCraft"]);
    for item in items {
        (item.activate)(&mut tray);
    }
    assert_eq!(heard.try_iter().collect::<Vec<_>>(), [Request::Show, Request::Quit]);
    assert_eq!(woken.load(Ordering::Relaxed), 2);
}

#[test]
fn a_tray_whose_window_is_gone_asks_nothing_and_does_not_panic() {
    let (mut tray, heard, _woken) = tray();
    drop(heard);
    tray.activate(0, 0);
}

#[test]
fn pixels_turn_from_rgba_into_argb() {
    assert_eq!(argb(&[1, 2, 3, 4, 5, 6, 7, 8]), [4, 1, 2, 3, 8, 5, 6, 7]);
    assert_eq!(argb(&[1, 2, 3]), Vec::<u8>::new(), "a part pixel is dropped");
}

/// A StatusNotifierItem's `ToolTip` as it goes over the bus: icon name, icon pixmaps, title and description.
type TipValue = (String, Vec<(i32, i32, Vec<u8>)>, String, String);

/// What the fake watcher was asked to show.
type Registered = Arc<Mutex<Vec<String>>>;

/// The desktop's StatusNotifierWatcher, as a panel serves it.
struct Watcher(Registered);

#[zbus::interface(name = "org.kde.StatusNotifierWatcher")]
impl Watcher {
    fn register_status_notifier_item(&self, service: &str) {
        self.0.lock().unwrap().push(service.to_owned());
    }

    #[zbus(property)]
    fn is_status_notifier_host_registered(&self) -> bool {
        true
    }
}

/// Set in the child run of [`child_shows_the_tray_on_the_session_bus`]; without it that test does nothing.
const CHILD_VAR: &str = "INBOXCRAFT_TRAY_CHILD";

/// In a child run on a private session bus: a fake panel, the tray on it, an unread count; prints what the panel reads.
#[test]
fn child_shows_the_tray_on_the_session_bus() {
    let Some(sandboxed) = std::env::var_os(CHILD_VAR) else { return };
    let registered = Registered::default();
    let _panel = zbus::blocking::connection::Builder::session()
        .unwrap()
        .name("org.kde.StatusNotifierWatcher")
        .unwrap()
        .serve_at("/StatusNotifierWatcher", Watcher(Arc::clone(&registered)))
        .unwrap()
        .build()
        .unwrap();
    let (tray, _heard, _woken) = tray();
    let mut badge = start(tray, sandboxed == "1").unwrap();
    badge.show(7);
    let service = registered.lock().unwrap().first().cloned().unwrap();
    println!("service={service}");
    let reader = zbus::blocking::Connection::session().unwrap();
    let item = zbus::blocking::Proxy::new(&reader, service, "/StatusNotifierItem", "org.kde.StatusNotifierItem").unwrap();
    let (_, _, title, description): TipValue = item.get_property("ToolTip").unwrap();
    println!("tooltip={title}: {description}");
}

/// What the child run prints, on a private bus, the tray in the sandbox or not.
fn child(sandboxed: bool) -> Option<Vec<String>> {
    let bus = PrivateBus::start()?;
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["tray::tests::child_shows_the_tray_on_the_session_bus", "--exact", "--nocapture", "--test-threads=1"])
        .env(CHILD_VAR, if sandboxed { "1" } else { "0" })
        .env("DBUS_SESSION_BUS_ADDRESS", bus.address())
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    // The test harness may print the test's name on the same line as the first of these.
    let found = |line: &str| ["service=", "tooltip="].iter().find_map(|key| line.find(key).and_then(|at| line.get(at..)).map(str::to_owned));
    Some(String::from_utf8(output.stdout).unwrap().lines().filter_map(found).collect())
}

#[test]
fn the_tray_shows_on_the_panel_with_the_unread_count() {
    let Some(lines) = child(false) else { return };
    assert!(lines[0].starts_with("service=org.kde.StatusNotifierItem-"), "{lines:?}");
    assert_eq!(lines[1], "tooltip=InboxCraft: 7 unread");
}

#[test]
fn in_the_sandbox_the_tray_goes_by_its_connection_s_name() {
    let Some(lines) = child(true) else { return };
    assert!(lines[0].starts_with("service=:"), "no well-known name of its own: {lines:?}");
    assert_eq!(lines[1], "tooltip=InboxCraft: 7 unread");
}

#[test]
fn without_a_panel_the_tray_is_an_error_not_a_panic() {
    let Some(bus) = PrivateBus::start() else { return };
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["tray::tests::child_starts_a_tray_with_no_panel", "--exact", "--nocapture", "--test-threads=1"])
        .env(CHILD_VAR, "0")
        .env("DBUS_SESSION_BUS_ADDRESS", bus.address())
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    assert!(String::from_utf8_lossy(&output.stdout).contains("error=the tray could not be shown"), "{output:?}");
}

/// In a child run on a private session bus without a panel: prints the error the tray gives.
#[test]
fn child_starts_a_tray_with_no_panel() {
    if std::env::var_os(CHILD_VAR).is_none() {
        return;
    }
    let (tray, _heard, _woken) = tray();
    if let Err(error) = start(tray, false) {
        println!("error={error}");
    }
}

#[test]
fn a_tray_shows_its_count_in_debug_output() {
    let (mut tray, _heard, _woken) = tray();
    tray.show(3);
    assert_eq!(format!("{tray:?}"), "InboxTray { unread: 3, .. }");
}
