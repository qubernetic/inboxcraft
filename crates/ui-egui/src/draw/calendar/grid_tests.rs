// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The grid's lines on whole pixels, at every scale, for any width.

use super::{Lines, lane_pitch, line, pixels, snap};

const SCALES: [f32; 4] = [1.0, 1.25, 1.5, 2.0];

fn whole(points: f32, ppp: f32) -> bool {
    let px = points * ppp;
    (px - px.round()).abs() < 1e-3
}

#[test]
fn every_line_starts_on_a_pixel_and_the_columns_differ_by_a_pixel_at_most() {
    for ppp in SCALES {
        for width in [955.0_f32, 956.0, 957.0, 300.3, 1.0] {
            let lines = Lines::cut(56.0, 56.0 + width, 7, ppp);
            assert_eq!(lines.columns(), 7);
            assert!(lines.at.iter().all(|at| whole(*at, ppp)), "{ppp} {width}: {:?}", lines.at);
            assert!(whole(lines.width, ppp));
            let px: Vec<i64> = lines.at.windows(2).map(|pair| ((pair[1] - pair[0]) * ppp).round() as i64).collect();
            let (least, most) = (px.iter().min().unwrap(), px.iter().max().unwrap());
            assert!(most - least <= 1, "{ppp} {width}: {px:?}");
        }
    }
}

#[test]
fn a_line_is_one_point_in_whole_pixels() {
    assert_eq!([1.0, 1.25, 1.5, 2.0].map(|ppp| (line(ppp) * ppp).round()), [1.0, 1.0, 2.0, 2.0]);
    assert_eq!(line(f32::NAN), 1.0);
    assert_eq!(line(-3.0), 1.0);
    assert!(whole(pixels(3.0, 1.25), 1.25));
    assert_eq!(snap(f32::INFINITY, 1.0), 0.0);
}

#[test]
fn a_cell_takes_both_lines_and_two_cells_share_one() {
    for ppp in SCALES {
        let lines = Lines::cut(0.0, 884.0, 7, ppp);
        let (left, right) = lines.cell(1, 2);
        assert_eq!(left, lines.line(1));
        assert_eq!(right, lines.line(2) + lines.width);
        let (next_left, _) = lines.cell(2, 3);
        assert_eq!(right - lines.width, next_left, "the shared line is the right cell's left edge");
        let (inside_left, inside_right) = lines.inside(1);
        assert_eq!(inside_left, left + lines.width);
        assert_eq!(inside_right, lines.line(2));
    }
}

#[test]
fn bands_share_one_line() {
    for ppp in SCALES {
        let pitch = lane_pitch(24.0, ppp);
        assert!(whole(pitch, ppp));
        assert_eq!(((24.0 - pitch) * ppp).round(), (line(ppp) * ppp).round());
    }
}

#[test]
fn junk_never_panics() {
    for (from, to, count) in [(f32::NAN, 10.0, 7), (0.0, f32::INFINITY, 0), (100.0, -5.0, usize::MAX), (0.0, 1e30, 3)] {
        let lines = Lines::cut(from, to, count, f32::NAN);
        assert!(lines.columns() >= 1 && lines.columns() <= 64);
        let _ = lines.cell(usize::MAX, 0);
        let _ = lines.inside(usize::MAX);
    }
}
