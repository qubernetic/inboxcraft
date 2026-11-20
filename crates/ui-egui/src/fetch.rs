// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! How the reader gets a remote picture: through a [`Fetch`] the app hands in.
//!
//! The front end opens no connection of its own. The desktop app gives a [`Threaded`] fetch over `net`'s picture
//! client (HTTPS only, a fixed user agent, no cookies, no referrer); the sample gives pictures drawn in code; a
//! window without either gets [`Offline`], which asks nobody. The reader only calls [`Fetch::start`] for a picture
//! the owner allowed.

use std::collections::VecDeque;
use std::fmt;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread;

/// What a fetch gave for one address: the bytes, or why there are none.
pub type Fetched = (String, Result<Vec<u8>, String>);

/// Fetches remote pictures.
pub trait Fetch: Send {
    /// Starts fetching `url`; the answer comes from [`Fetch::finished`].
    fn start(&mut self, url: &str);
    /// The fetches that ended since the last call.
    fn finished(&mut self) -> Vec<Fetched>;
}

/// The fetch of a window that may not fetch anything: every picture fails at once, and nobody is asked.
#[derive(Debug, Default)]
pub struct Offline {
    done: Vec<Fetched>,
}

impl Fetch for Offline {
    fn start(&mut self, url: &str) {
        self.done.push((url.to_owned(), Err(crate::tr!("Remote pictures are not fetched in this window.").to_owned())));
    }

    fn finished(&mut self) -> Vec<Fetched> {
        std::mem::take(&mut self.done)
    }
}

/// The function a [`Threaded`] fetch runs on its threads.
pub type Get = Arc<dyn Fn(&str) -> Result<Vec<u8>, String> + Send + Sync>;

/// Something to call when a fetch ended, so the window draws again.
pub type Wake = Arc<dyn Fn() + Send + Sync>;

/// Runs `get` on up to [`Threaded::MAX_RUNNING`] threads at a time, the rest waiting in turn.
pub struct Threaded {
    get: Get,
    wake: Option<Wake>,
    waiting: VecDeque<String>,
    running: usize,
    failed: Vec<Fetched>,
    sender: Sender<Fetched>,
    receiver: Receiver<Fetched>,
}

impl fmt::Debug for Threaded {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Threaded").field("waiting", &self.waiting.len()).field("running", &self.running).finish_non_exhaustive()
    }
}

impl Threaded {
    /// The most fetches running at once.
    pub const MAX_RUNNING: usize = 4;

    /// A fetch that runs `get`, and calls `wake` after each answer.
    #[must_use]
    pub fn new(get: Get, wake: Option<Wake>) -> Self {
        let (sender, receiver) = channel();
        Self { get, wake, waiting: VecDeque::new(), running: 0, failed: Vec::new(), sender, receiver }
    }

    fn launch(&mut self) {
        while self.running < Self::MAX_RUNNING {
            let Some(url) = self.waiting.pop_front() else { break };
            let (get, wake, sender, asked) = (Arc::clone(&self.get), self.wake.clone(), self.sender.clone(), url.clone());
            let spawned = thread::Builder::new().name("inboxcraft-picture".to_owned()).spawn(move || {
                let answer = get(&asked);
                // A closed window no longer listens; nothing to tell it then.
                let _ = sender.send((asked, answer));
                if let Some(wake) = wake {
                    wake();
                }
            });
            match spawned {
                Ok(_) => self.running = self.running.saturating_add(1),
                // The thread did not start: the picture fails, the reader carries on.
                Err(error) => self.failed.push((url, Err(error.to_string()))),
            }
        }
    }
}

impl Fetch for Threaded {
    fn start(&mut self, url: &str) {
        self.waiting.push_back(url.to_owned());
        self.launch();
    }

    fn finished(&mut self) -> Vec<Fetched> {
        let mut done: Vec<Fetched> = self.receiver.try_iter().collect();
        self.running = self.running.saturating_sub(done.len());
        done.append(&mut self.failed);
        self.launch();
        done
    }
}

/// The most addresses a [`Shared`] fetch remembers who asked for, across its readers: the oldest ask is forgotten
/// past it (its answer then goes to nobody, as an answer for a picture no reader keeps any more).
const MOST_ASKED: usize = 4096;

/// What the readers of a [`Shared`] fetch hold together: the one fetch, who asked for which address, and the answers
/// waiting for each reader.
struct Hub {
    fetch: Box<dyn Fetch>,
    asked: VecDeque<(u64, String)>,
    waiting: Vec<(u64, Fetched)>,
    next: u64,
}

/// One fetch shared by several readers (the pane's and a window of its own's): each gets the answers to what it asked
/// for, and only those. An address two readers asked for answers both.
pub(crate) struct Shared {
    hub: Arc<std::sync::Mutex<Hub>>,
    me: u64,
}

impl fmt::Debug for Shared {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Shared").field("me", &self.me).finish_non_exhaustive()
    }
}

impl Shared {
    /// The first reader of `fetch`.
    pub(crate) fn new(fetch: Box<dyn Fetch>) -> Self {
        let hub = Hub { fetch, asked: VecDeque::new(), waiting: Vec::new(), next: 1 };
        Self { hub: Arc::new(std::sync::Mutex::new(hub)), me: 0 }
    }

    /// Another reader of the same fetch.
    pub(crate) fn another(&self) -> Self {
        let mut hub = lock(&self.hub);
        let me = hub.next;
        hub.next = hub.next.wrapping_add(1);
        Self { hub: Arc::clone(&self.hub), me }
    }
}

/// The hub, also after a reader panicked while holding it: what it holds stays usable.
fn lock(hub: &std::sync::Mutex<Hub>) -> std::sync::MutexGuard<'_, Hub> {
    hub.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

impl Fetch for Shared {
    fn start(&mut self, url: &str) {
        let mut hub = lock(&self.hub);
        if hub.asked.len() >= MOST_ASKED {
            hub.asked.pop_front();
        }
        hub.asked.push_back((self.me, url.to_owned()));
        hub.fetch.start(url);
    }

    fn finished(&mut self) -> Vec<Fetched> {
        let mut hub = lock(&self.hub);
        let Hub { fetch, asked, waiting, .. } = &mut *hub;
        for (url, answer) in fetch.finished() {
            // Every reader that asked for this address gets the answer, once.
            let askers: Vec<u64> = asked.iter().filter(|(_, asked)| *asked == url).map(|(who, _)| *who).collect();
            asked.retain(|(_, asked)| *asked != url);
            waiting.extend(askers.into_iter().map(|who| (who, (url.clone(), answer.clone()))));
        }
        let (mine, others): (Vec<_>, Vec<_>) = std::mem::take(waiting).into_iter().partition(|(who, _)| *who == self.me);
        *waiting = others;
        mine.into_iter().map(|(_, fetched)| fetched).collect()
    }
}

impl Drop for Shared {
    fn drop(&mut self) {
        // What a reader that is gone asked for goes to nobody.
        let mut hub = lock(&self.hub);
        let me = self.me;
        hub.asked.retain(|(who, _)| *who != me);
        hub.waiting.retain(|(who, _)| *who != me);
    }
}

#[cfg(test)]
#[path = "fetch_tests.rs"]
mod tests;
