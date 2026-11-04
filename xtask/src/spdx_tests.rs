// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::fs;
use std::path::Path;

use super::{Finding, check, fix, gate};
use crate::test_support::{Broken, context, run_in};
use crate::{Error, FAILED, OK, USAGE};

const LICENCE: &str = "MIT OR Apache-2.0";
const GOOD: &str = "// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors\n\
                    // SPDX-License-Identifier: MIT OR Apache-2.0\n\nfn main() {}\n";

#[test]
fn a_file_with_both_lines_passes() {
    assert_eq!(check(GOOD, LICENCE), None);
    let other_holder = "// SPDX-FileCopyrightText: 2026 Someone Else\n// SPDX-License-Identifier: MIT OR Apache-2.0\n";
    assert_eq!(check(other_holder, LICENCE), None);
    let crlf = GOOD.replace('\n', "\r\n");
    assert_eq!(check(&crlf, LICENCE), None);
}

#[test]
fn a_file_without_the_lines_fails() {
    assert_eq!(check("fn main() {}\n", LICENCE), Some(Finding::Missing));
    assert_eq!(check("", LICENCE), Some(Finding::Missing));
    let late = "//! Docs\n// SPDX-License-Identifier: MIT OR Apache-2.0\n";
    assert_eq!(check(late, LICENCE), Some(Finding::Missing));
}

#[test]
fn a_file_without_the_copyright_line_fails() {
    assert_eq!(check("// SPDX-License-Identifier: MIT OR Apache-2.0\nfn f() {}\n", LICENCE), Some(Finding::MissingCopyright));
}

#[test]
fn a_file_with_another_licence_fails() {
    let text = "// SPDX-FileCopyrightText: Someone\n// SPDX-License-Identifier: GPL-3.0-only\n";
    assert_eq!(check(text, LICENCE), Some(Finding::WrongLicence("GPL-3.0-only".to_owned())));
}

#[test]
fn fix_adds_both_lines_and_a_blank_line() {
    assert_eq!(fix("fn main() {}\n", LICENCE).unwrap(), GOOD);
    let docs = "\n//! Docs\n";
    let fixed = fix(docs, LICENCE).unwrap();
    assert!(fixed.ends_with("Apache-2.0\n\n//! Docs\n"), "{fixed:?}");
    assert_eq!(check(&fixed, LICENCE), None);
}

#[test]
fn fix_of_an_empty_file_gives_just_the_lines() {
    let fixed = fix("", LICENCE).unwrap();
    assert_eq!(fixed.lines().count(), 2);
    assert_eq!(check(&fixed, LICENCE), None);
}

#[test]
fn fix_keeps_existing_lines() {
    let holder_only = "// SPDX-FileCopyrightText: 2026 Someone Else\n\nfn f() {}\n";
    assert_eq!(
        fix(holder_only, LICENCE).unwrap(),
        "// SPDX-FileCopyrightText: 2026 Someone Else\n// SPDX-License-Identifier: MIT OR Apache-2.0\n\nfn f() {}\n"
    );
    let licence_only = "// SPDX-License-Identifier: MIT OR Apache-2.0";
    assert_eq!(
        fix(licence_only, LICENCE).unwrap(),
        "// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors\n// SPDX-License-Identifier: MIT OR Apache-2.0\n"
    );
}

#[test]
fn fix_leaves_good_files_and_other_licences_alone() {
    assert_eq!(fix(GOOD, LICENCE), None);
    assert_eq!(fix("// SPDX-FileCopyrightText: X\n// SPDX-License-Identifier: MIT\n", LICENCE), None);
}

#[test]
fn every_finding_says_what_to_do() {
    assert_eq!(Finding::Missing.describe(LICENCE), "no SPDX lines at the top");
    assert_eq!(Finding::MissingCopyright.describe(LICENCE), "no SPDX-FileCopyrightText line");
    assert_eq!(
        Finding::WrongLicence("MIT".to_owned()).describe(LICENCE),
        "licence is `MIT`, the workspace licence is `MIT OR Apache-2.0`; change it by hand"
    );
}

fn workspace(files: &[(&str, &str)]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("Cargo.toml"), "[workspace.package]\nlicense = \"MIT OR Apache-2.0\"\n").unwrap();
    for (rel, text) in files {
        let path = dir.path().join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
    dir
}

fn read(root: &Path, rel: &str) -> String {
    fs::read_to_string(root.join(rel)).unwrap()
}

#[test]
fn the_gate_fails_on_a_file_without_the_lines_and_fix_adds_them() {
    let dir = workspace(&[("good.rs", GOOD), ("src/bad.rs", "fn f() {}\n")]);
    let root = dir.path().to_str().unwrap();

    let (code, out, _) = run_in(context(dir.path().into()), &["spdx", "--root", root]);
    assert_eq!(code, FAILED, "{out}");
    assert!(out.contains("spdx: src/bad.rs: no SPDX lines at the top"), "{out}");
    assert!(out.contains("1 of 2 files need the SPDX lines"), "{out}");
    assert_eq!(read(dir.path(), "src/bad.rs"), "fn f() {}\n", "the check alone never writes");

    let (code, out, _) = run_in(context(dir.path().into()), &["spdx", "--fix", "--root", root]);
    assert_eq!(code, OK, "{out}");
    assert!(out.contains("spdx: fixed src/bad.rs"), "{out}");
    assert_eq!(check(&read(dir.path(), "src/bad.rs"), LICENCE), None);

    let (code, out, _) = run_in(context(dir.path().into()), &["spdx", "--root", root]);
    assert_eq!(code, OK);
    assert!(out.contains("spdx: 2 files checked, all have the SPDX lines"), "{out}");
}

#[test]
fn fix_cannot_change_another_licence() {
    let other = "// SPDX-FileCopyrightText: X\n// SPDX-License-Identifier: MIT\n";
    let dir = workspace(&[("other.rs", other)]);
    let mut out = Vec::new();
    assert!(!gate(dir.path(), true, &mut out).unwrap());
    assert!(String::from_utf8_lossy(&out).contains("spdx: other.rs: licence is `MIT`"));
    assert_eq!(read(dir.path(), "other.rs"), other);
}

#[test]
fn fix_is_only_an_option_of_spdx() {
    let (code, _, err) = run_in(context(".".into()), &["help", "--fix"]);
    assert_eq!(code, USAGE);
    assert!(err.contains("`--fix` only goes with `spdx`"), "{err}");
}

#[test]
fn a_workspace_without_a_manifest_is_a_setup_error() {
    let dir = tempfile::tempdir().unwrap();
    let (code, _, err) = run_in(context(dir.path().into()), &["spdx"]);
    assert_eq!(code, USAGE);
    assert!(err.contains("Cargo.toml"), "{err}");
}

#[test]
fn an_unreadable_file_is_an_io_error() {
    let dir = workspace(&[("bad.rs", "")]);
    fs::write(dir.path().join("bad.rs"), [0xff, 0xfe]).unwrap();
    assert!(matches!(gate(dir.path(), false, &mut Vec::new()), Err(Error::Io { .. })));
}

#[test]
fn a_file_that_cannot_be_written_is_an_io_error() {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let dir = workspace(&[("ro/bad.rs", "fn f() {}\n")]);
        let file = dir.path().join("ro/bad.rs");
        fs::set_permissions(&file, fs::Permissions::from_mode(0o444)).unwrap();
        // Root writes read-only files anyway; only an unprivileged run can see the error.
        let privileged = fs::OpenOptions::new().write(true).open(&file).is_ok();
        let result = gate(dir.path(), true, &mut Vec::new());
        fs::set_permissions(&file, fs::Permissions::from_mode(0o644)).unwrap();
        if !privileged {
            assert!(matches!(result, Err(Error::Io { .. })), "{result:?}");
        }
    }
}

#[test]
fn a_broken_report_stops_the_gate() {
    let dir = workspace(&[("bad.rs", "fn f() {}\n")]);
    assert!(matches!(gate(dir.path(), false, &mut Broken), Err(Error::Output(_))));
}

#[test]
fn this_workspace_passes() {
    let mut out = Vec::new();
    assert!(gate(&crate::default_root(), false, &mut out).unwrap(), "{}", String::from_utf8_lossy(&out));
}
