// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use proptest::prelude::*;

use super::{Component, MAX_BYTES, MAX_DEPTH, MAX_ITEMS, Property, parse};
use crate::IcalError;

const SMALL: &str = "BEGIN:VCALENDAR\r\n\
    VERSION:2.0\r\n\
    BEGIN:VEVENT\r\n\
    UID:1@example.org\r\n\
    SUMMARY;LANGUAGE=en:Lunch\r\n\
    END:VEVENT\r\n\
    END:VCALENDAR\r\n";

#[test]
fn reads_a_tree_of_components_and_properties() {
    let calendar = parse(SMALL.as_bytes()).unwrap();
    assert_eq!(calendar.name(), "VCALENDAR");
    assert_eq!(calendar.property("version").unwrap().value(), "2.0");
    let event = calendar.components_named("vevent").next().unwrap();
    assert_eq!(event.property("UID").unwrap().value(), "1@example.org");
    let summary = event.property("summary").unwrap();
    assert_eq!(summary.value(), "Lunch");
    assert_eq!(summary.param("language"), Some("en"));
    assert_eq!(summary.param("altrep"), None);
    assert_eq!(summary.params()[0].name(), "LANGUAGE");
    assert_eq!(calendar.components().len(), 1);
    assert_eq!(calendar.properties().len(), 1);
}

#[test]
fn unfolds_lines_on_the_bytes_and_takes_bare_line_feeds() {
    // The fold splits the two bytes of "é".
    let input = b"BEGIN:X\nSUMMARY:Caf\xc3\r\n \xa9 au\r\n\tlait\n\r\n\nEND:X";
    let component = parse(input).unwrap();
    assert_eq!(component.property("SUMMARY").unwrap().value(), "Café aulait");
}

#[test]
fn bytes_that_are_no_utf8_become_replacement_characters() {
    let component = parse(b"BEGIN:X\r\nSUMMARY:a\xffb\r\nEND:X\r\n").unwrap();
    assert_eq!(component.property("SUMMARY").unwrap().value(), "a\u{fffd}b");
}

#[test]
fn reads_quoted_list_and_caret_escaped_parameters() {
    let input = "BEGIN:X\r\n\
        ATTENDEE;MEMBER=\"mailto:a@example.org\",\"mailto:b@example.org\";CN=\"Kov^'acs; ^^ ^n^x^\":mailto:c@example.org\r\n\
        X-EMPTY;X-P=:v:w\r\n\
        END:X\r\n";
    let component = parse(input.as_bytes()).unwrap();
    let attendee = component.property("ATTENDEE").unwrap();
    assert_eq!(attendee.params()[0].values(), ["mailto:a@example.org", "mailto:b@example.org"]);
    assert_eq!(attendee.param("CN"), Some("Kov\"acs; ^ \n^x^"));
    assert_eq!(attendee.value(), "mailto:c@example.org");
    let empty = component.property("x-empty").unwrap();
    assert_eq!(empty.param("X-P"), Some(""));
    assert_eq!(empty.value(), "v:w");
}

#[test]
fn refuses_lines_that_are_no_content_lines() {
    for line in ["no colon", ":value", "A B:c", "A;:c", "A;P:c", "A;P=\"open:c", "A;P=\"q\"x:c", "A;B C=d:e"] {
        let input = format!("BEGIN:X\r\n{line}\r\nEND:X\r\n");
        assert_eq!(parse(input.as_bytes()), Err(IcalError::BadLine { line: 2 }), "{line}");
    }
    assert_eq!(parse(b"BEGIN:A B\r\nEND:A B\r\n"), Err(IcalError::BadLine { line: 1 }));
    // A fold with nothing before it is a line of its own, and no content line.
    assert_eq!(parse(b" BEGIN:X\r\nEND:X\r\n"), Err(IcalError::BadLine { line: 1 }));
}

#[test]
fn refuses_unbalanced_and_unterminated_components() {
    assert_eq!(parse(b"END:X\r\n"), Err(IcalError::Unbalanced { line: 1 }));
    assert_eq!(parse(b"BEGIN:X\r\nEND:Y\r\n"), Err(IcalError::Unbalanced { line: 2 }));
    assert_eq!(parse(b"SUMMARY:x\r\n"), Err(IcalError::Unbalanced { line: 1 }));
    assert_eq!(parse(b"BEGIN:X\r\nBEGIN:Y\r\nEND:Y\r\n"), Err(IcalError::Unterminated));
    assert_eq!(parse(b""), Err(IcalError::NotACalendar));
    assert_eq!(parse(b"BEGIN:X\r\nEND:X\r\nBEGIN:X\r\nEND:X\r\n"), Err(IcalError::NotACalendar));
    // The END matches in any case.
    assert!(parse(b"BEGIN:x\r\nEND:X\r\n").is_ok());
}

#[test]
fn caps_size_items_and_depth() {
    let large = vec![b'A'; MAX_BYTES + 1];
    assert_eq!(parse(&large), Err(IcalError::TooLarge { limit: MAX_BYTES }));

    let mut many = String::from("BEGIN:X\r\n");
    many.push_str(&"A:b\r\n".repeat(MAX_ITEMS));
    many.push_str("END:X\r\n");
    assert_eq!(parse(many.as_bytes()), Err(IcalError::TooManyItems { limit: MAX_ITEMS }));

    let deep = "BEGIN:X\r\n".repeat(MAX_DEPTH + 1);
    assert_eq!(parse(deep.as_bytes()), Err(IcalError::TooDeep { limit: MAX_DEPTH }));
    let fits = format!("{}{}", "BEGIN:X\r\n".repeat(MAX_DEPTH), "END:X\r\n".repeat(MAX_DEPTH));
    assert!(parse(fits.as_bytes()).is_ok());
}

#[test]
fn writes_folded_crlf_lines() {
    let long = "é".repeat(60);
    let component = Component::new("vcalendar")
        .with_property(Property::new("x-long", long.clone()))
        .with_component(Component::new("VEVENT").with_property(Property::new("UID", "1")));
    let text = component.write().unwrap();
    assert!(text.starts_with("BEGIN:VCALENDAR\r\nX-LONG:"));
    assert!(text.ends_with("BEGIN:VEVENT\r\nUID:1\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n"));
    for line in text.split("\r\n") {
        assert!(line.len() <= 75, "{line:?} is {} octets", line.len());
    }
    assert_eq!(parse(text.as_bytes()).unwrap().property("X-LONG").unwrap().value(), long);
}

#[test]
fn writes_parameters_quoted_and_escaped_and_strips_line_breaks_from_values() {
    let property = Property::new("ATTENDEE", "mailto:a@example.org\r\nBCC:x\0")
        .with_param("cn", "A \"B\"; C^\nD\u{7}\tE")
        .with_param_values("member", vec!["mailto:x@example.org".into(), "plain".into()])
        .with_param_values("X-NONE", Vec::new())
        .with_param("CN", "Ann");
    let text = Component::new("X").with_property(property).write().unwrap();
    assert_eq!(
        text.replace("\r\n ", ""),
        "BEGIN:X\r\nATTENDEE;MEMBER=\"mailto:x@example.org\",plain;X-NONE=;CN=Ann:mailto:a@example.orgBCC:x\r\nEND:X\r\n"
    );
    let quoted = Component::new("X").with_property(Property::new("A", "v").with_param("P", "A \"B\"; C^\nD\u{7}\tE"));
    let text = quoted.write().unwrap();
    assert!(text.contains("A;P=\"A ^'B^'; C^^^nD\tE\":v"), "{text}");
    assert_eq!(parse(text.as_bytes()).unwrap().property("A").unwrap().param("P"), Some("A \"B\"; C^\nD\tE"));
}

#[test]
fn refuses_to_write_names_that_are_no_tokens() {
    assert_eq!(Component::new("A B").write(), Err(IcalError::bad_name("A B")));
    assert_eq!(Component::new("X").with_property(Property::new("A:B", "v")).write(), Err(IcalError::bad_name("A:B")));
    let bad_param = Property::new("A", "v").with_param("P;Q", "w");
    assert_eq!(Component::new("X").with_property(bad_param).write(), Err(IcalError::bad_name("P;Q")));
    assert_eq!(Component::new("").write(), Err(IcalError::bad_name("")));
    assert_eq!(Component::new("X").with_property(Property::new("end", "X")).write(), Err(IcalError::bad_name("END")));
    assert_eq!(Component::new("X").with_property(Property::new("BEGIN", "Y")).write(), Err(IcalError::bad_name("BEGIN")));
}

#[test]
fn pushing_adds_after_the_others() {
    let mut component = Component::new("X");
    component.push_property(Property::new("A", "1"));
    component.push_property(Property::new("a", "2"));
    component.push_component(Component::new("Y"));
    let values: Vec<_> = component.properties_named("A").map(Property::value).collect();
    assert_eq!(values, ["1", "2"]);
    assert_eq!(component.components()[0].name(), "Y");
}

fn name() -> impl Strategy<Value = String> {
    "[A-Z][A-Z0-9-]{0,12}".prop_filter("BEGIN and END open and close", |name| name != "BEGIN" && name != "END")
}

fn property() -> impl Strategy<Value = Property> {
    let params = prop::collection::vec((name(), prop::collection::vec("[ -~\téűŐ\n]{0,12}", 1..3)), 0..3);
    (name(), "[ -~\téűŐ]{0,120}", params).prop_map(|(name, value, params)| {
        let mut property = Property::new(&name, value);
        for (param, values) in params {
            property = property.with_param_values(&param, values);
        }
        property
    })
}

fn component() -> impl Strategy<Value = Component> {
    let leaf = (name(), prop::collection::vec(property(), 0..4))
        .prop_map(|(name, properties)| properties.into_iter().fold(Component::new(&name), Component::with_property));
    leaf.prop_recursive(3, 12, 3, |inner| {
        (name(), prop::collection::vec(property(), 0..3), prop::collection::vec(inner, 0..3)).prop_map(|(name, properties, components)| {
            let component = properties.into_iter().fold(Component::new(&name), Component::with_property);
            components.into_iter().fold(component, Component::with_component)
        })
    })
}

proptest! {
    #[test]
    fn what_is_written_reads_back_the_same(component in component()) {
        let text = component.write().unwrap();
        prop_assert!(text.split("\r\n").all(|line| line.len() <= 75));
        prop_assert_eq!(parse(text.as_bytes()).unwrap(), component);
    }

    #[test]
    fn any_bytes_read_without_panicking(bytes in prop::collection::vec(any::<u8>(), 0..512)) {
        let _ = parse(&bytes);
    }
}
