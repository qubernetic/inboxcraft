// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The `inboxcraft` binary: reads its arguments with [`inboxcraft::run`] and opens the window.

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use std::process::ExitCode;

use eframe::egui;
use inboxcraft::{Source, Step};
use inboxcraft_time::SystemClock;
use inboxcraft_ui_egui::InboxApp;

/// The window's content as eframe draws it.
struct Desktop(InboxApp);

impl eframe::App for Desktop {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.0.ui(ui);
    }
}

fn window(source: Source) -> ExitCode {
    let app = match inboxcraft::app(source, || Box::new(SystemClock::new())) {
        Ok(app) => app,
        Err(error) => {
            eprintln!("inboxcraft: {error}");
            return ExitCode::FAILURE;
        }
    };
    let mut viewport = egui::ViewportBuilder::default()
        .with_title("InboxCraft")
        .with_app_id("io.github.qubernetic.inboxcraft")
        .with_inner_size([1200.0, 760.0])
        .with_min_inner_size([720.0, 480.0]);
    // On Wayland the desktop finds the icon through the app id; X11 and other systems take it from the window.
    if let Ok(icon) = inboxcraft_ui_egui::icons::window_icon() {
        viewport = viewport.with_icon(icon);
    }
    let options = eframe::NativeOptions { viewport, ..eframe::NativeOptions::default() };
    match eframe::run_native("InboxCraft", options, Box::new(|_| Ok(Box::new(Desktop(app))))) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("inboxcraft: the window could not open: {error}");
            ExitCode::FAILURE
        }
    }
}

fn main() -> ExitCode {
    match inboxcraft::run(std::env::args_os().skip(1), &mut std::io::stdout(), &mut std::io::stderr()) {
        Step::Exit(code) => ExitCode::from(code),
        Step::Window(source) => window(source),
    }
}
