// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The kinds of characters, checked on known ones of every kind and at the ends of the table.

use super::{Class, RUNS, class};

#[test]
fn known_characters_have_their_kind() {
    for (kind, of) in [
        (Class::Letter, "aZéőñßΩжא日ꞌ\u{2CEB0}"),
        (Class::Mark, "\u{0301}\u{0308}\u{030B}\u{0315}\u{0670}\u{093F}\u{20E3}\u{FE0F}\u{E0100}"),
        (Class::Number, "09²½Ⅷ٣\u{1D7CE}"),
        (Class::Punctuation, "-–—/()&,.:!?'\"_…·„〝"),
        (Class::Symbol, "+<=>|~$€©°→\u{1F389}\u{1F44D}"),
        (Class::QuoteLike, "“”‘’«»‹›‛‟^`´¨¯¸ʰʹʺʻʼˈˊ˝΄΅᾿῍\u{1F3FB}\u{A67F}\u{FF40}\u{2E03}"),
        (Class::Other, " \t\n\u{00A0}\u{00AD}\u{0378}\u{200B}\u{2028}\u{E000}\u{FFFE}\u{E0001}\u{F0000}\u{10FFFF}"),
    ] {
        for c in of.chars() {
            assert_eq!(class(c), kind, "U+{:04X}", u32::from(c));
        }
    }
}

#[test]
fn the_table_is_in_order_and_names_only_known_kinds() {
    assert_eq!(RUNS.first(), Some(&0));
    assert!(RUNS.windows(2).all(|pair| matches!(pair, [one, next] if one >> 3 < next >> 3 && one & 7 != next & 7)));
    assert!(RUNS.iter().all(|run| run & 7 <= 6 && run >> 3 <= 0x10_FFFF));
}
