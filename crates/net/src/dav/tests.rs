// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use inboxcraft_dav::DavError;
use inboxcraft_model::ErrorKind;

use super::failure;

#[test]
fn each_dav_error_is_told_by_its_cause() {
    let cases = [
        (DavError::NotEncrypted, ErrorKind::NotEncrypted),
        (DavError::OtherOrigin { url: "https://other.example.org/".to_owned() }, ErrorKind::OtherServer),
        (DavError::Unauthorized, ErrorKind::LoginRefused),
        (DavError::Refused { status: 404 }, ErrorKind::NoCalendarServer),
        (DavError::Refused { status: 410 }, ErrorKind::NoCalendarServer),
        (DavError::BadUrl("a scheme other than https or http"), ErrorKind::NoCalendarServer),
        (DavError::Missing("calendar home set"), ErrorKind::NoCalendarServer),
        (DavError::Missing("redirect location"), ErrorKind::Protocol),
        (DavError::Refused { status: 500 }, ErrorKind::CommandRefused),
        (DavError::BadArgument("an entity tag that cannot be sent"), ErrorKind::CommandRefused),
        (DavError::TooLarge { what: "body", limit: 1 }, ErrorKind::TooLarge),
        (DavError::TooManyRedirects, ErrorKind::Protocol),
        (DavError::BadResponse("no status line"), ErrorKind::Protocol),
        (DavError::BadXml("not well-formed"), ErrorKind::Protocol),
        (DavError::Finished, ErrorKind::Protocol),
    ];
    for (error, kind) in cases {
        assert_eq!(failure(&error).kind(), kind, "{error:?}");
    }
}
