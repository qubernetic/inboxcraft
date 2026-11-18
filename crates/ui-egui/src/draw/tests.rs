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
use crate::iconset::IconId;
use crate::mailbox::{InboxView, Mailbox, Place};
use crate::sample;
use crate::theme::{Mode, ModePreference, TextScale, Theme, contrast};

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

/// Whether the last frame of `output` drew `id`: its glyph in the icon family.
pub(super) fn drew_icon(output: &egui::FullOutput, id: IconId) -> bool {
    fn texts<'a>(shape: &'a egui::Shape, found: &mut Vec<&'a egui::epaint::TextShape>) {
        match shape {
            egui::Shape::Text(text) => found.push(text),
            egui::Shape::Vec(shapes) => shapes.iter().for_each(|shape| texts(shape, found)),
            _ => {}
        }
    }
    let front = crate::iconset::glyph(id);
    let family = crate::theme::icon_font(1.0).family;
    let mut found = Vec::new();
    output.shapes.iter().for_each(|clipped| texts(&clipped.shape, &mut found));
    found.iter().any(|text| {
        text.galley.job.text == front.to_string() && text.galley.job.sections.iter().all(|section| section.format.font_id.family == family)
    })
}

/// The rect strokes of `colour` the last frame of `output` drew round or inside `near` (grown by 4 points): for each,
/// the outer bound of its line, its width, and whether its clip rect holds that bound whole.
pub(super) fn strokes_round(output: &egui::FullOutput, colour: egui::Color32, near: egui::Rect) -> Vec<(egui::Rect, f32, bool)> {
    fn rects<'a>(shape: &'a egui::Shape, found: &mut Vec<&'a egui::epaint::RectShape>) {
        match shape {
            egui::Shape::Rect(rect) => found.push(rect),
            egui::Shape::Vec(shapes) => shapes.iter().for_each(|shape| rects(shape, found)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    for clipped in &output.shapes {
        let mut found = Vec::new();
        rects(&clipped.shape, &mut found);
        for rect in found {
            if rect.stroke.color != colour || rect.stroke.width <= 0.0 {
                continue;
            }
            let width = rect.stroke.width;
            let outer = match rect.stroke_kind {
                egui::StrokeKind::Inside => rect.rect,
                egui::StrokeKind::Middle => rect.rect.expand(width / 2.0),
                egui::StrokeKind::Outside => rect.rect.expand(width),
            };
            if near.expand(4.0).contains_rect(outer) && outer.intersects(near) {
                out.push((outer, width, clipped.clip_rect.expand(0.01).contains_rect(outer)));
            }
        }
    }
    out
}

#[test]
fn a_flagged_row_shows_the_flag_icon() {
    let harness = harness();
    harness.get_by_label("Petra Harbour, Draft logo for the bakery, 08:10, unread, flagged");
    assert!(drew_icon(harness.output(), IconId::Flag));
}

#[test]
fn a_mark_in_a_row_is_14_points_and_an_icon_in_a_button_16() {
    let harness = harness();
    let theme = Theme::new(Mode::Light, TextScale::default());
    let sizes = |id: IconId| -> Vec<f32> {
        let glyph = crate::iconset::glyph(id).to_string();
        flat(&harness.output().shapes)
            .into_iter()
            .filter_map(|(_, shape)| match shape {
                egui::Shape::Text(text) if text.galley.job.text == glyph => {
                    text.galley.job.sections.first().map(|section| section.format.font_id.size)
                }
                _ => None,
            })
            .collect()
    };
    let flags = sizes(IconId::Flag);
    assert!(!flags.is_empty(), "the flagged rows draw their flag");
    assert!(flags.iter().all(|size| *size == theme.mark_side()), "{flags:?}");
    assert_eq!(theme.mark_side(), 14.0);
    let compose = sizes(IconId::Compose);
    assert_eq!(compose, vec![theme.icon_side()], "the compose button's icon");
}

#[test]
fn a_mail_row_and_a_text_button_keep_the_design_s_heights_at_every_text_scale() {
    let mut harness = harness();
    let row = "Odile Brannock, Allotment rota for November, 08:35, unread";
    for (percent, mail_row, button) in [(100, 56.0, 32.0), (130, 72.0, 42.0)] {
        let scale = TextScale::from_percent(percent).unwrap();
        harness.state_mut().set_text_scale(scale);
        harness.run();
        let theme = Theme::new(Mode::Light, scale);
        assert_eq!(theme.boxes().mail_row, mail_row);
        assert_eq!(harness.get_by_label(row).rect().height(), mail_row, "{percent} %");
        assert_eq!(harness.get_by_role_and_label(Role::Button, "Review").rect().height(), button, "{percent} %");
    }
}

#[test]
fn the_unified_inbox_lists_its_rows_by_sender_subject_and_date() {
    let mut app = sample_app();
    app.mailbox_mut().set_inbox_view(InboxView::List);
    // Tall enough for every row under the header and its pills.
    let mut harness = Harness::builder().with_size(vec2(1100.0, 800.0)).build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app);
    harness.run();
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
    assert_eq!(harness.ctx.style_of(egui::Theme::Dark).visuals.panel_fill, Theme::new(Mode::Dark, TextScale::default()).palette.bg);
}

#[test]
fn a_new_mode_shows_from_the_next_frame() {
    let mut harness = harness_with(sample_app(), egui::Theme::Light);
    harness.state_mut().set_mode(ModePreference::Dark);
    harness.run();
    assert_eq!(Mode::of(&harness.ctx), Mode::Dark);
    assert_eq!(harness.ctx.style_of(egui::Theme::Light).visuals.panel_fill, Theme::new(Mode::Light, TextScale::default()).palette.bg);
    assert_eq!(harness.ctx.style_of(egui::Theme::Dark).visuals.panel_fill, Theme::new(Mode::Dark, TextScale::default()).palette.bg);
    harness.state_mut().set_mode(ModePreference::Light);
    harness.run();
    assert_eq!(Mode::of(&harness.ctx), Mode::Light);
    harness.state_mut().set_mode(ModePreference::System);
    harness.run();
    assert_eq!(harness.ctx.options(|options| options.theme_preference), egui::ThemePreference::System);
}

/// The font of the first text section of the frame that starts with `start`.
pub(super) fn font_of(harness: &Harness<'static, InboxApp>, start: &str) -> egui::FontId {
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
    let fonts = Theme::new(Mode::Light, TextScale::default()).fonts;
    // Odile's row is unread, Teodor's is read.
    // Unread shows on the first line; the second line (subject and preview) is the caption either way.
    assert_eq!(font_of(&harness, "Odile Brannock"), fonts.strong.id());
    assert_eq!(font_of(&harness, "Allotment rota for November"), fonts.caption.id());
    assert_eq!(font_of(&harness, "Teodor Lisk"), fonts.body.id());
    assert_eq!(font_of(&harness, "Photos from the coast walk"), fonts.caption.id());
    assert_ne!(fonts.body.id(), fonts.strong.id());
    // Line two is one run, the subject and the preview: `muted` on a read row, `ink` on an unread one.
    let palette = Theme::new(Mode::Light, TextScale::default()).palette;
    let line_two = |start: &str| {
        flat(&harness.output().shapes).into_iter().find_map(|(_, shape)| match shape {
            egui::Shape::Text(text) if text.galley.job.text.starts_with(start) => {
                Some((text.galley.job.sections.len(), text.galley.job.sections.first().map(|section| section.format.color)))
            }
            _ => None,
        })
    };
    assert_eq!(line_two("Photos from the coast walk"), Some((1, Some(palette.muted))));
    assert_eq!(line_two("Allotment rota for November"), Some((1, Some(palette.ink))));
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
    assert_eq!(body, Some(Theme::new(Mode::Light, TextScale::default()).with_text_scale(large).fonts.body.size));
    assert_eq!(font_of(&harness, "Odile Brannock").size, Theme::new(Mode::Light, TextScale::default()).with_text_scale(large).fonts.strong.size);
    let after = harness.get_by_label(row).rect().height();
    // The two lines grow by 30 %, the air around them stays.
    assert!(after > before * 1.15, "the row follows the font: {before} then {after}");
}

#[test]
fn the_folder_pane_names_views_and_folders_with_their_unread_counts() {
    let harness = harness();
    harness.get_by_label("Inbox, 4 unread");
    harness.get_by_label("Junk, 1 unread");
    // An account lists only its own folders; its inbox is in the unified view above.
    assert!(harness.query_by_label("Inbox in Personal, 2 unread").is_none());
    harness.get_by_label("Receipts in Personal");
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
    harness.get_by_label_contains("holloway.studio@example.org · ");
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
fn a_rebound_find_key_focuses_the_search_and_the_old_one_does_nothing() {
    let mut harness = harness();
    harness.state_mut().keymap_mut().bind(crate::keymap::FIND, egui::KeyboardShortcut::new(Modifiers::COMMAND, Key::K));
    harness.key_press_modifiers(Modifiers::COMMAND, Key::F);
    harness.run();
    assert!(!harness.get_by_role(Role::TextInput).is_focused());
    harness.key_press_modifiers(Modifiers::COMMAND, Key::K);
    harness.run();
    assert!(harness.get_by_role(Role::TextInput).is_focused());
}

#[test]
fn the_window_installs_its_reduced_motion_flag_every_frame() {
    let mut harness = harness();
    assert!(!harness.state().reduce_motion(), "motion is on where the system cannot be read");
    assert!(!crate::motion::reduced(&harness.ctx));
    harness.state_mut().set_reduce_motion(true);
    harness.run();
    assert!(harness.state().reduce_motion());
    assert!(crate::motion::reduced(&harness.ctx));
    assert_eq!(harness.ctx.global_style().animation_time, 0.0);
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
    harness.get_by_label("No messages match.");
    fn holds(shape: &egui::Shape, wanted: &str) -> bool {
        match shape {
            egui::Shape::Text(text) => text.galley.text().starts_with(wanted),
            egui::Shape::Vec(shapes) => shapes.iter().any(|shape| holds(shape, wanted)),
            _ => false,
        }
    }
    let shown = |harness: &Harness<'static, InboxApp>, wanted: &str| harness.output().shapes.iter().any(|clipped| holds(&clipped.shape, wanted));
    assert!(shown(&harness, "No subject, sender or recipient in Inbox holds “zeppelin”."));
    assert!(shown(&harness, "INBOX · SEARCH"));
    harness.state_mut().mailbox_mut().open(Place::Folder(8));
    harness.run();
    harness.get_by_label("This folder holds no messages.");
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
    harness.get_by_label("dismiss").click();
    harness.run();
    assert_eq!(harness.state().mailbox().error(), None);
}

#[test]
fn without_accounts_the_window_says_how_to_look_around() {
    let session = Session::new(Box::new(Memory::new(StoreConfig::default())), clock()).unwrap();
    let harness = harness_with(InboxApp::new(Mailbox::new(session, clock())), egui::Theme::Dark);
    harness.get_by_label("This window cannot add an account.");
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

#[test]
fn nothing_repaints_while_nothing_moves() {
    for theme in [egui::Theme::Light, egui::Theme::Dark] {
        let mut harness = harness_with(sample_app(), theme);
        // Settled: one more run is a single frame that asks for no other.
        assert_eq!(harness.run(), 1, "{theme:?}: idle frames");
        let mut asked = 0;
        for _ in 0..10 {
            harness.step();
            asked += usize::from(harness.ctx.has_requested_repaint());
        }
        assert_eq!(asked, 0, "{theme:?}: repaints asked for while idle");
        // The same with a message open in the reader.
        harness.get_by_label("Teodor Lisk, Photos from the coast walk, 04:00, flagged").click();
        harness.run();
        assert_eq!(harness.run(), 1, "{theme:?}: idle with the reader");
    }
}

/// The bytes of every texture the window holds.
fn texture_bytes(harness: &Harness<'static, InboxApp>) -> usize {
    harness.ctx.tex_manager().read().allocated().map(|(_, meta)| meta.bytes_used()).sum()
}

#[test]
fn resizing_the_window_gives_the_memory_back() {
    for scale in [1.0, 2.0] {
        let mut harness = Harness::builder()
            .with_size(vec2(1100.0, 720.0))
            .with_pixels_per_point(scale)
            .build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), sample_app());
        harness.run();
        harness.get_by_label("Teodor Lisk, Photos from the coast walk, 04:00, flagged").click();
        harness.run();
        let idle = texture_bytes(&harness);
        for step in 0..20 {
            harness.set_size(vec2(1100.0 + 3.0 * step as f32, 720.0 + 2.0 * step as f32));
            harness.step();
        }
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
/// The galley of the first text that starts with `start`.
pub(super) fn galley_of(harness: &Harness<'static, InboxApp>, start: &str) -> std::sync::Arc<egui::Galley> {
    flat(&harness.output().shapes)
        .into_iter()
        .find_map(|(_, shape)| match shape {
            egui::Shape::Text(text) if text.galley.job.text.starts_with(start) => Some(text.galley),
            _ => None,
        })
        .unwrap_or_else(|| panic!("no text starts with {start:?}"))
}

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

/// The translucent shapes laid over text drawn before them. One that lies wholly on an opaque rectangle drawn after
/// that text (a float's edge on its own solid fill) is laid over the rectangle, not over the text.
fn translucency_over_text(harness: &Harness<'static, InboxApp>) -> Vec<String> {
    let mut texts: Vec<egui::Rect> = Vec::new();
    // Each opaque rectangle, with how many texts were drawn before it.
    let mut covers: Vec<(usize, egui::Rect)> = Vec::new();
    let mut found = Vec::new();
    for (clip, shape) in flat(&harness.output().shapes) {
        let rect = shape.visual_bounding_rect().intersect(clip);
        if !rect.is_positive() {
            continue;
        }
        if let egui::Shape::Text(_) = shape {
            texts.push(rect);
            continue;
        }
        if let egui::Shape::Rect(filled) = &shape
            && filled.fill.a() == 255
            && filled.brush.is_none()
        {
            covers.push((texts.len(), rect));
        }
        let covered = |index: usize| covers.iter().any(|(before, cover)| index < *before && cover.expand(1.0).contains_rect(rect));
        if translucent(&shape) && texts.iter().enumerate().any(|(index, text)| text.intersect(rect).is_positive() && !covered(index)) {
            found.push(format!("{rect:?}: {shape:?}").chars().take(200).collect());
        }
    }
    found
}

#[test]
fn translucency_is_never_laid_over_text() {
    {
        for theme in [egui::Theme::Light, egui::Theme::Dark] {
            let mut harness = harness_with(sample_app(), theme);
            let name = format!("{theme:?}");
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
fn the_window_is_flat() {
    for theme in [egui::Theme::Light, egui::Theme::Dark] {
        let mut harness = harness_with(sample_app(), theme);
        harness.get_by_label("Teodor Lisk, Photos from the coast walk, 04:00, flagged").click();
        harness.run();
        let shapes = flat(&harness.output().shapes);
        // A dashed edge (the unavailable Snooze's key chip under the pointer) is drawn as a mesh, pixel by pixel: the
        // design's dashes, not a gradient.
        let dashed: Vec<egui::Rect> = harness
            .ctx
            .data(|data| data.get_temp::<crate::draw::paint::DashedEdges>(crate::draw::paint::dashed_edges_id()))
            .map(|edges| edges.1.iter().map(|(_, rect, _, _)| rect.expand(1.0)).collect())
            .unwrap_or_default();
        let round: Vec<String> = shapes
            .iter()
            .filter_map(|(_, shape)| match shape {
                egui::Shape::Rect(rect)
                    if [rect.corner_radius.nw, rect.corner_radius.ne, rect.corner_radius.sw, rect.corner_radius.se].into_iter().any(|r| r > 2)
                        || rect.brush.is_some() =>
                {
                    Some(format!("{rect:?}"))
                }
                egui::Shape::Mesh(mesh) if dashed.iter().any(|edge| edge.contains_rect(mesh.calc_bounds())) => None,
                egui::Shape::Mesh(mesh) => Some(format!("a mesh {:?}", mesh.calc_bounds())),
                _ => None,
            })
            .collect();
        assert!(round.is_empty(), "{theme:?}: square, flat shapes only:\n{}", round.join("\n"));
        let surface = Theme::new(Mode::of(&harness.ctx), TextScale::default()).palette.surface;
        let panes = shapes.iter().filter(|(_, shape)| matches!(shape, egui::Shape::Rect(rect) if rect.fill == surface)).count();
        assert!(panes >= 3, "{theme:?}: the folder pane, the list and the reader are flat panes: {panes}");
    }
}

/// Whether a flat rect in `fill` covers `at` in the last frame.
fn filled_at(harness: &Harness<'static, InboxApp>, at: egui::Pos2, fill: egui::Color32) -> bool {
    flat(&harness.output().shapes).iter().any(|(_, shape)| matches!(shape, egui::Shape::Rect(rect) if rect.fill == fill && rect.rect.contains(at)))
}

#[test]
fn a_hovered_row_takes_the_fill_of_its_ground() {
    let mut harness = harness_with(sample_app(), egui::Theme::Dark);
    let p = Theme::new(Mode::Dark, TextScale::default()).palette;
    assert!(contrast(p.stage, p.surface) >= 1.15, "hover on a pane shows: {}", contrast(p.stage, p.surface));
    assert!(contrast(p.grid, p.panel) >= 1.05, "hover on the panel shows: {}", contrast(p.grid, p.panel));
    // A list row stands on a pane's `surface`: its hover is the `stage`.
    let row = harness.get_by_label("Odile Brannock, Allotment rota for November, 08:35, unread").rect().center();
    assert!(!filled_at(&harness, row, p.stage), "not hovered yet");
    harness.hover_at(row);
    harness.run();
    assert!(filled_at(&harness, row, p.stage), "a hovered list row is the stage");
    assert!(!filled_at(&harness, row, p.grid));
    // A rail button stands on the `panel`: its hover is the `grid`.
    let calendar = harness.get_by_role_and_label(Role::Button, "Calendar").rect().center();
    harness.hover_at(calendar);
    harness.run();
    assert!(filled_at(&harness, calendar, p.grid), "a hovered rail button is the grid");
    assert!(!filled_at(&harness, calendar, p.stage));
}

/// The physical columns that tall upright lines in `colour` cover within 4 points of `x` in the last frame of
/// `harness`, tessellated without feathering.
fn upright_line_columns(harness: &Harness<'static, InboxApp>, colour: egui::Color32, x: f32) -> std::collections::BTreeSet<i32> {
    let output = harness.output();
    let ppp = output.pixels_per_point;
    let options = egui::epaint::TessellationOptions { feathering: false, ..Default::default() };
    let mut tessellator = egui::epaint::Tessellator::new(ppp, options, [1, 1], Vec::new());
    let mut covered = std::collections::BTreeSet::new();
    for clipped in tessellator.tessellate_shapes(output.shapes.clone()) {
        let egui::epaint::Primitive::Mesh(mesh) = clipped.primitive else { continue };
        for triangle in mesh.indices.chunks(3) {
            let corners: Vec<_> = triangle.iter().map(|at| mesh.vertices[*at as usize]).collect();
            if corners.iter().any(|vertex| vertex.color != colour) {
                continue;
            }
            let span = egui::Rect::from_points(&corners.iter().map(|vertex| vertex.pos).collect::<Vec<_>>());
            if span.width() > 4.0 || span.height() < 200.0 || (span.left() - x).abs() > 4.0 {
                continue;
            }
            covered.extend((span.left() * ppp).round() as i32..(span.right() * ppp).round() as i32);
        }
    }
    covered
}

/// Where the rail ends: its buttons stand in its middle.
fn rail_right(harness: &Harness<'static, InboxApp>, theme: &Theme) -> f32 {
    harness.get_by_role_and_label(Role::Button, "Calendar").rect().center().x + super::pane::rail_width(theme) / 2.0
}

#[test]
fn one_line_stands_between_the_rail_and_the_pane_beside_it() {
    let theme = Theme::new(Mode::Dark, TextScale::default());
    for ppp in [1.0_f32, 1.5] {
        let wide = ppp.round().max(1.0) as usize;
        let mut harness = Harness::builder()
            .with_size(vec2(1100.0, 720.0))
            .with_pixels_per_point(ppp)
            .with_theme(egui::Theme::Dark)
            .build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), sample_app());
        harness.run();
        let seam = rail_right(&harness, &theme);
        let mail = upright_line_columns(&harness, theme.palette.line, seam);
        assert_eq!(mail.len(), wide, "{ppp}: one line between the rail and the folder pane: {mail:?}");
        harness.get_by_role_and_label(Role::Button, "Calendar").click();
        harness.run();
        let calendar = upright_line_columns(&harness, theme.palette.line, seam);
        assert_eq!(calendar.len(), wide, "{ppp}: one line between the rail and the calendar's side: {calendar:?}");
    }
}

#[test]
fn the_rail_keeps_its_line_where_no_pane_stands_beside_it() {
    let theme = Theme::new(Mode::Dark, TextScale::default());
    let session = Session::new(Box::new(Memory::new(StoreConfig::default())), clock()).unwrap();
    let harness = harness_with(InboxApp::new(Mailbox::new(session, clock())), egui::Theme::Dark);
    let columns = upright_line_columns(&harness, theme.palette.line, rail_right(&harness, &theme));
    assert_eq!(columns.len(), 1, "the rail's own edge: {columns:?}");
}

#[test]
fn the_account_picker_opens_by_enter_and_takes_an_account_by_enter() {
    let mut harness = harness();
    let field = harness.get_by_role_and_label(Role::ComboBox, "Accounts");
    assert_eq!(field.accesskit_node().value().as_deref(), Some("All accounts"));
    field.focus();
    harness.run();
    harness.key_down(Key::Enter);
    harness.run();
    harness.key_up(Key::Enter);
    harness.run();
    harness.get_by_role_and_label(Role::MenuItemRadio, "Studio (holloway.studio@example.org)").focus();
    harness.run();
    harness.key_down(Key::Enter);
    harness.run();
    harness.key_up(Key::Enter);
    harness.run();
    assert_eq!(harness.state().mailbox().narrowed(), Some(2));
    assert!(harness.query_by_role_and_label(Role::MenuItemRadio, "All accounts").is_none(), "the list closed");
    assert_eq!(harness.get_by_role_and_label(Role::ComboBox, "Accounts").accesskit_node().value().as_deref(), Some("Studio"));
}

#[test]
fn the_error_banner_s_dismiss_is_a_link_that_enter_presses() {
    let mut harness = harness();
    harness.state_mut().mailbox_mut().open(Place::Folder(4242));
    harness.run();
    assert!(harness.state().mailbox().error().is_some());
    harness.get_by_role_and_label(Role::Button, "dismiss").focus();
    harness.run();
    harness.key_press(Key::Enter);
    harness.run();
    assert_eq!(harness.state().mailbox().error(), None);
}

#[test]
fn the_error_banner_s_dismiss_is_pressed_by_space_as_well() {
    let mut harness = harness();
    harness.state_mut().mailbox_mut().open(Place::Folder(4242));
    harness.run();
    harness.get_by_role_and_label(Role::Button, "dismiss").focus();
    harness.run();
    harness.key_press(Key::Space);
    harness.run();
    assert_eq!(harness.state().mailbox().error(), None);
}

#[test]
fn the_error_banner_carries_the_sheets_error_label_in_minus() {
    for mode in [Mode::Dark, Mode::Light] {
        let theme = Theme::new(mode, TextScale::default());
        let mut harness = Harness::builder()
            .with_size(vec2(1100.0, 720.0))
            .with_theme(if mode == Mode::Dark { egui::Theme::Dark } else { egui::Theme::Light })
            .build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), sample_app());
        harness.state_mut().mailbox_mut().open(Place::Folder(4242));
        harness.run();
        harness.run();
        let error = harness.state().mailbox().error().unwrap().to_owned();
        let banner = harness.get_by_role_and_label(Role::Group, &format!("Error: {error}")).rect();
        let words = harness.get_by_label(&error).rect();
        let label = harness.output().shapes.iter().find_map(|clipped| match &clipped.shape {
            egui::Shape::Text(text) if text.galley.job.text == "ERROR" && banner.contains(text.pos) => Some(text.clone()),
            _ => None,
        });
        let label = label.unwrap_or_else(|| panic!("{mode:?}: no ERROR label in {banner:?}"));
        let colour = label.galley.job.sections.first().map(|section| section.format.color);
        assert!(colour == Some(theme.palette.minus) || label.override_text_color == Some(theme.palette.minus), "{mode:?}");
        assert!(label.galley.rect.translate(label.pos.to_vec2()).right() < words.left(), "{mode:?}: the label before the words");
    }
}

/// The physical rows that long level lines in `colours` cover between `from` and `to` (points) in the last frame of
/// `harness`, tessellated without feathering.
pub(super) fn level_line_rows(
    harness: &Harness<'static, InboxApp>,
    colours: &[egui::Color32],
    from: f32,
    to: f32,
) -> std::collections::BTreeSet<i32> {
    let output = harness.output();
    let ppp = output.pixels_per_point;
    let options = egui::epaint::TessellationOptions { feathering: false, ..Default::default() };
    let mut tessellator = egui::epaint::Tessellator::new(ppp, options, [1, 1], Vec::new());
    let mut covered = std::collections::BTreeSet::new();
    for clipped in tessellator.tessellate_shapes(output.shapes.clone()) {
        let egui::epaint::Primitive::Mesh(mesh) = clipped.primitive else { continue };
        for triangle in mesh.indices.chunks(3) {
            let corners: Vec<_> = triangle.iter().map(|at| mesh.vertices[*at as usize]).collect();
            let Some(first) = corners.first() else { continue };
            if !colours.contains(&first.color) || corners.iter().any(|vertex| vertex.color != first.color) {
                continue;
            }
            let span = egui::Rect::from_points(&corners.iter().map(|vertex| vertex.pos).collect::<Vec<_>>());
            if span.height() > 4.0 || span.width() < 200.0 || span.bottom() < from || span.top() > to {
                continue;
            }
            covered.extend((span.top() * ppp).round() as i32..(span.bottom() * ppp).round() as i32);
        }
    }
    covered
}

#[test]
fn one_line_stands_between_the_error_banner_and_the_panes_under_it() {
    let theme = Theme::new(Mode::Dark, TextScale::default());
    let p = theme.palette;
    for ppp in [1.0_f32, 1.5] {
        let wide = ppp.round().max(1.0) as usize;
        let mut harness = Harness::builder()
            .with_size(vec2(1100.0, 720.0))
            .with_pixels_per_point(ppp)
            .with_theme(egui::Theme::Dark)
            .build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), sample_app());
        harness.state_mut().mailbox_mut().open(Place::Folder(4242));
        harness.run();
        harness.run();
        let error = harness.state().mailbox().error().unwrap().to_owned();
        let text = harness.get_by_label(&error).rect();
        // The sheet's padding: 12 at the sides (from the window's left, where the rail starts), the ERROR label there,
        // the words 12 right of it.
        let window = rail_right(&harness, &theme) - super::pane::rail_width(&theme);
        let label = harness.output().shapes.iter().find_map(|clipped| match &clipped.shape {
            egui::Shape::Text(shape) if shape.galley.job.text == "ERROR" => Some(shape.galley.rect.translate(shape.pos.to_vec2())),
            _ => None,
        });
        let label = label.unwrap_or_else(|| panic!("{ppp}: no ERROR label"));
        assert!((label.left() - window - 12.0).abs() < 0.01, "{ppp}: the label {label:?} in a window from {window}");
        assert!((text.left() - (label.left() + crate::draw::paint::run_box(label.width()) + 12.0)).abs() < 0.01, "{ppp}: {text:?} after {label:?}");
        let dismiss = harness.get_by_role_and_label(Role::Button, "dismiss").rect();
        let rows = level_line_rows(&harness, &[p.line, p.minus], dismiss.bottom(), dismiss.bottom() + 16.0);
        assert_eq!(rows.len(), wide, "{ppp}: one line between the banner and the panes: {rows:?}");
    }
}

#[test]
fn the_error_banner_is_drawn_over_the_window_s_ground() {
    for mode in [Mode::Dark, Mode::Light] {
        let p = Theme::new(mode, TextScale::default()).palette;
        let mut app = sample_app();
        app.mailbox_mut().open(Place::Folder(4242));
        assert!(app.mailbox().error().is_some());
        let picture = crate::shot::render(&mut app, vec2(1100.0, 720.0), 1.0, mode).unwrap();
        let at = |x: usize, y: usize| picture.pixel(x, y).map(|pixel| (pixel.r(), pixel.g(), pixel.b()));
        let rgb = |colour: egui::Color32| Some((colour.r(), colour.g(), colour.b()));
        // Its `panel` ground across the window under its `minus` top line, its line at the foot.
        assert_eq!(at(700, 0), rgb(p.minus), "{mode:?}: the top line");
        assert_eq!(at(700, 4), rgb(p.panel), "{mode:?}: the banner's ground");
        let words = (12..400).any(|x| (8..26).any(|y| at(x, y).is_some_and(|c| Some(c) != rgb(p.panel))));
        assert!(words, "{mode:?}: its words are drawn");
    }
}

#[test]
fn the_account_picker_with_a_stale_account_shows_and_focuses_all_accounts() {
    let mut harness = harness();
    harness.state_mut().mailbox_mut().narrow_stale(4242);
    harness.run();
    let field = harness.get_by_role_and_label(Role::ComboBox, "Accounts");
    assert_eq!(field.accesskit_node().value().as_deref(), Some("All accounts"));
    field.focus();
    harness.run();
    harness.key_press(Key::Enter);
    harness.run();
    harness.run();
    let first = harness.get_by_role_and_label(Role::MenuItemRadio, "All accounts");
    assert!(first.accesskit_node().is_focused(), "the row the field shows takes the focus");
    assert_eq!(first.accesskit_node().toggled(), Some(Toggled::True), "and is the chosen one");
}

#[test]
fn the_line_under_the_error_banner_is_its_minus_edge() {
    let theme = Theme::new(Mode::Dark, TextScale::default());
    let p = theme.palette;
    for ppp in [1.0_f32, 1.5] {
        let wide = ppp.round().max(1.0) as usize;
        let mut harness = Harness::builder()
            .with_size(vec2(1100.0, 720.0))
            .with_pixels_per_point(ppp)
            .with_theme(egui::Theme::Dark)
            .build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), sample_app());
        harness.state_mut().mailbox_mut().open(Place::Folder(4242));
        harness.run();
        harness.run();
        let dismiss = harness.get_by_role_and_label(Role::Button, "dismiss").rect();
        let (from, to) = (dismiss.bottom(), dismiss.bottom() + 16.0);
        assert_eq!(level_line_rows(&harness, &[p.minus], from, to).len(), wide, "{ppp}: the sheet's minus edge all round");
        assert!(level_line_rows(&harness, &[p.line], from, to).is_empty(), "{ppp}: and no pane line beside it");
    }
}
