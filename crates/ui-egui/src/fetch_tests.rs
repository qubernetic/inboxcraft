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
