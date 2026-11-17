// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{TRAY_SIDES, TrayInk, WINDOW_ICON_SIDE, tray_icon, window_icon};

#[test]
fn the_window_icon_is_the_brand_s_256_px_mark() {
    let icon = window_icon().unwrap();
    assert_eq!((icon.width, icon.height), (256, 256));
    assert_eq!(WINDOW_ICON_SIDE, 256);
    assert_eq!(icon.rgba.len(), 262_144);
    let (pixels, _) = icon.rgba.as_chunks::<4>();
    let first = pixels[0];
    assert!(pixels.iter().any(|pixel| *pixel != first), "not all one colour");
    assert!(pixels.iter().any(|pixel| pixel[3] == 255), "the mark is opaque somewhere");
}

#[test]
fn a_tray_size_that_is_not_shipped_is_an_error() {
    for ink in [TrayInk::Light, TrayInk::Dark] {
        for unread in [false, true] {
            assert!(tray_icon(17, ink, unread).is_err());
            assert!(tray_icon(0, ink, unread).is_err());
            assert!(tray_icon(u32::MAX, ink, unread).is_err());
        }
    }
}

#[test]
fn every_shipped_tray_icon_is_its_side_square() {
    assert_eq!(TRAY_SIDES, [16, 22, 24]);
    for side in TRAY_SIDES {
        for ink in [TrayInk::Light, TrayInk::Dark] {
            for unread in [false, true] {
                let icon = tray_icon(side, ink, unread).unwrap();
                assert_eq!((icon.width, icon.height), (side, side), "{side} {ink:?} {unread}");
                assert_eq!(icon.rgba.len(), (side * side * 4) as usize);
            }
        }
    }
}

#[test]
fn the_dark_ink_draws_light_and_the_light_ink_draws_dark() {
    // The brightest opaque pixel of the plain icon: the drawing, as nothing else is opaque.
    let brightest = |ink| {
        let icon = tray_icon(22, ink, false).unwrap();
        let (pixels, _) = icon.rgba.as_chunks::<4>();
        pixels.iter().filter(|pixel| pixel[3] == 255).map(|pixel| pixel[0]).max().unwrap()
    };
    assert!(brightest(TrayInk::Dark) > 160, "a light drawing for a dark panel");
    assert!(brightest(TrayInk::Light) < 90, "a dark drawing for a light panel");
}
