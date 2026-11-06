// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The `inboxcraft` binary: reads its arguments with [`inboxcraft::run`] and opens the window.

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use std::process::ExitCode;
use std::sync::{Arc, OnceLock};

use eframe::egui;
use inboxcraft::{Source, Step};
use inboxcraft_time::SystemClock;
use inboxcraft_ui_egui::InboxApp;
use inboxcraft_ui_egui::fetch::Wake;

/// The window's content as eframe draws it.
struct Desktop(InboxApp);

impl eframe::App for Desktop {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.0.ui(ui);
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.0.on_exit();
    }
}

fn window(source: Source) -> ExitCode {
    // The window's context exists once eframe made it; a picture that arrives before then needs no redraw.
    let context: Arc<OnceLock<egui::Context>> = Arc::new(OnceLock::new());
    let woken = Arc::clone(&context);
    let wake: Wake = Arc::new(move || {
        if let Some(context) = woken.get() {
            context.request_repaint();
        }
    });
    let app = match inboxcraft::app(source, || Box::new(SystemClock::new()), Some(wake), inboxcraft::owner_keep()) {
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
    let made = move |creation: &eframe::CreationContext<'_>| {
        let _ = context.set(creation.egui_ctx.clone());
        // Figtree from the first frame on, so no text is drawn in another font first.
        inboxcraft_ui_egui::theme::install_fonts(&creation.egui_ctx);
        Ok(Box::new(Desktop(app)) as Box<dyn eframe::App>)
    };
    match eframe::run_native("InboxCraft", options, Box::new(made)) {
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
