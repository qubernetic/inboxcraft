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

#[cfg(test)]
#[path = "fetch_tests.rs"]
mod tests;
