// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{AccountId, AlertId, CalendarEventId, CalendarId, DraftId, FolderId, JobId, MessageId, ModSeq, ScheduleId, ThreadId, Uid, UidValidity};

#[test]
fn our_ids_show_their_kind_in_logs() {
    assert_eq!(AccountId::new(1).to_string(), "a1");
    assert_eq!(FolderId::new(22).to_string(), "f22");
    assert_eq!(MessageId::new(333).to_string(), "m333");
    assert_eq!(ThreadId::new(4).to_string(), "t4");
    assert_eq!(JobId::new(5).to_string(), "j5");
    assert_eq!(DraftId::new(6).to_string(), "d6");
    assert_eq!(ScheduleId::new(7).to_string(), "s7");
    assert_eq!(CalendarId::new(8).to_string(), "c8");
    assert_eq!(CalendarEventId::new(9).to_string(), "e9");
    assert_eq!(AlertId::new(10).to_string(), "l10");
    assert_eq!(MessageId::new(7).get(), 7);
}

#[test]
fn our_ids_serialise_as_plain_numbers() {
    assert_eq!(serde_json::to_string(&FolderId::new(9)).unwrap(), "9");
    assert_eq!(serde_json::from_str::<FolderId>("9").unwrap(), FolderId::new(9));
    assert!(serde_json::from_str::<FolderId>("-1").is_err());
}

#[test]
fn uids_are_never_zero() {
    assert_eq!(Uid::new(0), None);
    assert_eq!(Uid::new(1).map(Uid::get), Some(1));
    assert_eq!(Uid::new(u32::MAX).map(Uid::get), Some(u32::MAX));
    assert_eq!(UidValidity::new(0), None);
    assert_eq!(UidValidity::new(17).map(UidValidity::get), Some(17));
    assert_eq!(ModSeq::new(0), None);
    assert_eq!(ModSeq::new(u64::MAX).map(ModSeq::get), Some(u64::MAX));
}

#[test]
fn a_zero_uid_does_not_deserialise() {
    assert!(serde_json::from_str::<Uid>("0").is_err());
    assert!(serde_json::from_str::<UidValidity>("0").is_err());
    assert!(serde_json::from_str::<ModSeq>("0").is_err());
    assert_eq!(serde_json::from_str::<Uid>("12").ok(), Uid::new(12));
    assert_eq!(serde_json::to_string(&Uid::new(12)).unwrap(), "12");
}

#[test]
fn uids_order_by_number() {
    assert!(Uid::new(2) < Uid::new(10));
    assert_eq!(Uid::new(3).map(|uid| uid.to_string()).as_deref(), Some("3"));
}
