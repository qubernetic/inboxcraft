// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The `inboxcraft` binary: reads its arguments with [`inboxcraft::run`] and opens the window.
//!
//! On the owner's mailbox it first claims the store's folder ([`inboxcraft::arrive`]): a later start brings the running
//! window forward and leaves. The first start listens for later ones, puts the tray up on Linux, and in the Flatpak asks
//! the Background portal to let it run without a window. Closing the window leaves it running ([`Desktop`]); a new
//! window opens on the same app when a later start or the tray asks for one.

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use std::process::ExitCode;
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};

use eframe::egui;
use inboxcraft::desktop::{Desktop, WINDOWLESS_TICK};
use inboxcraft::instance::Request;
use inboxcraft::{Source, Step};
use inboxcraft_engine::appearance::WindowFrame;
use inboxcraft_time::SystemClock;
use inboxcraft_ui_egui::fetch::Wake;

/// The window's content as eframe draws it, for as long as one window lives.
struct Window<'a>(&'a mut Desktop);

impl eframe::App for Window<'_> {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.0.tick(ctx);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.0.app_mut().ui(ui);
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

/// The context of the window that is open, if one is; a wake redraws it.
type Shown = Arc<Mutex<Option<egui::Context>>>;

/// Opens windows on `desktop` until InboxCraft ends: while the desktop has no window it keeps the mail in step and
/// waits for a request for one.
fn windows(mut desktop: Desktop, shown: &Shown) -> ExitCode {
    loop {
        if desktop.is_quitting() {
            return ExitCode::SUCCESS;
        }
        if !desktop.has_window() {
            desktop.wait(WINDOWLESS_TICK);
            continue;
        }
        let mut viewport = egui::ViewportBuilder::default()
            .with_title("InboxCraft")
            .with_app_id("io.github.qubernetic.inboxcraft")
            .with_inner_size([1200.0, 760.0])
            .with_min_inner_size([720.0, 480.0])
            // See-through where the app paints nothing: the corners of its own frame.
            .with_transparent(true)
            // The app draws its own frame unless the owner chose the desktop's title bar (`ui.frame.set`).
            .with_decorations(desktop.app().mailbox().appearance().frame == WindowFrame::System);
        desktop.app_mut().set_own_frame(true);
        // On Wayland the desktop finds the icon through the app id; X11 and other systems take it from the window.
        if let Ok(icon) = inboxcraft_ui_egui::icons::window_icon() {
            viewport = viewport.with_icon(icon);
        }
        // `run_and_return` (the default) keeps eframe's event loop, so a later window can open on it.
        let options = eframe::NativeOptions { viewport, ..eframe::NativeOptions::default() };
        let context = Arc::clone(shown);
        let window = Window(&mut desktop);
        let made = move |creation: &eframe::CreationContext<'_>| {
            if let Ok(mut shown) = context.lock() {
                *shown = Some(creation.egui_ctx.clone());
            }
            // Figtree from the first frame on, so no text is drawn in another font first.
            inboxcraft_ui_egui::theme::install_fonts(&creation.egui_ctx);
            Ok(Box::new(window) as Box<dyn eframe::App + '_>)
        };
        let ran = eframe::run_native("InboxCraft", options, Box::new(made));
        if let Ok(mut shown) = shown.lock() {
            *shown = None;
        }
        if let Err(error) = ran {
            eprintln!("inboxcraft: the window could not open: {error}");
            return ExitCode::FAILURE;
        }
        // A window that went away without a close request (the session ended) ends InboxCraft as well.
        if desktop.has_window() {
            return ExitCode::SUCCESS;
        }
    }
}

/// A wake that redraws the window that is open, if one is.
fn wake_of(shown: &Shown) -> Wake {
    let shown = Arc::clone(shown);
    Arc::new(move || {
        if let Ok(shown) = shown.lock()
            && let Some(context) = shown.as_ref()
        {
            context.request_repaint();
        }
    })
}

/// The sample, in a window that ends InboxCraft when closed.
fn sample() -> ExitCode {
    let shown = Shown::default();
    let app = match inboxcraft::app(inboxcraft::Mail::Sample, || Box::new(SystemClock::new()), Some(wake_of(&shown)), inboxcraft::owner_keep()) {
        Ok(app) => app,
        Err(error) => {
            eprintln!("inboxcraft: {error}");
            return ExitCode::FAILURE;
        }
    };
    let (_send, heard) = mpsc::channel();
    windows(Desktop::new(app, heard).closing_ends(), &shown)
}

/// The owner's mailbox, with a window or (`background`) without one.
fn owner(background: bool) -> ExitCode {
    let Some(dir) = inboxcraft::instance_dir() else {
        eprintln!("inboxcraft: there is no folder to keep the mail in: HOME (or XDG_DATA_HOME) is not set");
        return ExitCode::FAILURE;
    };
    let instance = match inboxcraft::arrive(&dir, background, &mut std::io::stderr()) {
        Ok(instance) => instance,
        Err(code) => return ExitCode::from(code),
    };
    let shown = Shown::default();
    let wake = wake_of(&shown);
    let (send, heard) = mpsc::channel::<Request>();
    let asked = asking(send.clone(), Arc::clone(&wake));
    let listening = instance.listen(asked);
    let reachable = match listening.error() {
        Some(error) => {
            eprintln!("inboxcraft: {error}; a later start cannot bring this window back");
            false
        }
        None => true,
    };
    let mail = match inboxcraft::store::mail(Source::Owner) {
        Ok(mail) => mail,
        Err(error) => {
            eprintln!("inboxcraft: {error}");
            return ExitCode::FAILURE;
        }
    };
    let app = match inboxcraft::app(mail, || Box::new(SystemClock::new()), Some(Arc::clone(&wake)), inboxcraft::owner_keep()) {
        Ok(app) => app,
        Err(error) => {
            eprintln!("inboxcraft: {error}");
            return ExitCode::FAILURE;
        }
    };
    let mut desktop = Desktop::new(app, heard);
    if background {
        desktop = desktop.without_window();
    }
    let (desktop, tray) = linux::desktop(desktop, &send, &wake);
    // Closing the window ends InboxCraft when nothing could bring a window back.
    let desktop = if reachable || tray { desktop } else { desktop.closing_ends() };
    drop(send);
    let ended = windows(desktop, &shown);
    drop(listening);
    ended
}

/// Hands a request to the window and wakes it to answer.
fn asking(send: Sender<Request>, wake: Wake) -> impl Fn(Request) + Send + 'static {
    let send = Mutex::new(send);
    move |request| {
        if let Ok(send) = send.lock()
            && send.send(request).is_ok()
        {
            wake();
        }
    }
}

#[cfg(target_os = "linux")]
mod linux {
    use std::sync::mpsc::Sender;
    use std::time::Duration;

    use inboxcraft::autostart::{self, portal};
    use inboxcraft::desktop::Desktop;
    use inboxcraft::instance::Request;
    use inboxcraft::tray::{self, InboxTray};
    use inboxcraft_ui_egui::fetch::Wake;

    /// How long the portal may take: the desktop may ask the person first.
    pub(crate) const PORTAL_PATIENCE: Duration = Duration::from_secs(120);

    /// `desktop` with the tray, and whether the tray is up; in the Flatpak, the Background portal is asked on a thread.
    pub(crate) fn desktop(desktop: Desktop, send: &Sender<Request>, wake: &Wake) -> (Desktop, bool) {
        let sandboxed = autostart::sandboxed();
        if sandboxed {
            let autostart = autostart::autostart_dir().is_some_and(|dir| autostart::is_on(&dir));
            let asked = std::thread::Builder::new().name("inboxcraft-portal".to_owned()).spawn(move || {
                let granted = zbus::blocking::Connection::session()
                    .map_err(|error| error.to_string())
                    .and_then(|bus| portal::request(&bus, autostart, PORTAL_PATIENCE));
                if let Err(error) = granted {
                    eprintln!("inboxcraft: {error}; the desktop may end InboxCraft when its window is closed");
                }
            });
            if let Err(error) = asked {
                eprintln!("inboxcraft: the Background portal could not be asked: {error}");
            }
        }
        match tray::start(InboxTray::new(send.clone(), wake.clone()), sandboxed) {
            Ok(badge) => (desktop.with_badge(Box::new(badge)), true),
            Err(error) => {
                eprintln!("inboxcraft: {error}");
                (desktop, false)
            }
        }
    }

    /// Asks the Background portal over the session bus.
    pub(crate) fn ask(on: bool) -> Result<autostart::Granted, String> {
        let bus = zbus::blocking::Connection::session().map_err(|error| format!("the Background portal could not be asked: {error}"))?;
        portal::request(&bus, on, PORTAL_PATIENCE)
    }
}

#[cfg(not(target_os = "linux"))]
mod linux {
    use std::sync::mpsc::Sender;

    use inboxcraft::desktop::Desktop;
    use inboxcraft::instance::Request;
    use inboxcraft_ui_egui::fetch::Wake;

    /// No tray outside Linux yet.
    pub(crate) fn desktop(desktop: Desktop, _send: &Sender<Request>, _wake: &Wake) -> (Desktop, bool) {
        (desktop, false)
    }
}

/// `--autostart` and `--no-autostart`.
fn autostart(on: bool) -> ExitCode {
    let dir = inboxcraft::autostart::autostart_dir();
    let program = std::env::current_exe().unwrap_or_else(|_| "inboxcraft".into());
    #[cfg(target_os = "linux")]
    let portal: Option<inboxcraft::autostart::Ask<'_>> = if inboxcraft::autostart::sandboxed() { Some(&linux::ask) } else { None };
    #[cfg(not(target_os = "linux"))]
    let portal: Option<inboxcraft::autostart::Ask<'_>> = None;
    ExitCode::from(inboxcraft::set_autostart(on, dir.as_deref(), &program, portal, &mut std::io::stdout(), &mut std::io::stderr()))
}

/// `--quit`.
fn quit() -> ExitCode {
    match inboxcraft::instance_dir() {
        Some(dir) => ExitCode::from(inboxcraft::quit(&dir, &mut std::io::stdout(), &mut std::io::stderr())),
        None => {
            eprintln!("inboxcraft: InboxCraft is not running");
            ExitCode::FAILURE
        }
    }
}

fn main() -> ExitCode {
    match inboxcraft::run(std::env::args_os().skip(1), &mut std::io::stdout(), &mut std::io::stderr()) {
        Step::Exit(code) => ExitCode::from(code),
        Step::Window(Source::Sample) => sample(),
        Step::Window(Source::Owner) => owner(false),
        Step::Background => owner(true),
        Step::Quit => quit(),
        Step::Autostart(on) => autostart(on),
    }
}
