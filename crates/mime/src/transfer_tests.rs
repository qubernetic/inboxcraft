// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::borrow::Cow;

use super::{MAX_NAME_IN_ERROR, TransferEncoding, base64_decode, qp_decode};
use crate::MimeError;

#[test]
fn encodings_are_named_without_case_and_comments() {
    assert_eq!(TransferEncoding::parse("BASE64"), TransferEncoding::Base64);
    assert_eq!(TransferEncoding::parse(" Quoted-Printable (comment)"), TransferEncoding::QuotedPrintable);
    assert_eq!(TransferEncoding::parse("7bit"), TransferEncoding::SevenBit);
    assert_eq!(TransferEncoding::parse("8BIT"), TransferEncoding::EightBit);
    assert_eq!(TransferEncoding::parse("binary"), TransferEncoding::Binary);
    assert_eq!(TransferEncoding::parse(""), TransferEncoding::SevenBit);
    assert_eq!(TransferEncoding::default(), TransferEncoding::SevenBit);
    assert_eq!(TransferEncoding::parse("X-UUEncode"), TransferEncoding::Unknown("x-uuencode".to_owned()));
}

#[test]
fn an_unknown_name_is_capped_and_cleaned() {
    let long = format!("x-{}", "a".repeat(200));
    let TransferEncoding::Unknown(name) = TransferEncoding::parse(&long) else { panic!("not unknown") };
    assert_eq!(name.chars().count(), MAX_NAME_IN_ERROR);
    let TransferEncoding::Unknown(name) = TransferEncoding::parse("x-a\u{1}b") else { panic!("not unknown") };
    assert_eq!(name, "x-a\u{fffd}b");
}

#[test]
fn the_identity_encodings_borrow_the_body() {
    for encoding in [TransferEncoding::SevenBit, TransferEncoding::EightBit, TransferEncoding::Binary] {
        assert_eq!(encoding.decode(b"as \xff is"), Ok(Cow::Borrowed(&b"as \xff is"[..])));
    }
}

#[test]
fn an_unknown_encoding_cannot_be_decoded() {
    assert_eq!(TransferEncoding::parse("x-uue").decode(b"x"), Err(MimeError::UnknownEncoding("x-uue".to_owned())));
}

#[test]
fn base64_decodes_with_and_without_padding() {
    assert_eq!(base64_decode(b"SGVsbG8="), Ok(b"Hello".to_vec()));
    assert_eq!(base64_decode(b"SGVsbG8"), Ok(b"Hello".to_vec()));
    assert_eq!(base64_decode(b"SGVsbA=="), Ok(b"Hell".to_vec()));
    assert_eq!(base64_decode(b"SGVs"), Ok(b"Hel".to_vec()));
    assert_eq!(base64_decode(b""), Ok(Vec::new()));
    assert_eq!(base64_decode(b"="), Ok(Vec::new()));
    assert_eq!(TransferEncoding::Base64.decode(b"+/+/"), Ok(Cow::Owned(vec![0xfb, 0xff, 0xbf])));
}

#[test]
fn base64_skips_line_breaks_and_joins_padded_chunks() {
    assert_eq!(base64_decode(b"SGVs\r\nbG8g\r\nd29y bGQ=\r\n\t"), Ok(b"Hello world".to_vec()));
    assert_eq!(base64_decode(b"SGk=SGk="), Ok(b"HiHi".to_vec()));
}

#[test]
fn broken_base64_is_an_error() {
    for bad in [&b"SGVs*bG8="[..], b"S", b"SGVsb", b"SG=V", b"SGVs\x00", b"SGVs\xff", b"SGVs-_"] {
        assert_eq!(base64_decode(bad), Err(MimeError::BadBase64), "{bad:?}");
    }
    assert_eq!(TransferEncoding::Base64.decode(b"!"), Err(MimeError::BadBase64));
}

#[test]
fn quoted_printable_decodes() {
    assert_eq!(qp_decode(b"caf=E9 =3D ok"), b"caf\xe9 = ok");
    assert_eq!(qp_decode(b"lower=e9"), b"lower\xe9");
    assert_eq!(qp_decode(b"soft=\r\nbreak=\nand= \t\r\nmore"), b"softbreakandmore");
    assert_eq!(qp_decode(b"keep\r\nhard\nbreaks"), b"keep\r\nhard\nbreaks");
    assert_eq!(qp_decode(b"pad   \r\nded\t\nend  "), b"pad\r\nded\nend");
    assert_eq!(qp_decode(b"a  b"), b"a  b");
    assert_eq!(TransferEncoding::QuotedPrintable.decode(b"=41"), Ok(Cow::Owned(b"A".to_vec())));
}

#[test]
fn a_stray_equals_sign_in_quoted_printable_stays_as_it_is() {
    // RFC 2045 section 6.7, note (1): a robust decoder keeps what it cannot read.
    assert_eq!(qp_decode(b"a=b"), b"a=b");
    assert_eq!(qp_decode(b"a=ZZ"), b"a=ZZ");
    assert_eq!(qp_decode(b"a=4"), b"a=4");
    assert_eq!(qp_decode(b"x=="), b"x=");
    assert_eq!(qp_decode(b"end="), b"end");
    assert_eq!(qp_decode(b"=\r"), b"");
}

#[test]
fn long_runs_decode_in_linear_time() {
    let spaces = vec![b' '; 300_000];
    assert_eq!(qp_decode(&spaces), b"");
    let mut padded = spaces.clone();
    padded.push(b'x');
    assert_eq!(qp_decode(&padded).len(), 300_001);
    assert_eq!(qp_decode(&vec![b'='; 300_000]).len(), 299_999);
    // Every `= ` but the last stays; the last is a soft line break at the end.
    assert_eq!(qp_decode(&b"= ".repeat(150_000)).len(), 299_998);
    assert_eq!(base64_decode(&vec![b'\n'; 300_000]), Ok(Vec::new()));
}
