// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use inboxcraft_model::ModSeq;

use super::{Extension, modseq};

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
