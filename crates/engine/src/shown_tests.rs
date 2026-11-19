// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use serde_json::json;

use super::{MAX_PAGE, SETTINGS_PAGES, page};

#[test]
fn a_page_is_one_of_the_nine_exactly_and_no_longer_than_the_cap() {
    for name in SETTINGS_PAGES {
        assert_eq!(page(Some(&json!(name))), Some(name));
        assert!(name.len() <= MAX_PAGE, "{name} fits under the cap");
    }
    for junk in [json!(null), json!(1), json!(["accounts"]), json!({ "page": "accounts" }), json!("Accounts"), json!("accounts\u{0}"), json!("ai")] {
        assert_eq!(page(Some(&junk)), None, "{junk}");
    }
    assert_eq!(page(None), None);
}
