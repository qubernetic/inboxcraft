// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! A reply's quoted part in the composer's text, drawn as the design's quote block: a 2 px `muted` bar at the left,
//! the line that says who wrote ("Odile wrote on …:") in `mono_s`, the quoted lines in `body` (14/22) in `muted`, 14
//! in from the bar's left edge.
//!
//! The block is only how the text is laid out: the text keeps its plain `> ` lines, byte for byte, and they are what
//! is kept and sent. A quoted line's first `>` (with the spaces before it and one space after it) is still in the text,
//! laid out invisible in the room left of the quoted text, so the caret steps through it; a deeper `>` shows as text.
//! Each line is laid out on its own, so nothing in a quoted line (a bidi control included, which is laid out invisible
//! too) reaches the lines around it.
//!
//! The quoted text came in a message: at most [`MAX_LINES`] lines are drawn as a quote (the rest show as typed), the
//! attribution is at most [`MAX_ATTRIBUTION`] characters, and a long line wraps inside the block.

use std::sync::Arc;

use egui::text::{LayoutJob, TextFormat};
use egui::{Color32, Galley, Rangef, Ui};

use crate::theme::Theme;

/// The most lines of a text drawn as a quote; the lines after them show as typed.
pub(crate) const MAX_LINES: usize = 2_000;

/// The longest line, in characters, taken as the attribution over a quote.
pub(crate) const MAX_ATTRIBUTION: usize = 200;

/// The width of the block's bar.
pub(crate) const BAR: f32 = 2.0;

/// What a line of the text is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Role {
    /// Written by the owner (or anything else that is not a quote).
    Text,
    /// The line just over a quote that ends with a colon: who wrote it, and when.
    Attribution,
    /// A quoted line; `prefix` is the length in bytes of its hidden start (spaces, the `>` and one space).
    Quoted { prefix: usize },
}

/// The length in bytes of `line`'s quote mark (the spaces and tabs before it, the `>` and one space after it), if it is
/// quoted.
fn prefix(line: &str) -> Option<usize> {
    let rest = line.trim_start_matches([' ', '\t']).strip_prefix('>')?;
    let rest = rest.strip_prefix(' ').unwrap_or(rest);
    Some(line.len().saturating_sub(rest.len()))
}

/// Whether `line` can say who wrote the quote under it.
fn is_attribution(line: &str) -> bool {
    let line = line.trim();
    line.ends_with(':') && line.chars().count() <= MAX_ATTRIBUTION && prefix(line).is_none()
}

/// The role of each line of `text` (its lines split at `\n`, in order).
#[must_use]
pub(crate) fn roles(text: &str) -> Vec<Role> {
    let mut roles: Vec<Role> = Vec::new();
    let mut quoted = 0usize;
    let mut before: Option<&str> = None;
    for line in text.split('\n') {
        let role = match prefix(line) {
            Some(prefix) if quoted < MAX_LINES => {
                quoted = quoted.saturating_add(1);
                Role::Quoted { prefix }
            }
            _ => Role::Text,
        };
        if matches!(role, Role::Quoted { .. })
            && let Some(last) = roles.last_mut()
            && *last == Role::Text
            && before.is_some_and(is_attribution)
        {
            *last = Role::Attribution;
        }
        roles.push(role);
        before = Some(line);
    }
    roles
}

/// The text laid out with its quotes as blocks, and the blocks' extents (top to bottom, in the galley's points).
#[derive(Clone)]
pub(crate) struct Laid {
    /// The text laid out.
    pub galley: Arc<Galley>,
    /// Where each block's bar runs.
    pub blocks: Arc<Vec<Rangef>>,
}

/// How a line is set: its formats and the width it wraps at.
struct Styles {
    quote: TextFormat,
    attribution: TextFormat,
    plain: TextFormat,
    indent: f32,
    gap: f32,
}

fn styles(theme: &Theme) -> Styles {
    let p = &theme.palette;
    let quote = theme.fonts.body.format(p.muted);
    // The quoted lines start 6 under the attribution's last row: 22 below its top.
    let gap = (theme.fonts.body.line - theme.fonts.mono_s.line).max(0.0);
    let attribution = theme.fonts.mono_s.format(p.muted);
    Styles { quote, attribution, plain: theme.fonts.read.format(p.ink), indent: BAR + theme.spacing.s3, gap }
}

/// Whether `c` is laid out hidden inside a block: a bidi control, or a control character other than the tab.
fn is_hidden(c: char) -> bool {
    matches!(c, '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}' | '\u{200E}' | '\u{200F}' | '\u{061C}') || (c.is_control() && c != '\t')
}

/// `format` drawn invisible: the same type, so the caret steps through what it sets.
fn hidden(format: &TextFormat) -> TextFormat {
    TextFormat { color: Color32::TRANSPARENT, ..format.clone() }
}

/// Appends `text` to `job` in `format`, the hidden characters invisible.
fn append_shown(job: &mut LayoutJob, text: &str, format: &TextFormat) {
    let hidden = hidden(format);
    let mut start = 0;
    for (at, c) in text.char_indices().filter(|(_, c)| is_hidden(*c)) {
        if let Some(run) = text.get(start..at)
            && !run.is_empty()
        {
            job.append(run, 0.0, format.clone());
        }
        let end = at.saturating_add(c.len_utf8());
        if let Some(run) = text.get(at..end) {
            job.append(run, 0.0, hidden.clone());
        }
        start = end;
    }
    match text.get(start..) {
        Some(run) if !run.is_empty() => job.append(run, 0.0, format.clone()),
        // An empty line keeps its row: an empty section in the line's format.
        _ if text.is_empty() => job.append("", 0.0, format.clone()),
        _ => {}
    }
}

/// The job of one line in `role`, wrapping at `width`.
fn line_job(theme: &Theme, styles: &Styles, marks: &[String], line: &str, role: Role, width: f32) -> LayoutJob {
    let mut job = LayoutJob::default();
    job.wrap.max_width = match role {
        Role::Text => width,
        Role::Attribution | Role::Quoted { .. } => (width - styles.indent).max(1.0),
    };
    match role {
        Role::Text if line.is_empty() => job.append("", 0.0, styles.plain.clone()),
        Role::Text => super::append_marked(&mut job, theme, marks, line),
        Role::Attribution => append_shown(&mut job, line, &styles.attribution),
        Role::Quoted { prefix } => {
            let (mark, rest) = (line.get(..prefix).unwrap_or_default(), line.get(prefix..).unwrap_or_default());
            job.append(mark, 0.0, hidden(&styles.quote));
            if !rest.is_empty() {
                append_shown(&mut job, rest, &styles.quote);
            }
        }
    }
    job
}

/// `galley` (one line in `role`) moved into the block: every row 14 in, the first row's text (after the hidden `>`)
/// at 14 too; an attribution's last row `gap` higher, the room under it.
fn indented(galley: &Galley, role: Role, indent: f32, gap: f32) -> Galley {
    let mut moved = galley.clone();
    if role == Role::Attribution
        && let Some(last) = moved.rows.last_mut()
    {
        let mut row = (*last.row).clone();
        row.size.y += gap;
        last.row = Arc::new(row);
    }
    let marked = match role {
        Role::Quoted { prefix } => galley.job.text.get(..prefix).map_or(0, |mark| mark.chars().count()),
        Role::Text | Role::Attribution => 0,
    };
    for (at, row) in moved.rows.iter_mut().enumerate() {
        let mark = if at == 0 { row.glyphs.get(marked).map_or(row.size.x, |glyph| glyph.pos.x).min(indent) } else { 0.0 };
        row.pos.x += indent - mark;
    }
    moved
}

/// `text` laid out at `width` with its quotes as blocks, or `None` when it quotes nothing (it is then laid out as
/// before, in one piece).
#[must_use]
pub(crate) fn layout(ui: &Ui, theme: &Theme, marks: &[String], text: &str, width: f32) -> Option<Laid> {
    let roles = roles(text);
    if !roles.iter().any(|role| matches!(role, Role::Quoted { .. })) {
        return None;
    }
    Some(lay_out(ui, theme, marks, text, &roles, width))
}

/// `text` laid out line by line in `roles`.
pub(crate) fn lay_out(ui: &Ui, theme: &Theme, marks: &[String], text: &str, roles: &[Role], width: f32) -> Laid {
    let styles = styles(theme);
    let mut whole = LayoutJob::default();
    whole.wrap.max_width = width;
    let mut lines = Vec::with_capacity(roles.len());
    for (at, (line, role)) in text.split('\n').zip(roles.iter().copied()).enumerate() {
        if at > 0 {
            whole.append("\n", 0.0, styles.plain.clone());
        }
        let job = line_job(theme, &styles, marks, line, role, width);
        for section in &job.sections {
            let range = section.byte_range.start.0..section.byte_range.end.0;
            whole.append(job.text.get(range).unwrap_or_default(), section.leading_space, section.format.clone());
        }
        let galley = ui.fonts_mut(|fonts| fonts.layout_job(job));
        lines.push(match role {
            Role::Text => galley,
            Role::Attribution | Role::Quoted { .. } => Arc::new(indented(&galley, role, styles.indent, styles.gap)),
        });
    }
    let galley = Arc::new(Galley::concat(Arc::new(whole), &lines, ui.ctx().pixels_per_point()));
    let laid = Laid { blocks: Arc::new(blocks(&galley, roles)), galley };
    #[cfg(test)]
    ui.data_mut(|data| data.insert_temp(laid_id(), laid.clone()));
    laid
}

#[cfg(test)]
fn laid_id() -> egui::Id {
    egui::Id::unique("compose-quote-laid")
}

/// The text the composer laid out with quote blocks last.
#[cfg(test)]
pub(crate) fn last_laid(ctx: &egui::Context) -> Option<Arc<Galley>> {
    ctx.data(|data| data.get_temp::<Laid>(laid_id())).map(|laid| laid.galley)
}

/// The extents of the blocks in `galley`, laid out line by line in `roles`: from the top of an attribution or the
/// first quoted line to the bottom of the last quoted line under it.
fn blocks(galley: &Galley, roles: &[Role]) -> Vec<Rangef> {
    let mut blocks: Vec<Rangef> = Vec::new();
    let mut line = 0usize;
    // Whether the row before belongs to a block, and whether this row is the first of its line.
    let mut open = false;
    let mut first = true;
    for row in &galley.rows {
        let role = roles.get(line).copied().unwrap_or(Role::Text);
        let span = Rangef::new(row.pos.y, row.pos.y + row.size.y);
        match role {
            Role::Text => {}
            // An attribution starts a block of its own, even right under another block.
            _ if open && !(first && role == Role::Attribution) => {
                if let Some(block) = blocks.last_mut() {
                    block.max = span.max;
                }
            }
            _ => blocks.push(span),
        }
        open = role != Role::Text;
        first = row.ends_with_newline;
        if row.ends_with_newline {
            line = line.saturating_add(1);
        }
    }
    blocks
}

#[cfg(test)]
#[path = "compose_quote_tests.rs"]
mod tests;
