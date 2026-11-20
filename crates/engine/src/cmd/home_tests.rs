// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `greeting.set`: the owner's, kept by the session, junk refused.

use serde_json::json;

use crate::cmd::act_tests::session;
use crate::greet::GreetBy;
use crate::{AgentClass, Caller, Registry};

#[test]
fn how_home_greets_is_the_owner_s_and_the_session_keeps_it() {
    let registry = Registry::builtin().unwrap();
    assert_eq!(registry.lookup("greeting.set").map(|(_, class)| class), Some(AgentClass::Owner));
    let mut session = session();
    assert_eq!(session.greet_by(), GreetBy::First, "the first word by default");
    assert_eq!(session.run_as(Caller::Agent, "greeting.set", &json!({ "greeting": "full" })).unwrap_err().code(), "not_allowed");
    assert_eq!(session.greet_by(), GreetBy::First);
    for (name, by) in [("last", GreetBy::Last), ("full", GreetBy::Full), ("none", GreetBy::None), ("first", GreetBy::First)] {
        assert_eq!(session.run("greeting.set", &json!({ "greeting": name })).unwrap(), json!({ "greeting": name }));
        assert_eq!(session.greet_by(), by, "{name}");
    }
    for junk in
        [json!({ "greeting": "middle" }), json!({ "greeting": "Last" }), json!({ "greeting": "last\u{0}" }), json!({ "greeting": 2 }), json!({})]
    {
        assert!(session.run("greeting.set", &junk).is_err(), "{junk}");
        assert_eq!(session.greet_by(), GreetBy::First, "a refused value changes nothing");
    }
    session.set_greet_by(GreetBy::None);
    assert_eq!(session.greet_by(), GreetBy::None, "the front end hands back what it kept");
}
