// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::fs;
use std::path::Path;

use super::{gate, inline_test_modules, is_test_file};
use crate::test_support::{Broken, context, run_in};
use crate::{Error, FAILED, OK};

#[test]
fn an_inline_tests_module_is_found() {
    let text = "pub fn f() {}\n\n#[cfg(test)]\nmod tests {\n    #[test]\n    fn t() {}\n}\n";
    assert_eq!(inline_test_modules(text), [(4, "tests".to_owned())]);
}

#[test]
fn every_spelling_of_an_inline_test_module_is_found() {
    for (text, name) in [
        ("mod tests {}", "tests"),
        ("mod tests{", "tests"),
        ("pub mod tests {", "tests"),
        ("pub(crate) mod tests {", "tests"),
        ("pub(in crate::a) mod tests {", "tests"),
        ("  mod parser_tests {", "parser_tests"),
        ("#[cfg(test)] mod checks {", "checks"),
        ("#[cfg(test)]\n#[allow(dead_code)]\nmod checks {", "checks"),
        ("#[cfg(all(test, unix))]\nmod checks {", "checks"),
        ("#[cfg(test)]\n\nmod checks {", "checks"),
    ] {
        let found = inline_test_modules(text);
        assert_eq!(found.len(), 1, "{text:?}");
        assert_eq!(found.first().map(|(_, n)| n.as_str()), Some(name), "{text:?}");
    }
}

#[test]
fn test_modules_in_their_own_files_and_ordinary_modules_pass() {
    for text in [
        "#[cfg(test)]\nmod tests;\n",
        "#[cfg(test)]\n#[path = \"cli_tests.rs\"]\nmod tests;\n",
        "mod parser {\n}\n",
        "pub mod tests_helpers {\n}\n",
        "mod testsuite {\n}\n",
        "// mod tests {\n",
        "/// mod tests {\n",
        "let s = \"mod tests {\";\n",
        "#[cfg(test)]\nuse std::fs;\nmod parser {\n}\n",
        "pub(crate\n",
        "pub fn f() {}\n",
        "mod {\n",
        "#[cfg(test)\n",
    ] {
        assert_eq!(inline_test_modules(text), [], "{text:?}");
    }
}

#[test]
fn test_files_are_exempt() {
    for rel in ["src/tests.rs", "src/cli_tests.rs", "tests/cli.rs", "crates/a/tests/it/main.rs"] {
        assert!(is_test_file(Path::new(rel)), "{rel}");
    }
    for rel in ["src/lib.rs", "src/testsuite.rs", "src/latests.rs", "src/test_support.rs"] {
        assert!(!is_test_file(Path::new(rel)), "{rel}");
    }
}

fn workspace(files: &[(&str, &str)]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    for (rel, text) in files {
        let path = dir.path().join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
    dir
}

#[test]
fn the_gate_fails_on_a_fixture_with_an_inline_mod_tests() {
    let dir = workspace(&[
        ("src/lib.rs", "pub fn f() {}\n\n#[cfg(test)]\nmod tests {\n}\n"),
        ("src/ok.rs", "#[cfg(test)]\nmod tests;\n"),
        ("src/tests.rs", "mod tests {}\n"),
    ]);
    let (code, out, _) = run_in(context(dir.path().into()), &["test-files"]);
    assert_eq!(code, FAILED, "{out}");
    assert!(out.contains("test-files: src/lib.rs:4: inline test module `tests`"), "{out}");
    assert!(!out.contains("src/tests.rs"), "{out}");
    assert!(out.contains("test-files: 1 inline test module in 3 files"), "{out}");
}

#[test]
fn the_gate_passes_without_inline_test_modules() {
    let dir = workspace(&[("src/lib.rs", "#[cfg(test)]\nmod tests;\n")]);
    let (code, out, _) = run_in(context(dir.path().into()), &["test-files", "--root", dir.path().to_str().unwrap()]);
    assert_eq!(code, OK, "{out}");
    assert!(out.contains("test-files: 1 files checked, no inline test modules"), "{out}");
}

#[test]
fn an_unreadable_file_is_an_io_error() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("bad.rs"), [0xff]).unwrap();
    assert!(matches!(gate(dir.path(), &mut Vec::new()), Err(Error::Io { .. })));
}

#[test]
fn a_broken_report_stops_the_gate() {
    let dir = workspace(&[("lib.rs", "mod tests {}\n")]);
    assert!(matches!(gate(dir.path(), &mut Broken), Err(Error::Output(_))));
    let dir = workspace(&[("lib.rs", "")]);
    assert!(matches!(gate(dir.path(), &mut Broken), Err(Error::Output(_))));
}

#[test]
fn this_workspace_passes() {
    let mut out = Vec::new();
    assert!(gate(&crate::default_root(), &mut out).unwrap(), "{}", String::from_utf8_lossy(&out));
}
