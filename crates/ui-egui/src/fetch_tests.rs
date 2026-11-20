// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use super::{Fetch, Fetched, Offline, Threaded};

fn wait_for(fetch: &mut dyn Fetch, count: usize) -> Vec<Fetched> {
    let mut done = Vec::new();
    // At most ten seconds.
    for _ in 0..2000 {
        if done.len() >= count {
            break;
        }
        done.extend(fetch.finished());
        std::thread::sleep(Duration::from_millis(5));
    }
    done.sort_by(|a, b| a.0.cmp(&b.0));
    done
}

#[test]
fn offline_asks_nobody_and_fails_each_picture() {
    let mut offline = Offline::default();
    assert!(offline.finished().is_empty());
    offline.start("https://pictures.example.org/a.png");
    let done = offline.finished();
    assert_eq!(done.len(), 1);
    assert_eq!(done[0].0, "https://pictures.example.org/a.png");
    assert!(done[0].1.as_ref().unwrap_err().contains("not fetched"));
    assert!(offline.finished().is_empty());
}

#[test]
fn threaded_runs_the_fetch_off_the_window_thread_and_wakes_it() {
    let woken = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&woken);
    let mut fetch = Threaded::new(
        Arc::new(|url: &str| if url.ends_with(".png") { Ok(url.as_bytes().to_vec()) } else { Err("no".to_owned()) }),
        Some(Arc::new(move || {
            counter.fetch_add(1, Ordering::SeqCst);
        })),
    );
    fetch.start("https://a.example.org/1.png");
    fetch.start("https://a.example.org/2.gif");
    let done = wait_for(&mut fetch, 2);
    assert_eq!(
        done,
        [
            ("https://a.example.org/1.png".to_owned(), Ok(b"https://a.example.org/1.png".to_vec())),
            ("https://a.example.org/2.gif".to_owned(), Err("no".to_owned())),
        ]
    );
    assert_eq!(woken.load(Ordering::SeqCst), 2);
    assert!(format!("{fetch:?}").starts_with("Threaded"));
}

#[test]
fn threaded_runs_at_most_four_at_once_and_the_rest_in_turn() {
    let running = Arc::new(AtomicUsize::new(0));
    let most = Arc::new(Mutex::new(0_usize));
    let (now, high) = (Arc::clone(&running), Arc::clone(&most));
    let mut fetch = Threaded::new(
        Arc::new(move |_url: &str| {
            let current = now.fetch_add(1, Ordering::SeqCst) + 1;
            let mut highest = high.lock().unwrap();
            *highest = (*highest).max(current);
            drop(highest);
            std::thread::sleep(Duration::from_millis(20));
            now.fetch_sub(1, Ordering::SeqCst);
            Ok(Vec::new())
        }),
        None,
    );
    for n in 0..10 {
        fetch.start(&format!("https://a.example.org/{n:02}.png"));
    }
    let done = wait_for(&mut fetch, 10);
    assert_eq!(done.len(), 10);
    assert!(*most.lock().unwrap() <= Threaded::MAX_RUNNING);
}

/// A fetch that answers every address at once with its own bytes.
#[derive(Default)]
struct Echo(Vec<Fetched>);

impl Fetch for Echo {
    fn start(&mut self, url: &str) {
        self.0.push((url.to_owned(), Ok(url.as_bytes().to_vec())));
    }

    fn finished(&mut self) -> Vec<Fetched> {
        std::mem::take(&mut self.0)
    }
}

fn urls(done: &[Fetched]) -> Vec<&str> {
    done.iter().map(|(url, _)| url.as_str()).collect()
}

/// The pane's reader and a window's share one fetch: each gets the answers to what it asked, whoever polls first.
#[test]
fn a_shared_fetch_answers_each_reader_what_it_asked() {
    let first = super::Shared::new(Box::new(Echo::default()));
    let (mut pane, mut window) = (first.another(), first.another());
    pane.start("https://pictures.example.org/pane.png");
    window.start("https://pictures.example.org/window.png");
    // The window polls first: the pane's answer waits for the pane.
    assert_eq!(urls(&window.finished()), ["https://pictures.example.org/window.png"]);
    assert_eq!(urls(&pane.finished()), ["https://pictures.example.org/pane.png"]);
    assert!(pane.finished().is_empty() && window.finished().is_empty(), "each answer once");
    // An address both asked for answers both.
    pane.start("https://pictures.example.org/both.png");
    window.start("https://pictures.example.org/both.png");
    assert_eq!(urls(&pane.finished()), ["https://pictures.example.org/both.png"]);
    assert_eq!(urls(&window.finished()), ["https://pictures.example.org/both.png"]);
    // A reader that is gone leaves nothing waiting.
    window.start("https://pictures.example.org/gone.png");
    drop(window);
    assert!(pane.finished().is_empty());
}
