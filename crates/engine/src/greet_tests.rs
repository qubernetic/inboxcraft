// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Which word of the sending name Home greets by: each choice on a set of names, the titles, the comma, the
//! addresses, and hostile names.

use crate::greet::{GreetBy, MOST};

/// The word `by` gives for `name`, as a plain `&str` to compare (`""` for none).
fn word(by: GreetBy, name: &str) -> String {
    by.word(name).unwrap_or_default()
}

#[test]
fn the_first_word_is_the_default_and_every_choice_has_its_name() {
    assert_eq!(GreetBy::default(), GreetBy::First);
    for (by, name) in [(GreetBy::First, "first"), (GreetBy::Last, "last"), (GreetBy::Full, "full"), (GreetBy::None, "none")] {
        assert_eq!(by.name(), name);
        assert_eq!(GreetBy::named(name), Some(by));
    }
    for junk in ["", "First", "first ", "middle", "first\u{0}", "\u{202e}last"] {
        assert_eq!(GreetBy::named(junk), None, "{junk:?}");
    }
}

#[test]
fn each_choice_reads_a_set_of_names_as_written() {
    // name, first, last, full
    let names = [
        ("Odile Brannock", "Odile", "Brannock", "Odile Brannock"),
        ("Odile", "Odile", "Odile", "Odile"),
        ("Anna Maria  de la Cruz", "Anna", "Cruz", "Anna Maria de la Cruz"),
        // A family-name-first name is not turned round: the order is not guessed.
        ("Kovács Anna", "Kovács", "Anna", "Kovács Anna"),
        // The comma says "last, first".
        ("Kovács, Anna", "Anna", "Kovács", "Kovács, Anna"),
        ("Brannock, Odile Mae", "Odile", "Brannock", "Brannock, Odile Mae"),
    ];
    for (name, first, last, full) in names {
        assert_eq!(word(GreetBy::First, name), first, "first of {name:?}");
        assert_eq!(word(GreetBy::Last, name), last, "last of {name:?}");
        assert_eq!(word(GreetBy::Full, name), full, "full of {name:?}");
        assert_eq!(GreetBy::None.word(name), None, "none of {name:?}");
    }
}

#[test]
fn titles_are_skipped_for_the_first_and_the_last_word_only() {
    for name in ["Dr. Anna Kovács", "dr. Anna Kovács", "DR Anna Kovács", "Prof. Dr. Anna Kovács", "Mrs Anna Kovács", "Sir Anna Kovács"] {
        assert_eq!(word(GreetBy::First, name), "Anna", "{name:?}");
        assert_eq!(word(GreetBy::Last, name), "Kovács", "{name:?}");
    }
    // A title after the name, and the Hungarian order with the title first and the comma.
    assert_eq!(word(GreetBy::Last, "Anna Kovács Dame"), "Kovács");
    assert_eq!(word(GreetBy::First, "Kovács, dr. Anna"), "Anna");
    for (title, first) in [("Mr. Ansel Moor", "Ansel"), ("Ms Mira Vale", "Mira"), ("Dame Mira Vale", "Mira"), ("Prof Ansel Moor", "Ansel")] {
        assert_eq!(word(GreetBy::First, title), first, "{title:?}");
    }
    assert_eq!(word(GreetBy::Full, "Dr. Anna Kovács"), "Dr. Anna Kovács", "the full name keeps its title");
    // A word that only starts like a title is a name.
    assert_eq!(word(GreetBy::First, "Drew Moor"), "Drew");
    assert_eq!(word(GreetBy::First, "Mrs. Dr."), "", "nothing but titles: no name");
}

#[test]
fn an_address_or_an_empty_name_gives_no_word() {
    for by in GreetBy::ALL {
        for name in ["", "   ", "anna@example.org", "Anna <anna@example.org>", "- , .", "\u{0}\r\n"] {
            assert_eq!(by.word(name), None, "{by:?} of {name:?}");
        }
    }
}

#[test]
fn hostile_names_give_one_clean_line_or_none() {
    let long = "A".repeat(10_000);
    let hostile = [
        "Anna\r\nBcc: x@example.org".to_owned(),
        "Anna\u{0}Kovács".to_owned(),
        "\u{202e}Anna\u{202c} \u{2066}Kovács\u{2069}".to_owned(),
        "\u{200f}Dr.\u{200e} Anna".to_owned(),
        format!("{long} Kovács"),
        "Dr. Mr. Prof. Sir Dame Ms Mrs".to_owned(),
        ",,,".to_owned(),
        "Anna,".to_owned(),
        ", Anna".to_owned(),
    ];
    for by in GreetBy::ALL {
        for name in &hostile {
            let Some(word) = by.word(name) else { continue };
            assert!(!word.is_empty(), "{by:?} of {name:?}");
            assert!(
                !word.chars().any(|c| c.is_control() || matches!(c, '\u{200e}' | '\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}'))
            );
            assert!(word.chars().count() <= MOST, "{by:?}: {} characters", word.chars().count());
        }
    }
    assert_eq!(word(GreetBy::First, "Anna\r\nBcc: x@example.org"), "", "an address anywhere: no word");
    assert_eq!(word(GreetBy::First, "Anna\u{0}Kovács"), "Anna");
    assert_eq!(word(GreetBy::Last, "\u{202e}Anna\u{202c} \u{2066}Kovács\u{2069}"), "Kovács");
    assert_eq!(word(GreetBy::First, "\u{200f}Dr.\u{200e} Anna"), "Anna", "a title hidden behind a mark is still a title");
    assert_eq!(word(GreetBy::First, "Dr. Mr. Prof. Sir Dame Ms Mrs"), "");
    assert_eq!(word(GreetBy::First, "Anna,"), "Anna");
    assert_eq!(word(GreetBy::Last, ", Anna"), "Anna");
    assert_eq!(word(GreetBy::Last, &format!("{long} Kovács")).chars().count(), MOST, "read up to the cap, the rest unread");
}
