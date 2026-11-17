// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::path::{Path, PathBuf};

use super::{MAX_CONFIG, MAX_CONFIGS, MAX_INCLUDE_DEPTH, Places, nesting_within, read};

/// A fontconfig file holding `body`.
fn conf(body: &str) -> String {
    format!("<?xml version=\"1.0\"?>\n<!DOCTYPE fontconfig SYSTEM \"urn:fontconfig:fonts.dtd\">\n<fontconfig>\n{body}\n</fontconfig>\n")
}

fn write(path: &Path, text: &str) {
    std::fs::write(path, text).unwrap();
}

fn places(root: &Path) -> Places {
    Places { home: Some(root.join("home")), config_home: Some(root.join("config")), data_home: Some(root.join("data")), cwd: Some(root.join("cwd")) }
}

#[test]
fn dirs_remap_dirs_and_the_sans_serif_are_read() {
    let root = tempfile::tempdir().unwrap();
    let top = root.path().join("fonts.conf");
    write(
        &top,
        &conf(
            r#"<dir>/opt/fonts</dir>
            <dir prefix="xdg">fonts</dir>
            <dir prefix="relative">near</dir>
            <dir prefix="cwd">here</dir>
            <dir>~/.fonts</dir>
            <remap-dir as-path="/usr/share/fonts">/run/host/fonts</remap-dir>
            <dir>  </dir>
            <match target="font"><edit name="rgba"><const>rgb</const></edit></match>
            <alias><family>serif</family><prefer><family>Some Serif</family></prefer></alias>
            <alias><family>sans-serif</family><prefer><family>Owner Sans</family><family>Other</family></prefer></alias>"#,
        ),
    );
    let found = read(&[top], &places(root.path()));
    let r = root.path().canonicalize().unwrap();
    assert_eq!(
        found.folders,
        [
            PathBuf::from("/opt/fonts"),
            root.path().join("data/fonts"),
            r.join("near"),
            root.path().join("cwd/here"),
            root.path().join("home/.fonts"),
            PathBuf::from("/run/host/fonts"),
        ]
    );
    assert_eq!(found.sans.as_deref(), Some("Owner Sans"));
    assert!(found.skipped.is_empty());
}

#[test]
fn includes_are_read_and_a_folder_of_them_in_name_order() {
    let root = tempfile::tempdir().unwrap();
    let conf_d = root.path().join("conf.d");
    std::fs::create_dir(&conf_d).unwrap();
    write(&conf_d.join("20-b.conf"), &conf("<dir>/b</dir>"));
    write(&conf_d.join("10-a.conf"), &conf("<dir>/a</dir>"));
    write(&conf_d.join("README"), &conf("<dir>/not-read</dir>"));
    write(&conf_d.join("x-c.conf"), &conf("<dir>/not-read</dir>"));
    std::fs::create_dir_all(root.path().join("config/fontconfig")).unwrap();
    write(&root.path().join("config/fontconfig/fonts.conf"), &conf("<dir>/user</dir>"));
    let top = root.path().join("fonts.conf");
    write(
        &top,
        &conf(
            r#"<include ignore_missing="yes">conf.d</include>
            <include ignore_missing="yes" prefix="xdg">fontconfig/fonts.conf</include>
            <include ignore_missing="yes">missing.conf</include>"#,
        ),
    );
    let found = read(&[top], &places(root.path()));
    assert_eq!(found.folders, [PathBuf::from("/a"), PathBuf::from("/b"), PathBuf::from("/user")]);
}

#[test]
fn an_include_loop_ends() {
    let root = tempfile::tempdir().unwrap();
    write(&root.path().join("a.conf"), &conf("<dir>/a</dir><include>b.conf</include><include>a.conf</include>"));
    write(&root.path().join("b.conf"), &conf("<dir>/b</dir><include>a.conf</include>"));
    let found = read(&[root.path().join("a.conf")], &places(root.path()));
    assert_eq!(found.folders, [PathBuf::from("/a"), PathBuf::from("/b")]);
}

#[test]
fn includes_go_at_most_eight_deep() {
    let root = tempfile::tempdir().unwrap();
    for at in 0..20 {
        write(&root.path().join(format!("{at}.conf")), &conf(&format!("<dir>/{at}</dir><include>{}.conf</include>", at + 1)));
    }
    let found = read(&[root.path().join("0.conf")], &places(root.path()));
    assert_eq!(found.folders.len(), MAX_INCLUDE_DEPTH + 1, "{:?}", found.folders);
    assert_eq!(found.skipped.len(), 1);
}

#[cfg(unix)]
#[test]
fn a_fifo_a_device_and_a_folder_in_conf_d_are_skipped() {
    let root = tempfile::tempdir().unwrap();
    let conf_d = root.path().join("conf.d");
    std::fs::create_dir(&conf_d).unwrap();
    let fifo = conf_d.join("10-fifo.conf");
    let made = std::process::Command::new("mkfifo").arg(&fifo).status();
    assert!(made.is_ok_and(|status| status.success()), "mkfifo is there");
    std::os::unix::fs::symlink("/dev/zero", conf_d.join("20-zero.conf")).unwrap();
    std::fs::create_dir(conf_d.join("30-folder.conf")).unwrap();
    write(&conf_d.join("40-good.conf"), &conf("<dir>/good</dir>"));
    let top = root.path().join("fonts.conf");
    write(&top, &conf("<dir>/top</dir><include>conf.d</include><include>/dev/zero</include>"));
    let found = read(&[top, PathBuf::from("/dev/zero"), fifo.clone()], &places(root.path()));
    assert_eq!(found.folders, [PathBuf::from("/top"), PathBuf::from("/good")]);
    for skipped in [fifo, conf_d.join("20-zero.conf"), conf_d.join("30-folder.conf"), PathBuf::from("/dev/zero")] {
        assert!(found.skipped.contains(&skipped), "{skipped:?} in {:?}", found.skipped);
    }
}

#[test]
fn a_file_over_one_mib_is_skipped() {
    let root = tempfile::tempdir().unwrap();
    let padding = format!("<!--{}-->", "x".repeat(usize::try_from(MAX_CONFIG).unwrap()));
    write(&root.path().join("big.conf"), &conf(&format!("<dir>/big</dir>{padding}")));
    let top = root.path().join("fonts.conf");
    write(&top, &conf("<dir>/top</dir><include>big.conf</include>"));
    let found = read(&[top], &places(root.path()));
    assert_eq!(found.folders, [PathBuf::from("/top")]);
    assert_eq!(found.skipped.len(), 1);
}

#[test]
fn xml_nested_ten_thousand_deep_is_skipped() {
    let root = tempfile::tempdir().unwrap();
    let deep = format!("<dir>/deep</dir><match>{}{}</match>", "<plus>".repeat(10_000), "</plus>".repeat(10_000));
    write(&root.path().join("deep.conf"), &conf(&deep));
    write(&root.path().join("junk.conf"), "<fontconfig><dir>/junk</dir>");
    write(&root.path().join("other.conf"), "<other><dir>/other</dir></other>");
    std::fs::write(root.path().join("binary.conf"), [0xff, 0xfe, 0x00, 0x3c]).unwrap();
    let top = root.path().join("fonts.conf");
    write(
        &top,
        &conf("<dir>/top</dir><include>deep.conf</include><include>junk.conf</include><include>other.conf</include><include>binary.conf</include>"),
    );
    let found = read(&[top], &places(root.path()));
    assert_eq!(found.folders, [PathBuf::from("/top")]);
    assert_eq!(found.skipped.len(), 3, "deep, junk and binary: {:?}", found.skipped);
}

#[test]
fn three_hundred_include_files_stop_at_the_cap() {
    let root = tempfile::tempdir().unwrap();
    let conf_d = root.path().join("conf.d");
    std::fs::create_dir(&conf_d).unwrap();
    for at in 0..300 {
        write(&conf_d.join(format!("{at:03}.conf")), &conf(&format!("<dir>/{at}</dir>")));
    }
    let top = root.path().join("fonts.conf");
    write(&top, &conf("<dir>/top</dir><include>conf.d</include>"));
    let found = read(&[top], &places(root.path()));
    assert_eq!(found.folders.first(), Some(&PathBuf::from("/top")));
    assert_eq!(found.folders.len(), MAX_CONFIGS, "the top file and {} included", MAX_CONFIGS - 1);
    assert!(!found.skipped.is_empty());
}

#[test]
fn nothing_there_gives_nothing() {
    let root = tempfile::tempdir().unwrap();
    let found = read(&[root.path().join("missing.conf")], &Places::default());
    assert!(found.folders.is_empty() && found.sans.is_none());
    let top = root.path().join("fonts.conf");
    write(&top, &conf(r#"<dir prefix="xdg">fonts</dir><dir>~/fonts</dir><include prefix="xdg">x.conf</include>"#));
    assert!(read(&[top], &Places::default()).folders.is_empty(), "no home, no XDG folders");
}

#[test]
fn the_nesting_is_counted_before_the_parser_sees_it() {
    assert!(nesting_within(&conf("<dir>/a</dir><alias><family>x</family></alias>"), 3));
    assert!(!nesting_within(&conf("<a><b><c><d/></c></b></a>"), 3));
    assert!(nesting_within(&conf("<a><b/><b/><b/></a>"), 2), "a self-closed element does not nest");
    assert!(!nesting_within("<a x=\"/>\"><b y='>'><c>", 2), "quotes are honoured");
    assert!(nesting_within("<a><!-- <b><c><d> --><![CDATA[<b><c><d>]]><?pi <b> ?></a>", 1), "comments, CDATA and instructions hold no elements");
    assert!(nesting_within("<!DOCTYPE fontconfig SYSTEM \"urn:fontconfig:fonts.dtd\"><a/>", 1));
    assert!(!nesting_within("<!DOCTYPE a [ <!ELEMENT a ANY> ]><a/>", 1), "an internal DTD is refused");
    for cut in ["<a", "<a x=\"", "<!-- open", "<![CDATA[ open", "<? open", "<!DOCTYPE a ["] {
        assert!(!nesting_within(cut, 64), "{cut}");
    }
    assert!(nesting_within("", 0) && nesting_within("no markup", 0));
}

#[test]
fn an_internal_dtd_is_refused() {
    let root = tempfile::tempdir().unwrap();
    let top = root.path().join("fonts.conf");
    write(&top, "<?xml version=\"1.0\"?>\n<!DOCTYPE fontconfig [ <!ENTITY d \"<dir>/hidden</dir>\"> ]>\n<fontconfig>&d;<dir>/b</dir></fontconfig>");
    let found = read(&[top], &places(root.path()));
    assert!(found.folders.is_empty(), "entities could hide markup: {:?}", found.folders);
    assert_eq!(found.skipped.len(), 1);
}
