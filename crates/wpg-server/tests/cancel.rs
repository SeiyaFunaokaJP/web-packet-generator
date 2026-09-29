//! Cancel safety: dropping the application state (or the job manager) stops the
//! TX worker threads of running jobs. Kept in its own test binary so that no
//! other test's threads are counted.

mod common;

use std::time::Duration;

use wpg_server::jobs::{JobSpec, JobStream};
use wpg_server::AppState;

/// Threads of this process whose name starts with `wpg-tx` (Linux).
fn tx_threads() -> usize {
    std::fs::read_dir("/proc/self/task")
        .map(|d| {
            d.filter_map(Result::ok)
                .filter_map(|t| std::fs::read_to_string(t.path().join("comm")).ok())
                .filter(|c| c.starts_with("wpg-tx"))
                .count()
        })
        .unwrap_or(0)
}

fn endless() -> JobSpec {
    JobSpec {
        streams: vec![JobStream {
            template_id: Some("examples-arp-sweep".into()),
            weight: 1,
            ..JobStream::default()
        }],
        backend: Some("null".into()),
        rate: Some("unlimited".into()),
        count: Some(u64::MAX / 2),
        workers: Some(2),
        ..JobSpec::default()
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn dropping_the_state_stops_worker_threads() {
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::new(common::config(&dir)).unwrap();
    let a = state.jobs.create(endless(), true).await.unwrap();
    let b = state.jobs.create(endless(), true).await.unwrap();
    tokio::time::sleep(Duration::from_millis(200)).await;
    if cfg!(target_os = "linux") {
        // 2 jobs × (2 workers + monitor)
        assert_eq!(tx_threads(), 6);
    }
    let st = state.jobs.stats(&a.id).unwrap();
    assert!(st.packets > 0);
    assert_eq!(b.workers, 2);

    // Deleting one job joins its threads.
    state.jobs.delete(&a.id).await.unwrap();
    if cfg!(target_os = "linux") {
        assert_eq!(tx_threads(), 3);
    }
    // Dropping everything (e.g. an aborted server task) stops the rest.
    tokio::task::spawn_blocking(move || drop(state))
        .await
        .unwrap();
    // the monitor task may still hold a strong reference for one tick
    for _ in 0..100 {
        if tx_threads() == 0 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert_eq!(tx_threads(), 0);
}
