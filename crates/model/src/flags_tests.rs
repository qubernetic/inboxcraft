// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{Flag, FlagError, Flags, Keyword};

fn keyword(text: &str) -> Keyword {
    Keyword::new(text).unwrap()
}

#[test]
fn system_flags_read_without_case() {
    let cases = [
        ("\\Seen", Flag::Seen),
        ("\\SEEN", Flag::Seen),
        ("\\answered", Flag::Answered),
        ("\\Flagged", Flag::Flagged),
        ("\\Deleted", Flag::Deleted),
        ("\\Draft", Flag::Draft),
    ];
    for (text, flag) in cases {
        assert_eq!(Flag::parse(text), Ok(flag), "{text:?}");
    }
}

#[test]
fn keywords_read_as_keywords() {
    assert_eq!(Flag::parse("$Forwarded"), Ok(Flag::Keyword(keyword("$Forwarded"))));
    assert_eq!(Flag::parse("NonJunk"), Ok(Flag::Keyword(keyword("NonJunk"))));
}

#[test]
fn other_system_flags_and_bad_keywords_are_errors() {
    assert_eq!(Flag::parse("\\Recent"), Err(FlagError::UnknownSystemFlag));
    assert_eq!(Flag::parse("\\*"), Err(FlagError::UnknownSystemFlag));
    for text in ["", "a b", "a(b", "a)b", "a{b", "a%b", "a*b", "a\"b", "a]b", "a\\b", "é", "a\rb", "a\u{7f}"] {
        assert_eq!(Keyword::new(text), Err(FlagError::BadKeyword), "{text:?}");
    }
    assert_eq!(Keyword::new(&"k".repeat(Keyword::MAX_LEN + 1)), Err(FlagError::BadKeyword));
    assert!(Keyword::new(&"k".repeat(Keyword::MAX_LEN)).is_ok());
    assert_eq!(FlagError::UnknownSystemFlag.to_string(), "the flag is a system flag this program does not know");
    assert_eq!(FlagError::BadKeyword.to_string(), "the flag is not a valid keyword");
}

#[test]
fn keywords_compare_without_case_and_keep_their_text() {
    assert_eq!(keyword("$Junk"), keyword("$JUNK"));
    assert_eq!(keyword("$Junk").as_str(), "$Junk");
    assert!(keyword("a") < keyword("B"));
    let hashed: std::collections::HashSet<_> = [keyword("x"), keyword("X")].into();
    assert_eq!(hashed.len(), 1);
}

#[test]
fn flags_hold_the_system_flags_and_the_keywords() {
    let mut flags = Flags::default();
    assert!(flags.is_empty());
    assert!(flags.insert(Flag::Seen));
    assert!(!flags.insert(Flag::Seen));
    assert!(flags.insert(Flag::Keyword(keyword("$Forwarded"))));
    assert!(!flags.insert(Flag::Keyword(keyword("$FORWARDED"))));
    assert!(flags.contains(&Flag::Seen));
    assert!(flags.contains(&Flag::Keyword(keyword("$forwarded"))));
    assert!(!flags.contains(&Flag::Draft));
    assert!(flags.remove(&Flag::Seen));
    assert!(!flags.remove(&Flag::Seen));
    assert!(flags.remove(&Flag::Keyword(keyword("$forwarded"))));
    assert!(flags.is_empty());
}

#[test]
fn flags_list_in_a_fixed_order() {
    let flags: Flags = [Flag::Keyword(keyword("b")), Flag::Draft, Flag::Keyword(keyword("a")), Flag::Seen, Flag::Answered].into_iter().collect();
    let listed: Vec<String> = flags.iter().map(|flag| flag.to_string()).collect();
    assert_eq!(listed, ["\\Seen", "\\Answered", "\\Draft", "a", "b"]);
    let every: Flags = [Flag::Seen, Flag::Answered, Flag::Flagged, Flag::Deleted, Flag::Draft].into_iter().collect();
    assert_eq!(every.iter().count(), 5);
    let written: Vec<String> = every.iter().map(|flag| flag.to_string()).collect();
    assert_eq!(written, ["\\Seen", "\\Answered", "\\Flagged", "\\Deleted", "\\Draft"]);
    for flag in every.iter() {
        assert!(every.contains(&flag), "{flag}");
        assert_eq!(Flag::parse(&flag.to_string()), Ok(flag));
    }
    assert_eq!(keyword("$Junk").to_string(), "$Junk");
    assert!(every.is_seen() && every.is_flagged() && every.is_deleted());
}

#[test]
fn a_message_holds_at_most_max_keywords() {
    let mut flags = Flags::default();
    for n in 0..Flags::MAX_KEYWORDS {
        assert!(flags.insert(Flag::Keyword(keyword(&format!("k{n}")))));
    }
    assert!(!flags.insert(Flag::Keyword(keyword("one-too-many"))));
    assert!(flags.insert(Flag::Seen));
    assert_eq!(flags.iter().count(), Flags::MAX_KEYWORDS + 1);
}

#[test]
fn flags_serialise_and_are_checked_on_the_way_in() {
    let flags: Flags = [Flag::Seen, Flag::Keyword(keyword("$Junk"))].into_iter().collect();
    let json = serde_json::to_string(&flags).unwrap();
    assert_eq!(serde_json::from_str::<Flags>(&json).unwrap(), flags);
    let bad = r#"{"seen":false,"answered":false,"flagged":false,"deleted":false,"draft":false,"keywords":["a b"]}"#;
    assert!(serde_json::from_str::<Flags>(bad).is_err());
    let many: Vec<String> = (0..Flags::MAX_KEYWORDS + 5).map(|n| format!("\"k{n}\"")).collect();
    let json = format!(r#"{{"seen":true,"answered":false,"flagged":false,"deleted":false,"draft":false,"keywords":[{}]}}"#, many.join(","));
    let read = serde_json::from_str::<Flags>(&json).unwrap();
    assert_eq!(read.iter().count(), Flags::MAX_KEYWORDS + 1);
}
