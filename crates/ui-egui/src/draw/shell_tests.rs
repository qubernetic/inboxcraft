// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The frame's four regions: the command line, the index, the body and the status line, measured from the frame sheet
//! of the shell, on whole pixels at every scale, never negative or no number whatever size the desktop reports.

use egui::{Rect, pos2, vec2};

use super::{self as shell, COMMAND_LINE, INDEX, INDEX_NARROW, NARROW_INDEX_UNDER, Regions, STATUS_LINE};
use crate::theme::{Mode, TextScale, Theme};

fn theme(percent: u16) -> Theme {
    Theme::new(Mode::Dark, TextScale::from_percent(percent).unwrap())
}

fn rect(x: f32, y: f32, w: f32, h: f32) -> Rect {
    Rect::from_min_size(pos2(x, y), vec2(w, h))
}

fn every(regions: &Regions) -> [(&'static str, Rect); 4] {
    [("command", regions.command), ("index", regions.index), ("body", regions.body), ("status", regions.status)]
}

#[test]
fn the_regions_are_the_frame_s() {
    assert_eq!((COMMAND_LINE, STATUS_LINE, INDEX, INDEX_NARROW, NARROW_INDEX_UNDER), (44.0, 26.0, 232.0, 56.0, 1280.0));
    // The sheet at 1440 × 900: the window's outline (1) round everything; the command line 1438 × 44, a hairline under
    // it; the index 232 and the body 1205 wide, 826 high, one hairline between them; a hairline over the status line,
    // 1438 × 26.
    let regions = shell::regions(vec2(1440.0, 900.0), &theme(100), 1.0);
    assert_eq!(regions.command, rect(1.0, 1.0, 1438.0, 44.0));
    assert_eq!(regions.index, rect(1.0, 46.0, 232.0, 826.0));
    assert_eq!(regions.body, rect(234.0, 46.0, 1205.0, 826.0));
    assert_eq!(regions.status, rect(1.0, 873.0, 1438.0, 26.0));
    // At 130 %: the heights × 1.3 to the nearest even pixel, the widths stay.
    let large = shell::regions(vec2(1440.0, 900.0), &theme(130), 1.0);
    assert_eq!((large.command.height(), large.status.height()), (58.0, 34.0));
    assert_eq!((large.index.width(), large.body.width()), (232.0, 1205.0));
    assert_eq!(large.body.top(), 1.0 + 58.0 + 1.0);
    assert_eq!(large.status.bottom(), 899.0);
    // Under 1280 the index is the narrow one.
    let narrow = shell::regions(vec2(1100.0, 900.0), &theme(100), 1.0);
    assert_eq!(narrow.index.width(), 56.0);
    assert_eq!(narrow.body, rect(58.0, 46.0, 1100.0 - 58.0 - 1.0, 826.0));
    assert_eq!(shell::regions(vec2(1280.0, 900.0), &theme(100), 1.0).index.width(), 232.0);
    // Never narrower than 800.
    assert_eq!(shell::regions(vec2(799.0, 900.0), &theme(100), 1.0), shell::regions(vec2(800.0, 900.0), &theme(100), 1.0));
}

#[test]
fn every_edge_is_on_a_whole_pixel() {
    for ppp in [1.0, 1.25, 1.5, 2.0] {
        for percent in [100, 130] {
            for size in [vec2(1440.0, 900.0), vec2(1100.0, 700.0), vec2(1283.3, 811.7), vec2(800.0, 600.0)] {
                let regions = shell::regions(size, &theme(percent), ppp);
                for (name, rect) in every(&regions) {
                    for edge in [rect.left(), rect.right(), rect.top(), rect.bottom()] {
                        let pixels = edge * ppp;
                        assert!((pixels - pixels.round()).abs() < 1e-3, "{ppp} {percent} {size:?}: {name} edge {edge} is not whole");
                    }
                }
                // The regions touch nothing of each other: one hairline at least between them.
                assert!(regions.index.right() < regions.body.left(), "{ppp} {percent}: a line between index and body");
                assert!(regions.command.bottom() < regions.body.top() && regions.body.bottom() < regions.status.top(), "{ppp} {percent}");
            }
        }
    }
}

#[test]
fn any_size_from_the_desktop_lays_out_as_at_least_800_wide() {
    let least = shell::regions(vec2(800.0, 0.0), &theme(100), 1.0);
    let sizes = [
        vec2(0.0, 0.0),
        vec2(-50.0, -50.0),
        vec2(f32::NAN, f32::NAN),
        vec2(f32::INFINITY, f32::INFINITY),
        vec2(f32::NEG_INFINITY, 900.0),
        vec2(f32::MAX, f32::MAX),
        vec2(1.0e-30, 3.0),
        vec2(799.0, 1.0),
    ];
    for size in sizes {
        for ppp in [1.0, 1.5, 0.0, -1.0, f32::NAN, f32::INFINITY] {
            for percent in [90, 100, 130] {
                let regions = shell::regions(size, &theme(percent), ppp);
                for (name, rect) in every(&regions) {
                    assert!(rect.is_finite(), "{size:?} {ppp}: {name} is a number");
                    assert!(rect.width() >= 0.0 && rect.height() >= 0.0, "{size:?} {ppp}: {name} {rect:?} is not negative");
                }
                let at_least = shell::regions(vec2(800.0, 0.0), &theme(percent), ppp);
                assert!(regions.command.right() >= at_least.command.right(), "{size:?} {ppp}: laid out 800 wide at least");
                assert!(regions.status.bottom() >= at_least.status.bottom(), "{size:?} {ppp}: as high as the regions need at least");
                assert!(regions.status.bottom() >= regions.command.bottom(), "{size:?} {ppp}: the status line under the command line");
            }
        }
    }
    assert!(least.status.top() >= least.body.bottom(), "the least height holds every region");
}
