//! Acceptance tests for the governor: snapshot/registry agreement and watch admission.
use kube::core::GroupVersionKind;
use openkite_engine::governor::{Admission, Decision, Governor, RequestKind};
use openkite_engine::interest::{InformerState, Interest, InterestClass, Scope};

fn interest(index: usize, class: InterestClass) -> Interest {
    Interest {
        gvk: GroupVersionKind {
            group: String::new(),
            version: "v1".to_owned(),
            kind: "Pod".to_owned(),
        },
        scope: Scope::Namespace(format!("ns-{index}")),
        selector: None,
        class,
    }
}

#[test]
fn snapshot_matches_registry_state_under_churn() {
    let governor = Governor::new(16, 20.0, 40.0);

    let pinned = interest(0, InterestClass::Pinned);
    let visible = interest(1, InterestClass::Visible);
    let probe = interest(2, InterestClass::Probe);

    assert_eq!(governor.acquire(&pinned), Admission::Admitted);
    assert_eq!(governor.acquire(&visible), Admission::Admitted);
    assert_eq!(governor.acquire(&probe), Admission::Admitted);
    // Second acquire on the same key bumps the refcount, not a new watch.
    governor.acquire(&visible);

    governor.set_state(&probe, InformerState::Parked);
    governor.release(&pinned);

    let snapshot = governor.snapshot();
    assert_eq!(
        governor.state(&pinned),
        Some(InformerState::Parked),
        "releasing the only holder parks the watch immediately"
    );
    assert_eq!(governor.state(&visible), Some(InformerState::Running));
    assert_eq!(governor.state(&probe), Some(InformerState::Parked));

    assert_eq!(snapshot.watches_running, 1, "only `visible` is running");
    assert_eq!(
        snapshot.watches_parked, 2,
        "`pinned` and `probe` are parked"
    );
    assert_eq!(snapshot.running_by_class.visible, 1);
    assert_eq!(snapshot.parked_by_class.pinned, 1);
    assert_eq!(snapshot.parked_by_class.probe, 1);
    assert_eq!(governor.count(), 3, "three distinct interests are tracked");
}

#[test]
fn seventeenth_watch_parks_instead_of_opening() {
    let governor = Governor::new(16, 20.0, 40.0);

    for index in 0..16 {
        assert_eq!(
            governor.acquire(&interest(index, InterestClass::Visible)),
            Admission::Admitted,
            "watch {index} is within the limit"
        );
    }

    let seventeenth = interest(16, InterestClass::Visible);
    assert_eq!(
        governor.acquire(&seventeenth),
        Admission::Parked,
        "the 17th watch must park instead of opening"
    );
    assert_eq!(governor.state(&seventeenth), Some(InformerState::Parked));

    let snapshot = governor.snapshot();
    assert_eq!(snapshot.watches_running, 16);
    assert_eq!(snapshot.watches_parked, 1);
    assert!(
        !snapshot.warnings.is_empty(),
        "hitting the watch limit must surface a warning"
    );
}

#[test]
fn higher_class_evicts_the_lowest_class_running_watch() {
    let governor = Governor::new(16, 20.0, 40.0);

    for index in 0..16 {
        assert_eq!(
            governor.acquire(&interest(index, InterestClass::Probe)),
            Admission::Admitted
        );
    }

    let pinned = interest(16, InterestClass::Pinned);
    assert_eq!(
        governor.acquire(&pinned),
        Admission::Admitted,
        "a pinned watch evicts a probe when the limit is full"
    );
    assert_eq!(governor.state(&pinned), Some(InformerState::Running));

    let snapshot = governor.snapshot();
    assert_eq!(snapshot.watches_running, 16, "limit is still respected");
    assert_eq!(snapshot.watches_parked, 1, "exactly one probe was parked");
    assert_eq!(snapshot.parked_by_class.probe, 1);
}

#[test]
fn writes_queue_and_probes_shed_once_tokens_are_exhausted() {
    let governor = Governor::new(16, 0.0, 1.0);

    assert_eq!(governor.check(RequestKind::Write), Decision::Allow);
    assert_eq!(governor.check(RequestKind::Write), Decision::Queued);
    assert_eq!(governor.check(RequestKind::Probe), Decision::Shed);

    let snapshot = governor.snapshot();
    assert_eq!(snapshot.queued_writes, 1);
    assert_eq!(snapshot.shed_probes, 1);
}
