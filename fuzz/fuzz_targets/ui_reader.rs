// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The reader's own readers of mail on any bytes: the quote folding of a plain-text body stays within its caps and
//! lets no control character but the line break and the tab through, and the picture decoder either refuses or gives
//! exactly four bytes a pixel within the size limits. Neither panics.

#![no_main]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use inboxcraft_ui_egui::picture::{MAX_PIXELS, MAX_SIDE, decode};
use inboxcraft_ui_egui::quote::{MAX_SEGMENTS, MAX_TEXT, Segment, split};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let text = String::from_utf8_lossy(data);
    let segments = split(&text);
    assert!(segments.len() <= MAX_SEGMENTS);
    let mut total = 0_usize;
    for segment in &segments {
        let (Segment::Text(shown) | Segment::Quote { text: shown, .. }) = segment;
        total = total.saturating_add(shown.len());
        assert!(!shown.chars().any(|c| c.is_control() && c != '\n' && c != '\t'), "{shown:?}");
    }
    assert!(total <= MAX_TEXT);
    if let Ok(picture) = decode(data) {
        assert!(picture.width <= MAX_SIDE && picture.height <= MAX_SIDE);
        assert!(picture.width.saturating_mul(picture.height) <= MAX_PIXELS);
        assert_eq!(picture.rgba.len(), picture.width * picture.height * 4);
    }
    // The same bytes behind each picture signature.
    for signature in [&b"\x89PNG\r\n\x1a\n"[..], &[0xFF, 0xD8, 0xFF][..]] {
        let mut bytes = signature.to_vec();
        bytes.extend_from_slice(data);
        let _ = decode(&bytes);
    }
});
