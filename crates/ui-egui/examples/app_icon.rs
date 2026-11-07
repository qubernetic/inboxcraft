// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `app_icon`: exports the app icon drawn in `inboxcraft_ui_egui::icons` as SVG, to the path given or to
//! `assets/app-icon/inboxcraft.svg`.
//!
//! ```text
//! cargo run -p inboxcraft-ui-egui --example app_icon
//! ```

use std::process::ExitCode;

use inboxcraft_ui_egui::icons::{EXPORTED, app_icon, svg};

fn main() -> ExitCode {
    let path = std::env::args_os().nth(1).unwrap_or_else(|| EXPORTED.into());
    match std::fs::write(&path, svg(&app_icon())) {
        Ok(()) => {
            println!("{}", path.to_string_lossy());
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("app_icon: {}: {error}", path.to_string_lossy());
            ExitCode::FAILURE
        }
    }
}
