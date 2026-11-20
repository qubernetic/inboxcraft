// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Where a surface lives: docked in its place, floating over the window, or in a window of its own ([`Host`]). The
//! window of its own is an egui immediate viewport: the desktop's frame around it, or the app's own (the title bar
//! of the window frame page: the mark, the title, the window controls, the resizing edges and the outline, drawn by
//! [`super::frame`] as the main window's own frame is). Where no second window can open (egui embeds viewports: the
//! web app, a headless context, a backend without them) the window host falls back to the docked one; it never
//! opens an embedded egui window in the main one.
//!
//! The window's keys are its own: its input is its viewport's, its [`Surfaces`] are built from it
//! ([`Surfaces::window`]), one `Esc` there closes only that window (or a float in it first), and the OS close asks
//! the same. Nothing here runs a command of the engine or keeps what the surface holds: the caller keeps its text
//! and decides what closing means.

use egui::{Context, CornerRadius, Rect, Ui, UiBuilder, Vec2, ViewportBuilder, ViewportId, pos2};
use inboxcraft_engine::appearance::WindowFrame;

use super::{frame, paint, shell};
use crate::surface::{Surface, Surfaces};
use crate::theme::Theme;

/// Where a surface is shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Host {
    /// In its place in the main window (the composer in the reader's place, the reader in its pane).
    Docked,
    /// Over the main window, in a float of its own.
    #[cfg_attr(not(test), allow(dead_code, reason = "the floating composer moves into the host in its own task"))]
    Floating,
    /// In a window of its own, the viewport `0`.
    Window(ViewportId),
}

impl Host {
    /// The host that can show in `ctx`: a window only where egui opens real windows, docked where it embeds them.
    pub(crate) fn usable(self, ctx: &Context) -> Self {
        match self {
            Self::Window(_) if ctx.embed_viewports() => Self::Docked,
            host => host,
        }
    }
}

/// How a window of its own looks: the theme, who draws its frame, and its inner size in points when it opens.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Look<'a> {
    /// The look of the app.
    pub theme: &'a Theme,
    /// Who draws the window's frame: the app's own ([`WindowFrame::Own`]) or the desktop's.
    pub frame: WindowFrame,
    /// The window's inner size when it opens, in points.
    pub size: Vec2,
}

/// What a window of its own gave back this frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Windowed<R> {
    /// What the surface's own drawing gave back.
    pub inner: R,
    /// The window was asked to close this frame: one `Esc` in it, its Close control or the desktop's close. The caller
    /// keeps what it holds and stops showing it.
    pub closed: bool,
}

/// Where the surface was shown this frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Hosted<R> {
    /// Not in a window: the caller draws it in its place, docked or floating as the host says (a window host that
    /// cannot open says docked).
    Here(Host),
    /// In its window of its own.
    Window(Windowed<R>),
}

/// The most characters of a window's title: the desktop's title bar and the own frame's show one line of it.
const TITLE_MOST: usize = 200;

/// The room left of the title in the own frame's title bar, in points: the mark 16 in, its 22, and 12 after it.
const TITLE_IN: f32 = 50.0;

/// Shows a surface on `host`: in a window of its own titled `title`, looking as `look` says, where one can open; then
/// `add` draws its content and is given the window's [`Surfaces`] for its keys. Elsewhere `add` is not called and the
/// caller draws the surface in its place ([`Hosted::Here`]).
#[cfg_attr(not(test), expect(dead_code, reason = "the reader moves into the host in its own task"))]
pub(crate) fn show<R>(ctx: &Context, host: Host, title: &str, look: Look<'_>, add: impl FnMut(&mut Ui, &Surfaces) -> R) -> Hosted<R> {
    show_with(ctx, host, title, look, false, add)
}

/// [`show`] for a surface that holds the composer (`composer`): the window's keys then include the composer's own.
pub(crate) fn show_with<R>(
    ctx: &Context,
    host: Host,
    title: &str,
    look: Look<'_>,
    composer: bool,
    mut add: impl FnMut(&mut Ui, &Surfaces) -> R,
) -> Hosted<R> {
    let Host::Window(id) = host.usable(ctx) else { return Hosted::Here(host.usable(ctx)) };
    let title = clean_title(title);
    let own = frame::drawn(true, look.frame);
    let builder = ViewportBuilder::default()
        .with_title(title.clone())
        .with_inner_size(look.size)
        .with_min_inner_size(Vec2::splat(shell::COMMAND_LINE * 4.0))
        .with_transparent(own)
        .with_decorations(!own);
    ctx.show_viewport_immediate(id, builder, |ui, _class| {
        let ctx = ui.ctx().clone();
        // Built from this window's input, before its content reads a key: its own float first, then the window.
        let surfaces = Surfaces::window(crate::component::popup::any_open(&ctx), composer);
        let escaped = surfaces.escape(&ctx) == Some(Surface::Window);
        let asked = ctx.input(|input| input.viewport().close_requested());
        let content = window_frame(ui, look, own, &title);
        let mut inside = ui.new_child(UiBuilder::new().max_rect(content));
        inside.set_clip_rect(content);
        let inner = add(&mut inside, &surfaces);
        if own {
            frame::outline(&ctx, look.theme, frame::corner(&ctx, true));
        }
        Hosted::Window(Windowed { inner, closed: escaped || asked })
    })
}

/// The window's background and, in the own frame, its title bar over the content: the mark, the title, the handle that
/// moves the window, the resizing edges and the window controls, with the hairline under them. The rect the content
/// takes.
fn window_frame(ui: &Ui, look: Look<'_>, own: bool, title: &str) -> Rect {
    let ctx = ui.ctx();
    let (theme, rect) = (look.theme, ui.max_rect());
    let corner = frame::corner(ctx, own);
    ui.painter().rect_filled(rect, CornerRadius::same(corner.clamp(0.0, 255.0) as u8), theme.palette.bg);
    if !own {
        return rect;
    }
    // The title bar one point in from the outline, as tall as the main window's command line.
    let line = Rect::from_min_max(pos2(rect.left() + 1.0, rect.top() + 1.0), pos2(rect.right() - 1.0, rect.top() + 1.0 + shell::COMMAND_LINE));
    let top = (corner - 1.0).clamp(0.0, 255.0) as u8;
    ui.painter().rect_filled(line, CornerRadius { nw: top, ne: top, sw: 0, se: 0 }, theme.palette.panel);
    frame::title_bar(ui, line);
    frame::edges(ctx);
    frame::mark(ui, theme, line, false);
    let room = (line.width() - TITLE_IN - frame::CONTROLS_ROOM).max(1.0);
    let galley = paint::line(ui, &[(title, theme.fonts.mono, theme.palette.ink)], room);
    let at = pos2(line.left() + TITLE_IN, line.center().y - theme.fonts.mono.line / 2.0);
    paint::run_at(ui.painter(), at, galley, theme.palette.ink);
    frame::controls(ctx, theme, line);
    paint::hairline(ui.painter(), pos2(line.left(), line.bottom()), pos2(line.right(), line.bottom()), theme.palette.line);
    Rect::from_min_max(pos2(rect.left() + 1.0, line.bottom() + 1.0), pos2(rect.right() - 1.0, rect.bottom() - 1.0))
}

/// `title` as a window may show it: one line, no control or direction-overriding characters, at most
/// [`TITLE_MOST`] characters. A title comes from outside (a message's subject).
pub(crate) fn clean_title(title: &str) -> String {
    let overrides = |c: char| matches!(c, '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}' | '\u{200E}' | '\u{200F}');
    title.chars().map(|c| if c.is_control() { ' ' } else { c }).filter(|c| !overrides(*c)).take(TITLE_MOST).collect()
}

#[cfg(test)]
#[path = "host_tests.rs"]
mod tests;
