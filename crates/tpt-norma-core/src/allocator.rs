// Copyright (c) TPT Solutions
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::{check_action, ActionProposal, PolicyBounds, Rejection, SystemState};

/// A rejection in one dimension of a multi-resource proposal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DimensionRejection {
    pub dimension: usize,
    pub reason: Rejection,
}

/// Validate one proposal across several independent resource dimensions.
///
/// The check is all-or-nothing. Every dimension is checked with [`check_action`], and the call
/// returns the new loads only if all of them pass. Otherwise it returns the first rejected
/// dimension.
///
/// Each dimension is checked in isolation. The composition is sound only when the dimensions are
/// orthogonal, meaning no dimension's action changes an input that another dimension's check
/// depends on (spec §5.2). That assumption is not verified here, so callers must establish it
/// for their policy.
///
/// Panics if the three slices differ in length.
pub fn validate_multi(
    loads: &[f64],
    deltas: &[f64],
    policies: &[PolicyBounds],
) -> Result<Vec<f64>, DimensionRejection> {
    assert!(
        loads.len() == deltas.len() && deltas.len() == policies.len(),
        "one load, delta and policy per dimension"
    );

    let mut new_loads = Vec::with_capacity(loads.len());
    for (dimension, ((&load, &delta), &policy)) in
        loads.iter().zip(deltas).zip(policies).enumerate()
    {
        let state = SystemState { current_load: load };
        let proposal = ActionProposal {
            resource_delta: delta,
        };
        match check_action(state, proposal, policy) {
            Ok(new_load) => new_loads.push(new_load),
            Err(reason) => return Err(DimensionRejection { dimension, reason }),
        }
    }
    Ok(new_loads)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bounds(max_capacity: f64) -> PolicyBounds {
        PolicyBounds {
            max_capacity,
            max_delta_per_call: 5.0,
        }
    }

    #[test]
    fn all_dimensions_pass_returns_every_new_load() {
        let policies = vec![bounds(10.0); 3];
        assert_eq!(
            validate_multi(&[1.0, 2.0, 3.0], &[1.0, 1.0, 1.0], &policies),
            Ok(vec![2.0, 3.0, 4.0])
        );
    }

    #[test]
    fn one_rejected_dimension_rejects_the_whole_proposal() {
        let policies = vec![bounds(10.0), bounds(10.0), bounds(10.0)];
        let result = validate_multi(&[1.0, 9.0, 3.0], &[1.0, 4.0, 1.0], &policies);
        assert_eq!(
            result,
            Err(DimensionRejection {
                dimension: 1,
                reason: Rejection::Capacity,
            })
        );
    }

    #[test]
    fn the_first_rejection_is_reported() {
        let policies = vec![bounds(10.0); 2];
        let result = validate_multi(&[0.0, 0.0], &[-1.0, 9.0], &policies);
        assert_eq!(result.unwrap_err().dimension, 0);
    }
}
