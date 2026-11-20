// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Right-to-left words in the order they are read: the Unicode bidirectional algorithm (UAX #9, through
//! `unicode-bidi`) over a line before egui lays it out, and a cut at the line's logical end.
//!
//! egui lays text out left to right in the written order and lets its shaper turn round only a run of one face whose
//! script is written right to left (a Hebrew or Arabic word): the letters of a word stand right, but the words of a
//! right-to-left phrase stood left to right, and egui's own cut kept the visual left, a right-to-left word's end.
//! Here each section of a job (a label: a name, a subject) is one bidi paragraph of its own, its direction that of its
//! first strong letter, so nothing in one section (an override, an isolate, a mark) reaches another. Its runs are put
//! in visual order, each piece a section of its own so the shaper never joins two of them, and a piece the shaper
//! turns round itself (a word in a right-to-left script) is handed to it in the written order. The explicit
//! embeddings, overrides, isolates and direction marks are left out of what is drawn: they have no width, and they
//! settle the order only.
//!
//! Limits: brackets in a right-to-left run are mirrored for the common pairs only; a run of more than one
//! right-to-left script with no space between them is shaped by egui face by face.

use std::ops::Range;
use std::sync::Arc;

use egui::text::LayoutJob;
use egui::{Galley, Painter};
use unicode_bidi::{BidiClass, BidiInfo, Level};
use unicode_segmentation::UnicodeSegmentation as _;

/// The most characters of one job put in visual order at all: far more than a line shows. A job longer than this is
/// cut there, with an ellipsis at its logical end, so a 10 000-character subject costs what a line costs.
pub(crate) const MOST: usize = 1024;

/// The most rows [`wrapped`] breaks itself; a text wrapped onto more rows is laid out by egui as it was.
pub(crate) const MOST_ROWS: usize = 8;

/// Whether `c` asks for the bidi algorithm: a letter of a right-to-left script, an Arabic digit, or an explicit
/// embedding, override, isolate or right-to-left mark. Everything below U+0590 (Latin, Greek, Cyrillic) never does,
/// so a line in those costs a comparison per character.
fn stirs(c: char) -> bool {
    use BidiClass::{AL, AN, FSI, LRE, LRI, LRO, PDF, PDI, R, RLE, RLI, RLO};
    c >= '\u{0590}' && matches!(unicode_bidi::bidi_class(c), R | AL | AN | LRE | LRO | RLE | RLO | PDF | LRI | RLI | FSI | PDI)
}

/// Whether `text` holds anything the bidi algorithm would move.
pub(crate) fn needs(text: &str) -> bool {
    text.chars().any(stirs)
}

/// The signs that only settle the order and are never drawn: the direction marks and the explicit embeddings,
/// overrides and isolates.
fn unseen(c: char) -> bool {
    matches!(c, '\u{200E}' | '\u{200F}' | '\u{061C}' | '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}')
}

/// `c` as it stands in a right-to-left run: a bracket of the common pairs turned round.
fn mirrored(c: char) -> char {
    match c {
        '(' => ')',
        ')' => '(',
        '[' => ']',
        ']' => '[',
        '{' => '}',
        '}' => '{',
        '<' => '>',
        '>' => '<',
        '«' => '»',
        '»' => '«',
        '‹' => '›',
        '›' => '‹',
        _ => c,
    }
}

/// The script the shaper writes `grapheme` right to left in, if it does: what egui's shaper guesses for a run made of it.
fn shaped_backwards(buffer: &mut harfrust::UnicodeBuffer, grapheme: &str) -> Option<harfrust::Script> {
    buffer.clear();
    buffer.push_str(grapheme);
    buffer.guess_segment_properties();
    (buffer.direction() == harfrust::Direction::RightToLeft).then(|| buffer.script())
}

/// The pieces of `line` (a byte range of `info`'s text inside `para`) in visual order, left to right: each to be laid
/// out alone, written so that the shaper's own turn of a right-to-left word gives the visual order.
fn line_pieces(info: &BidiInfo<'_>, para: &unicode_bidi::ParagraphInfo, line: Range<usize>, out: &mut Vec<String>) {
    let text = info.text;
    let mut buffer = harfrust::UnicodeBuffer::new();
    let (levels, runs) = info.visual_runs(para, line);
    for run in runs {
        let backwards = levels.get(run.start).is_some_and(Level::is_rtl);
        let Some(slice) = text.get(run) else { continue };
        // The run's graphemes in the written order, by the script the shaper turns round (none for one it does not).
        let mut found: Vec<(Option<harfrust::Script>, Vec<&str>)> = Vec::new();
        for grapheme in slice.graphemes(true) {
            // A control is a grapheme of its own (it breaks clusters): it is left out whole.
            if grapheme.chars().all(unseen) {
                continue;
            }
            let key = shaped_backwards(&mut buffer, grapheme);
            match found.last_mut() {
                Some((last, graphemes)) if *last == key => graphemes.push(grapheme),
                _ => found.push((key, vec![grapheme])),
            }
        }
        if backwards {
            found.reverse();
        }
        for (key, mut graphemes) in found {
            // The shaper turns round a piece in a right-to-left script: hand it over in the order that gives the
            // run's own direction.
            if backwards != key.is_some() {
                graphemes.reverse();
            }
            let mirror = backwards && key.is_none();
            let piece: String = graphemes.iter().flat_map(|grapheme| grapheme.chars()).map(|c| if mirror { mirrored(c) } else { c }).collect();
            if !piece.is_empty() {
                out.push(piece);
            }
        }
    }
}

/// `text`'s pieces in visual order, each paragraph's direction that of its first strong letter, or `base` when given.
fn pieces(text: &str, base: Option<Level>) -> Vec<String> {
    let info = BidiInfo::new(text, base);
    let mut out = Vec::new();
    for para in &info.paragraphs {
        line_pieces(&info, para, para.range.clone(), &mut out);
    }
    out
}

/// The direction of `text`'s first paragraph, as UAX #9 P2 and P3 find it (its first strong letter outside an isolate);
/// `None` when it has none.
fn base(text: &str) -> Option<Level> {
    match unicode_bidi::get_base_direction(text) {
        unicode_bidi::Direction::Ltr => Some(Level::ltr()),
        unicode_bidi::Direction::Rtl => Some(Level::rtl()),
        unicode_bidi::Direction::Mixed => None,
    }
}

/// Whether `text` reads right to left: its first strong letter (outside an isolate) is of a right-to-left script.
pub(crate) fn right_to_left(text: &str) -> bool {
    needs(text) && base(text).is_some_and(|level| level.is_rtl())
}

/// The sections' directions of `job`, each by the start of its section: what a cut of `job` keeps, whatever it keeps.
fn bases(job: &LayoutJob) -> Vec<(usize, Option<Level>)> {
    let section_text = |section: &egui::text::LayoutSection| job.text.get(section.byte_range.start.0..section.byte_range.end.0);
    job.sections.iter().map(|section| (section.byte_range.start.0, section_text(section).and_then(base))).collect()
}

/// `text` cut after at most `most` characters between grapheme clusters, and whether it was.
fn capped(text: &str, most: usize) -> (&str, bool) {
    let mut chars = 0_usize;
    for (at, grapheme) in text.grapheme_indices(true) {
        chars = chars.saturating_add(grapheme.chars().count());
        if chars > most {
            return (text.get(..at).unwrap_or_default(), true);
        }
    }
    (text, false)
}

/// `job` with every section in visual order, each section a bidi paragraph of its own; a job of no right-to-left text
/// is given back as it is. At most [`MOST`] characters are kept, a cut there ending in an ellipsis at the logical end.
/// Each piece after a section's first takes the section's letter spacing before it, as egui puts it between clusters.
pub(crate) fn visual(job: LayoutJob) -> LayoutJob {
    ordered(job, &[])
}

/// [`visual`], each section whose start `bases` names in that direction (a cut of a longer label: the direction of the
/// whole), the others in the direction of their first strong letter. A section cut at [`MOST`] keeps its own direction.
fn ordered(job: LayoutJob, bases: &[(usize, Option<Level>)]) -> LayoutJob {
    let turned = |level: &Option<Level>| level.is_some_and(|level| level.is_rtl());
    if !needs(&job.text) && !bases.iter().any(|(_, level)| turned(level)) {
        return job;
    }
    let mut out = job;
    let text = std::mem::take(&mut out.text);
    let sections = std::mem::take(&mut out.sections);
    let mut left = MOST;
    for section in sections {
        let Some(slice) = text.get(section.byte_range.start.0..section.byte_range.end.0) else { continue };
        let named = bases.iter().find(|(start, _)| *start == section.byte_range.start.0).map(|(_, level)| *level);
        let (kept, cut) = capped(slice, left);
        left = left.saturating_sub(kept.chars().count());
        let format = section.format;
        if !cut && !needs(kept) && !named.as_ref().is_some_and(turned) {
            if !kept.is_empty() {
                push(&mut out, kept, section.leading_space, format);
            }
            continue;
        }
        let owned;
        let kept = if cut {
            owned = format!("{}…", kept.trim_end());
            owned.as_str()
        } else {
            kept
        };
        let between = format.extra_letter_spacing.max(0.0);
        let base = named.unwrap_or_else(|| if cut { base(slice) } else { None });
        for (index, piece) in pieces(kept, base).iter().enumerate() {
            push(&mut out, piece, if index == 0 { section.leading_space } else { between }, format.clone());
        }
        if cut {
            break;
        }
    }
    out
}

/// `job` kept to its first `end` bytes (a grapheme boundary), the spaces before the cut dropped, and `ellipsis` at the
/// end of what is kept, in the format of the section it ends. Nothing kept with an ellipsis is the ellipsis alone.
pub(crate) fn prefix(job: &LayoutJob, end: usize, ellipsis: Option<char>) -> LayoutJob {
    let kept = job.text.get(..end).unwrap_or_default();
    let end = if ellipsis.is_some() { kept.trim_end().len() } else { kept.len() };
    let mut out = empty_like(job);
    for section in &job.sections {
        let range = section.byte_range.start.0..section.byte_range.end.0.min(end);
        if range.is_empty() {
            continue;
        }
        let Some(slice) = job.text.get(range) else { continue };
        push(&mut out, slice, section.leading_space, section.format.clone());
    }
    if let Some(ellipsis) = ellipsis {
        let mut sign = [0_u8; 4];
        let sign = ellipsis.encode_utf8(&mut sign);
        if let Some(section) = out.sections.last_mut() {
            out.text.push_str(sign);
            section.byte_range.end = egui::text::ByteIndex(out.text.len());
        } else {
            let format = job.sections.first().map(|section| section.format.clone()).unwrap_or_default();
            push(&mut out, sign, 0.0, format);
        }
    }
    out
}

/// Puts `text` at the end of `job` as a section of its own, never joined to the one before it as
/// [`LayoutJob::append`] joins two of the same format: the shaper must never see two pieces as one run.
fn push(job: &mut LayoutJob, text: &str, leading_space: f32, format: egui::TextFormat) {
    let start = job.text.len();
    job.text.push_str(text);
    let byte_range = egui::text::ByteIndex(start)..egui::text::ByteIndex(job.text.len());
    job.sections.push(egui::text::LayoutSection { leading_space, byte_range, format });
}

/// A job like `job` that holds no text yet.
fn empty_like(job: &LayoutJob) -> LayoutJob {
    LayoutJob {
        text: String::new(),
        sections: Vec::new(),
        wrap: job.wrap.clone(),
        first_row_min_height: job.first_row_min_height,
        break_on_newline: job.break_on_newline,
        halign: job.halign,
        justify: job.justify,
        round_output_to_gui: job.round_output_to_gui,
        keep_trailing_whitespace: job.keep_trailing_whitespace,
    }
}

/// `galley` marked as cut, as egui marks a line it cut itself.
pub(crate) fn marked_cut(galley: Arc<Galley>) -> Arc<Galley> {
    if galley.elided {
        return galley;
    }
    let mut cut = (*galley).clone();
    cut.elided = true;
    Arc::new(cut)
}

/// One line of `job` (its wrap ignored) in visual order, whole if it fits in `width` as the cutting rule measures it
/// ([`crate::draw::paint::within`]), else cut at its logical end: its longest beginning in the written order that ends
/// at a grapheme boundary, then one ellipsis (`job`'s overflow character). A right-to-left label so keeps its first
/// words, wherever they stand. At most [`MOST`] characters are laid out; each step of the search is one layout.
pub(crate) fn cut_line(painter: &Painter, job: LayoutJob, width: f32) -> Arc<Galley> {
    let ellipsis = job.wrap.overflow_character.unwrap_or('…');
    let mut job = job;
    job.wrap.max_width = f32::INFINITY;
    let (kept, was_capped) = capped(&job.text, MOST - 1);
    let ends: Vec<usize> = kept.grapheme_indices(true).map(|(at, grapheme)| at + grapheme.len()).collect();
    // Each section's direction, from the whole of it: a cut keeps it.
    let bases = bases(&job);
    let lay = |count: usize, cut: bool| {
        let end = if count == 0 { 0 } else { ends.get(count - 1).copied().unwrap_or(0) };
        painter.layout_job(ordered(prefix(&job, end, cut.then_some(ellipsis)), &bases))
    };
    let fits = |galley: &Galley| crate::draw::paint::within(galley, width);
    if !was_capped {
        let whole = lay(ends.len(), false);
        if fits(&whole) {
            return whole;
        }
    }
    if !fits(&lay(0, true)) {
        return marked_cut(lay(0, false));
    }
    // The most clusters whose cut fits: a binary search.
    let (mut low, mut high) = (0_usize, ends.len());
    while low < high {
        let middle = low + (high - low).div_ceil(2);
        if fits(&lay(middle, true)) {
            low = middle;
        } else {
            high = middle - 1;
        }
    }
    marked_cut(lay(low, true))
}

/// `job` (one section, wrapped at its width onto at most its rows) broken into rows in the written order and each row
/// put in visual order, the last cut at its logical end with the job's overflow character when the text goes on; `None`
/// when it needs no bidi or asks for more than [`MOST_ROWS`] rows or no finite width (egui lays it out as before).
/// A row breaks after the last space that lets it fit, or between clusters in a word longer than the row; a line break
/// ends a row when the job breaks on them. Each row of a paragraph is ordered with the paragraph's levels (UAX #9 L1
/// to L4 per line); a row that holds the end of one paragraph and the start of the next (a paragraph separator, U+2029,
/// and the other signs of bidi class B) is ordered a paragraph at a time.
pub(crate) fn wrapped(painter: &Painter, job: &LayoutJob) -> Option<Arc<Galley>> {
    let width = job.wrap.max_width;
    let rows = job.wrap.max_rows.max(1);
    if !needs(&job.text) || rows > MOST_ROWS || !width.is_finite() || job.sections.len() != 1 {
        return None;
    }
    let section = job.sections.first()?;
    let format = section.format.clone();
    let ellipsis = job.wrap.overflow_character.unwrap_or('…');
    let (text, was_capped) = capped(job.text.get(section.byte_range.start.0..section.byte_range.end.0)?, MOST.saturating_mul(rows));
    // The paragraphs and their directions, from the whole text: a row and a cut keep them.
    let info = BidiInfo::new(text, None);
    let base_at = |at: usize| info.paragraphs.iter().find(|para| para.range.contains(&at)).map(|para| para.level);
    let one = |line: &str, base: Option<Level>| {
        let mut single = LayoutJob::single_section(line.to_owned(), format.clone());
        single.wrap.max_width = f32::INFINITY;
        single.break_on_newline = false;
        painter.layout_job(ordered(single, &[(0, base)]))
    };
    let fits = |line: &str, base: Option<Level>| crate::draw::paint::within(&one(line, base), width);
    // The rows: byte ranges of `text`, and the cut last row's own text when there is one.
    let mut found: Vec<Range<usize>> = Vec::new();
    let mut cut_row: Option<String> = None;
    let mut start = 0_usize;
    while start < text.len() && found.len() < rows {
        let rest = text.get(start..).unwrap_or_default();
        let (segment, after) = match rest.find('\n').filter(|_| job.break_on_newline) {
            Some(at) => (rest.get(..at).unwrap_or_default(), at + 1),
            None => (rest, rest.len()),
        };
        let base = base_at(start);
        let fits = |line: &str| fits(line, base);
        let last = found.len() + 1 >= rows;
        let ends_text = start + after >= text.len() && !was_capped;
        if fits(segment.trim_end()) && (!last || ends_text) {
            found.push(start..start + segment.trim_end().len());
            start += after;
            continue;
        }
        if last {
            let rest: String = rest.chars().map(|c| if c == '\n' { ' ' } else { c }).collect();
            cut_row = Some(cut_text(&rest, ellipsis, &fits));
            break;
        }
        // The longest beginning that ends a word and fits, else the most clusters that fit (one at least).
        let breaks: Vec<usize> = segment
            .char_indices()
            .zip(segment.chars().skip(1))
            .filter(|((_, c), next)| c.is_whitespace() && !next.is_whitespace())
            .map(|((at, c), _)| at + c.len_utf8())
            .collect();
        let taken = best(&breaks, |end| fits(segment.get(..end).unwrap_or_default().trim_end())).or_else(|| {
            let ends: Vec<usize> = segment.grapheme_indices(true).map(|(at, grapheme)| at + grapheme.len()).collect();
            best(&ends, |end| fits(segment.get(..end).unwrap_or_default())).or_else(|| ends.first().copied())
        });
        let Some(taken) = taken.filter(|taken| *taken > 0) else { break };
        let shown = segment.get(..taken).unwrap_or_default().trim_end().len();
        found.push(start..start + shown);
        start += taken;
    }
    let mut out = empty_like(job);
    out.wrap.max_width = f32::INFINITY;
    out.break_on_newline = true;
    let between = format.extra_letter_spacing.max(0.0);
    let put = |pieces: Vec<String>, out: &mut LayoutJob| {
        if !out.text.is_empty() {
            push(out, "\n", 0.0, format.clone());
        }
        for (index, piece) in pieces.iter().enumerate() {
            push(out, piece, if index == 0 { section.leading_space } else { between }, format.clone());
        }
    };
    for row in &found {
        // A row may hold a paragraph's end (a paragraph separator, which breaks no row): each part in the order of its own
        // paragraph.
        let mut pieces_of_row = Vec::new();
        for para in &info.paragraphs {
            let part = row.start.max(para.range.start)..row.end.min(para.range.end);
            if !part.is_empty() {
                line_pieces(&info, para, part, &mut pieces_of_row);
            }
        }
        put(pieces_of_row, &mut out);
    }
    let cut = cut_row.is_some();
    if let Some(row) = cut_row {
        // The cut row in its paragraph's direction: the paragraph it starts in.
        put(pieces(&row, base_at(start)), &mut out);
    }
    let galley = painter.layout_job(out);
    Some(if cut { marked_cut(galley) } else { galley })
}

/// `text` cut at its logical end with `ellipsis` so it `fits`: its longest beginning between clusters, the spaces
/// before the ellipsis dropped; the ellipsis alone when nothing more fits.
fn cut_text(text: &str, ellipsis: char, fits: &dyn Fn(&str) -> bool) -> String {
    let ends: Vec<usize> = text.grapheme_indices(true).map(|(at, grapheme)| at + grapheme.len()).collect();
    let cut = |end: usize| format!("{}{ellipsis}", text.get(..end).unwrap_or_default().trim_end());
    best(&ends, |end| fits(&cut(end))).map_or_else(|| ellipsis.to_string(), cut)
}

/// The last of the rising `ends` for which `fits` holds, by a binary search (`fits` is taken to hold for a prefix of
/// them); `None` when it holds for none.
fn best(ends: &[usize], fits: impl Fn(usize) -> bool) -> Option<usize> {
    let (mut low, mut high) = (0_usize, ends.len());
    // `low` ends that fit are known; `high` is the most that may.
    while low < high {
        let middle = low + (high - low).div_ceil(2);
        if ends.get(middle - 1).is_some_and(|end| fits(*end)) {
            low = middle;
        } else {
            high = middle - 1;
        }
    }
    low.checked_sub(1).and_then(|index| ends.get(index).copied())
}

/// The left-to-right mark (U+200E): a strong letter of no width, never drawn ([`unseen`]).
pub(crate) const LEFT_TO_RIGHT_MARK: char = '\u{200E}';

/// An e-mail address or a domain as it is drawn: left to right, its parts in the written order, as a browser shows an
/// address. The bidi algorithm would take `שלום.קום` in `x@שלום.קום.example.org` as one right-to-left run and turn it
/// round, so that `קום` stood left of `שלום`: the labels would seem to be in another order next to `example.org`.
/// Here each run of right-to-left letters stands between two left-to-right marks, and one leads the address, so every
/// run keeps its place and only its own letters are read right to left. The marks, overrides and isolates it held are
/// left out: they would move its parts again. An address of no right-to-left letter is given back as it is (without
/// those signs). A line that holds an address and words in a right-to-left script (a name before it) starts with
/// [`LEFT_TO_RIGHT_MARK`] too, so the address is laid out in a left-to-right line.
pub(crate) fn address(text: &str) -> std::borrow::Cow<'_, str> {
    let clean: std::borrow::Cow<'_, str> = if text.chars().any(unseen) {
        std::borrow::Cow::Owned(text.chars().filter(|c| !unseen(*c)).collect())
    } else {
        std::borrow::Cow::Borrowed(text)
    };
    if !needs(&clean) {
        return clean;
    }
    let turned = |c: char| matches!(unicode_bidi::bidi_class(c), BidiClass::R | BidiClass::AL);
    let mut out = String::with_capacity(clean.len().saturating_add(16));
    out.push(LEFT_TO_RIGHT_MARK);
    let mut inside = false;
    for grapheme in clean.graphemes(true) {
        let right_to_left = grapheme.chars().any(turned);
        if right_to_left != inside {
            if !out.ends_with(LEFT_TO_RIGHT_MARK) {
                out.push(LEFT_TO_RIGHT_MARK);
            }
            inside = right_to_left;
        }
        out.push_str(grapheme);
    }
    if inside {
        out.push(LEFT_TO_RIGHT_MARK);
    }
    std::borrow::Cow::Owned(out)
}

/// `line` (a name and its address) led by [`LEFT_TO_RIGHT_MARK`] when it holds right-to-left text, so the address in it
/// stands in a left-to-right line ([`address`]); as it is otherwise.
pub(crate) fn left_to_right(line: String) -> String {
    if needs(&line) { format!("{LEFT_TO_RIGHT_MARK}{line}") } else { line }
}

/// `text` as one section of a job is laid out: its characters in visual order ([`visual`]).
#[cfg(test)]
pub(crate) fn seen_text(text: &str) -> String {
    visual(LayoutJob::single_section(text.to_owned(), egui::TextFormat::default())).text
}

#[cfg(test)]
#[path = "bidi_tests.rs"]
mod tests;
