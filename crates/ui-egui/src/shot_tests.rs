// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::collections::HashMap;
use std::io::{self, Write};

use egui::epaint::{ClippedPrimitive, Mesh, Primitive, TextureId, Vertex, WHITE_UV};
use egui::{Color32, ColorImage, Rect, TextureOptions, pos2, vec2};

use super::{Canvas, Open, Picture, Shot, Texture, apply_textures, fill, parse_script, render, shoot};
use crate::theme::{Mode, PRESETS, Preset, Theme, contrast, over};

fn shot(json: &str) -> Shot {
    parse_script(json).unwrap().remove(0)
}

#[test]
fn a_script_is_one_picture_a_line_with_blank_and_comment_lines_skipped() {
    let script = "# pictures for the PR\n{\"out\": \"a.png\", \"mode\": \"light\"}\n\n{\"out\": \"b.png\", \"mode\": \"dark\", \"size\": [800, 600], \"scale\": 2, \"narrow\": 2, \"open\": {\"folder\": 15}, \"search\": \"x\", \"select\": \"y\"}\n";
    let shots = parse_script(script).unwrap();
    assert_eq!(shots.len(), 2);
    assert_eq!(shots[0].out, "a.png");
    assert_eq!(shots[0].size, None);
    assert_eq!(shots[1].open, Some(Open::Folder(15)));
    assert_eq!(shots[1].size, Some([800.0, 600.0]));
    assert_eq!(shot(r#"{"out": "c.png", "mode": "dark", "open": {"role": "sent"}}"#).open, Some(Open::Role("sent".to_owned())));
}

#[test]
fn a_bad_line_is_named() {
    let error = parse_script("{\"out\": \"a.png\", \"mode\": \"light\"}\n\nnot json\n").unwrap_err().to_string();
    assert!(error.contains("line 3"), "{error}");
    let error = parse_script(r#"{"out": "a.png", "mode": "light", "colour": "red"}"#).unwrap_err().to_string();
    assert!(error.contains("colour"), "{error}");
}

#[test]
fn the_sample_window_is_drawn_in_the_tokens_of_each_preset_in_each_mode() {
    for preset in &PRESETS {
        for mode in [Mode::Light, Mode::Dark] {
            let name = if mode == Mode::Light { "light" } else { "dark" };
            let id = preset.id;
            let picture = shoot(&shot(&format!(r#"{{"out": "x.png", "mode": "{name}", "preset": "{id}", "size": [1000, 700]}}"#))).unwrap();
            assert_eq!((picture.width(), picture.height()), (1000, 700));
            let palette = Theme::new(mode, preset).palette;
            // Below the list's rows: its glass over the colour fields (rasterised for 1024 by 768).
            let theme = Theme::new(mode, preset);
            let [r, g, b, _] = crate::glass::fields(&theme).colour_at(pos2(980.5, 686.5), vec2(1024.0, 768.0)).map(|c| (c * 255.0).round() as u8);
            let expected = over(palette.glass, Color32::from_rgb(r, g, b));
            let glass = picture.pixel(980, 686).unwrap();
            assert!(contrast(glass, expected) < 1.03, "{id} {name}: glass over the fields below the list: {glass:?} {expected:?}");
            let mut colours = std::collections::BTreeSet::new();
            for y in (0..700).step_by(7) {
                for x in (0..1000).step_by(7) {
                    colours.insert(picture.pixel(x, y).unwrap().to_array());
                }
            }
            assert!(colours.len() > 20, "{id} {name}: text and markers were drawn, not just the panels ({} colours)", colours.len());
            assert!(colours.contains(&palette.text.to_array()), "{id} {name}: text in the text colour");
        }
    }
}

#[test]
fn an_icon_sheet_shows_every_icon_of_the_set_in_the_tokens_of_its_preset() {
    for preset in &PRESETS {
        for mode in [Mode::Light, Mode::Dark] {
            let name = if mode == Mode::Light { "light" } else { "dark" };
            let id = preset.id;
            let picture = shoot(&shot(&format!(r#"{{"out": "x.png", "mode": "{name}", "preset": "{id}", "icons": true}}"#))).unwrap();
            let palette = Theme::new(mode, preset).palette;
            assert_eq!(picture.pixel(0, 0), Some(palette.panel), "{id} {name}: the sheet is on the panel");
            let mut colours = std::collections::BTreeSet::new();
            for y in 0..picture.height() {
                for x in 0..picture.width() {
                    colours.insert(picture.pixel(x, y).unwrap().to_array());
                }
            }
            for (token, colour) in [("text", palette.text), ("accent", palette.accent)] {
                assert!(colours.contains(&colour.to_array()), "{id} {name}: icons in the {token} colour");
            }
        }
    }
}

#[test]
fn an_icon_sheet_is_not_a_window() {
    let error = shoot(&shot(r#"{"out": "x.png", "mode": "light", "icons": true, "search": "logo"}"#)).unwrap_err();
    assert!(error.to_string().contains("x.png: an icon sheet"), "{error}");
}

#[test]
fn the_component_gallery_is_drawn_in_the_tokens_of_its_preset_and_mode() {
    let mut pictures = Vec::new();
    for preset in &PRESETS {
        for mode in [Mode::Light, Mode::Dark] {
            let name = if mode == Mode::Light { "light" } else { "dark" };
            let id = preset.id;
            let picture = shoot(&shot(&format!(r#"{{"out": "x.png", "mode": "{name}", "preset": "{id}", "components": true}}"#))).unwrap();
            let size = crate::component::gallery::size();
            assert_eq!((picture.width(), picture.height()), (size.x as usize, size.y as usize));
            let theme = Theme::new(mode, preset);
            let mut colours = std::collections::BTreeSet::new();
            for y in 0..picture.height() {
                for x in 0..picture.width() {
                    colours.insert(picture.pixel(x, y).unwrap().to_array());
                }
            }
            let avatar = crate::component::avatar::colour(&theme, "odile@example.org");
            assert!(colours.contains(&avatar.to_array()), "{id} {name}: the avatars in their palette");
            assert!(colours.contains(&theme.palette.surface.to_array()), "{id} {name}: the cards on the surface");
            pictures.push(picture);
        }
    }
    for (index, one) in pictures.iter().enumerate() {
        assert!(pictures.iter().skip(index + 1).all(|other| other != one), "every preset and mode looks its own");
    }
}

#[test]
fn the_component_gallery_is_not_a_window_nor_an_icon_sheet() {
    for extra in [r#""select": "logo""#, r#""icons": true"#, r#""size": [10, 10]"#] {
        let error = shoot(&shot(&format!(r#"{{"out": "x.png", "mode": "light", "components": true, {extra}}}"#))).unwrap_err();
        assert!(error.to_string().contains("x.png: the component gallery"), "{error}");
    }
}

#[test]
fn a_shot_without_a_preset_is_electric() {
    let picture = shoot(&shot(r#"{"out": "x.png", "mode": "light", "size": [600, 400]}"#)).unwrap();
    let electric = shoot(&shot(r#"{"out": "x.png", "mode": "light", "preset": "electric", "size": [600, 400]}"#)).unwrap();
    let citrus = shoot(&shot(r#"{"out": "x.png", "mode": "light", "preset": "citrus", "size": [600, 400]}"#)).unwrap();
    assert_eq!(picture, electric);
    assert_ne!(picture.pixel(4, 390), citrus.pixel(4, 390));
    assert_eq!(Preset::default().id, "electric");
}

#[test]
fn a_text_scale_draws_the_window_with_larger_type() {
    let plain = shoot(&shot(r#"{"out": "x.png", "mode": "light", "size": [600, 400]}"#)).unwrap();
    let hundred = shoot(&shot(r#"{"out": "x.png", "mode": "light", "size": [600, 400], "text_scale": 100}"#)).unwrap();
    let large = shoot(&shot(r#"{"out": "x.png", "mode": "light", "size": [600, 400], "text_scale": 130}"#)).unwrap();
    assert_eq!(plain, hundred, "100 % is the default");
    assert_ne!(plain, large);
}

#[test]
fn a_scale_of_two_doubles_the_picture() {
    let picture = shoot(&shot(r#"{"out": "x.png", "mode": "light", "size": [300, 200], "scale": 2}"#)).unwrap();
    assert_eq!((picture.width(), picture.height()), (600, 400));
}

#[test]
fn a_shot_can_narrow_open_search_and_select() {
    for json in [
        r#"{"out": "x.png", "mode": "dark", "size": [600, 400], "narrow": 2, "open": {"role": "inbox"}, "select": "Draft logo for the bakery"}"#,
        r#"{"out": "x.png", "mode": "light", "size": [600, 400], "open": {"folder": 7}}"#,
        r#"{"out": "x.png", "mode": "light", "size": [600, 400], "search": "logo"}"#,
        r#"{"out": "x.png", "mode": "light", "size": [900, 500], "select": "Your bike is due for a service", "pictures": "once", "details": true}"#,
        r#"{"out": "x.png", "mode": "dark", "size": [900, 500], "open": {"folder": 5}, "select": "You have won a lighthouse", "pictures": "always"}"#,
        r#"{"out": "x.png", "mode": "dark", "size": [900, 500], "open": {"folder": 3}, "select": "Re: Allotment rota for November", "quotes": true, "full_width": true}"#,
    ] {
        shoot(&shot(json)).unwrap();
    }
}

#[test]
fn a_shot_that_names_something_wrong_is_refused() {
    for (json, says) in [
        (r#"{"out": "x.png", "mode": "sepia"}"#, "sepia"),
        (r#"{"out": "x.png", "mode": "light", "preset": "teal"}"#, "no preset \"teal\""),
        (r#"{"out": "x.png", "mode": "light", "open": {"role": "attic"}}"#, "attic"),
        (r#"{"out": "x.png", "mode": "light", "select": "No such subject"}"#, "No such subject"),
        (r#"{"out": "x.png", "mode": "light", "select": "Your bike is due for a service", "pictures": "maybe"}"#, "maybe"),
        (r#"{"out": "x.png", "mode": "light", "full_width": true}"#, "need a selected row"),
        (r#"{"out": "x.png", "mode": "light", "scale": 0.5}"#, "scale"),
        (r#"{"out": "x.png", "mode": "light", "text_scale": 95}"#, "text scale 95"),
        (r#"{"out": "x.png", "mode": "light", "text_scale": 200}"#, "text scale 200"),
        (r#"{"out": "x.png", "mode": "light", "scale": 9}"#, "scale"),
        (r#"{"out": "x.png", "mode": "light", "size": [0, 100]}"#, "pixels a side"),
        (r#"{"out": "x.png", "mode": "light", "size": [100000, 100]}"#, "pixels a side"),
        (r#"{"out": "x.png", "mode": "light", "size": [100, -5]}"#, "pixels a side"),
    ] {
        let error = shoot(&shot(json)).unwrap_err().to_string();
        assert!(error.contains(says), "{json}: {error}");
    }
}

#[test]
fn a_picture_is_written_as_png() {
    let picture = shoot(&shot(r#"{"out": "x.png", "mode": "light", "size": [120, 80]}"#)).unwrap();
    let mut out = Vec::new();
    picture.write_png(&mut out).unwrap();
    assert_eq!(&out[..8], b"\x89PNG\r\n\x1a\n");
    assert!(picture.pixel(120, 0).is_none());
    assert!(picture.pixel(0, 80).is_none());
    assert!(picture.pixel(usize::MAX, usize::MAX).is_none());
}

struct Broken;

impl Write for Broken {
    fn write(&mut self, _: &[u8]) -> io::Result<usize> {
        Err(io::Error::other("disk full"))
    }

    fn flush(&mut self) -> io::Result<()> {
        Err(io::Error::other("disk full"))
    }
}

#[test]
fn a_failing_writer_is_an_error_not_a_panic() {
    let picture = Picture { width: 2, height: 2, rgba: vec![0; 16] };
    assert!(picture.write_png(&mut Broken).is_err());
    Broken.flush().unwrap_err();
    let too_wide = Picture { width: usize::MAX, height: 1, rgba: Vec::new() };
    assert!(too_wide.write_png(&mut Vec::new()).unwrap_err().to_string().contains("too wide"));
    let too_high = Picture { width: 1, height: usize::MAX, rgba: Vec::new() };
    assert!(too_high.write_png(&mut Vec::new()).unwrap_err().to_string().contains("too high"));
}

fn white() -> HashMap<TextureId, Texture> {
    HashMap::from([(TextureId::default(), Texture { width: 1, height: 1, pixels: vec![Color32::WHITE] })])
}

fn square(colour: Color32, clip: Rect) -> ClippedPrimitive {
    let mut mesh = Mesh::default();
    for (x, y) in [(0.0, 0.0), (8.0, 0.0), (8.0, 8.0), (0.0, 8.0)] {
        mesh.vertices.push(Vertex { pos: pos2(x, y), uv: WHITE_UV, color: colour });
    }
    // Clockwise and anticlockwise, sharing the diagonal, plus a degenerate one and one with a missing vertex.
    mesh.indices = vec![0, 1, 2, 0, 3, 2, 0, 0, 1, 0, 1, 99];
    ClippedPrimitive { clip_rect: clip, primitive: Primitive::Mesh(mesh) }
}

#[test]
fn two_triangles_sharing_an_edge_fill_each_pixel_once() {
    let half = Color32::from_rgba_premultiplied(100, 0, 0, 128);
    let picture = fill(&[square(half, Rect::EVERYTHING)], &white(), 8, 8, 1.0);
    for y in 0..8 {
        for x in 0..8 {
            assert_eq!(picture.pixel(x, y), Some(half), "{x}, {y}");
        }
    }
}

#[test]
fn the_clip_rect_and_an_unknown_texture_are_respected() {
    let picture = fill(&[square(Color32::RED, Rect::from_min_max(pos2(0.0, 0.0), pos2(4.0, 8.0)))], &white(), 8, 8, 1.0);
    assert_eq!(picture.pixel(3, 3), Some(Color32::RED));
    assert_eq!(picture.pixel(5, 3), Some(Color32::TRANSPARENT));
    let mut other = square(Color32::RED, Rect::EVERYTHING);
    if let Primitive::Mesh(mesh) = &mut other.primitive {
        mesh.texture_id = TextureId::User(7);
    }
    let picture = fill(&[other], &white(), 8, 8, 1.0);
    assert_eq!(picture.pixel(3, 3), Some(Color32::TRANSPARENT));
    let picture = fill(&[square(Color32::RED, Rect::NOTHING)], &white(), 8, 8, 2.0);
    assert_eq!(picture.pixel(3, 3), Some(Color32::TRANSPARENT));
}

#[test]
fn textures_are_set_patched_and_freed() {
    let mut textures = HashMap::new();
    let id = TextureId::Managed(3);
    let mut delta = egui::TexturesDelta::default();
    delta.set.insert(id, [egui::epaint::ImageDelta::full(ColorImage::filled([4, 4], Color32::BLACK), TextureOptions::LINEAR)].into());
    apply_textures(&mut textures, &mut delta);
    let mut delta = egui::TexturesDelta::default();
    delta.set.insert(id, [egui::epaint::ImageDelta::partial([3, 3], ColorImage::filled([2, 2], Color32::WHITE), TextureOptions::LINEAR)].into());
    delta.set.insert(
        TextureId::Managed(9),
        [egui::epaint::ImageDelta::partial([0, 0], ColorImage::filled([1, 1], Color32::WHITE), TextureOptions::LINEAR)].into(),
    );
    apply_textures(&mut textures, &mut delta);
    let texture = textures.get(&id).unwrap();
    assert_eq!(texture.pixels[15], Color32::WHITE, "the patch lands at 3, 3 and the part outside is dropped");
    assert_eq!(texture.pixels[0], Color32::BLACK);
    assert!(!textures.contains_key(&TextureId::Managed(9)), "a patch of an unknown texture is ignored");
    assert_eq!(texture.sample(pos2(0.0, 0.0)), [0.0, 0.0, 0.0, 1.0]);
    assert_eq!(texture.sample(pos2(1.0, 1.0)), [1.0, 1.0, 1.0, 1.0]);
    let mut delta = egui::TexturesDelta::default();
    delta.free.insert(id);
    apply_textures(&mut textures, &mut delta);
    assert!(textures.is_empty());
}

#[test]
fn rendering_any_app_needs_a_sane_size() {
    let mut app = crate::InboxApp::new(crate::mailbox::Mailbox::new(
        crate::sample::session(Box::new(inboxcraft_time::FakeClock::new(inboxcraft_time::Timestamp::UNIX_EPOCH))).unwrap(),
        Box::new(inboxcraft_time::FakeClock::new(inboxcraft_time::Timestamp::UNIX_EPOCH)),
    ));
    assert!(render(&mut app, vec2(f32::NAN, 10.0), 1.0, Mode::Light).is_err());
    assert!(render(&mut app, vec2(10.0, 10.0), f32::NAN, Mode::Light).is_err());
    let canvas = Canvas::new(0, 0);
    assert_eq!(canvas.picture().width(), 0);
}

#[test]
fn a_shot_shows_the_add_account_page_in_each_state_and_refuses_another() {
    for state in ["form", "detected", "checking", "passed", "failed"] {
        let picture = shoot(&shot(&format!(r#"{{"out": "x.png", "mode": "dark", "size": [700, 500], "onboard": "{state}"}}"#))).unwrap();
        assert_eq!((picture.width(), picture.height()), (700, 500), "{state}");
    }
    let error = shoot(&shot(r#"{"out": "x.png", "mode": "light", "onboard": "half"}"#)).unwrap_err().to_string();
    assert!(error.contains("onboard \"half\" is not form, detected, checking, passed or failed"), "{error}");
}

#[test]
fn the_picture_credential_store_takes_and_keeps_nothing() {
    use crate::onboard::Keychain as _;
    assert_eq!(super::Pictured.store(1, "invented-pass"), Ok(()));
    assert_eq!(super::Pictured.forget(1), Ok(()));
}
