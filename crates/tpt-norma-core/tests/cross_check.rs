// Copyright (c) TPT Solutions
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Property tests cross-checking `validate_action` (f64) against
//! `symbolic_evaluate_ideal` (exact rationals), per spec §6's realization contracts.

use proptest::prelude::*;
use tpt_math_exact::BigRational;
use tpt_norma_core::{
    symbolic_evaluate_ideal, validate_action, ActionProposal, Decision, IdealDecision,
    PolicyBounds, SystemState,
};

fn exact(x: f64) -> BigRational {
    BigRational::from_float(x).expect("finite input")
}

/// Upper bound on the rounding error of one f64 addition `load + delta`.
/// The realization's conservative margin is derived from this same quantity.
fn load_error_bound(load: f64, delta: f64) -> f64 {
    (load.abs() + delta.abs()) * f64::EPSILON
}

/// Finite f64s, weighted toward the boundary-prone magnitudes: zero, subnormals,
/// unit-scale values, and large magnitudes where ulps are coarse.
fn finite_f64() -> impl Strategy<Value = f64> {
    prop_oneof![
        Just(0.0),
        Just(-0.0),
        Just(f64::MIN_POSITIVE),
        Just(f64::MAX),
        Just(-f64::MAX),
        any::<f64>().prop_filter("finite", |x| x.is_finite()),
        (-1.0e6f64..1.0e6f64),
        (-1.0e15f64..1.0e15f64),
    ]
}

/// Generate a state, policy and delta that land near the capacity boundary, where
/// ideal and f64 decisions are most likely to disagree.
fn boundary_case() -> impl Strategy<Value = (SystemState, ActionProposal, PolicyBounds)> {
    (finite_f64(), finite_f64(), -8i64..=8, finite_f64()).prop_map(
        |(load, cap_seed, ulps, max_delta_seed)| {
            let load = load.abs();
            let max_capacity = load + cap_seed.abs();
            // Land the projected load a few ulps around the capacity boundary.
            let mut target = max_capacity;
            for _ in 0..ulps.unsigned_abs() {
                target = if ulps > 0 {
                    next_up(target)
                } else {
                    next_down(target)
                };
            }
            let delta = target - load;
            let max_delta_per_call = delta.abs() + max_delta_seed.abs();
            (
                SystemState { current_load: load },
                ActionProposal {
                    resource_delta: delta,
                },
                PolicyBounds {
                    max_capacity,
                    max_delta_per_call,
                },
            )
        },
    )
}

fn next_up(x: f64) -> f64 {
    if x.is_nan() || x == f64::INFINITY {
        return x;
    }
    if x == 0.0 {
        return f64::from_bits(1);
    }
    let bits = x.to_bits();
    f64::from_bits(if x > 0.0 { bits + 1 } else { bits - 1 })
}

fn next_down(x: f64) -> f64 {
    -next_up(-x)
}

fn check_realization(
    state: SystemState,
    proposal: ActionProposal,
    policy: PolicyBounds,
) -> Result<(), TestCaseError> {
    let ideal = symbolic_evaluate_ideal(state, proposal, policy);
    let result = validate_action(state, proposal, policy);

    // Conservative denial: an ideal Deny is never an f64 Allow.
    if ideal == IdealDecision::Deny {
        prop_assert_eq!(
            result,
            Decision::Deny,
            "false allow: state={:?} proposal={:?} policy={:?}",
            state,
            proposal,
            policy
        );
    }

    // Bounded numerical error on allow, checked exactly.
    if let (
        IdealDecision::Allow {
            new_load: exact_load,
        },
        Decision::Allow { new_load },
    ) = (&ideal, result)
    {
        let bound = exact(load_error_bound(
            state.current_load,
            proposal.resource_delta,
        ));
        let gap = (exact_load - exact(new_load)).abs();
        prop_assert!(
            gap <= bound,
            "numerical error {} exceeds bound {}: state={:?} proposal={:?}",
            gap,
            bound,
            state,
            proposal
        );
    }
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(4096))]

    #[test]
    fn realization_matches_ideal_on_arbitrary_finite_inputs(
        load in finite_f64(),
        delta in finite_f64(),
        max_capacity in finite_f64(),
        max_delta_per_call in finite_f64(),
    ) {
        check_realization(
            SystemState { current_load: load },
            ActionProposal { resource_delta: delta },
            PolicyBounds { max_capacity, max_delta_per_call },
        )?;
    }

    #[test]
    fn realization_matches_ideal_near_capacity_boundary((state, proposal, policy) in boundary_case()) {
        check_realization(state, proposal, policy)?;
    }
}

#[test]
fn ideal_evaluator_is_exact_at_the_boundary() {
    // 0.1 + 0.2 is not exactly 0.3 in f64, but the ideal layer compares exact values.
    let state = SystemState { current_load: 0.1 };
    let proposal = ActionProposal {
        resource_delta: 0.2,
    };
    let policy = PolicyBounds {
        max_capacity: 0.3,
        max_delta_per_call: 1.0,
    };
    assert_eq!(
        symbolic_evaluate_ideal(state, proposal, policy),
        IdealDecision::Deny
    );
    assert_eq!(validate_action(state, proposal, policy), Decision::Deny);
}
