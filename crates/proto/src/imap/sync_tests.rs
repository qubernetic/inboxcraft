// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use inboxcraft_model::{ModSeq, Uid, UidSet};

use super::{Extension, modseq, uid_set, vanished};
use crate::imap::value::{Piece, values};

fn set(ranges: &[(u32, u32)]) -> UidSet {
    ranges.iter().filter_map(|(a, b)| Some((Uid::new(*a)?, Uid::new(*b)?))).collect()
}

fn read(line: &str) -> Option<(UidSet, bool)> {
    vanished(values(vec![Piece::Text(line.as_bytes().to_vec())]).unwrap_or_default())
}

#[test]
fn extensions_are_named_as_imap_names_them() {
    assert_eq!(Extension::CondStore.name(), "CONDSTORE");
    assert_eq!(Extension::QResync.name(), "QRESYNC");
}

#[test]
fn a_modseq_is_digits_within_u64_and_not_zero() {
    assert_eq!(modseq("90060115205545359"), ModSeq::new(90_060_115_205_545_359));
    assert_eq!(modseq("18446744073709551615"), ModSeq::new(u64::MAX));
    for bad in ["", "0", "+5", "-1", " 5", "5a", "18446744073709551616"] {
        assert_eq!(modseq(bad), None, "{bad:?}");
    }
}

#[test]
fn a_uid_set_as_the_server_writes_one() {
    assert_eq!(uid_set("41,43:116,120:118,7"), Some(set(&[(7, 7), (41, 41), (43, 116), (118, 120)])));
    assert_eq!(uid_set("4294967295"), Some(set(&[(u32::MAX, u32::MAX)])));
    for bad in ["", "0", "1:*", "*", "1,", ",1", "1::2", "1:2:3", "a", "1 2", "4294967296", "+1", "1:0"] {
        assert_eq!(uid_set(bad), None, "{bad:?}");
    }
}

#[test]
fn vanished_now_and_earlier() {
    assert_eq!(read("VANISHED 405,407"), Some((set(&[(405, 405), (407, 407)]), false)));
    assert_eq!(read("vanished (earlier) 300:310"), Some((set(&[(300, 310)]), true)));
    for bad in ["VANISHED", "VANISHED (EARLIER)", "VANISHED (LATER) 1", "VANISHED () 1", "VANISHED 1 2", "VANISHED \"1\"", "VANISHED 0", "EXPUNGE 1"]
    {
        assert_eq!(read(bad), None, "{bad}");
    }
}
