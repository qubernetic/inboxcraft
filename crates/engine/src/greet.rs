// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Which word of the owner's sending name Home's headline greets them by: the owner's choice, set by `greeting.set`
//! (`owner`). The session keeps it; the front end keeps it on disk and hands it back at the start
//! ([`crate::Session::set_greet_by`]).
//!
//! [`GreetBy::word`] reads a name the way it is written, with no guess at its order: "Kovács Anna" is greeted by
//! "Kovács" with the first word, as a family-name-first name is written. Only a comma turns it round: "Kovács, Anna"
//! reads as last name, then first. The titles of [`TITLES`] are skipped for the first and the last word, never for
//! the full name. A name that is an address (it holds `@`), is empty, or has nothing but titles gives no word, and
//! the headline greets without a name.

/// The titles the first and the last word skip, compared without case and without one dot at the end: "Dr", "Dr.",
/// "dr." (the Hungarian), "Mr", "Mrs", "Ms", "Prof", "Sir", "Dame".
pub const TITLES: [&str; 7] = ["dr", "mr", "mrs", "ms", "prof", "sir", "dame"];

/// The most characters of a name read: a longer one is read up to here.
pub const MOST: usize = 256;

/// Which word of the sending name the greeting uses.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum GreetBy {
    /// The first word that is not a title: "Anna" of "Dr. Anna Kovács".
    #[default]
    First,
    /// The last word that is not a title: "Kovács" of "Dr. Anna Kovács".
    Last,
    /// The whole name, as written.
    Full,
    /// No name: "Good morning."
    None,
}

impl GreetBy {
    /// Every choice, in the order the settings show them.
    pub const ALL: [Self; 4] = [Self::First, Self::Last, Self::Full, Self::None];

    /// The choice's name in `greeting.set` and on disk.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::First => "first",
            Self::Last => "last",
            Self::Full => "full",
            Self::None => "none",
        }
    }

    /// The choice named `name`; none for any other text.
    #[must_use]
    pub fn named(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|by| by.name() == name)
    }

    /// The word of `name` the greeting uses, on one line, with no control or bidi character; `None` for no word.
    #[must_use]
    pub fn word(self, name: &str) -> Option<String> {
        if self == Self::None {
            return None;
        }
        let plain: String = name.chars().take(MOST).map(|c| if c.is_control() { ' ' } else { c }).filter(|c| !bidi(*c)).collect();
        if plain.contains('@') {
            return None;
        }
        let words =
            |text: &str| text.split_whitespace().filter(|word| word.chars().any(char::is_alphanumeric)).map(str::to_owned).collect::<Vec<_>>();
        if self == Self::Full {
            let full = plain.split_whitespace().collect::<Vec<_>>().join(" ");
            return (!words(&full).is_empty()).then_some(full);
        }
        // "Last, First …": the words after the comma come first.
        let ordered = match plain.split_once(',') {
            Some((last, first)) => [words(first), words(last)].concat(),
            None => words(&plain),
        };
        let mut names = ordered.into_iter().filter(|word| !title(word));
        if self == Self::Last { names.next_back() } else { names.next() }
    }
}

/// Whether `word` is one of [`TITLES`], without case and one dot at the end.
fn title(word: &str) -> bool {
    let bare = word.strip_suffix('.').unwrap_or(word).to_lowercase();
    TITLES.contains(&bare.as_str())
}

/// Whether `c` steers the direction of text: the marks, embeddings, overrides and isolates.
const fn bidi(c: char) -> bool {
    matches!(c, '\u{061c}' | '\u{200e}' | '\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
}
