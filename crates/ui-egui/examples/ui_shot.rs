// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `ui_shot`: headless pictures of the sample window, one per line of a JSON lines script (see
//! `inboxcraft_ui_egui::shot`).
//!
//! ```text
//! cargo run -p inboxcraft-ui-egui --example ui_shot -- crates/ui-egui/shots.jsonl
//! cargo run -p inboxcraft-ui-egui --example ui_shot -- --out-dir target/shots crates/ui-egui/shots.jsonl
//! ```
//!
//! Each picture goes to its line's `out`; with `--out-dir`, to that path under the folder instead (`cargo xtask
//! shots` compares them there), and an `out` that is absolute or climbs out with `..` is refused. The path of each
//! picture written is printed as the script names it.

use std::fs::File;
use std::io::{BufWriter, Read};
use std::path::{Component, Path, PathBuf};
use std::process::ExitCode;

use inboxcraft_ui_egui::UiError;
use inboxcraft_ui_egui::shot::{MAX_SCRIPT, parse_script, shoot};

const USAGE: &str = "usage: ui_shot [--out-dir <dir>] <script.jsonl>";

fn main() -> ExitCode {
    let mut args = std::env::args_os().skip(1);
    let (dir, path) = match (args.next(), args.next(), args.next()) {
        (Some(path), None, None) => (None, path),
        (Some(flag), Some(dir), Some(path)) if flag == "--out-dir" => (Some(PathBuf::from(dir)), path),
        _ => {
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        }
    };
    let script = match read(Path::new(&path)) {
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
        let written = target(dir.as_deref(), &shot.out).and_then(|target| {
            let picture = shoot(shot)?;
            if let Some(parent) = target.parent().filter(|parent| !parent.as_os_str().is_empty()) {
                std::fs::create_dir_all(parent).map_err(|error| UiError::Shot(error.to_string()))?;
            }
            let file = File::create(&target).map_err(|error| UiError::Shot(error.to_string()))?;
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

/// The script at `path`, at most [`MAX_SCRIPT`] bytes (one more is read, so `parse_script` refuses a longer one).
fn read(path: &Path) -> std::io::Result<String> {
    let mut text = String::new();
    File::open(path)?.take(u64::try_from(MAX_SCRIPT).unwrap_or(u64::MAX).saturating_add(1)).read_to_string(&mut text)?;
    Ok(text)
}

/// Where the picture `out` goes: `out` itself, or `out` under `dir`, where it must stay.
fn target(dir: Option<&Path>, out: &str) -> Result<PathBuf, UiError> {
    let Some(dir) = dir else { return Ok(PathBuf::from(out)) };
    let relative = Path::new(out);
    if out.is_empty() || !relative.components().all(|part| matches!(part, Component::Normal(_))) {
        return Err(UiError::Shot(format!("{out:?} is not a relative path that stays under --out-dir")));
    }
    Ok(dir.join(relative))
}
