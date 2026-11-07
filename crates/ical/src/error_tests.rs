// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{IcalError, MAX_NAME_IN_ERROR};

#[test]
fn every_error_says_what_went_wrong() {
    let cases = [
        (IcalError::TooLarge { limit: 7 }, "the calendar is longer than 7 bytes"),
        (IcalError::TooManyItems { limit: 7 }, "the calendar has more than 7 properties and components"),
        (IcalError::TooDeep { limit: 7 }, "components are nested deeper than 7"),
        (IcalError::BadLine { line: 3 }, "content line 3 cannot be read"),
        (IcalError::Unbalanced { line: 3 }, "content line 3 ends no open component"),
        (IcalError::Unterminated, "a component is not ended"),
        (IcalError::NotACalendar, "the input holds no single calendar"),
        (IcalError::bad_value("DTSTART"), "the value of DTSTART cannot be read"),
        (IcalError::Missing { component: "VEVENT", property: "UID" }, "VEVENT has no UID"),
        (IcalError::bad_name("A B"), "\"A B\" is no property or component name"),
    ];
    for (error, text) in cases {
        assert_eq!(error.to_string(), text);
    }
}

#[test]
fn names_in_errors_are_capped_and_clean() {
    let long = format!("X\u{7}\n{}", "Y".repeat(500));
    let IcalError::BadValue { property } = IcalError::bad_value(&long) else { panic!("not a bad value") };
    assert_eq!(property.chars().count(), MAX_NAME_IN_ERROR);
    assert!(!property.chars().any(char::is_control));
    let error: Box<dyn std::error::Error> = Box::new(IcalError::Unterminated);
    assert!(error.source().is_none());
}
