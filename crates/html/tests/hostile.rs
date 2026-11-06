// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The hostile fixture set: scripts, event attributes, CSS tricks, nested tables, forms, links and
//! broken markup. In every fixture, what must never reach the owner says `EVIL` and what must be
//! shown says `SHOWN`; each gives display blocks without a panic, with or without consent.

use inboxcraft_html::{Block, Document, LinkTarget, MAX_TABLE_DEPTH, Options, RemoteContent, display};

const FIXTURES: [(&str, &str); 7] = [
    ("scripts", include_str!("hostile/scripts.html")),
    ("events", include_str!("hostile/events.html")),
    ("css", include_str!("hostile/css.html")),
    ("tables", include_str!("hostile/tables.html")),
    ("forms", include_str!("hostile/forms.html")),
    ("links", include_str!("hostile/links.html")),
    ("broken", include_str!("hostile/broken.html")),
];

/// Every text and picture text in the blocks, table cells included.
fn shown(blocks: &[Block], out: &mut String) {
    for block in blocks {
        match block {
            Block::Text { .. } => out.push_str(&block.text()),
            Block::Image { image, .. } => out.push_str(&image.alt),
            Block::Table { rows, .. } => rows.iter().flatten().for_each(|cell| shown(&cell.blocks, out)),
            Block::Rule { .. } => {}
        }
        out.push('\n');
    }
}

fn both(html: &str) -> [Document; 2] {
    [display(html, &Options::default()), display(html, &Options { remote_content: RemoteContent::Allowed })]
}

#[test]
fn nothing_marked_evil_survives_and_everything_marked_shown_does() {
    for (name, html) in FIXTURES {
        for doc in both(html) {
            let all = format!("{doc:?}");
            assert!(!all.contains("EVIL"), "{name}: {all}");
            let mut text = String::new();
            shown(&doc.blocks, &mut text);
            assert_eq!(text.matches("SHOWN").count(), html.matches("SHOWN").count(), "{name}: {text}");
            assert!(!doc.truncated, "{name}");
        }
    }
}

#[test]
fn hostile_link_targets_are_refused_and_phishing_is_flagged() {
    let doc = display(include_str!("hostile/links.html"), &Options::default());
    assert_eq!(doc.links.len(), 13);
    for link in &doc.links[..6] {
        assert_eq!(link.target, LinkTarget::Refused, "{}", link.text);
    }
    assert!(doc.links[6..12].iter().all(|l| l.mismatch));
    assert!(!doc.links[12].mismatch);
    assert!(matches!(&doc.links[6].target, LinkTarget::Web { host, .. } if host == "evil.example"));
}

#[test]
fn nested_tables_stop_at_the_cap() {
    let doc = display(include_str!("hostile/tables.html"), &Options::default());
    fn depth(blocks: &[Block]) -> usize {
        blocks
            .iter()
            .map(|b| match b {
                Block::Table { rows, .. } => 1 + rows.iter().flatten().map(|c| depth(&c.blocks)).max().unwrap_or(0),
                _ => 0,
            })
            .max()
            .unwrap_or(0)
    }
    assert_eq!(depth(&doc.blocks), MAX_TABLE_DEPTH);
}

#[test]
fn forms_lose_every_way_to_submit() {
    let doc = display(include_str!("hostile/forms.html"), &Options::default());
    let all = format!("{doc:?}");
    for word in ["action", "post", "password", "token", "evil.example"] {
        assert!(!all.contains(word), "{word}");
    }
}

#[test]
fn remote_pictures_and_styles_load_nothing_without_consent() {
    for (name, html) in FIXTURES {
        let doc = display(html, &Options::default());
        assert!(!format!("{:?}", doc.blocks).contains("evil.example"), "{name}");
    }
}
