// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The `inboxcraft` binary: reads its arguments with [`inboxcraft::run`] and opens the window.

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use std::process::ExitCode;
use std::sync::{Arc, OnceLock};

use eframe::egui;
use inboxcraft::{Source, Step};
use inboxcraft_engine::appearance::WindowFrame;
use inboxcraft_time::SystemClock;
use inboxcraft_ui_egui::InboxApp;
use inboxcraft_ui_egui::fetch::Wake;

/// The window's content as eframe draws it.
struct Desktop(InboxApp);

impl eframe::App for Desktop {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.0.ui(ui);
    }

    fn raw_input_hook(&mut self, ctx: &egui::Context, raw_input: &mut egui::RawInput) {
        inboxcraft_ui_egui::InboxApp::release_after_window_grab(ctx, raw_input);
    }

    /// Nothing behind the app: the window's own frame has round corners, and the desktop shows through them. The app
    /// paints every pixel inside them itself.
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0; 4]
    }

    // No `on_exit`: the store keeps the accounts from one start to the next, so their passwords stay in the
    // credential store when the window closes (`InboxApp::on_exit` is for a window whose accounts are forgotten).
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
    let mail = match inboxcraft::store::mail(source) {
        Ok(mail) => mail,
        Err(error) => {
            eprintln!("inboxcraft: {error}");
            return ExitCode::FAILURE;
        }
    };
    let mut app = match inboxcraft::app(mail, || Box::new(SystemClock::new()), Some(wake), inboxcraft::owner_keep()) {
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
        .with_min_inner_size([720.0, 480.0])
        // See-through where the app paints nothing: the corners of its own frame.
        .with_transparent(true)
        // The app draws its own frame unless the owner chose the desktop's title bar (`ui.frame.set`).
        .with_decorations(app.mailbox().appearance().frame == WindowFrame::System);
    app.set_own_frame(true);
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
