// Copyright (c) TPT Solutions
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Shadow-mode pilot host. It admits each proposed tool call against the pilot budget and
//! records the result, but it does not run the tool.
//!
//! The numbers match `examples/pilot/quota.policy`. Run with
//! `cargo run -p tpt-norma-runtime --example pilot_shadow`.

use std::time::Duration;

use tpt_norma_core::PolicyBounds;
use tpt_norma_eval::{
    ActionId, ActionRequest, ActorId, CanonicalBytes, CoreEvaluator, ResourceId, StateVersion,
};
use tpt_norma_runtime::{Gate, Refusal, StateStore};

const CALLS_TO_PROPOSE: u64 = 105;

fn main() {
    let policy = CoreEvaluator {
        policy: PolicyBounds {
            max_capacity: 101.0,
            max_delta_per_call: 1.0,
        },
        policy_version: 1,
        verifier_identity: "tpt-norma-core/validate_action".into(),
    };
    let mut gate = Gate::new(policy, StateStore::new(0.0), Duration::from_millis(100));

    let mut admitted = 0;
    for n in 0..CALLS_TO_PROPOSE {
        let request = ActionRequest {
            action_id: ActionId(format!("call-{n}")),
            actor: ActorId("pilot-agent".into()),
            resource: ResourceId("pilot-calls".into()),
            expected_state_version: StateVersion(gate.store().version().0),
            arguments: CanonicalBytes::from_delta(1.0),
        };
        match gate.admit(&request) {
            Ok(()) => admitted += 1,
            Err(Refusal::Denied { violations }) => {
                println!("call {n:>3}: denied ({violations:?}), tool not run");
            }
            Err(other) => println!("call {n:>3}: refused ({other:?})"),
        }
    }

    println!("admitted {admitted} of {CALLS_TO_PROPOSE} proposed calls");
    println!("audit entries: {}", gate.log().len());
}
