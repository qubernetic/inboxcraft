// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! A private D-Bus session bus for the tests that talk D-Bus: a `dbus-daemon` of their own, ended with the test. It
//! never reaches the desktop's bus, so no tray icon or portal dialog of the person's own desktop is touched.

use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};

/// A `dbus-daemon` that lives as long as this.
pub(crate) struct PrivateBus {
    daemon: Child,
    address: String,
}

impl PrivateBus {
    /// Starts one; `None` (and a note on stderr) where `dbus-daemon` is not installed.
    pub(crate) fn start() -> Option<Self> {
        let started = Command::new("dbus-daemon")
            .args(["--session", "--nofork", "--nopidfile", "--print-address=1"])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn();
        let Ok(mut daemon) = started else {
            eprintln!("dbus-daemon is not installed: the D-Bus test is skipped");
            return None;
        };
        let mut address = String::new();
        BufReader::new(daemon.stdout.take().unwrap()).read_line(&mut address).unwrap();
        Some(Self { daemon, address: address.trim().to_owned() })
    }

    /// Its address, as `DBUS_SESSION_BUS_ADDRESS` takes it.
    pub(crate) fn address(&self) -> &str {
        &self.address
    }

    /// A new connection to it.
    pub(crate) fn connect(&self) -> zbus::blocking::Connection {
        zbus::blocking::connection::Builder::address(self.address.as_str()).unwrap().build().unwrap()
    }
}

impl Drop for PrivateBus {
    fn drop(&mut self) {
        let _ = self.daemon.kill();
        let _ = self.daemon.wait();
    }
}
