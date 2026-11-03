// Copyright (c) TPT Solutions
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Deterministic boundary check of the conservative-denial claim (docs/conservative-denial.md).
//!
//! For each capacity in a fixed list, including powers of two, the smallest normal, and
//! subnormals, this sweeps the delta over a few ulps around the exact capacity boundary, for a
//! few loads. Every case where the exact oracle denies must also be denied by `validate_action`.

use tpt_norma_core::{
    symbolic_evaluate_ideal, validate_action, ActionProposal, Decision, IdealDecision,
    PolicyBounds, SystemState,
};

fn next_up(x: f64) -> f64 {
    f64::from_bits(if x >= 0.0 {
        x.to_bits() + 1
    } else {
        x.to_bits() - 1
    })
}

fn next_down(x: f64) -> f64 {
    f64::from_bits(if x > 0.0 {
        x.to_bits() - 1
    } else {
        x.to_bits() + 1
    })
}

fn nudge(mut x: f64, ulps: i64) -> f64 {
    for _ in 0..ulps.unsigned_abs() {
        x = if ulps > 0 { next_up(x) } else { next_down(x) };
    }
    x
}

#[test]
fn no_false_allow_near_special_capacities() {
    let capacities = [
        f64::MIN_POSITIVE,
        2.0_f64.powi(-1021),
        2.0_f64.powi(-1023),
        f64::MIN_POSITIVE - f64::from_bits(1),
        f64::from_bits(1),
        f64::from_bits(3),
        1.0,
        0.1,
        2.0_f64.powi(100),
        f64::MAX / 4.0,
    ];

    let mut checked = 0usize;
    for max_capacity in capacities {
        let policy = PolicyBounds {
            max_capacity,
            max_delta_per_call: f64::MAX,
        };
        for load in [
            0.0,
            max_capacity * 0.5,
            nudge(max_capacity, -1),
            max_capacity,
        ] {
            let base = max_capacity - load;
            for ulps in -4..=4 {
                let delta = nudge(base, ulps);
                if delta < 0.0 {
                    continue;
                }
                let state = SystemState { current_load: load };
                let proposal = ActionProposal {
                    resource_delta: delta,
                };
                let ideal = symbolic_evaluate_ideal(state, proposal, policy);
                let code = validate_action(state, proposal, policy);
                if matches!(ideal, IdealDecision::Deny) {
                    assert!(
                        matches!(code, Decision::Deny),
                        "false allow: capacity {max_capacity:e}, load {load:e}, delta {delta:e}"
                    );
                }
                checked += 1;
            }
        }
    }
    assert!(checked > 0);
}
