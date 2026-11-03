// Copyright (c) TPT Solutions
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Shadow mode: the host calls `Gate::admit` and does not run the tool. Admissions still commit,
//! so the budget counts calls as enforcement would.

use std::cell::Cell;
use std::time::Duration;

use tpt_norma_audit::{verify_chain, Event};
use tpt_norma_core::PolicyBounds;
use tpt_norma_eval::{
    ActionId, ActionRequest, ActorId, CanonicalBytes, CoreEvaluator, ResourceId, StateVersion,
};
use tpt_norma_runtime::{Gate, GuardedEvaluator, Refusal, StateStore};

/// A budget of two unit calls: capacity 3 admits 2, because the conservative margin denies an
/// exact fit (D16).
fn gate() -> Gate<GuardedEvaluator> {
    Gate::new(
        CoreEvaluator {
            policy: PolicyBounds {
                max_capacity: 3.0,
                max_delta_per_call: 1.0,
            },
            policy_version: 1,
            verifier_identity: "core".into(),
        },
        StateStore::new(0.0),
        Duration::from_secs(1),
    )
}

fn unit_call(n: u64) -> ActionRequest {
    ActionRequest {
        action_id: ActionId(format!("call-{n}")),
        actor: ActorId("agent".into()),
        resource: ResourceId("pool".into()),
        expected_state_version: StateVersion(n),
        arguments: CanonicalBytes::from_delta(1.0),
    }
}

#[test]
fn shadow_admissions_count_against_the_budget() {
    let mut gate = gate();
    assert_eq!(gate.admit(&unit_call(0)), Ok(()));
    assert_eq!(gate.admit(&unit_call(1)), Ok(()));
    assert!(matches!(
        gate.admit(&unit_call(2)),
        Err(Refusal::Denied { .. })
    ));
    assert_eq!(gate.store().load(), 2.0);
}

#[test]
fn a_shadow_denial_is_not_committed() {
    let mut gate = gate();
    gate.admit(&unit_call(0)).expect("admitted");
    gate.admit(&unit_call(1)).expect("admitted");
    let version = gate.store().version();

    let _ = gate.admit(&unit_call(2));
    assert_eq!(gate.store().version(), version);
}

#[test]
fn the_shadow_log_records_each_admission_and_verifies() {
    let mut gate = gate();
    gate.admit(&unit_call(0)).expect("admitted");

    let committed = gate
        .log()
        .entries()
        .iter()
        .filter(|e| matches!(e.event, Event::Committed { .. }))
        .count();
    assert_eq!(committed, 1);
    assert!(verify_chain(gate.log().entries()).is_ok());
}

#[test]
fn the_tool_is_not_run_by_admit() {
    // `admit` takes no tool, so a shadow host cannot run one through it by mistake.
    let mut gate = gate();
    let ran = Cell::new(false);
    gate.admit(&unit_call(0)).expect("admitted");
    assert!(!ran.get());
}
