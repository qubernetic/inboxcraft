// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The app's own icons from the brand's files: the window icon and the tray icon. The icons inside the window come
//! from an icon set, see [`crate::iconset`].
//!
//! Both are the InboxCraft mark, a trademark of Qubernetic that is not under the code's licences (see `NOTICE` and
//! `ATTRIBUTION.md`). The files are pinned in `.config/assets.toml` and built into the binary; the desktop's own
//! icon is the same mark, which the Flatpak installs from `assets/brand/inboxcraft/linux/hicolor/`.

use egui::IconData;

use crate::UiError;
use crate::picture;

/// The side of the window icon in pixels.
pub const WINDOW_ICON_SIDE: u32 = 256;

/// The sides, in pixels, the tray icon ships at.
pub const TRAY_SIDES: [u32; 3] = [16, 22, 24];

/// The window icon: the mark on its tile.
const WINDOW_ICON: &[u8] = include_bytes!("../../../assets/brand/inboxcraft/png/icon-256.png");

/// Which panel a tray icon is drawn for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayInk {
    /// A light panel: the mark drawn dark.
    Light,
    /// A dark panel: the mark drawn light.
    Dark,
}

/// The tray icon files, by side, ink and whether mail is unread (the flap cyan).
const TRAY: [(u32, TrayInk, bool, &[u8]); 12] = [
    (16, TrayInk::Light, false, include_bytes!("../../../assets/brand/inboxcraft/tray/tray-16-light.png")),
    (16, TrayInk::Dark, false, include_bytes!("../../../assets/brand/inboxcraft/tray/tray-16-dark.png")),
    (16, TrayInk::Light, true, include_bytes!("../../../assets/brand/inboxcraft/tray/tray-unread-16-light.png")),
    (16, TrayInk::Dark, true, include_bytes!("../../../assets/brand/inboxcraft/tray/tray-unread-16-dark.png")),
    (22, TrayInk::Light, false, include_bytes!("../../../assets/brand/inboxcraft/tray/tray-22-light.png")),
    (22, TrayInk::Dark, false, include_bytes!("../../../assets/brand/inboxcraft/tray/tray-22-dark.png")),
    (22, TrayInk::Light, true, include_bytes!("../../../assets/brand/inboxcraft/tray/tray-unread-22-light.png")),
    (22, TrayInk::Dark, true, include_bytes!("../../../assets/brand/inboxcraft/tray/tray-unread-22-dark.png")),
    (24, TrayInk::Light, false, include_bytes!("../../../assets/brand/inboxcraft/tray/tray-24-light.png")),
    (24, TrayInk::Dark, false, include_bytes!("../../../assets/brand/inboxcraft/tray/tray-24-dark.png")),
    (24, TrayInk::Light, true, include_bytes!("../../../assets/brand/inboxcraft/tray/tray-unread-24-light.png")),
    (24, TrayInk::Dark, true, include_bytes!("../../../assets/brand/inboxcraft/tray/tray-unread-24-dark.png")),
];

/// The app icon for the window, [`WINDOW_ICON_SIDE`] pixels a side, with straight alpha as the window system wants.
///
/// # Errors
///
/// [`UiError::Picture`] when the built-in file does not decode as a square of [`WINDOW_ICON_SIDE`] pixels.
pub fn window_icon() -> Result<IconData, UiError> {
    decoded(WINDOW_ICON, WINDOW_ICON_SIDE)
}

/// The tray icon of `side` pixels for a panel of `ink`, its flap cyan when mail is `unread`; straight alpha.
///
/// # Errors
///
/// [`UiError::Picture`] for a side not in [`TRAY_SIDES`], or a built-in file that does not decode at its side.
pub fn tray_icon(side: u32, ink: TrayInk, unread: bool) -> Result<IconData, UiError> {
    let (_, _, _, bytes) = TRAY
        .iter()
        .find(|(shipped, drawn, flagged, _)| *shipped == side && *drawn == ink && *flagged == unread)
        .ok_or_else(|| UiError::Picture(format!("no tray icon of {side} px")))?;
    decoded(bytes, side)
}

/// `bytes` decoded as an icon `side` pixels square.
fn decoded(bytes: &[u8], side: u32) -> Result<IconData, UiError> {
    let picture = picture::decode(bytes)?;
    let width = u32::try_from(picture.width).map_err(|_| UiError::Picture("the icon is too wide".to_owned()))?;
    let height = u32::try_from(picture.height).map_err(|_| UiError::Picture("the icon is too high".to_owned()))?;
    if (width, height) != (side, side) {
        return Err(UiError::Picture(format!("the icon is {width} by {height} px, not {side} px square")));
    }
    Ok(IconData { rgba: picture.rgba, width, height })
}

#[cfg(test)]
#[path = "icons_tests.rs"]
mod tests;
