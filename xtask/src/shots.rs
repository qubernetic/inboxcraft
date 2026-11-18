// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `cargo xtask shots`: the committed pictures are what the code draws.
//!
//! Every scene of `crates/ui-egui/shots.jsonl` is drawn by the `ui_shot` example, as `just shot` runs it, into
//! `<target>/shots/` (`ui_shot --out-dir`; `<target>` is `$CARGO_TARGET_DIR`, else `target/`), and each picture is
//! compared byte for byte with the one committed at the scene's `out`. The gate fails, naming them, on a committed
//! picture that differs from the render, a scene with no committed picture, two scenes that write one picture, and a
//! PNG in a folder of the pictures that no scene writes. `--fix` copies the renders over the committed pictures and
//! deletes the pictures of no scene; two scenes that write one picture still fail. The render is deterministic, so
//! there is no tolerance: one pixel is a difference.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Write;
use std::path::{Component, Path, PathBuf};

use crate::{Context, Error, count};

/// The script of the scenes.
pub(crate) const SCRIPT: &str = "crates/ui-egui/shots.jsonl";

/// The folder of the renders under the target directory.
const RENDERS: &str = "shots";

/// Draws every scene and compares, or with `fix` writes, the committed pictures; reports to `out`.
pub(crate) fn gate(ctx: &mut Context, fix: bool, out: &mut dyn Write) -> Result<bool, Error> {
    let renders = match &ctx.shots {
        Some(dir) => dir.clone(),
        None => target_dir(&ctx.root, std::env::var_os("CARGO_TARGET_DIR").map(PathBuf::from)).join(RENDERS),
    };
    let Some(dir) = renders.to_str() else {
        return Err(Error::Config { path: renders.clone(), message: "the folder of the renders is not UTF-8".to_owned() });
    };
    let args = ["run", "-q", "-p", "inboxcraft-ui-egui", "--example", "ui_shot", "--", "--out-dir", dir, SCRIPT];
    let Some(stdout) = ctx.runner.output(&ctx.cargo, &args, &ctx.root)? else {
        writeln!(out, "shots: ui_shot failed (see above); no picture was compared")?;
        return Ok(false);
    };
    let printed = String::from_utf8_lossy(&stdout);
    let scenes: Vec<&str> = printed.lines().map(str::trim).filter(|line| !line.is_empty()).collect();
    compare(&ctx.root, &renders, &scenes, fix, out)
}

/// `$CARGO_TARGET_DIR` (relative to `root` when it is relative), else `root/target`.
pub(crate) fn target_dir(root: &Path, var: Option<PathBuf>) -> PathBuf {
    match var {
        Some(dir) if !dir.as_os_str().is_empty() => root.join(dir),
        _ => root.join("target"),
    }
}

/// Compares the picture of each scene in `scenes` (their `out` paths, in script order) under `renders` with the one
/// committed under `root`; with `fix`, makes the committed ones the renders. `Ok(false)` when anything is left wrong.
pub(crate) fn compare(root: &Path, renders: &Path, scenes: &[&str], fix: bool, out: &mut dyn Write) -> Result<bool, Error> {
    let mut writers: BTreeMap<&str, usize> = BTreeMap::new();
    for scene in scenes {
        let writes = writers.entry(scene).or_default();
        *writes = writes.saturating_add(1);
    }
    let mut problems = 0_usize;
    let (mut differ, mut missing, mut fixed, mut twice) = (0_usize, 0_usize, 0_usize, 0_usize);
    let mut folders = BTreeSet::new();
    for (scene, writes) in &writers {
        let relative = Path::new(scene);
        if !relative.components().all(|part| matches!(part, Component::Normal(_))) {
            writeln!(out, "shots: {scene}: not a path under the workspace")?;
            problems = problems.saturating_add(1);
            continue;
        }
        if *writes > 1 {
            writeln!(out, "shots: {scene}: {writes} scenes write it; only the last one shows")?;
            problems = problems.saturating_add(1);
            twice = twice.saturating_add(1);
        }
        folders.insert(relative.parent().map(Path::to_path_buf).unwrap_or_default());
        let render_path = renders.join(relative);
        let render = fs::read(&render_path).map_err(|source| Error::Io { path: render_path.clone(), source })?;
        let committed_path = root.join(relative);
        let state = match fs::read(&committed_path) {
            Ok(committed) if committed == render => continue,
            Ok(_) => "differs from the render",
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => "has no picture",
            Err(source) => return Err(Error::Io { path: committed_path, source }),
        };
        if state == "has no picture" {
            missing = missing.saturating_add(1);
        } else {
            differ = differ.saturating_add(1);
        }
        if fix {
            fs::write(&committed_path, &render).map_err(|source| Error::Io { path: committed_path.clone(), source })?;
            writeln!(out, "shots: {scene} {state}: written")?;
            fixed = fixed.saturating_add(1);
        } else {
            writeln!(out, "shots: {scene} {state}")?;
            problems = problems.saturating_add(1);
        }
    }
    let mut strays = 0_usize;
    for folder in &folders {
        for stray in strays_in(&root.join(folder), folder, &writers)? {
            strays = strays.saturating_add(1);
            if fix {
                let path = root.join(&stray);
                fs::remove_file(&path).map_err(|source| Error::Io { path, source })?;
                writeln!(out, "shots: {} belongs to no scene: deleted", stray.display())?;
                fixed = fixed.saturating_add(1);
            } else {
                writeln!(out, "shots: {} belongs to no scene", stray.display())?;
                problems = problems.saturating_add(1);
            }
        }
    }
    let scenes = count(scenes.len(), "scene");
    if problems == 0 && fixed == 0 {
        writeln!(out, "shots: {scenes}, every committed picture is the render")?;
    } else {
        writeln!(
            out,
            "shots: {scenes}: {differ} differ, {missing} without a picture, {strays} of no scene, {twice} written by more than one{}",
            if fix { format!("; {fixed} fixed") } else { String::new() }
        )?;
    }
    Ok(problems == 0)
}

/// The PNG files in `dir` (`folder` under the root) that no scene writes, as paths under the root, sorted.
fn strays_in(dir: &Path, folder: &Path, writers: &BTreeMap<&str, usize>) -> Result<Vec<PathBuf>, Error> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(source) => return Err(Error::Io { path: dir.to_path_buf(), source }),
    };
    let mut strays = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|source| Error::Io { path: dir.to_path_buf(), source })?;
        let path = folder.join(entry.file_name());
        let png = path.extension().is_some_and(|extension| extension.eq_ignore_ascii_case("png"));
        let named = path.to_str().is_some_and(|name| writers.contains_key(name));
        if png && !named && entry.file_type().map_err(|source| Error::Io { path: dir.to_path_buf(), source })?.is_file() {
            strays.push(path);
        }
    }
    strays.sort();
    Ok(strays)
}

#[cfg(test)]
#[path = "shots_tests.rs"]
mod tests;
