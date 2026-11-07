// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `ui_shot`: headless pictures of the sample window, one per line of a JSON lines script (see
//! `inboxcraft_ui_egui::shot`).
//!
//! ```text
//! cargo run -p inboxcraft-ui-egui --example ui_shot -- crates/ui-egui/shots.jsonl
//! ```

use std::fs::File;
use std::io::BufWriter;
use std::process::ExitCode;

use inboxcraft_ui_egui::shot::{parse_script, shoot};

fn main() -> ExitCode {
    let Some(path) = std::env::args_os().nth(1) else {
        eprintln!("usage: ui_shot <script.jsonl>");
        return ExitCode::from(2);
    };
    let script = match std::fs::read_to_string(&path) {
        Ok(script) => script,
        Err(error) => {
            eprintln!("ui_shot: {}: {error}", path.to_string_lossy());
            return ExitCode::FAILURE;
        }
    };
    let shots = match parse_script(&script) {
        Ok(shots) => shots,
        Err(error) => {
            eprintln!("ui_shot: {error}");
            return ExitCode::FAILURE;
        }
    };
    for shot in &shots {
        let written = shoot(shot).and_then(|picture| {
            let file = File::create(&shot.out).map_err(|error| inboxcraft_ui_egui::UiError::Shot(error.to_string()))?;
            picture.write_png(&mut BufWriter::new(file))
        });
        match written {
            Ok(()) => println!("{}", shot.out),
            Err(error) => {
                eprintln!("ui_shot: {}: {error}", shot.out);
                return ExitCode::FAILURE;
            }
        }
    }
    ExitCode::SUCCESS
}
