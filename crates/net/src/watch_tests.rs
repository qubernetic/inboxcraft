// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::sync::mpsc;
use std::time::Duration;

use inboxcraft_model::{AccountId, Job, JobId, JobKind, Uid, UidValidity};
use inboxcraft_proto::imap::{Event, FetchItem};

use super::{WatchConfig, is_change, poll};

fn quick() -> WatchConfig {
    WatchConfig { poll: Duration::from_millis(10), ..WatchConfig::default() }
}

#[test]
fn the_defaults_renew_idle_before_servers_end_it() {
    let config = WatchConfig::default();
    assert_eq!(config.tick, Duration::from_millis(250));
    assert!(config.renew < Duration::from_secs(29 * 60));
    assert_eq!(config.poll, Duration::from_secs(300));
}

#[test]
fn polling_ends_with_a_job_a_resync_or_the_end() {
    let (jobs, queue) = mpsc::channel();
    let job = Job::new(JobId::new(1), AccountId::new(1), JobKind::SyncFolderList);
    jobs.send(job.clone()).unwrap();
    let wake = poll(&quick(), &queue);
    assert_eq!(wake.job, Some(job));
    assert!(!wake.changed && !wake.closed);
    let wake = poll(&quick(), &queue);
    assert!(wake.changed && wake.job.is_none() && !wake.closed);
    drop(jobs);
    let wake = poll(&quick(), &queue);
    assert!(wake.closed && !wake.changed);
}

#[test]
fn what_tells_of_a_change() {
    let uid = Uid::new(1).unwrap();
    assert!(is_change(&Event::Exists(4)));
    assert!(is_change(&Event::Expunged(1)));
    assert!(is_change(&Event::Vanished { uids: std::iter::once((uid, uid)).collect(), earlier: false }));
    assert!(is_change(&Event::Fetched(FetchItem::default())));
    assert!(is_change(&Event::UidValidityChanged { old: UidValidity::new(1).unwrap(), new: None }));
    assert!(!is_change(&Event::Idling));
    assert!(!is_change(&Event::Alert("hello".to_owned())));
}
