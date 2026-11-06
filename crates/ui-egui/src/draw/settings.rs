// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The appearance settings page: a preview drawn in the look the controls choose, then one control each for the
//! mode, the accent preset, the custom accent, the typeface, the icon set and the text size, and the "Reduce motion"
//! switch. Wide, the preview stands beside the controls; narrow, above them.

use egui::{Align, Frame, Layout, Rect, Response, RichText, Role, ScrollArea, Sense, Stroke, StrokeKind, TextEdit, Ui, WidgetInfo, pos2, vec2};
use inboxcraft_engine::appearance::{Accent, Appearance, Icons, Motion, ThemeMode, Typeface, WindowFrame};

use super::paint::{line, line_height, selection};
use crate::component::avatar;
use crate::component::button::TextButton;
use crate::component::pill::pill;
use crate::gradient::{self, Gradient, Stop};
use crate::iconset::{IconId, IconSet, Style};
use crate::keymap::Keymap;
use crate::motion;
use crate::settings::{MAX_CHARS, Settings};
use crate::theme::{self, Look, PRESETS, Preset, TextScale, Theme};
use crate::tr;

/// The width of the controls' column, in points.
const COLUMN: f32 = 440.0;

/// The narrowest the preview beside the controls gets, in points.
const PREVIEW: f32 = 300.0;

/// What the owner asked for on the page this frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Asked {
    Mode(ThemeMode),
    Preset(&'static str),
    Custom([String; 4]),
    Font(Typeface),
    Icons(Icons),
    Scale(TextScale),
    ReduceMotion(bool),
    Frame(WindowFrame),
    Close,
}

/// The name of a mode.
fn mode_name(mode: ThemeMode) -> &'static str {
    match mode {
        ThemeMode::System => tr!("System"),
        ThemeMode::Light => tr!("Light"),
        ThemeMode::Dark => tr!("Dark"),
    }
}

/// The name of a shipped preset; the id for one this front end does not name.
fn preset_name(id: &'static str) -> &'static str {
    match id {
        "electric" => tr!("Electric"),
        "aurora" => tr!("Aurora"),
        "citrus" => tr!("Citrus"),
        _ => id,
    }
}

/// The name of a typeface, as it is written.
fn font_name(typeface: Typeface) -> &'static str {
    match typeface {
        Typeface::Figtree => "Figtree",
        Typeface::Inter => "Inter",
    }
}

/// The name of an icon set, as it is written.
fn icons_name(icons: Icons) -> &'static str {
    match icons {
        Icons::Phosphor => "Phosphor",
        Icons::Lucide => "Lucide",
    }
}

/// The preset's gradient from left to right.
fn band(preset: &Preset) -> Gradient {
    let [first, middle, last] = preset.gradient;
    Gradient::Linear { from: pos2(0.0, 0.5), to: pos2(1.0, 0.5), stops: vec![Stop::new(0.0, first), Stop::new(0.5, middle), Stop::new(1.0, last)] }
}

fn heading(ui: &mut Ui, theme: &Theme, text: &str) {
    ui.add_space(theme.spacing.l);
    ui.label(RichText::new(text).font(theme.fonts.heading.id()).color(theme.palette.text));
    ui.add_space(theme.spacing.xs);
}

fn note(ui: &mut Ui, theme: &Theme, text: &str) {
    ui.add(egui::Label::new(RichText::new(text).font(theme.fonts.caption.id()).color(theme.palette.muted)).wrap());
}

/// A row of pills that wraps when it is too wide.
fn pills<T: Copy + PartialEq>(ui: &mut Ui, theme: &Theme, choices: &[(T, &str)], current: T) -> Option<T> {
    let mut chosen = None;
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(theme.spacing.s, theme.spacing.s);
        for (choice, name) in choices {
            if pill(ui, theme, name, *choice == current).clicked() {
                chosen = Some(*choice);
            }
        }
    });
    chosen
}

/// The switch: a track with a knob that slides to the right when it is on; its response is clicked when it was
/// pressed.
fn switch(ui: &mut Ui, theme: &Theme, name: &str, on: bool) -> Response {
    let p = &theme.palette;
    let s = &theme.spacing;
    let height = line_height(ui, theme.fonts.body);
    let galley = line(ui, &[(name, theme.fonts.body, p.text)], ui.available_width());
    let track = vec2(height * 1.8, height);
    let size = vec2(track.x + s.s + galley.size().x, height.max(galley.size().y) + s.xs);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    response.widget_info(|| WidgetInfo::selected(Role::Switch, true, on, name));
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        let track = Rect::from_min_size(pos2(rect.left(), rect.center().y - track.y / 2.0), track);
        let round = track.height() / 2.0;
        if on {
            selection(painter, theme, track, round);
        } else {
            painter.rect_filled(track, round, p.field);
        }
        painter.rect_stroke(track, round, Stroke::new(1.0, p.field_border), StrokeKind::Inside);
        let at = motion::toggle(ui.ctx(), response.id.with("knob"), on, theme.motion.hover_fade);
        let knob = track.height() / 2.0 - 2.0;
        let x = track.left() + round + (track.width() - 2.0 * round) * at;
        painter.circle_filled(pos2(x, track.center().y), knob, if on { p.accent } else { p.muted });
        painter.galley(pos2(track.right() + s.s, rect.center().y - galley.size().y / 2.0), galley, p.text);
        if response.has_focus() {
            painter.rect_stroke(track.expand(2.0), round + 2.0, Stroke::new(2.0, p.focus), StrokeKind::Outside);
        }
    }
    response
}

/// The four colour fields of the custom accent, each with a picker; whether "Use these colours" was pressed.
fn custom(ui: &mut Ui, theme: &Theme, keymap: &Keymap, settings: &mut Settings) -> Option<Asked> {
    let names = [tr!("Gradient start"), tr!("Gradient middle"), tr!("Gradient end"), tr!("Accent")];
    egui::Grid::new("custom-accent").num_columns(3).spacing(vec2(theme.spacing.m, theme.spacing.s)).show(ui, |ui| {
        for (field, name) in settings.colours_mut().iter_mut().zip(names) {
            let label = ui.label(name);
            let mut picked = crate::settings::colour(field).unwrap_or(theme.palette.muted);
            if ui.color_edit_button_srgba(&mut picked).labelled_by(label.id).changed() {
                *field = crate::settings::hex(picked);
            }
            let edit = TextEdit::singleline(field)
                .char_limit(MAX_CHARS)
                .margin(crate::component::field::margin(theme))
                .desired_width(theme.spacing.xl * 4.0)
                .font(theme.fonts.mono.id());
            let _ = ui.add(edit).labelled_by(label.id);
            ui.end_row();
        }
    });
    let custom = settings.custom();
    if custom.is_none() {
        note(ui, theme, tr!("Write each colour as # and six hex digits, like #2f6bff."));
    }
    if let Some(text) = settings.note() {
        note(ui, theme, text);
    }
    ui.add_space(theme.spacing.xs);
    let pressed =
        ui.add_enabled_ui(custom.is_some(), |ui| TextButton::new(tr!("Use these colours")).primary().show(ui, theme, keymap)).inner.clicked();
    custom.filter(|_| pressed).map(Asked::Custom)
}

/// The controls; what was asked for.
fn controls(ui: &mut Ui, theme: &Theme, keymap: &Keymap, settings: &mut Settings, current: &Appearance, reduce: bool) -> Option<Asked> {
    let mut asked = None;
    heading(ui, theme, tr!("Mode"));
    let modes = ThemeMode::ALL.map(|mode| (mode, mode_name(mode)));
    asked = asked.or(pills(ui, theme, &modes, current.theme).map(Asked::Mode));

    heading(ui, theme, tr!("Accent"));
    let chosen = match current.accent {
        Accent::Preset(id) => id,
        Accent::Custom(_) => "custom",
    };
    let typed = settings.custom();
    let draft = settings.draft().unwrap_or_default();
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(theme.spacing.s, theme.spacing.s);
        let side = line_height(ui, theme.fonts.button) + theme.spacing.s;
        for (id, colours) in PRESETS.iter().map(|preset| (preset.id, *preset)).chain([("custom", draft)]) {
            let name = if id == "custom" { tr!("Custom") } else { preset_name(id) };
            // The swatch and its pill wrap together: a new row when the pair would not fit.
            let text = ui.painter().layout_no_wrap(name.to_owned(), theme.fonts.button.id(), theme.palette.text).size().x;
            let pair = side + theme.spacing.xs + text + theme.spacing.m * 2.0 + ui.spacing().item_spacing.x;
            let right = ui.max_rect().right().min(ui.clip_rect().right());
            if right - ui.cursor().min.x < pair && ui.cursor().min.x > ui.max_rect().left() {
                ui.end_row();
            }
            let clicked = ui
                .horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = theme.spacing.xs;
                    let (swatch, _) = ui.allocate_exact_size(vec2(side, side), Sense::hover());
                    gradient::fill(ui.painter(), swatch, side / 2.0, &band(&colours));
                    ui.painter().circle_filled(swatch.center(), side / 6.0, colours.accent);
                    pill(ui, theme, name, chosen == id).clicked()
                })
                .inner;
            if clicked {
                // Custom takes the colours typed below, when they are colours.
                asked = if id == "custom" { typed.clone().map(Asked::Custom) } else { Some(Asked::Preset(id)) };
            }
        }
    });

    heading(ui, theme, tr!("Custom accent"));
    note(ui, theme, tr!("Three colours for the gradient and one for links and accent text; the preview shows them as you type."));
    ui.add_space(theme.spacing.xs);
    asked = asked.or(custom(ui, theme, keymap, settings));

    heading(ui, theme, tr!("Typeface"));
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(theme.spacing.s, theme.spacing.s);
        for typeface in Typeface::ALL {
            // Each name is written in its own typeface.
            if pill(ui, &theme.with_typeface(typeface), font_name(typeface), current.font == typeface).clicked() {
                asked = Some(Asked::Font(typeface));
            }
        }
    });

    heading(ui, theme, tr!("Icons"));
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(theme.spacing.s, theme.spacing.s);
        for icons in Icons::ALL {
            let side = line_height(ui, theme.fonts.button);
            for id in [IconId::Inbox, IconId::Archive, IconId::Snooze] {
                let (rect, _) = ui.allocate_exact_size(vec2(side, side), Sense::hover());
                IconSet::of(icons).paint(ui.painter(), rect, id, Style::Regular, theme.palette.muted);
            }
            if pill(ui, theme, icons_name(icons), current.icons == icons).clicked() {
                asked = Some(Asked::Icons(icons));
            }
            ui.add_space(theme.spacing.m);
        }
    });

    heading(ui, theme, tr!("Text size"));
    let scales: Vec<(TextScale, String)> = TextScale::ALL.iter().map(|scale| (*scale, tr!("{percent} %", percent = scale.percent()))).collect();
    let scales: Vec<(TextScale, &str)> = scales.iter().map(|(scale, name)| (*scale, name.as_str())).collect();
    let now = TextScale::from_percent(current.text_scale).unwrap_or_default();
    asked = asked.or(pills(ui, theme, &scales, now).map(Asked::Scale));

    heading(ui, theme, tr!("Motion"));
    if switch(ui, theme, tr!("Reduce motion"), reduce).clicked() {
        asked = Some(Asked::ReduceMotion(!reduce));
    }
    if current.motion == Motion::System {
        note(ui, theme, tr!("Until you choose, motion follows the system where InboxCraft can read it, and runs elsewhere."));
    }

    heading(ui, theme, tr!("Window frame"));
    let frames = WindowFrame::ALL.map(|frame| (frame, frame_name(frame)));
    asked = asked.or(pills(ui, theme, &frames, current.frame).map(Asked::Frame));
    note(ui, theme, tr!("The system's title bar, the window controls in the top row, or a slim strip of InboxCraft's own."));
    asked
}

/// The name of a window frame.
fn frame_name(frame: WindowFrame) -> &'static str {
    match frame {
        WindowFrame::System => tr!("Title bar"),
        WindowFrame::TopRow => tr!("Top row"),
        WindowFrame::Strip => tr!("Strip"),
    }
}

/// A message row of the preview, drawn like the list's.
fn preview_row(ui: &mut Ui, theme: &Theme, sender: (&str, &str), subject: &str, time: &str, selected: bool, unread: bool) {
    let p = &theme.palette;
    let s = &theme.spacing;
    let f = &theme.fonts;
    let first = line_height(ui, f.sender_unread).max(line_height(ui, f.caption));
    let second = line_height(ui, f.subject);
    let height = first + second + s.s * 1.5;
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::hover());
    if !ui.is_rect_visible(rect) {
        return;
    }
    let painter = ui.painter();
    if selected {
        selection(painter, theme, rect, s.corner);
    }
    let side = (first + second).min(height - s.s);
    let face = Rect::from_min_size(pos2(rect.left() + s.m, rect.center().y - side / 2.0), vec2(side, side));
    avatar::paint(painter, theme, face, sender.0, sender.1);
    if unread {
        painter.circle_filled(pos2(rect.left() + s.xs + 1.0, rect.center().y), 3.5, p.unread);
    }
    let left = face.right() + s.m;
    let time = line(ui, &[(time, f.caption, p.muted)], rect.width());
    let right = rect.right() - s.m;
    let top = rect.top() + s.s * 0.75;
    painter.galley(pos2(right - time.size().x, top + (first - time.size().y) / 2.0), time.clone(), p.muted);
    let sender_font = if unread { f.sender_unread } else { f.sender };
    let name = line(ui, &[(sender.0, sender_font, p.text)], (right - time.size().x - s.s - left).max(1.0));
    painter.galley(pos2(left, top + (first - name.size().y) / 2.0), name, p.text);
    let subject = line(ui, &[(subject, if unread { f.subject_unread } else { f.subject }, p.text)], (right - left).max(1.0));
    painter.galley(pos2(left, top + first), subject, p.text);
}

/// The preview: the preset's band, two rows, a few icons and a primary button, in `theme`.
fn preview(ui: &mut Ui, theme: &Theme) {
    let p = &theme.palette;
    let s = &theme.spacing;
    ui.label(RichText::new(tr!("Preview")).font(theme.fonts.heading.id()).color(p.text));
    ui.add_space(s.xs);
    Frame::new().fill(p.surface).stroke(Stroke::new(1.0, p.line)).corner_radius(s.card).inner_margin(super::margin(s.m)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        let title = line_height(ui, theme.fonts.title);
        let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), title + s.m * 2.0), Sense::hover());
        if ui.is_rect_visible(rect) {
            gradient::fill(ui.painter(), rect, s.corner, &band(&theme.preset));
            let ink = theme::ink_over(&theme.preset.gradient);
            let galley = line(ui, &[("InboxCraft", theme.fonts.title, ink)], rect.width() - s.m * 2.0);
            ui.painter().galley(pos2(rect.left() + s.m, rect.center().y - galley.size().y / 2.0), galley, ink);
        }
        ui.add_space(s.s);
        preview_row(ui, theme, ("Mira Castell", "mira@example.org"), tr!("Walk on Saturday?"), "09:12", true, true);
        preview_row(ui, theme, ("Odile Brannock", "odile@example.org"), tr!("Seed swap at the hall"), "08:40", false, true);
        preview_row(ui, theme, ("Paper & Pine", "orders@example.org"), tr!("Your order is on its way"), tr!("Monday"), false, false);
        ui.add_space(s.s);
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(s.s, s.s);
            let side = line_height(ui, theme.fonts.button) + s.s;
            for (id, style) in
                [(IconId::Reply, Style::Regular), (IconId::Archive, Style::Regular), (IconId::Pin, Style::Fill), (IconId::Flag, Style::Fill)]
            {
                let (rect, _) = ui.allocate_exact_size(vec2(side, side), Sense::hover());
                let colour = if style == Style::Fill { p.accent } else { p.text };
                theme.icons.paint(ui.painter(), rect, id, style, colour);
            }
            let _ = pill(ui, theme, tr!("Reply"), true);
            let _ = pill(ui, theme, tr!("Later"), false);
        });
        ui.add_space(s.xs);
        let link = line(ui, &[(tr!("A link looks like this."), theme.fonts.body, p.accent)], ui.available_width());
        let (rect, _) = ui.allocate_exact_size(link.size(), Sense::hover());
        ui.painter().galley(rect.min, link, p.accent);
    });
}

/// Draws the page in `theme`, the preview in the draft's colours while they differ from those in use; returns what
/// was asked for, applied by the caller after the frame's drawing.
pub(super) fn show(ui: &mut Ui, theme: &Theme, keymap: &Keymap, settings: &mut Settings, current: &Appearance, reduce: bool) -> Option<Asked> {
    let mut asked = None;
    ui.horizontal(|ui| {
        ui.label(RichText::new(tr!("Appearance")).font(theme.fonts.title.id()).color(theme.palette.text));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if TextButton::new(tr!("Close")).secondary().show(ui, theme, keymap).clicked() {
                asked = Some(Asked::Close);
            }
        });
    });
    note(ui, theme, tr!("Every change shows at once and is kept for the next start."));
    let shown = match settings.draft() {
        Some(draft) if matches!(current.accent, Accent::Custom(_)) || draft != Preset::of(&current.accent) => {
            Theme::of(theme.mode, &Look { preset: draft, text_scale: theme.text_scale, typeface: theme.typeface, icons: theme.icons })
        }
        _ => *theme,
    };
    ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        let wide = ui.available_width() >= COLUMN + PREVIEW + theme.spacing.xl;
        if wide {
            ui.horizontal_top(|ui| {
                ui.vertical(|ui| {
                    ui.set_width(COLUMN);
                    asked = asked.clone().or(controls(ui, theme, keymap, settings, current, reduce));
                });
                ui.add_space(theme.spacing.xl);
                ui.vertical(|ui| {
                    ui.add_space(theme.spacing.l);
                    preview(ui, &shown);
                });
            });
        } else {
            ui.add_space(theme.spacing.l);
            preview(ui, &shown);
            asked = asked.clone().or(controls(ui, theme, keymap, settings, current, reduce));
        }
    });
    asked
}
