// FORK-ONLY probe for the `JUnit` report pipeline. Never merge upstream.

use std::sync::atomic::{AtomicU32, Ordering};

#[test]
fn probe_passes() {
    assert_eq!(2 + 2, 4);
}

/// Same shape as the failure the reporting work is meant to surface (PR #5247).
#[test]
fn a_rate_limit_seen_by_one_request_pauses_every_other_request() {
    let paused = AtomicU32::new(0);
    paused.fetch_add(1, Ordering::SeqCst);
    assert_eq!(
        paused.load(Ordering::SeqCst),
        3,
        "every other in-flight request should have been paused"
    );
}

#[test]
#[ignore = "probe: reported as skipped"]
fn probe_ignored() {}
