// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The window driven through its AccessKit tree, as a screen reader sees it. The harness refuses any control
//! without an accessible name after every run.

use egui::accesskit::{Role, Toggled};
use egui::{Key, Modifiers, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use inboxcraft_engine::Session;
use inboxcraft_model::Role as Folder;
use inboxcraft_store::{Memory, StoreConfig};
use inboxcraft_time::{FakeClock, Timestamp};

use super::InboxApp;
use crate::mailbox::{Mailbox, Place};
use crate::sample;
use crate::theme::{Mode, ModePreference, PRESETS, Preset, TextScale, Theme};

/// 2026-11-05T09:00:00Z.
fn clock() -> Box<FakeClock> {
    Box::new(FakeClock::new(Timestamp::from_unix(1_793_869_200, 0).unwrap()))
}

fn sample_app() -> InboxApp {
    InboxApp::new(Mailbox::new(sample::session(clock()).unwrap(), clock()))
}

fn harness_with(app: InboxApp, theme: egui::Theme) -> Harness<'static, InboxApp> {
    let mut harness = Harness::builder().with_size(vec2(1100.0, 720.0)).with_theme(theme).build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app);
    harness.run();
    harness
}

fn harness() -> Harness<'static, InboxApp> {
    harness_with(sample_app(), egui::Theme::Light)
}

#[test]
fn the_unified_inbox_lists_its_rows_by_sender_subject_and_date() {
    let harness = harness();
    harness.get_by_label("Odile Brannock, Allotment rota for November, 08:35, unread");
    harness.get_by_label("Petra Harbour, Draft logo for the bakery, 08:10, unread, flagged");
    harness.get_by_label("Inbox");
    harness.get_by_label("All accounts · 11 messages");
    assert_eq!(harness.get_all_by_role(Role::ListBoxOption).count(), 11);
}

#[test]
fn every_control_has_an_accessible_name_in_both_modes() {
    for theme in [egui::Theme::Light, egui::Theme::Dark] {
        let mut harness = harness_with(sample_app(), theme);
        harness.check_accessibility();
        harness.state_mut().search("logo");
        harness.run();
        harness.check_accessibility();
        harness.get_by_label("Accounts").click();
        harness.run();
        harness.check_accessibility();
    }
}

#[test]
fn the_theme_follows_the_mode() {
    let harness = harness_with(sample_app(), egui::Theme::Dark);
    assert_eq!(Mode::of(&harness.ctx), Mode::Dark);
    assert_eq!(harness.state().mode(), ModePreference::System);
    assert_eq!(*harness.state().preset(), Preset::default());
    assert_eq!(harness.ctx.style_of(egui::Theme::Dark).visuals.panel_fill, Theme::new(Mode::Dark, &Preset::default()).palette.panel);
}

#[test]
fn a_new_preset_and_mode_show_from_the_next_frame() {
    let mut harness = harness_with(sample_app(), egui::Theme::Light);
    let citrus = *Preset::named("citrus").unwrap();
    harness.state_mut().set_preset(citrus);
    harness.state_mut().set_mode(ModePreference::Dark);
    harness.run();
    assert_eq!(Mode::of(&harness.ctx), Mode::Dark);
    assert_eq!(harness.ctx.style_of(egui::Theme::Light).visuals.panel_fill, Theme::new(Mode::Light, &citrus).palette.panel);
    assert_eq!(harness.ctx.style_of(egui::Theme::Dark).visuals.panel_fill, Theme::new(Mode::Dark, &citrus).palette.panel);
    harness.state_mut().set_mode(ModePreference::Light);
    harness.run();
    assert_eq!(Mode::of(&harness.ctx), Mode::Light);
    harness.state_mut().set_mode(ModePreference::System);
    harness.run();
    assert_eq!(harness.ctx.options(|options| options.theme_preference), egui::ThemePreference::System);
}

/// The font of the first text section of the frame that starts with `start`.
fn font_of(harness: &Harness<'static, InboxApp>, start: &str) -> egui::FontId {
    flat(&harness.output().shapes)
        .into_iter()
        .find_map(|(_, shape)| match shape {
            egui::Shape::Text(text) => text
                .galley
                .job
                .sections
                .iter()
                .find(|section| {
                    text.galley.job.text.get(section.byte_range.start.0..section.byte_range.end.0).is_some_and(|text| text.starts_with(start))
                })
                .map(|section| section.format.font_id.clone()),
            _ => None,
        })
        .unwrap_or_else(|| panic!("no text starts with {start:?}"))
}

#[test]
fn sender_and_subject_are_strong_the_preview_muted_and_unread_rows_heavier() {
    let harness = harness();
    let fonts = Theme::new(Mode::Light, &Preset::default()).fonts;
    // Odile's row is unread, Teodor's is read.
    assert_eq!(font_of(&harness, "Odile Brannock"), fonts.sender_unread.id());
    assert_eq!(font_of(&harness, "Allotment rota for November"), fonts.subject_unread.id());
    assert_eq!(font_of(&harness, "Teodor Lisk"), fonts.sender.id());
    assert_eq!(font_of(&harness, "Photos from the coast walk"), fonts.subject.id());
    assert_ne!(fonts.sender.id(), fonts.sender_unread.id());
    let preview = flat(&harness.output().shapes).into_iter().find_map(|(_, shape)| match shape {
        egui::Shape::Text(text) if text.galley.job.text.starts_with("Teodor Lisk") => None,
        egui::Shape::Text(text) if text.galley.job.text.starts_with("Photos from the coast walk") => {
            text.galley.job.sections.get(1).map(|section| section.format.clone())
        }
        _ => None,
    });
    let preview = preview.unwrap();
    assert_eq!(preview.font_id, fonts.preview.id());
    assert_eq!(preview.color, Theme::new(Mode::Light, &Preset::default()).palette.muted);
}

#[test]
fn a_new_text_scale_shows_from_the_next_frame_and_the_rows_grow_with_it() {
    let mut harness = harness();
    assert_eq!(harness.state().text_scale(), TextScale::default());
    let row = "Odile Brannock, Allotment rota for November, 08:35, unread";
    let before = harness.get_by_label(row).rect().height();
    let large = TextScale::from_percent(130).unwrap();
    harness.state_mut().set_text_scale(large);
    harness.run();
    let body = harness.ctx.style_of(egui::Theme::Light).text_styles.get(&egui::TextStyle::Body).map(|font| font.size);
    assert_eq!(body, Some(Theme::new(Mode::Light, &Preset::default()).with_text_scale(large).fonts.body.size));
    assert_eq!(font_of(&harness, "Odile Brannock").size, Theme::new(Mode::Light, &Preset::default()).with_text_scale(large).fonts.sender_unread.size);
    let after = harness.get_by_label(row).rect().height();
    // The two lines grow by 30 %, the air around them stays.
    assert!(after > before * 1.15, "the row follows the font: {before} then {after}");
}

#[test]
fn the_folder_pane_names_views_and_folders_with_their_unread_counts() {
    let harness = harness();
    harness.get_by_label("Inbox, 4 unread");
    harness.get_by_label("Junk, 1 unread");
    harness.get_by_label("Inbox in Personal, 2 unread");
    harness.get_by_label("Inbox in Studio, 2 unread");
    harness.get_by_label("Harbour Bakery in Studio");
    harness.get_by_label("mira@example.org");
    harness.get_by_label("holloway.studio@example.org");
}

#[test]
fn clicking_a_folder_or_a_view_opens_it() {
    let mut harness = harness();
    harness.get_by_label("Receipts in Personal").click();
    harness.run();
    assert_eq!(harness.state().mailbox().place(), Place::Folder(7));
    harness.get_by_label("Paper & Pine, Receipt for your order 4417, Friday");
    harness.get_by_role_and_label(Role::Button, "Sent").click();
    harness.run();
    assert_eq!(harness.state().mailbox().place(), Place::View(Folder::Sent));
    assert!(harness.get_by_role_and_label(Role::Button, "Sent").accesskit_node().toggled() == Some(Toggled::True));
}

#[test]
fn the_account_picker_narrows_to_one_account_with_its_address_in_the_header() {
    let mut harness = harness();
    harness.get_by_label("Accounts").click();
    harness.run();
    harness.get_by_label("Studio (holloway.studio@example.org)").click();
    harness.run();
    assert_eq!(harness.state().mailbox().narrowed(), Some(2));
    harness.get_by_label("holloway.studio@example.org · 4 messages");
    assert!(harness.query_by_label("Inbox in Personal, 2 unread").is_none());
    harness.get_by_label("Accounts").click();
    harness.run();
    harness.get_by_label("All accounts").click();
    harness.run();
    assert_eq!(harness.state().mailbox().narrowed(), None);
}

#[test]
fn typing_in_the_search_field_searches_and_clear_ends_it() {
    let mut harness = harness();
    harness.get_by_role(Role::TextInput).focus();
    harness.run();
    harness.get_by_role(Role::TextInput).type_text("logo");
    harness.run();
    assert_eq!(harness.state().mailbox().query(), "logo");
    harness.get_by_label("Search");
    harness.get_by_label("All accounts · 2 messages");
    harness.get_by_label("Clear").click();
    harness.run();
    assert_eq!(harness.state().mailbox().query(), "");
    assert_eq!(harness.state().search_text(), "");
    assert_eq!(harness.state().mailbox().messages().len(), 11);
}

#[test]
fn ctrl_f_focuses_the_search_and_escape_empties_it() {
    let mut harness = harness();
    harness.key_press_modifiers(Modifiers::COMMAND, Key::F);
    harness.run();
    assert!(harness.get_by_role(Role::TextInput).is_focused());
    harness.get_by_role(Role::TextInput).type_text("odile");
    harness.run();
    assert_eq!(harness.state().mailbox().messages().len(), 3);
    harness.key_press(Key::Escape);
    harness.run();
    assert_eq!(harness.state().mailbox().query(), "");
}

#[test]
fn opening_a_folder_empties_the_search_field() {
    let mut harness = harness();
    harness.state_mut().search("logo");
    harness.run();
    harness.get_by_role_and_label(Role::Button, "Archive").click();
    harness.run();
    assert_eq!(harness.state().search_text(), "");
    assert_eq!(harness.state().mailbox().query(), "");
}

#[test]
fn a_search_without_results_and_an_empty_folder_say_so() {
    let mut harness = harness();
    harness.state_mut().search("zeppelin");
    harness.run();
    harness.get_by_label("No messages match");
    harness.get_by_label("No subject, sender or recipient holds “zeppelin”.");
    harness.state_mut().mailbox_mut().open(Place::Folder(8));
    harness.run();
    harness.get_by_label("Nothing here");
    harness.get_by_label("All accounts · 0 messages");
}

#[test]
fn clicking_a_row_selects_it() {
    let mut harness = harness();
    harness.get_by_label("Teodor Lisk, Photos from the coast walk, 04:00, flagged").click();
    harness.run();
    let selected = harness.state().mailbox().selected().unwrap();
    let row = harness.state().mailbox().messages().iter().find(|row| row.id == selected).unwrap();
    assert_eq!(row.subject, "Photos from the coast walk");
    assert!(harness.get_by_label("Teodor Lisk, Photos from the coast walk, 04:00, flagged").accesskit_node().toggled() == Some(Toggled::True));
}

#[test]
fn hovering_rows_and_folders_draws_without_trouble() {
    let mut harness = harness();
    let row = harness.get_by_label("Odile Brannock, Allotment rota for November, 08:35, unread").rect().center();
    harness.hover_at(row);
    harness.run();
    let folder = harness.get_by_label("Receipts in Personal").rect().center();
    harness.hover_at(folder);
    harness.run();
    harness.get_by_label("Receipts in Personal").focus();
    harness.run();
    harness.check_accessibility();
}

#[test]
fn an_error_shows_in_a_banner_until_dismissed() {
    let mut harness = harness();
    harness.state_mut().mailbox_mut().open(Place::Folder(4242));
    harness.run();
    let error = harness.state().mailbox().error().unwrap().to_owned();
    harness.get_by_label(&error);
    harness.get_by_label("Dismiss").click();
    harness.run();
    assert_eq!(harness.state().mailbox().error(), None);
}

#[test]
fn without_accounts_the_window_says_how_to_look_around() {
    let session = Session::new(Box::new(Memory::new(StoreConfig::default())), clock()).unwrap();
    let harness = harness_with(InboxApp::new(Mailbox::new(session, clock())), egui::Theme::Dark);
    harness.get_by_label("No accounts yet");
    harness.get_by_label_contains("--sample");
    assert!(harness.query_by_label("Accounts").is_none());
}

#[test]
fn the_app_shows_its_debug_form_without_message_text() {
    let app = sample_app();
    let shown = format!("{app:?}");
    assert!(shown.starts_with("InboxApp { mailbox: Mailbox {"), "{shown}");
    assert!(!shown.contains("Allotment"), "{shown}");
}

fn preset_app(preset: Preset) -> InboxApp {
    let mut app = sample_app();
    app.set_preset(preset);
    app
}

#[test]
fn nothing_repaints_while_nothing_moves() {
    for preset in PRESETS {
        for theme in [egui::Theme::Light, egui::Theme::Dark] {
            let mut harness = harness_with(preset_app(preset), theme);
            // Settled: one more run is a single frame that asks for no other.
            assert_eq!(harness.run(), 1, "{} {theme:?}: idle frames", preset.id);
            let kept = crate::gradient::kept(&harness.ctx);
            let mut asked = 0;
            for _ in 0..10 {
                harness.step();
                asked += usize::from(harness.ctx.has_requested_repaint());
            }
            assert_eq!(asked, 0, "{} {theme:?}: repaints asked for while idle", preset.id);
            assert_eq!(crate::gradient::kept(&harness.ctx), kept, "no picture rasterised again");
            // The same with a message open in the reader.
            harness.get_by_label("Teodor Lisk, Photos from the coast walk, 04:00, flagged").click();
            harness.run();
            assert_eq!(harness.run(), 1, "{} {theme:?}: idle with the reader", preset.id);
        }
    }
}

/// The bytes of every texture the window holds.
fn texture_bytes(harness: &Harness<'static, InboxApp>) -> usize {
    harness.ctx.tex_manager().read().allocated().map(|(_, meta)| meta.bytes_used()).sum()
}

#[test]
fn resizing_the_window_rasterises_little_and_gives_the_memory_back() {
    for scale in [1.0, 2.0] {
        let mut harness = Harness::builder()
            .with_size(vec2(1100.0, 720.0))
            .with_pixels_per_point(scale)
            .build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), sample_app());
        harness.run();
        harness.get_by_label("Teodor Lisk, Photos from the coast walk, 04:00, flagged").click();
        harness.run();
        let idle = texture_bytes(&harness);
        let mut most = 0;
        for step in 0..20 {
            let before = crate::gradient::rasterised(&harness.ctx);
            harness.set_size(vec2(1100.0 + 3.0 * step as f32, 720.0 + 2.0 * step as f32));
            harness.step();
            most = most.max(crate::gradient::rasterised(&harness.ctx) - before);
        }
        assert!(most <= 2, "at scale {scale}: {most} pictures rasterised in one frame of the resize");
        harness.set_size(vec2(1100.0, 720.0));
        harness.run();
        let after = texture_bytes(&harness);
        assert!(after <= idle + idle / 4, "at scale {scale}: {after} bytes kept after the resize, {idle} before");
    }
}

/// Whether `shape` lets what is under it show through.
fn translucent(shape: &egui::Shape) -> bool {
    let see_through = |colour: egui::Color32| colour.a() < 255 && colour != egui::Color32::TRANSPARENT;
    match shape {
        egui::Shape::Rect(rect) => rect.brush.is_some() || see_through(rect.fill) || (rect.stroke.width > 0.0 && see_through(rect.stroke.color)),
        egui::Shape::Path(path) => {
            see_through(path.fill)
                || (path.stroke.width > 0.0
                    && match &path.stroke.color {
                        egui::epaint::ColorMode::Solid(colour) => see_through(*colour),
                        egui::epaint::ColorMode::UV(_) => true,
                    })
        }
        egui::Shape::Circle(circle) => see_through(circle.fill),
        egui::Shape::LineSegment { stroke, .. } => see_through(stroke.color),
        egui::Shape::Mesh(mesh) => mesh.vertices.iter().any(|vertex| see_through(vertex.color)),
        _ => false,
    }
}

/// Every shape of `shapes`, groups opened, with its clip, in paint order.
fn flat(shapes: &[egui::epaint::ClippedShape]) -> Vec<(egui::Rect, egui::Shape)> {
    fn open(clip: egui::Rect, shape: &egui::Shape, out: &mut Vec<(egui::Rect, egui::Shape)>) {
        if let egui::Shape::Vec(inner) = shape {
            inner.iter().for_each(|shape| open(clip, shape, out));
        } else {
            out.push((clip, shape.clone()));
        }
    }
    let mut out = Vec::new();
    shapes.iter().for_each(|clipped| open(clipped.clip_rect, &clipped.shape, &mut out));
    out
}

/// The translucent shapes laid over text drawn before them.
fn translucency_over_text(harness: &Harness<'static, InboxApp>) -> Vec<String> {
    let mut texts: Vec<egui::Rect> = Vec::new();
    let mut found = Vec::new();
    for (clip, shape) in flat(&harness.output().shapes) {
        let rect = shape.visual_bounding_rect().intersect(clip);
        if !rect.is_positive() {
            continue;
        }
        if let egui::Shape::Text(_) = shape {
            texts.push(rect);
        } else if translucent(&shape) && texts.iter().any(|text| text.intersect(rect).is_positive()) {
            found.push(format!("{rect:?}: {shape:?}").chars().take(200).collect());
        }
    }
    found
}

#[test]
fn translucency_is_never_laid_over_text() {
    for preset in PRESETS {
        for theme in [egui::Theme::Light, egui::Theme::Dark] {
            let mut harness = harness_with(preset_app(preset), theme);
            let name = format!("{} {theme:?}", preset.id);
            assert_eq!(translucency_over_text(&harness), Vec::<String>::new(), "{name}: the list");
            harness.hover_at(harness.get_by_label("Odile Brannock, Allotment rota for November, 08:35, unread").rect().center());
            harness.run();
            assert_eq!(translucency_over_text(&harness), Vec::<String>::new(), "{name}: a hovered row");
            harness.get_by_label("Teodor Lisk, Photos from the coast walk, 04:00, flagged").click();
            harness.run();
            assert_eq!(translucency_over_text(&harness), Vec::<String>::new(), "{name}: the reader");
            harness.state_mut().mailbox_mut().select(None);
            harness.state_mut().search("zeppelin");
            harness.run();
            assert_eq!(translucency_over_text(&harness), Vec::<String>::new(), "{name}: an empty search");
        }
    }
}

#[test]
fn the_window_is_glass_over_colour_fields() {
    let mut harness = harness_with(sample_app(), egui::Theme::Dark);
    harness.get_by_label("Teodor Lisk, Photos from the coast walk, 04:00, flagged").click();
    harness.run();
    let shapes = flat(&harness.output().shapes);
    let fields = shapes.iter().position(|(_, shape)| matches!(shape, egui::Shape::Rect(rect) if rect.brush.is_some()));
    let text = shapes.iter().position(|(_, shape)| matches!(shape, egui::Shape::Text(_)));
    assert!(fields.is_some() && fields < text, "the colour fields come before any text: {fields:?} {text:?}");
    let glass = Theme::new(Mode::Dark, &Preset::default()).palette.glass;
    let layers = shapes.iter().filter(|(_, shape)| matches!(shape, egui::Shape::Rect(rect) if rect.fill == glass)).count();
    assert_eq!(layers, 3, "the folder pane, the list and the reader are glass");
}
