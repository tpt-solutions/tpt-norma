// Copyright (c) TPT Solutions
// SPDX-License-Identifier: MIT OR Apache-2.0

use tpt_math_exact::BigRational;

use crate::{ActionProposal, PolicyBounds, SystemState};

/// Outcome of the exact-rational reference evaluator.
#[derive(Debug, Clone, PartialEq)]
pub enum IdealDecision {
    Allow { new_load: BigRational },
    Deny,
}

/// The exact-rational reference evaluator the realization-layer contracts refer to
/// (spec §6, `symbolic_evaluate_ideal`).
///
/// Each `f64` input is converted to its exact rational value, so every comparison
/// and sum here is exact. This function is the ideal-layer oracle that
/// [`crate::validate_action`] is checked against. Non-finite inputs have no exact
/// value, so they are denied.
pub fn symbolic_evaluate_ideal(
    state: SystemState,
    proposal: ActionProposal,
    policy: PolicyBounds,
) -> IdealDecision {
    let (Some(load), Some(delta), Some(max_capacity), Some(max_delta)) = (
        BigRational::from_float(state.current_load),
        BigRational::from_float(proposal.resource_delta),
        BigRational::from_float(policy.max_capacity),
        BigRational::from_float(policy.max_delta_per_call),
    ) else {
        return IdealDecision::Deny;
    };
    let zero = BigRational::from_float(0.0).expect("0.0 is finite");

    if delta < zero || delta > max_delta {
        return IdealDecision::Deny;
    }

    let new_load = load + delta;
    if new_load > max_capacity {
        return IdealDecision::Deny;
    }

    IdealDecision::Allow { new_load }
}
