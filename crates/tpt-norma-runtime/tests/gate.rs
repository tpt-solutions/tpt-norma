// Copyright (c) TPT Solutions
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::cell::Cell;

use std::time::Duration;
use tpt_norma_audit::{verify_chain, Event};
use tpt_norma_core::PolicyBounds;
use tpt_norma_eval::{
    ActionId, ActionRequest, ActorId, CanonicalBytes, CoreEvaluator, ResourceId, StateVersion,
};

use tpt_norma_runtime::{Gate, GuardedEvaluator, Refusal, StateStore};

fn gate(load: f64) -> Gate<GuardedEvaluator> {
    Gate::new(
        CoreEvaluator {
            policy: PolicyBounds {
                max_capacity: 10.0,
                max_delta_per_call: 5.0,
            },
            policy_version: 2,
            verifier_identity: "core".into(),
        },
        StateStore::new(load),
        Duration::from_secs(1),
    )
}

fn request(delta: f64, version: u64) -> ActionRequest {
    ActionRequest {
        action_id: ActionId("tx".into()),
        actor: ActorId("agent".into()),
        resource: ResourceId("pool".into()),
        expected_state_version: StateVersion(version),
        arguments: CanonicalBytes::from_delta(delta),
    }
}

#[test]
fn an_allowed_request_runs_the_tool_and_advances_the_state() {
    let mut gate = gate(2.0);
    let ran = Cell::new(0);
    let out = gate.submit(&request(3.0, 0), |_| {
        ran.set(ran.get() + 1);
        "done"
    });
    assert_eq!(out, Ok("done"));
    assert_eq!(ran.get(), 1);
    assert_eq!(gate.store().load(), 5.0);
    assert_eq!(gate.store().version(), StateVersion(1));
    assert_eq!(verify_chain(gate.log().entries()), Ok(()));
}

#[test]
fn a_denied_request_never_reaches_the_tool() {
    let mut gate = gate(2.0);
    let ran = Cell::new(false);
    let out = gate.submit(&request(6.0, 0), |_| ran.set(true));
    assert!(matches!(out, Err(Refusal::Denied { .. })));
    assert!(!ran.get(), "the tool must not run on a denial");
    assert_eq!(gate.store().load(), 2.0, "a denial must not change state");
    assert_eq!(gate.store().version(), StateVersion(0));
}

#[test]
fn a_stale_request_is_indeterminate_and_does_not_run() {
    let mut gate = gate(0.0);
    let ran = Cell::new(false);
    let out = gate.submit(&request(1.0, 4), |_| ran.set(true));
    assert!(matches!(out, Err(Refusal::Indeterminate { .. })));
    assert!(!ran.get());
}

#[test]
fn every_lifecycle_step_is_recorded_and_the_chain_verifies() {
    let mut gate = gate(2.0);
    let _ = gate.submit(&request(3.0, 0), |_| ());
    let _ = gate.submit(&request(9.0, 1), |_| ());

    let entries = gate.log().entries();
    assert_eq!(verify_chain(entries), Ok(()));
    let kinds: Vec<&str> = entries
        .iter()
        .map(|e| match e.event {
            Event::Requested { .. } => "requested",
            Event::Decided { .. } => "decided",
            Event::Committed { .. } => "committed",
            Event::Refused { .. } => "refused",
        })
        .collect();
    assert_eq!(
        kinds,
        [
            "requested",
            "decided",
            "committed",
            "requested",
            "decided",
            "refused"
        ]
    );
}

#[test]
fn a_tampered_audit_copy_fails_verification() {
    let mut gate = gate(2.0);
    let _ = gate.submit(&request(3.0, 0), |_| ());
    let mut copy = gate.log().entries().to_vec();
    copy[2].event = Event::Committed {
        new_version: 1,
        new_load_bits: 9.0f64.to_bits(),
    };
    assert!(verify_chain(&copy).is_err());
}
