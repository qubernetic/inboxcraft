// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Filling in a template never panics, whatever its braces, and every mark it remembers is in the draft.

use inboxcraft_model::{AccountId, Draft, DraftId, Template};
use proptest::prelude::*;

use super::{apply, open_blanks};
use crate::test_support::{NOW, account, at};

/// Text with many braces, question marks, brackets, line breaks and characters outside ASCII.
fn braced() -> impl Strategy<Value = String> {
    proptest::collection::vec(
        prop_oneof![
            Just("{".to_owned()),
            Just("}".to_owned()),
            Just("?".to_owned()),
            Just("[[".to_owned()),
            Just("]]".to_owned()),
            Just("\n".to_owned()),
            Just("name".to_owned()),
            Just("first_name".to_owned()),
            Just("my_name".to_owned()),
            "\\PC{0,4}"
        ],
        0..60,
    )
    .prop_map(|pieces| pieces.concat())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn a_template_fills_without_panic(subject in braced(), text in braced(), to in braced(), held in braced()) {
        let Some(template) = Template::new("t", &subject, &text) else { return Ok(()) };
        let mut draft = Draft { to: vec![to], text: held.clone(), ..Draft::new(DraftId::new(1), AccountId::new(1), at(NOW)) };
        apply(&template, &mut draft, &account(1, "anna@example.org"));
        prop_assert!(draft.text.ends_with(&held));
        for mark in &draft.blanks {
            prop_assert!(draft.subject.contains(mark.as_str()) || draft.text.contains(mark.as_str()), "{mark}");
        }
        prop_assert_eq!(open_blanks(&draft).len(), draft.blanks.len());
    }
}
