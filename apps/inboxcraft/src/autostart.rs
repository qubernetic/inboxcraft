// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Starting InboxCraft at login, in the background (`--autostart`, `--no-autostart`).
//!
//! Outside the Flatpak the choice is an XDG autostart entry, [`ENTRY_FILE`] in the `autostart` folder of the owner's
//! config folder, which starts this program with `--background`. Inside the Flatpak the sandbox cannot write the
//! desktop's autostart folder: the Background portal does it ([`portal`], Linux only), and the same entry in the app's
//! own config folder keeps the choice, so each start can tell the portal again. A start in the Flatpak asks the
//! portal to let InboxCraft run in the background too, so a desktop that ends background apps leaves it running.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// The autostart entry's file name: the app id.
pub const ENTRY_FILE: &str = "io.github.qubernetic.inboxcraft.desktop";

/// What the Background portal allowed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Granted {
    /// InboxCraft may run with no window shown.
    pub background: bool,
    /// InboxCraft starts at login.
    pub autostart: bool,
}

/// The `autostart` folder in the owner's config folder: `XDG_CONFIG_HOME` when it is absolute (the Flatpak's own),
/// else `.config` in the home folder; `None` without either.
#[must_use]
pub fn autostart_dir() -> Option<PathBuf> {
    autostart_dir_from(std::env::var_os("XDG_CONFIG_HOME"), std::env::var_os("HOME"))
}

/// [`autostart_dir`] from the values of `XDG_CONFIG_HOME` and `HOME`.
#[must_use]
pub fn autostart_dir_from(config: Option<OsString>, home: Option<OsString>) -> Option<PathBuf> {
    let absolute = |value: OsString| Some(PathBuf::from(value)).filter(|path| path.is_absolute());
    let config = config.and_then(absolute).or_else(|| home.and_then(absolute).map(|home| home.join(".config")))?;
    Some(config.join("autostart"))
}

/// Whether this run is inside the Flatpak sandbox.
#[must_use]
pub fn sandboxed() -> bool {
    Path::new("/.flatpak-info").exists()
}

/// The entry's `Exec` value: `program` quoted as the Desktop Entry specification asks, then `--background`. `None` for a
/// program that is not text, holds a control character, or is a relative path; a bare name is looked up on `PATH`.
#[must_use]
pub fn exec_line(program: &Path) -> Option<String> {
    let text = program.to_str().filter(|text| !text.is_empty() && !text.chars().any(char::is_control))?;
    if text.contains('/') && !program.is_absolute() {
        return None;
    }
    let mut quoted = String::from("\"");
    for c in text.chars() {
        match c {
            // Escaped inside the quotes, and the backslash escaped again as the entry's string type wants.
            '"' | '`' | '$' => {
                quoted.push_str("\\\\");
                quoted.push(c);
            }
            '\\' => quoted.push_str("\\\\\\\\"),
            '%' => quoted.push_str("%%"),
            _ => quoted.push(c),
        }
    }
    quoted.push_str("\" --background");
    Some(quoted)
}

/// The autostart entry that starts `program` in the background; `None` where [`exec_line`] has none.
#[must_use]
pub fn entry(program: &Path) -> Option<String> {
    let exec = exec_line(program)?;
    Some(format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name=InboxCraft\n\
         Comment=Keeps your mail in step in the background\n\
         Exec={exec}\n\
         Icon=io.github.qubernetic.inboxcraft\n\
         Terminal=false\n\
         NoDisplay=true\n\
         X-GNOME-Autostart-enabled=true\n"
    ))
}

/// Whether the entry is in `dir`.
#[must_use]
pub fn is_on(dir: &Path) -> bool {
    dir.join(ENTRY_FILE).is_file()
}

/// Asks the Background portal: whether InboxCraft starts at login, and that it may run in the background.
pub type Ask<'a> = &'a dyn Fn(bool) -> Result<Granted, String>;

/// Turns starting at login on or off: the entry for `program` in `dir`, and, inside the Flatpak, the portal through
/// `portal` first. Says what it did.
///
/// # Errors
///
/// The portal refused or could not be reached (the choice stays as it was), a program path no entry can name, or a
/// folder that cannot be written.
pub fn choose(on: bool, dir: &Path, program: &Path, portal: Option<Ask<'_>>) -> Result<String, String> {
    if let Some(ask) = portal {
        let granted = ask(on)?;
        if on && !granted.autostart {
            return Err("the desktop did not allow InboxCraft to start at login".to_owned());
        }
    }
    let file = dir.join(ENTRY_FILE);
    let written = if on {
        let text = entry(program).ok_or_else(|| format!("{} cannot be started at login: an entry cannot name it", program.display()))?;
        std::fs::create_dir_all(dir).and_then(|()| std::fs::write(&file, text))
    } else {
        match std::fs::remove_file(&file) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            other => other,
        }
    };
    written.map_err(|error| format!("the autostart entry {} could not be changed: {error}", file.display()))?;
    Ok(if on { "InboxCraft starts at login, in the background".to_owned() } else { "InboxCraft no longer starts at login".to_owned() })
}

/// The Background portal (`org.freedesktop.portal.Background`), over the session bus.
#[cfg(target_os = "linux")]
pub mod portal {
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::time::Duration;

    use zbus::blocking::{Connection, Proxy};
    use zbus::zvariant::{OwnedObjectPath, OwnedValue, Value};

    use super::Granted;

    /// The portal's bus name.
    pub const DESTINATION: &str = "org.freedesktop.portal.Desktop";

    /// The portal's object.
    pub const PATH: &str = "/org/freedesktop/portal/desktop";

    /// Why InboxCraft asks, as the desktop may show it.
    pub const REASON: &str = "InboxCraft keeps your mail in step while its window is closed, and starts at login when you ask it to.";

    /// What the Background portal starts at login.
    pub const COMMANDLINE: [&str; 2] = ["inboxcraft", "--background"];

    static TOKENS: AtomicU32 = AtomicU32::new(0);

    /// Asks the portal on `bus` to let InboxCraft run in the background and to start it at login or not (`autostart`),
    /// and waits up to `patience` for the answer: the desktop may ask the person first.
    ///
    /// # Errors
    ///
    /// No portal, a refusal or a cancel, or no answer in time.
    pub fn request(bus: &Connection, autostart: bool, patience: Duration) -> Result<Granted, String> {
        let failed = |error: zbus::Error| format!("the Background portal could not be asked: {error}");
        let token = format!("inboxcraft{}_{}", std::process::id(), TOKENS.fetch_add(1, Ordering::Relaxed));
        let sender = bus.unique_name().map(|name| name.trim_start_matches(':').replace('.', "_")).unwrap_or_default();
        let path = format!("{PATH}/request/{sender}/{token}");
        // Listen before asking, so an answer that comes at once is not missed.
        let request = Proxy::new(bus, DESTINATION, path, "org.freedesktop.portal.Request").map_err(failed)?;
        let mut responses = request.receive_signal("Response").map_err(failed)?;
        let options: HashMap<&str, Value<'_>> = HashMap::from([
            ("handle_token", Value::from(token.as_str())),
            ("reason", Value::from(REASON)),
            ("autostart", Value::from(autostart)),
            ("commandline", Value::from(COMMANDLINE.to_vec())),
            ("dbus-activatable", Value::from(false)),
        ]);
        let background = Proxy::new(bus, DESTINATION, PATH, "org.freedesktop.portal.Background").map_err(failed)?;
        let _: OwnedObjectPath = background.call("RequestBackground", &("", options)).map_err(failed)?;
        let (send, answer) = std::sync::mpsc::channel();
        // The wait is on a thread of its own, so it can be given up; a thread whose answer never comes ends with the run.
        std::thread::spawn(move || {
            let body = responses.next().map(|message| message.body().deserialize::<(u32, HashMap<String, OwnedValue>)>());
            let _ = send.send(body);
        });
        match answer.recv_timeout(patience) {
            Ok(Some(Ok((0, results)))) => {
                let flag = |key: &str| results.get(key).and_then(|value| value.downcast_ref::<bool>().ok()).unwrap_or(false);
                Ok(Granted { background: flag("background"), autostart: flag("autostart") })
            }
            Ok(Some(Ok((1, _)))) => Err("the Background portal refused".to_owned()),
            Ok(Some(Ok(_))) => Err("the Background portal was cancelled".to_owned()),
            Ok(Some(Err(error))) => Err(format!("the Background portal answered something else: {error}")),
            Ok(None) | Err(_) => Err("the Background portal did not answer".to_owned()),
        }
    }
}

#[cfg(test)]
#[path = "autostart_tests.rs"]
mod tests;
