// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::path::{Path, PathBuf};

use super::{MAX_CONFIG, MAX_CONFIGS, MAX_INCLUDE_DEPTH, MAX_LISTED, MAX_NESTING, MAX_NODES, Named, Places, depth_of, parse, plain, read};

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

/// Whether `text` gets past the count, measured the way `parse` does it: on the plain text.
fn within(text: &str, max: usize) -> bool {
    plain(text).and_then(|plain| depth_of(&plain)).is_some_and(|depth| depth <= max)
}

#[test]
fn the_nesting_is_counted_before_the_parser_sees_it() {
    assert!(within(&conf("<dir>/a</dir><alias><family>x</family></alias>"), 3));
    assert!(!within(&conf("<a><b><c><d/></c></b></a>"), 3));
    assert!(within(&conf("<a><b/><b/><b/></a>"), 3), "self-closed siblings are one level");
    assert!(!within(&conf("<a><b/></a>"), 2), "a self-closed element is a level");
    assert!(!within("<a x=\"/>\"><b y='>'><c>", 2), "quotes are honoured");
    assert!(within("<a><!-- <b><c><d> --><?pi <b> ?></a>", 1), "comments and instructions hold no elements");
    assert!(within("<!DOCTYPE fontconfig SYSTEM \"urn:fontconfig:fonts.dtd\"><a/>", 1));
    assert!(!within("<!DOCTYPE a [ <!ELEMENT a ANY> ]><a/>", 1), "an internal DTD is refused");
    for cut in ["<a", "<a x=\"", "<!-- open", "<![CDATA[ open", "<? open", "<!DOCTYPE a [", "<!ELEMENT a ANY>"] {
        assert!(!within(cut, 64), "{cut}");
    }
    assert!(within("", 0) && within("no markup", 0));
}

#[test]
fn the_count_and_the_parser_see_the_same_text() {
    assert_eq!(plain("<?xml version=\"1.0\"?><!DOCTYPE f SYSTEM \"x\"><f><!-- c --><a x=\"<!--\"/></f>").as_deref(), Some("<f><a x=\"<!--\"/></f>"));
    // A terminator counts only after its whole opener: `<!-->` and `<!--->` open a comment, they do not close one.
    assert_eq!(plain("<a><!--></a>--></a>").as_deref(), Some("<a></a>"));
    assert_eq!(plain("<a><!---></a>--></a>").as_deref(), Some("<a></a>"));
    assert_eq!(plain("<a><?></a>?></a>").as_deref(), Some("<a></a>"));
    assert_eq!(plain("<a><?pi?></a>").as_deref(), Some("<a></a>"));
    assert_eq!(plain("<!-->"), None, "left open");
    // A removed span leaves no trace in the text around it.
    let named = parse(b"<fontconfig><dir>/usr/<!--x-->share/<?pi?>fonts</dir></fontconfig>", None, &Places::default());
    assert_eq!(named, Some(vec![Named::Folder(PathBuf::from("/usr/share/fonts"))]));
    assert_eq!(plain("<?>"), None, "left open");
    // CDATA is refused whole, however it is cut.
    for cdata in ["<a><![CDATA[]]></a>", "<a><![CDATA[]></a>", "<a><![CDATA[]]]></a>", "<a><![CDATA[</a>]]>", "<![CDATA["] {
        assert_eq!(plain(cdata), None, "{cdata}");
    }
}

#[test]
fn a_comment_that_opens_like_a_close_cannot_hide_the_nesting() {
    // The re-review's input: each `<!-->` opens a comment that swallows 60 closes; a scan that took `<!-->` for a
    // whole comment would count them and let 10 200 levels through to the parser, which would overflow the stack.
    let mut text = "<fontconfig>".to_owned();
    for _ in 0..170 {
        text.push_str(&"<a>".repeat(60));
        text.push_str("<!-->");
        text.push_str(&"</a>".repeat(60));
        text.push_str("-->");
    }
    text.push_str(&"</a>".repeat(10_200));
    text.push_str("</fontconfig>");
    assert!(!within(&text, MAX_NESTING));
    assert_eq!(parse(text.as_bytes(), None, &Places::default()), None);
    let pi = text.replace("<!-->", "<?>").replace("-->", "?>");
    assert_eq!(parse(pi.as_bytes(), None, &Places::default()), None, "the same with an instruction");
    let cdata = text.replace("<!-->", "<![CDATA[]>").replace("-->", "]]>");
    assert_eq!(parse(cdata.as_bytes(), None, &Places::default()), None, "the same with CDATA");
}

#[test]
fn parse_takes_bytes_and_touches_no_file() {
    let places = places(Path::new("/p"));
    let named = parse(conf(r#"<dir>/a</dir><include>b.conf</include><alias><family>sans-serif</family><default><family>S</family></default></alias><remap-dir as-path="/x">/c</remap-dir>"#).as_bytes(), Some(Path::new("/etc/fonts")), &places);
    assert_eq!(
        named,
        Some(vec![
            Named::Folder(PathBuf::from("/a")),
            Named::Include(PathBuf::from("/etc/fonts/b.conf")),
            Named::Sans("S".to_owned()),
            Named::Folder(PathBuf::from("/c"))
        ])
    );
    assert_eq!(parse(b"\xff\xfe", None, &places), None, "not UTF-8");
    assert_eq!(parse(b"<other><dir>/a</dir></other>", None, &places), Some(Vec::new()), "not fontconfig's");
    let big = vec![b' '; usize::try_from(MAX_CONFIG).unwrap() + 1];
    assert_eq!(parse(&big, None, &places), None);
}

#[test]
fn a_folder_of_configuration_files_is_listed_for_at_most_4096_entries() {
    let root = tempfile::tempdir().unwrap();
    let conf_d = root.path().join("conf.d");
    std::fs::create_dir(&conf_d).unwrap();
    for at in 0..5_000 {
        std::fs::write(conf_d.join(format!("note-{at}")), "").unwrap();
    }
    let top = root.path().join("fonts.conf");
    write(&top, &conf("<dir>/top</dir><include>conf.d</include>"));
    let found = read(&[top], &places(root.path()));
    assert_eq!(found.listed, MAX_LISTED);
    assert_eq!(found.folders, [PathBuf::from("/top")]);
}

#[test]
fn a_folder_included_again_is_not_listed_again() {
    let root = tempfile::tempdir().unwrap();
    let conf_d = root.path().join("conf.d");
    std::fs::create_dir(&conf_d).unwrap();
    for at in 0..10 {
        std::fs::write(conf_d.join(format!("note-{at}")), "").unwrap();
    }
    write(&conf_d.join("10-a.conf"), &conf("<dir>/a</dir><include>.</include><include>../conf.d</include>"));
    let includes = "<include>conf.d</include>".repeat(1_000) + "<include>./conf.d/</include>";
    let top = root.path().join("fonts.conf");
    write(&top, &conf(&includes));
    let found = read(&[top], &places(root.path()));
    assert_eq!(found.listed, 11, "listed once");
    assert_eq!(found.folders, [PathBuf::from("/a")]);
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

proptest::proptest! {
    /// What the fuzz target checks, on stable Rust: any bytes, and any mix of the pieces that tripped the count.
    #[test]
    fn parse_never_panics(bytes in proptest::collection::vec(proptest::prelude::any::<u8>(), 0..512)) {
        let _ = parse(&bytes, Some(Path::new("/etc/fonts")), &Places::default());
    }

    /// The count and the parser agree on every document that parses: the depth counted on the plain text is the
    /// depth the parser builds from the text as written, comments, instructions and the document type included.
    #[test]
    fn the_count_is_the_parser_s_depth(steps in proptest::collection::vec(0_u8..16, 0..300), head in 0_u8..4) {
        let text = document(&steps, head);
        let options = roxmltree::ParsingOptions { allow_dtd: true, nodes_limit: MAX_NODES };
        let tree = roxmltree::Document::parse_with_options(&text, options);
        proptest::prop_assert!(tree.is_ok(), "the generated document parses: {}", text);
        if let Ok(tree) = tree {
            let real = tree
                .descendants()
                .filter(roxmltree::Node::is_element)
                .map(|node| node.ancestors().filter(roxmltree::Node::is_element).count())
                .max()
                .unwrap_or(0);
            let counted = plain(&text).and_then(|plain| depth_of(&plain));
            proptest::prop_assert_eq!(counted, Some(real), "{}", text);
        }
    }
}

/// A balanced document from `steps`: opens, closes (only while one is open), and pieces that hold no element of
/// their own, among them comments and instructions that hold what looks like closes and opens (`<!-->` and `<!--->`
/// forms included); `head` picks what comes before the root (nothing, the XML declaration, a document type, both).
fn document(steps: &[u8], head: u8) -> String {
    let heads = [
        "",
        "<?xml version=\"1.0\"?>",
        "<!DOCTYPE fontconfig SYSTEM \"urn:fontconfig:fonts.dtd\">",
        "<?xml version=\"1.0\"?><!DOCTYPE fontconfig SYSTEM \"x\">",
    ];
    let mut text = heads[usize::from(head % 4)].to_owned();
    text.push_str("<fontconfig>");
    let mut open: Vec<&str> = Vec::new();
    for step in steps {
        match step {
            0..=4 => {
                let (tag, name) =
                    [("<a>", "a"), ("<dir x=\"&lt;/a>\">", "dir"), ("<b y='&lt;!--'>", "b"), ("<a >", "a"), ("<c\n>", "c")][usize::from(*step)];
                text.push_str(tag);
                open.push(name);
            }
            5..=7 => {
                if let Some(name) = open.pop() {
                    text.push_str(&format!("</{name}>"));
                }
            }
            8 => text.push_str("<!--></a></a>-->"),
            9 => text.push_str("<!---> <b><c> -->"),
            10 => text.push_str("<!-- </a> <a> -->"),
            11 => text.push_str("<?pi </a><a>?>"),
            12 => text.push_str("<a/>"),
            13 => text.push_str("/usr/<!--x-->share"),
            14 => text.push_str("&lt;a&gt; text"),
            _ => text.push_str("<e k=\"v\"/>"),
        }
    }
    while let Some(name) = open.pop() {
        text.push_str(&format!("</{name}>"));
    }
    text.push_str("</fontconfig>");
    text
}
