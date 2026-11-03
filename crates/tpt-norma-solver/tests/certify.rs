// Copyright (c) TPT Solutions
// SPDX-License-Identifier: MIT OR Apache-2.0

use proptest::prelude::*;
use tpt_norma_core::{validate_action, ActionProposal, Decision as CoreDecision, PolicyBounds, SystemState};
use tpt_norma_eval::{
    ActionId, ActionRequest, ActorId, CanonicalBytes, CoreEvaluator, Decision, EvaluationFailure,
    PolicyEvaluator, ResourceId, StateSnapshot, StateVersion,
};
use tpt_norma_solver::{certify, CertifiedEvaluator, Certification};

fn policy(max_capacity: f64, max_delta_per_call: f64) -> PolicyBounds {
    PolicyBounds {
        max_capacity,
        max_delta_per_call,
    }
}

fn request(delta: f64) -> ActionRequest {
    ActionRequest {
        action_id: ActionId("a".into()),
        actor: ActorId("agent".into()),
        resource: ResourceId("r".into()),
        expected_state_version: StateVersion(0),
        arguments: CanonicalBytes::from_delta(delta),
    }
}

fn snapshot(load: f64) -> StateSnapshot {
    StateSnapshot {
        version: StateVersion(0),
        current_load: Some(load),
    }
}

#[test]
fn allow_within_capacity_is_verified() {
    assert_eq!(certify(2.0, 3.0, policy(10.0, 5.0), Some(5.0)), Certification::Verified);
}

#[test]
fn allow_over_capacity_is_contradicted() {
    // A claimed Allow whose exact sum exceeds capacity: the solver refutes it.
    assert_eq!(certify(9.0, 4.0, policy(10.0, 5.0), Some(13.0)), Certification::Contradicted);
}

#[test]
fn allow_with_new_load_off_by_more_than_the_error_bound_is_contradicted() {
    assert_eq!(certify(2.0, 3.0, policy(10.0, 5.0), Some(5.5)), Certification::Contradicted);
}

#[test]
fn allow_with_new_load_within_the_error_bound_is_verified() {
    // One ulp away from the exact sum is inside EPSILON * (|load| + |delta|).
    let claim = f64::from_bits(5.0f64.to_bits() + 1);
    assert_eq!(certify(2.0, 3.0, policy(10.0, 5.0), Some(claim)), Certification::Verified);
}

#[test]
fn deny_for_a_real_violation_is_certified() {
    assert_eq!(certify(0.0, 6.0, policy(100.0, 5.0), None), Certification::CertifiedDenial);
    assert_eq!(certify(9.0, 4.0, policy(10.0, 5.0), None), Certification::CertifiedDenial);
}

#[test]
fn deny_that_the_ideal_layer_allows_is_a_conservative_denial() {
    // Exact sum 10 equals capacity 10, which the ideal layer allows. The f64 check denies.
    assert_eq!(certify(5.0, 5.0, policy(10.0, 10.0), None), Certification::ConservativeDenial);
}

#[test]
fn values_outside_the_exact_range_are_inconclusive() {
    assert_eq!(certify(1e-300, 1.0, policy(10.0, 5.0), Some(1.0)), Certification::Inconclusive);
    assert_eq!(certify(1e300, 1.0, policy(1e300, 5.0), Some(1e300)), Certification::Inconclusive);
}

struct Lying;
impl PolicyEvaluator for Lying {
    fn evaluate(&self, request: &ActionRequest, snapshot: &StateSnapshot) -> Decision {
        // Allows everything, including actions over capacity.
        CoreEvaluator {
            policy: policy(1e9, 1e9),
            policy_version: 1,
            verifier_identity: "lying".into(),
        }
        .evaluate(request, snapshot)
    }
}

#[test]
fn certified_evaluator_refutes_an_allow_the_solver_contradicts() {
    let certified = CertifiedEvaluator::new(Lying, policy(10.0, 5.0));
    let decision = certified.evaluate(&request(4.0), &snapshot(9.0));
    assert_eq!(
        decision,
        Decision::Indeterminate {
            reason: EvaluationFailure::SolverRejected,
        }
    );
}

#[test]
fn certified_evaluator_passes_a_verified_allow_and_keeps_denials() {
    let certified = CertifiedEvaluator::new(
        CoreEvaluator {
            policy: policy(10.0, 5.0),
            policy_version: 3,
            verifier_identity: "core".into(),
        },
        policy(10.0, 5.0),
    );
    assert!(certified.evaluate(&request(3.0), &snapshot(2.0)).is_allow());
    assert!(matches!(
        certified.evaluate(&request(6.0), &snapshot(2.0)),
        Decision::Deny { .. }
    ));
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// Every decision the f64 validator makes is accepted by the solver: Allow is verified,
    /// and Deny is never contradicted.
    #[test]
    fn solver_accepts_the_f64_validator_on_random_inputs(
        load in -1.0e6f64..1.0e6,
        delta in -1.0e6f64..1.0e6,
        cap in 0.0f64..2.0e6,
        max_delta in 0.0f64..2.0e6,
    ) {
        let p = policy(cap, max_delta);
        let state = SystemState { current_load: load };
        let proposal = ActionProposal { resource_delta: delta };
        match validate_action(state, proposal, p) {
            CoreDecision::Allow { new_load } => {
                prop_assert_eq!(certify(load, delta, p, Some(new_load)), Certification::Verified);
            }
            CoreDecision::Deny => {
                let c = certify(load, delta, p, None);
                prop_assert!(
                    c == Certification::CertifiedDenial || c == Certification::ConservativeDenial
                        || c == Certification::Inconclusive,
                    "denial certified as {c:?} for load={load} delta={delta} cap={cap} max={max_delta}"
                );
            }
        }
    }
}
