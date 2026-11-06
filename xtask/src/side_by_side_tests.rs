// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::fs;
use std::path::Path;

use super::{GAP, GAP_COLOUR, Image, MAX_SIDE, Options, beside, decode, encode, gate, resize, sketch_for};
use crate::Error;
use crate::test_support::{Broken, run_in};

fn filled(width: usize, height: usize, rgba: [u8; 4]) -> Image {
    Image { width, height, rgba: rgba.repeat(width * height) }
}

fn pixel(image: &Image, x: usize, y: usize) -> [u8; 4] {
    let at = (y * image.width + x) * 4;
    let slice = &image.rgba[at..at + 4];
    [slice[0], slice[1], slice[2], slice[3]]
}

fn write_png(path: &Path, image: &Image) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, encode(image, path).unwrap()).unwrap();
}

fn read_png(path: &Path) -> Image {
    decode(&fs::read(path).unwrap(), path).unwrap()
}

/// A workspace with a shot script of `lines`, the pictures named in it and the mood sketches named in `sketches`.
fn fixture(lines: &str, shots: &[(&str, Image)], sketches: &[(&str, Image)]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("shots.jsonl"), lines).unwrap();
    for (rel, image) in shots {
        write_png(&dir.path().join(rel), image);
    }
    for (name, image) in sketches {
        write_png(&dir.path().join("plan/mood").join(format!("{name}.png")), image);
    }
    dir
}

fn options() -> Options {
    Options { script: Some("shots.jsonl".into()), mood: None, out: None }
}

#[test]
fn the_sketch_is_scaled_to_the_height_of_the_shot() {
    let shot = filled(4, 2, [255, 0, 0, 255]);
    let sketch = filled(6, 4, [0, 0, 255, 255]);
    let both = beside(&shot, &sketch).unwrap();
    assert_eq!((both.width, both.height), (4 + GAP + 3, 2));
    assert_eq!(pixel(&both, 0, 0), [255, 0, 0, 255]);
    assert_eq!(pixel(&both, 3, 1), [255, 0, 0, 255]);
    assert_eq!(pixel(&both, 4, 0), GAP_COLOUR);
    assert_eq!(pixel(&both, 4 + GAP - 1, 1), GAP_COLOUR);
    assert_eq!(pixel(&both, 4 + GAP, 0), [0, 0, 255, 255]);
    assert_eq!(pixel(&both, 4 + GAP + 2, 1), [0, 0, 255, 255]);
}

#[test]
fn shrinking_averages_the_area_and_growing_keeps_the_colour() {
    let checker = Image { width: 2, height: 2, rgba: [[0, 0, 0, 255], [255, 255, 255, 255], [255, 255, 255, 255], [0, 0, 0, 255]].concat() };
    let small = resize(&checker, 1, 1);
    assert_eq!((small.width, small.height), (1, 1));
    let grey = pixel(&small, 0, 0);
    assert!(grey[0].abs_diff(128) <= 1 && grey[0] == grey[1] && grey[1] == grey[2] && grey[3] == 255, "{grey:?}");

    let big = resize(&filled(1, 1, [10, 20, 30, 40]), 3, 2);
    assert_eq!((big.width, big.height), (3, 2));
    assert!(big.rgba.chunks(4).all(|p| p == [10, 20, 30, 40]));
}

#[test]
fn a_shot_pairs_with_the_sketch_of_its_preset_and_mode() {
    assert_eq!(
        sketch_for(r#"{"out": "docs/images/list-light.png", "mode": "light"}"#).unwrap(),
        ("docs/images/list-light.png".to_owned(), "electric-light".to_owned())
    );
    assert_eq!(
        sketch_for(r#"{"out": "a/b.png", "mode": "dark", "preset": "citrus", "size": [1, 2]}"#).unwrap(),
        ("a/b.png".to_owned(), "citrus-dark".to_owned())
    );
    assert!(sketch_for(r#"{"mode": "dark"}"#).is_err());
    assert!(sketch_for("not json").is_err());
    assert!(sketch_for(r#"{"out": "a.png", "mode": "../x"}"#).is_err());
    assert!(sketch_for(r#"{"out": "a.png", "mode": "dark", "preset": "a/b"}"#).is_err());
}

#[test]
fn every_shot_gets_one_side_by_side_picture() {
    let lines = "# the pictures\n\n{\"out\": \"img/list-light.png\", \"mode\": \"light\"}\n{\"out\": \"img/list-aurora-dark.png\", \"mode\": \"dark\", \"preset\": \"aurora\"}\n";
    let dir = fixture(
        lines,
        &[("img/list-light.png", filled(8, 4, [1, 2, 3, 255])), ("img/list-aurora-dark.png", filled(8, 4, [4, 5, 6, 255]))],
        &[("electric-light", filled(4, 4, [7, 8, 9, 255])), ("aurora-dark", filled(2, 2, [9, 9, 9, 255]))],
    );
    let mut out = Vec::new();
    assert!(gate(dir.path(), &options(), &mut out).unwrap());
    let report = String::from_utf8(out).unwrap();
    assert!(report.contains("side-by-side: plan/side-by-side/list-light.png"), "{report}");
    assert!(report.contains("side-by-side: 2 pictures in plan/side-by-side\n"), "{report}");
    let light = read_png(&dir.path().join("plan/side-by-side/list-light.png"));
    assert_eq!((light.width, light.height), (8 + GAP + 4, 4));
    assert_eq!(pixel(&light, 8 + GAP, 0), [7, 8, 9, 255]);
    let dark = read_png(&dir.path().join("plan/side-by-side/list-aurora-dark.png"));
    assert_eq!((dark.width, dark.height), (8 + GAP + 4, 4));
}

#[test]
fn a_missing_picture_or_sketch_is_reported_and_fails() {
    let lines = "{\"out\": \"img/a-light.png\", \"mode\": \"light\"}\n{\"out\": \"img/b-dark.png\", \"mode\": \"dark\"}\n";
    let dir = fixture(lines, &[("img/a-light.png", filled(2, 2, [0, 0, 0, 255]))], &[("electric-light", filled(2, 2, [0, 0, 0, 255]))]);
    let mut out = Vec::new();
    assert!(!gate(dir.path(), &options(), &mut out).unwrap());
    let report = String::from_utf8(out).unwrap();
    assert!(report.contains("img/b-dark.png: no such picture, run `just shot` first"), "{report}");
    assert!(dir.path().join("plan/side-by-side/a-light.png").is_file());

    fs::write(dir.path().join("shots.jsonl"), "{\"out\": \"img/a-light.png\", \"mode\": \"dark\"}\n").unwrap();
    let mut out = Vec::new();
    assert!(!gate(dir.path(), &options(), &mut out).unwrap());
    let report = String::from_utf8(out).unwrap();
    assert!(report.contains("electric-dark.png: no such mood sketch"), "{report}");
}

#[test]
fn the_folders_can_be_named() {
    let dir = fixture("{\"out\": \"a-light.png\", \"mode\": \"light\"}\n", &[("a-light.png", filled(2, 2, [0, 0, 0, 255]))], &[]);
    write_png(&dir.path().join("sketches/electric-light.png"), &filled(2, 2, [0, 0, 0, 255]));
    let named = Options { script: Some("shots.jsonl".into()), mood: Some("sketches".into()), out: Some(dir.path().join("pairs")) };
    assert!(gate(dir.path(), &named, &mut Vec::new()).unwrap());
    assert!(dir.path().join("pairs/a-light.png").is_file());
}

#[test]
fn a_bad_line_or_picture_stops_with_an_error() {
    let dir = fixture("{\"out\": \"a.png\"}\n", &[], &[]);
    match gate(dir.path(), &options(), &mut Vec::new()) {
        Err(Error::Config { message, .. }) => assert!(message.contains("line 1"), "{message}"),
        other => panic!("{other:?}"),
    }

    let dir = fixture("{\"out\": \"a-light.png\", \"mode\": \"light\"}\n", &[], &[("electric-light", filled(1, 1, [0, 0, 0, 255]))]);
    fs::write(dir.path().join("a-light.png"), b"not a png").unwrap();
    assert!(matches!(gate(dir.path(), &options(), &mut Vec::new()), Err(Error::Image { .. })));

    let missing = Options { script: Some("nowhere.jsonl".into()), mood: None, out: None };
    assert!(matches!(gate(dir.path(), &missing, &mut Vec::new()), Err(Error::Io { .. })));
}

#[test]
fn a_closed_report_is_an_output_error() {
    let dir = fixture(
        "{\"out\": \"a-light.png\", \"mode\": \"light\"}\n",
        &[("a-light.png", filled(1, 1, [0, 0, 0, 255]))],
        &[("electric-light", filled(1, 1, [0, 0, 0, 255]))],
    );
    assert!(matches!(gate(dir.path(), &options(), &mut Broken), Err(Error::Output(_))));
}

#[test]
fn decoding_turns_every_colour_type_into_rgba_and_refuses_huge_pictures() {
    let path = Path::new("x.png");
    for (colour, data, expected) in [
        (png::ColorType::Rgb, vec![1, 2, 3], [1, 2, 3, 255]),
        (png::ColorType::GrayscaleAlpha, vec![9, 100], [9, 9, 9, 100]),
        (png::ColorType::Grayscale, vec![9], [9, 9, 9, 255]),
    ] {
        let mut bytes = Vec::new();
        let mut encoder = png::Encoder::new(&mut bytes, 1, 1);
        encoder.set_color(colour);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.write_header().unwrap().write_image_data(&data).unwrap();
        assert_eq!(decode(&bytes, path).unwrap().rgba, expected);
    }

    let mut bytes = Vec::new();
    let side = u32::try_from(MAX_SIDE + 1).unwrap();
    let mut encoder = png::Encoder::new(&mut bytes, side, 1);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().unwrap();
    writer.write_image_data(&vec![0; (MAX_SIDE + 1) * 4]).unwrap();
    writer.finish().unwrap();
    assert!(matches!(decode(&bytes, path), Err(Error::Image { .. })));
}

#[test]
fn the_command_line_reaches_the_gate() {
    let dir = fixture(
        "{\"out\": \"a-light.png\", \"mode\": \"light\"}\n",
        &[("a-light.png", filled(1, 1, [0, 0, 0, 255]))],
        &[("electric-light", filled(1, 1, [0, 0, 0, 255]))],
    );
    let root = dir.path().to_str().unwrap();
    let (code, out, err) =
        run_in(crate::test_support::context(dir.path().to_path_buf()), &["side-by-side", "--root", root, "--script", "shots.jsonl"]);
    assert_eq!((code, err.as_str()), (crate::OK, ""), "{out}");
    assert!(dir.path().join("plan/side-by-side/a-light.png").is_file());
}
