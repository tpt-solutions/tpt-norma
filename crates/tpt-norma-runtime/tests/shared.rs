// Copyright (c) TPT Solutions
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;

use std::time::Duration;
use tpt_norma_core::PolicyBounds;
use tpt_norma_eval::{
    ActionId, ActionRequest, ActorId, CanonicalBytes, CoreEvaluator, ResourceId, StateVersion,
};

use tpt_norma_runtime::{Gate, GuardedEvaluator, Refusal, SharedGate, StateStore};

fn shared(load: f64, capacity: f64) -> SharedGate<GuardedEvaluator> {
    SharedGate::new(Gate::new(
        CoreEvaluator {
            policy: PolicyBounds {
                max_capacity: capacity,
                max_delta_per_call: 1.0,
            },
            policy_version: 1,
            verifier_identity: "core".into(),
        },
        StateStore::new(load),
        Duration::from_secs(1),
    ))
}

fn request_at(delta: f64, version: StateVersion, id: usize) -> ActionRequest {
    ActionRequest {
        action_id: ActionId(format!("tx-{id}")),
        actor: ActorId("agent".into()),
        resource: ResourceId("pool".into()),
        expected_state_version: version,
        arguments: CanonicalBytes::from_delta(delta),
    }
}

#[test]
fn concurrent_callers_never_admit_more_than_capacity_allows() {
    // Unit steps from load 0 against capacity 10.5 admit exactly 10 actions, however the threads
    // interleave. Capacity 10 would admit only 9: the conservative margin denies an exact match.
    // Without per-resource serialization, two callers could both read load 9 and both be admitted.
    const THREADS: usize = 32;
    const CAPACITY: usize = 10;

    let gate = Arc::new(shared(0.0, CAPACITY as f64 + 0.5));
    let admitted = Arc::new(AtomicUsize::new(0));

    let handles: Vec<_> = (0..THREADS)
        .map(|id| {
            let gate = Arc::clone(&gate);
            let admitted = Arc::clone(&admitted);
            thread::spawn(move || loop {
                // A stale request is refused, so re-read the version and resubmit. A capacity
                // denial is final for this action.
                let version = gate.version().expect("lock not poisoned");
                let req = request_at(1.0, version, id);
                match gate.submit(&req, |_| ()) {
                    Ok(()) => {
                        admitted.fetch_add(1, Ordering::SeqCst);
                        return;
                    }
                    Err(Refusal::Indeterminate { .. }) => continue,
                    Err(Refusal::Denied { .. }) => return,
                    Err(other) => panic!("unexpected refusal: {other:?}"),
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().expect("thread completes");
    }

    assert_eq!(admitted.load(Ordering::SeqCst), CAPACITY);
    assert_eq!(gate.load(), Some(CAPACITY as f64));
    assert_eq!(gate.version(), Some(StateVersion(CAPACITY as u64)));
}

#[test]
fn the_tool_runs_outside_the_lock() {
    // If the tool ran while the lock was held, a tool that reads the gate would deadlock.
    // Completing shows the lock is released first.
    let gate = shared(0.0, 10.0);
    let version = gate.version().expect("lock");
    let req = request_at(1.0, version, 0);
    let out = gate.submit(&req, |_| gate.load());
    assert_eq!(out, Ok(Some(1.0)));
}
