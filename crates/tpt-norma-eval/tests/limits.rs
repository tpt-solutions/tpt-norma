// Copyright (c) TPT Solutions
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::time::Duration;

use tpt_norma_core::PolicyBounds;
use tpt_norma_eval::{
    ActionId, ActionRequest, ActorId, CanonicalBytes, CoreEvaluator, Decision, EvaluationFailure,
    Guarded, PolicyEvaluator, ResourceId, StateSnapshot, StateVersion, DEFAULT_MAX_ARGUMENT_BYTES,
};

fn evaluator() -> CoreEvaluator {
    CoreEvaluator {
        policy: PolicyBounds {
            max_capacity: 10.0,
            max_delta_per_call: 5.0,
        },
        policy_version: 1,
        verifier_identity: "core".into(),
    }
}

fn snapshot() -> StateSnapshot {
    StateSnapshot {
        version: StateVersion(0),
        current_load: Some(1.0),
    }
}

fn request(arguments: Vec<u8>) -> ActionRequest {
    ActionRequest {
        action_id: ActionId("tx".into()),
        actor: ActorId("agent".into()),
        resource: ResourceId("pool".into()),
        expected_state_version: StateVersion(0),
        arguments: CanonicalBytes::from_bytes(arguments),
    }
}

#[test]
fn oversized_arguments_are_refused_before_the_evaluator_runs() {
    let guarded = Guarded::new(evaluator(), Duration::from_secs(1));
    let big = vec![0u8; DEFAULT_MAX_ARGUMENT_BYTES + 1];
    assert_eq!(
        guarded.evaluate(&request(big.clone()), &snapshot()),
        Decision::Indeterminate {
            reason: EvaluationFailure::ArgumentsTooLarge {
                limit: DEFAULT_MAX_ARGUMENT_BYTES,
                actual: big.len(),
            },
        }
    );
}

#[test]
fn a_limit_can_be_set_and_arguments_at_the_limit_pass() {
    let guarded = Guarded::new(evaluator(), Duration::from_secs(1)).with_max_argument_bytes(8);
    let delta = 2.0f64.to_bits().to_le_bytes().to_vec();
    assert_eq!(delta.len(), 8);
    assert!(matches!(
        guarded.evaluate(&request(delta), &snapshot()),
        Decision::Allow { .. }
    ));
    let nine = vec![0u8; 9];
    assert!(matches!(
        guarded.evaluate(&request(nine), &snapshot()),
        Decision::Indeterminate {
            reason: EvaluationFailure::ArgumentsTooLarge {
                limit: 8,
                actual: 9
            }
        }
    ));
}
