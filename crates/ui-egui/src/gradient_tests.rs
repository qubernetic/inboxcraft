// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use egui::{Color32, ColorImage, pos2, vec2};

use super::{Field, Gradient, Stop, dither, image, raster};

fn grey(value: u8) -> Color32 {
    Color32::from_rgba_premultiplied(value, value, value, 255)
}

/// A gradient across `width` from grey `from` to grey `to`.
fn ramp(from: u8, to: u8) -> Gradient {
    Gradient::Linear { from: pos2(0.0, 0.5), to: pos2(1.0, 0.5), stops: vec![Stop::new(0.0, grey(from)), Stop::new(1.0, grey(to))] }
}

fn red(image: &ColorImage, x: usize, y: usize) -> u8 {
    image.pixels.get(y * image.size[0] + x).map_or(0, |pixel| pixel.r())
}

/// The longest run of equal neighbours along row `y`.
fn longest_run(image: &ColorImage, y: usize) -> usize {
    let (mut longest, mut run) = (1, 1);
    for x in 1..image.size[0] {
        run = if red(image, x, y) == red(image, x - 1, y) { run + 1 } else { 1 };
        longest = longest.max(run);
    }
    longest
}

/// The largest gap between the mean of a column and the ideal value at its centre, in steps of 1/255.
fn worst_column_error(image: &ColorImage, from: f32, to: f32) -> f32 {
    let [width, height] = image.size;
    (0..width)
        .map(|x| {
            let mean = (0..height).map(|y| f32::from(red(image, x, y))).sum::<f32>() / height as f32;
            let ideal = from + (to - from) * ((x as f32 + 0.5) / width as f32);
            (mean - ideal).abs()
        })
        .fold(0.0, f32::max)
}

#[test]
fn a_linear_gradient_runs_through_its_stops() {
    let gradient = Gradient::Linear {
        from: pos2(0.0, 0.0),
        to: pos2(1.0, 0.0),
        stops: vec![Stop::new(0.0, grey(0)), Stop::new(0.25, grey(100)), Stop::new(1.0, grey(200))],
    };
    let size = vec2(100.0, 10.0);
    let at = |x: f32| (gradient.colour_at(pos2(x, 5.0), size)[0] * 255.0).round();
    assert_eq!(at(0.0), 0.0);
    assert_eq!(at(12.5), 50.0);
    assert_eq!(at(25.0), 100.0);
    assert_eq!(at(62.5), 150.0);
    assert_eq!(at(100.0), 200.0);
    assert_eq!(at(-50.0), 0.0, "before the first stop");
    assert_eq!(at(500.0), 200.0, "after the last stop");
    assert_eq!(at(f32::NAN), 0.0, "nonsense is the start");
}

#[test]
fn a_gradient_without_stops_or_length_is_harmless() {
    let size = vec2(10.0, 10.0);
    let empty = Gradient::Linear { from: pos2(0.0, 0.0), to: pos2(1.0, 0.0), stops: Vec::new() };
    assert_eq!(empty.colour_at(pos2(5.0, 5.0), size), [0.0; 4]);
    let point = Gradient::Linear { from: pos2(0.5, 0.5), to: pos2(0.5, 0.5), stops: vec![Stop::new(0.0, grey(10)), Stop::new(1.0, grey(90))] };
    assert_eq!((point.colour_at(pos2(5.0, 5.0), size)[0] * 255.0).round(), 10.0);
    let nothing = Gradient::Linear { from: pos2(0.0, 0.0), to: pos2(1.0, 0.0), stops: vec![Stop::new(0.0, grey(10))] };
    assert_eq!(nothing.colour_at(pos2(5.0, 5.0), vec2(0.0, 0.0))[3], 1.0, "a shape of no size");
}

#[test]
fn colour_fields_glow_at_their_centres_and_fade_into_the_base() {
    let base = grey(20);
    let glow = Color32::from_rgba_unmultiplied(250, 20, 150, 128);
    let gradient = Gradient::Fields { base, fields: vec![Field { centre: pos2(0.25, 0.5), radius: 0.4, colour: glow }] };
    let size = vec2(200.0, 100.0);
    let centre = gradient.colour_at(pos2(50.0, 50.0), size);
    assert!((centre[0] * 255.0 - (250.0 * 0.5 + 20.0 * 0.5)).abs() < 1.5, "half the glow over the base: {centre:?}");
    assert_eq!(centre[3], 1.0, "over an opaque base the field stays opaque");
    let far = gradient.colour_at(pos2(199.0, 0.0), size);
    assert!((far[0] * 255.0 - 20.0).abs() < 0.01, "beyond the radius only the base: {far:?}");
    let between = gradient.colour_at(pos2(90.0, 50.0), size);
    assert!(between[0] < centre[0] && between[0] > far[0], "it fades: {between:?}");
}

#[test]
fn a_shadow_is_dark_under_the_edge_fades_outwards_and_leaves_the_inside_clear() {
    let colour = Color32::from_rgba_unmultiplied(0, 0, 0, 100);
    let gradient = Gradient::Shadow { colour, blur: 10.0, drop: 0.0, corner: 0.0 };
    let size = vec2(120.0, 120.0);
    assert_eq!(gradient.colour_at(pos2(60.0, 60.0), size)[3], 0.0, "nothing under the layer itself");
    let edge = gradient.colour_at(pos2(9.5, 60.0), size)[3];
    assert!((edge * 255.0 - 100.0).abs() < 4.0, "full at the edge: {edge}");
    let half = gradient.colour_at(pos2(5.0, 60.0), size)[3];
    assert!(half < edge && half > 0.0, "fading: {half}");
    assert_eq!(gradient.colour_at(pos2(0.0, 0.0), size)[3], 0.0, "gone at the outer edge");
    let dropped = Gradient::Shadow { colour, blur: 10.0, drop: 4.0, corner: 6.0 };
    let above = dropped.colour_at(pos2(60.0, 8.0), size)[3];
    let below = dropped.colour_at(pos2(60.0, 112.0), size)[3];
    assert!(below > above, "the shadow falls downwards: {above} {below}");
}

#[test]
fn the_dither_keeps_the_mean_and_stays_within_one_step() {
    for value in [0.0, 0.1, 30.4 / 255.0, 0.5, 0.9, 1.0] {
        let mut sum = 0.0;
        for y in 0..64 {
            for x in 0..64 {
                let quantised = f32::from(dither(value, x, y));
                assert!((quantised - value * 255.0).abs() < 1.0, "{value}: {quantised}");
                sum += quantised;
            }
        }
        assert!((sum / 4096.0 - value * 255.0).abs() < 0.05, "{value}: the mean of a block is the value: {}", sum / 4096.0);
    }
    assert_eq!(dither(f32::NAN, 3, 4), 0);
    assert_eq!(dither(7.0, 3, 4), 255);
    assert_eq!(dither(-1.0, 3, 4), 0);
}

#[test]
fn dithering_breaks_up_the_bands_of_a_wide_low_contrast_gradient() {
    // Eight steps of grey over 1024 pixels: undithered, every step is a band 128 pixels wide.
    let gradient = ramp(32, 40);
    let size = vec2(1024.0, 32.0);
    let banded = raster(&gradient, size, [1024, 32], false);
    assert!(longest_run(&banded, 7) >= 100, "the undithered gradient shows bands: {}", longest_run(&banded, 7));
    assert!(worst_column_error(&banded, 32.0, 40.0) > 0.45, "and its columns are up to half a step off");
    let dithered = image(&gradient, size, [1024, 32]);
    for y in [0, 7, 31] {
        // Only next to a whole step, where the colour is exact, may a run come near half a band.
        assert!(longest_run(&dithered, y) <= 64, "row {y}: runs of {} pixels", longest_run(&dithered, y));
    }
    let error = worst_column_error(&dithered, 32.0, 40.0);
    assert!(error < 0.2, "the dithered columns follow the ideal ramp: {error}");
}

#[test]
fn the_picture_is_premultiplied_and_never_brighter_than_its_alpha() {
    let gradient = Gradient::Linear {
        from: pos2(0.0, 0.0),
        to: pos2(1.0, 0.0),
        stops: vec![
            Stop::new(0.0, Color32::from_rgba_unmultiplied(255, 255, 255, 0)),
            Stop::new(1.0, Color32::from_rgba_unmultiplied(255, 200, 100, 90)),
        ],
    };
    let picture = image(&gradient, vec2(300.0, 4.0), [300, 4]);
    for pixel in &picture.pixels {
        assert!(pixel.r() <= pixel.a() && pixel.g() <= pixel.a() && pixel.b() <= pixel.a(), "{pixel:?}");
    }
    assert_eq!(picture.pixels.first().map(|pixel| pixel.a()), Some(0), "transparent at the start");
}

#[test]
fn a_picture_of_nothing_or_too_much_is_capped() {
    let gradient = ramp(0, 255);
    assert_eq!(image(&gradient, vec2(10.0, 10.0), [0, 0]).size, [1, 1]);
    let capped = image(&gradient, vec2(10.0, 10.0), [100_000, 3]);
    assert_eq!(capped.size, [super::MAX_SIDE, 3]);
}

/// The picture of `draw` on a transparent window of `size` points.
fn painted(size: egui::Vec2, draw: impl FnMut(&mut egui::Ui)) -> crate::shot::Picture {
    crate::shot::paint(size, 1.0, crate::theme::Mode::Dark, draw).unwrap()
}

fn column_error(picture: &crate::shot::Picture, width: usize, rows: std::ops::Range<usize>, from: f32, to: f32) -> f32 {
    let height = rows.len() as f32;
    (0..width)
        .map(|x| {
            let mean = rows.clone().map(|y| f32::from(picture.pixel(x, y).unwrap().r())).sum::<f32>() / height;
            let ideal = from + (to - from) * ((x as f32 + 0.5) / width as f32);
            (mean - ideal).abs()
        })
        .fold(0.0, f32::max)
}

#[test]
fn a_filled_shape_shows_the_dithered_gradient_on_screen() {
    let gradient = ramp(32, 40);
    let picture =
        painted(vec2(1024.0, 40.0), |ui| super::fill(ui.painter(), egui::Rect::from_min_size(pos2(0.0, 0.0), vec2(1024.0, 32.0)), 0.0, &gradient));
    let error = column_error(&picture, 1024, 0..32, 32.0, 40.0);
    assert!(error < 0.2, "the window shows the dithered ramp: {error}");
    assert_eq!(picture.pixel(10, 36).map(|pixel| pixel.a()), Some(0), "nothing outside the shape");
}

#[test]
fn a_rounded_fill_keeps_its_corners_clear() {
    let gradient = ramp(200, 220);
    let picture =
        painted(vec2(100.0, 100.0), |ui| super::fill(ui.painter(), egui::Rect::from_min_size(pos2(0.0, 0.0), vec2(100.0, 100.0)), 30.0, &gradient));
    assert_eq!(picture.pixel(1, 1).map(|pixel| pixel.a()), Some(0), "the corner is cut");
    assert_eq!(picture.pixel(50, 50).map(|pixel| pixel.a()), Some(255));
}

#[test]
fn a_picture_is_rasterised_once_and_kept_while_it_is_painted() {
    let ctx = egui::Context::default();
    let gradient = ramp(0, 255);
    let frame = |ctx: &egui::Context, width: f32| {
        let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, vec2(400.0, 300.0))), ..Default::default() };
        ctx.run_ui(input, |ui| super::fill(ui.painter(), egui::Rect::from_min_size(pos2(0.0, 0.0), vec2(width, 20.0)), 0.0, &gradient))
            .textures_delta
            .clear();
    };
    for _ in 0..5 {
        frame(&ctx, 200.0);
    }
    assert_eq!(super::kept(&ctx), 1, "one picture for five frames");
    for width in 1..=60 {
        frame(&ctx, width as f32);
    }
    assert_eq!(super::kept(&ctx), super::KEEP, "no more than the cap");
}

#[test]
fn colour_fields_are_not_rasterised_again_for_every_pixel_of_a_resize() {
    let ctx = egui::Context::default();
    let gradient = Gradient::Fields { base: grey(10), fields: vec![Field { centre: pos2(0.5, 0.5), radius: 0.5, colour: grey(200) }] };
    for width in 1000..1020 {
        let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, vec2(1100.0, 800.0))), ..Default::default() };
        ctx.run_ui(input, |ui| super::fill(ui.painter(), egui::Rect::from_min_size(pos2(0.0, 0.0), vec2(width as f32, 700.0)), 0.0, &gradient))
            .textures_delta
            .clear();
    }
    assert_eq!(super::kept(&ctx), 1);
}

#[test]
fn a_line_runs_through_the_gradient_from_end_to_end() {
    let gradient = Gradient::Linear {
        from: pos2(0.0, 0.5),
        to: pos2(1.0, 0.5),
        stops: vec![Stop::new(0.0, Color32::from_rgb(250, 0, 0)), Stop::new(1.0, Color32::from_rgb(0, 0, 250))],
    };
    let picture = painted(vec2(220.0, 20.0), |ui| super::line(ui.painter(), vec![pos2(10.0, 10.5), pos2(210.0, 10.5)], 3.0, &gradient));
    let start = picture.pixel(14, 10).unwrap();
    let end = picture.pixel(206, 10).unwrap();
    assert!(start.r() > 200 && start.b() < 50, "red at the start: {start:?}");
    assert!(end.b() > 200 && end.r() < 50, "blue at the end: {end:?}");
    let middle = picture.pixel(110, 10).unwrap();
    assert!(middle.r() > 80 && middle.b() > 80, "both in the middle: {middle:?}");
}

#[test]
fn text_takes_the_gradient_glyph_by_glyph() {
    let gradient = Gradient::Linear {
        from: pos2(0.0, 0.5),
        to: pos2(1.0, 0.5),
        stops: vec![Stop::new(0.0, Color32::from_rgb(250, 0, 0)), Stop::new(1.0, Color32::from_rgb(0, 0, 250))],
    };
    let picture = painted(vec2(400.0, 60.0), |ui| {
        let galley = ui.painter().layout_no_wrap("MMMMMMMMMMMMMMMMMMMM".to_owned(), egui::FontId::proportional(24.0), Color32::from_rgb(0, 250, 0));
        super::text(ui.painter(), pos2(10.0, 10.0), &galley, &gradient);
    });
    let ink = |columns: std::ops::Range<usize>| {
        let mut sum = [0u32; 3];
        for x in columns {
            for y in 0..60 {
                let pixel = picture.pixel(x, y).unwrap();
                sum[0] += u32::from(pixel.r());
                sum[1] += u32::from(pixel.g());
                sum[2] += u32::from(pixel.b());
            }
        }
        sum
    };
    let left = ink(10..60);
    let right = ink(240..290);
    assert!(left[0] > left[2] * 3 && left[1] == 0, "red glyphs on the left, none of the text's own green: {left:?}");
    assert!(right[2] > right[0], "bluer glyphs further right: {right:?}");
}

/// Whether `line` paints a colour transition of its own: a mesh with coloured vertices, a stroke coloured per
/// vertex, a blurred shape or an egui shadow (the helper's own shadow gradient aside).
fn paints_a_gradient(line: &str) -> bool {
    let line = line.replace("Gradient::Shadow", "");
    ["Mesh", "Vertex", "colored_vertex", "new_uv", "ColorMode", "blur_width", "Shadow {", "Shadow::new", ".shadow(", "with_texture("]
        .iter()
        .any(|word| line.contains(word))
        || (line.contains("Shadow") && !line.contains("Shadow::NONE") && !line.contains("use egui::epaint::Shadow"))
}

#[test]
fn the_gradient_check_finds_meshes_strokes_and_shadows() {
    assert!(paints_a_gradient("let mut mesh = Mesh::default();"));
    assert!(paints_a_gradient("mesh.colored_vertex(pos, colour);"));
    assert!(paints_a_gradient("PathStroke::new_uv(2.0, |rect, pos| colour)"));
    assert!(paints_a_gradient("Frame::new().shadow(Shadow { offset: [0, 4], blur: 8, spread: 0, color })"));
    assert!(paints_a_gradient("RectShape::filled(rect, 4.0, fill).with_blur_width(6.0)"));
    assert!(!paints_a_gradient("visuals.window_shadow = Shadow::NONE;"));
    assert!(!paints_a_gradient("painter.rect_filled(rect, 4.0, p.selection);"));
    assert!(!paints_a_gradient("let gradient = Gradient::Shadow { colour, blur, drop, corner };"));
}

/// The `.rs` files under `dir`, test files left out.
fn sources(dir: &std::path::Path, found: &mut Vec<std::path::PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().and_then(|name| name.to_str()).unwrap_or_default().to_owned();
        if path.is_dir() {
            sources(&path, found);
        } else if name.ends_with(".rs") && name != "tests.rs" && !name.ends_with("_tests.rs") {
            found.push(path);
        }
    }
}

#[test]
fn no_other_code_paints_a_gradient() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = Vec::new();
    for dir in ["src", "examples"] {
        sources(&root.join(dir), &mut files);
    }
    assert!(files.len() > 10, "the walk found the sources");
    // The gradient helper itself, and the rasteriser of ui_shot, which fills egui's meshes rather than painting.
    let allowed = [root.join("src").join("gradient.rs"), root.join("src").join("shot.rs")];
    let mut found = Vec::new();
    for file in files.iter().filter(|file| !allowed.contains(file)) {
        let text = std::fs::read_to_string(file).unwrap();
        for (index, line) in text.lines().enumerate() {
            let code = line.split("//").next().unwrap_or_default();
            if paints_a_gradient(code) {
                found.push(format!("{}:{}: {}", file.strip_prefix(root).unwrap().display(), index + 1, line.trim()));
            }
        }
    }
    assert!(found.is_empty(), "gradients belong in gradient.rs:\n{}", found.join("\n"));
}

#[test]
fn a_line_gets_a_vertex_every_few_points_and_no_more_than_the_cap() {
    let points = super::subdivided(&[pos2(0.0, 0.0), pos2(40.0, 0.0), pos2(40.0, 2.0)]);
    assert_eq!(points.first(), Some(&pos2(0.0, 0.0)));
    assert_eq!(points.last(), Some(&pos2(40.0, 2.0)));
    assert!(points.windows(2).all(|pair| (pair[1] - pair[0]).length() <= super::LINE_STEP + 0.001));
    assert_eq!(super::subdivided(&[pos2(0.0, 0.0), pos2(1e9, 0.0)]).len(), super::MAX_LINE_POINTS);
    assert_eq!(super::subdivided(&[pos2(0.0, 0.0), pos2(f32::INFINITY, 0.0)]).len(), 2);
    assert!(super::subdivided(&[]).is_empty());
}

fn shadow() -> Gradient {
    Gradient::Shadow { colour: Color32::from_rgba_unmultiplied(0, 0, 0, 120), blur: 18.0, drop: 6.0, corner: 12.0 }
}

/// One frame of `ctx` at `scale` that fills each of `rects` with `gradient`, then sweeps the pictures.
fn fill_frame(ctx: &egui::Context, scale: f32, rects: &[egui::Rect], gradient: &Gradient) {
    let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, vec2(2000.0, 1400.0))), ..Default::default() };
    ctx.set_pixels_per_point(scale);
    ctx.run_ui(input, |ui| {
        for rect in rects {
            super::fill(ui.painter(), *rect, 0.0, gradient);
        }
        super::sweep(ui.ctx());
    })
    .textures_delta
    .clear();
}

#[test]
fn a_shadow_picture_does_not_depend_on_the_size_of_its_layer() {
    let ctx = egui::Context::default();
    for scale in [1.0, 2.0] {
        let before = super::rasterised(&ctx);
        for width in (300..1900).step_by(37) {
            let around = egui::Rect::from_min_size(pos2(0.0, 0.0), vec2(width as f32, width as f32 * 0.6));
            fill_frame(&ctx, scale, &[around], &shadow());
        }
        assert_eq!(super::rasterised(&ctx) - before, 1, "at scale {scale} one picture for every size");
        assert_eq!(super::kept(&ctx), 1);
        let bytes: usize = ctx.tex_manager().read().allocated().filter(|(_, meta)| meta.name == "gradient").map(|(_, meta)| meta.bytes_used()).sum();
        assert!(bytes < 256 * 1024, "the picture holds only the border: {bytes} bytes");
    }
}

#[test]
fn a_sliced_shadow_paints_what_the_whole_picture_would() {
    let around = egui::Rect::from_min_size(pos2(4.0, 4.0), vec2(300.0, 180.0));
    let picture = painted(vec2(310.0, 190.0), |ui| super::fill(ui.painter(), around, 0.0, &shadow()));
    let mut worst = 0.0_f32;
    for y in 4..184 {
        for x in 4..304 {
            let at = pos2(x as f32 + 0.5 - 4.0, y as f32 + 0.5 - 4.0);
            let ideal = shadow().colour_at(at, around.size())[3] * 255.0;
            let shown = f32::from(picture.pixel(x, y).unwrap().a());
            worst = worst.max((shown - ideal).abs());
        }
    }
    assert!(worst <= 2.0, "the slices meet without a seam: off by {worst}");
    assert_eq!(picture.pixel(150, 90).map(|pixel| pixel.a()), Some(0), "the inside stays clear");
}

#[test]
fn a_shadow_too_small_to_slice_is_painted_whole() {
    let around = egui::Rect::from_min_size(pos2(0.0, 0.0), vec2(40.0, 40.0));
    let picture = painted(vec2(50.0, 50.0), |ui| super::fill(ui.painter(), around, 0.0, &shadow()));
    let ideal = shadow().colour_at(pos2(20.5, 2.5), around.size())[3] * 255.0;
    let shown = f32::from(picture.pixel(20, 2).unwrap().a());
    assert!((shown - ideal).abs() <= 2.0, "{shown} {ideal}");
}

#[test]
fn a_sweep_gives_back_the_pictures_this_frame_did_not_paint() {
    let ctx = egui::Context::default();
    let rect = |width: f32| egui::Rect::from_min_size(pos2(0.0, 0.0), vec2(width, 20.0));
    for width in 100..120 {
        fill_frame(&ctx, 1.0, &[rect(width as f32), rect(50.0)], &ramp(0, 255));
    }
    assert_eq!(super::kept(&ctx), 2, "only what the last frame painted");
    fill_frame(&ctx, 1.0, &[], &ramp(0, 255));
    assert_eq!(super::kept(&ctx), 0);
    assert_eq!(ctx.tex_manager().read().allocated().len(), 1, "only egui's own font texture is left");
}
