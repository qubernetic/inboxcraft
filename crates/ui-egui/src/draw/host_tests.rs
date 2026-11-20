// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The host, headless: no real window ever opens. A context that embeds viewports (egui's default, the web app) is the
//! fallback; a window of its own is simulated by a renderer that runs the window's pass inside the main one, as eframe
//! does, with input of the window's own.

use std::cell::RefCell;

use egui::{Context, Event, Key, Modifiers, RawInput, Rect, ViewportEvent, ViewportId, ViewportInfo, vec2};
use inboxcraft_engine::appearance::WindowFrame;

use super::{Host, Hosted, Look, Windowed, clean_title, show};
use crate::keymap::{self, Keymap};
use crate::surface::{Facts, Surface, Surfaces};
use crate::theme::{Mode, TextScale, Theme, install_fonts};

thread_local! {
    /// The input the next window pass gets.
    static WINDOW_INPUT: RefCell<RawInput> = RefCell::new(RawInput::default());
}

/// The window's viewport in these tests.
fn window_id() -> ViewportId {
    ViewportId::from_hash_of("host-tests-window")
}

/// A context that opens windows of their own: a renderer on this test's thread runs each window's pass at once, inside
/// the main one, with [`WINDOW_INPUT`] as its input.
fn windowing() -> Context {
    Context::set_immediate_viewport_renderer(|ctx, mut viewport| {
        let mut raw = WINDOW_INPUT.with(|input| std::mem::take(&mut *input.borrow_mut()));
        raw.viewport_id = viewport.ids.this;
        raw.viewports.entry(viewport.ids.this).or_default();
        raw.screen_rect = Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(704.0, 720.0)));
        let mut output = ctx.run_ui(raw, |ui| (viewport.viewport_ui_cb)(ui));
        output.textures_delta.clear();
    });
    let ctx = Context::default();
    install_fonts(&ctx);
    ctx.set_embed_viewports(false);
    ctx
}

fn key(key: Key) -> Event {
    Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE }
}

/// One frame of the main window with `main` as its events and `window` as the window's; `run` shows what it shows.
fn frame<R>(ctx: &Context, main: Vec<Event>, window: Vec<Event>, run: impl FnOnce(&Context) -> R) -> R {
    WINDOW_INPUT.with(|input| *input.borrow_mut() = RawInput { events: window, ..RawInput::default() });
    let (mut run, mut answer) = (Some(run), None);
    let mut output = ctx.run_ui(RawInput { events: main, ..RawInput::default() }, |ui| answer = run.take().map(|run| run(ui.ctx())));
    output.textures_delta.clear();
    answer.unwrap_or_else(|| panic!("the main pass ran"))
}

fn look(theme: &Theme, frame: WindowFrame) -> Look<'_> {
    Look { theme, frame, size: vec2(704.0, 720.0) }
}

fn light() -> Theme {
    Theme::new(Mode::Light, TextScale::default())
}

/// The main window with the composer in front on the mail: its `Esc` closes the composer.
const COMPOSER: Facts = Facts {
    float: false,
    dialog: false,
    command: false,
    capture: false,
    editor: false,
    composer: true,
    full_width: false,
    one_pane_reader: false,
    mail: true,
    settings: false,
    home: false,
    composer_drawn: true,
};

/// The main window's mail with the list shown, nothing in front.
const MAIL: Facts = Facts { composer: false, composer_drawn: false, ..COMPOSER };

#[test]
fn a_window_host_falls_back_to_docked_where_viewports_are_embedded() {
    let ctx = Context::default();
    install_fonts(&ctx);
    assert!(ctx.embed_viewports(), "a headless context embeds, as the web app does");
    let theme = light();
    for (host, here) in [(Host::Window(window_id()), Host::Docked), (Host::Docked, Host::Docked), (Host::Floating, Host::Floating)] {
        let mut drawn = false;
        let shown = frame(&ctx, Vec::new(), Vec::new(), |ctx| show(ctx, host, "Note", look(&theme, WindowFrame::Own), |_, _| drawn = true));
        assert_eq!(shown, Hosted::Here(here), "{host:?}");
        assert!(!drawn, "{host:?}: the caller draws it in its place, no embedded window opens");
    }
}

#[test]
fn a_window_host_opens_a_window_where_viewports_are_real() {
    let ctx = windowing();
    let theme = light();
    let mut drawn_in = None;
    let shown = frame(&ctx, Vec::new(), Vec::new(), |ctx| {
        show(ctx, Host::Window(window_id()), "Note", look(&theme, WindowFrame::System), |ui, _| drawn_in = Some(ui.ctx().viewport_id()))
    });
    assert_eq!(shown, Hosted::Window(Windowed { inner: (), closed: false }));
    assert_eq!(drawn_in, Some(window_id()), "the content is drawn in the window's own pass");
}

#[test]
fn esc_in_a_window_of_its_own_closes_only_that_window() {
    let ctx = windowing();
    let theme = light();
    let window = |ctx: &Context| show(ctx, Host::Window(window_id()), "Note", look(&theme, WindowFrame::Own), |_, _| ());
    // Esc in the window: the window closes, the main window's composer stays.
    let (main, shown) = frame(&ctx, Vec::new(), vec![key(Key::Escape)], |ctx| (Surfaces::new(COMPOSER).escape(ctx), window(ctx)));
    assert_eq!(main, None, "the main window heard no Esc");
    assert_eq!(shown, Hosted::Window(Windowed { inner: (), closed: true }));
    // Esc in the main window: its composer closes, the window stays.
    let (main, shown) = frame(&ctx, vec![key(Key::Escape)], Vec::new(), |ctx| (Surfaces::new(COMPOSER).escape(ctx), window(ctx)));
    assert_eq!(main, Some(Surface::Composer));
    assert_eq!(shown, Hosted::Window(Windowed { inner: (), closed: false }), "the window heard no Esc");
}

#[test]
fn a_float_in_the_main_window_never_holds_the_window_s_esc() {
    let ctx = windowing();
    let theme = light();
    let menu = egui::Id::unique("host-tests-menu");
    let float = |ctx: &Context| {
        let float = crate::component::popup::Float {
            id: menu,
            anchor: Rect::from_min_size(egui::pos2(10.0, 10.0), vec2(80.0, 20.0)),
            width: 160.0,
            returns_to: menu,
        };
        crate::component::popup::float(ctx, &theme, &float, |ui| ui.label("Menu"));
    };
    let window = |ctx: &Context| show(ctx, Host::Window(window_id()), "Note", look(&theme, WindowFrame::Own), |_, _| ());
    // A float shows in the main window two frames running, then Esc in the window closes the window.
    frame(&ctx, Vec::new(), Vec::new(), |ctx| {
        float(ctx);
        window(ctx)
    });
    let shown = frame(&ctx, Vec::new(), vec![key(Key::Escape)], |ctx| {
        float(ctx);
        window(ctx)
    });
    assert_eq!(shown, Hosted::Window(Windowed { inner: (), closed: true }), "the main window's float holds the main window's Esc only");
}

#[test]
fn the_desktop_s_close_closes_the_window() {
    let ctx = windowing();
    let theme = light();
    let mut info = ViewportInfo::default();
    info.events.push(ViewportEvent::Close);
    let shown = {
        let mut raw = RawInput::default();
        raw.viewports.insert(window_id(), info);
        WINDOW_INPUT.with(|input| *input.borrow_mut() = raw);
        let mut answer = None;
        let mut output = ctx.run_ui(RawInput::default(), |ui| {
            answer = Some(show(ui.ctx(), Host::Window(window_id()), "Note", look(&theme, WindowFrame::System), |_, _| ()));
        });
        output.textures_delta.clear();
        answer
    };
    assert_eq!(shown, Some(Hosted::Window(Windowed { inner: (), closed: true })));
}

#[test]
fn keys_pressed_in_a_window_go_to_the_window_s_surface_never_to_the_main_window_s_row() {
    let ctx = windowing();
    let theme = light();
    let keys = Keymap::default();
    let pressed = |ctx: &Context| ctx.input(|input| input.key_pressed(Key::ArrowDown) || input.key_pressed(Key::E));
    let (main_heard, main_list, shown) = frame(&ctx, Vec::new(), vec![key(Key::ArrowDown), key(Key::E)], |ctx| {
        let main = Surfaces::new(MAIL);
        let shown = show(ctx, Host::Window(window_id()), "Note", look(&theme, WindowFrame::Own), |ui, surfaces| {
            let heard = pressed(ui.ctx());
            let acts = [keymap::NEXT, keymap::DOWN, keymap::ARCHIVE, keymap::COMPOSE, keymap::PALETTE, keymap::UNDO]
                .into_iter()
                .filter(|action| surfaces.allows(action, &keys))
                .collect::<Vec<_>>();
            (heard, surfaces.list_has_keyboard(), acts)
        });
        (pressed(ctx), main.list_has_keyboard(), shown)
    });
    assert!(!main_heard, "the main window's input holds none of the window's keys");
    assert!(main_list, "the main window's list keeps its keyboard for its own keys");
    let Hosted::Window(Windowed { inner: (heard, list, acts), closed }) = shown else { panic!("the window showed: {shown:?}") };
    assert!(heard, "the window's content hears its keys");
    assert!(!list, "in the window no list has the keyboard");
    assert!(acts.is_empty(), "no act of the main window runs from the window: {acts:?}");
    assert!(!closed);
}

#[test]
fn the_own_frame_puts_the_title_bar_over_the_content_and_the_desktop_s_frame_none() {
    let ctx = windowing();
    let theme = light();
    for (frame_by, top) in [(WindowFrame::Own, 1.0 + crate::draw::shell::COMMAND_LINE + 1.0), (WindowFrame::System, 0.0)] {
        let shown =
            frame(&ctx, Vec::new(), Vec::new(), |ctx| show(ctx, Host::Window(window_id()), "Note", look(&theme, frame_by), |ui, _| ui.max_rect()));
        let Hosted::Window(Windowed { inner, .. }) = shown else { panic!("{frame_by:?}: the window showed") };
        assert_eq!(inner.top(), top, "{frame_by:?}: the content starts under the title bar and its hairline");
    }
}

#[test]
fn a_window_s_title_is_one_clean_line_of_bounded_length() {
    assert_eq!(clean_title("Re: a\r\nb\u{0}c"), "Re: a  b c");
    assert_eq!(clean_title("a\u{202E}b\u{2066}c"), "abc", "no direction overrides");
    assert_eq!(clean_title(&"x".repeat(10_000)).chars().count(), 200);
}
