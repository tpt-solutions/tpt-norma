// Copyright (c) TPT Solutions
// SPDX-License-Identifier: MIT OR Apache-2.0

/// Represents an AI agent's proposed action (e.g., amount to transfer, records to query).
///
/// Only this type is treated as adversarial input.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ActionProposal {
    pub resource_delta: f64,
}

/// Represents the current state of the system.
///
/// Assumed to come from a trusted state layer, not from the LLM.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SystemState {
    pub current_load: f64,
}

/// Represents the policy bounds enforced on the agent.
///
/// Assumed to come from trusted configuration, not from the LLM.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PolicyBounds {
    pub max_capacity: f64,
    pub max_delta_per_call: f64,
}

/// The verifier's decision.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Decision {
    Allow { new_load: f64 },
    Deny,
}

/// Formally verified stateful action validator.
///
/// # Scope note
/// All contracts assume finite, non-NaN inputs. Malformed LLM outputs (NaN, Infinity)
/// are rejected upstream by the JSON schema parser before reaching this function.
/// As a defence in depth, any non-finite input is denied here rather than allowed,
/// because every comparison against NaN is false and would otherwise fall through
/// to `Allow`.
///
/// This function validates a single proposal against a single snapshot of `SystemState`.
/// It does not itself guarantee atomicity across concurrent calls — see Section 8
/// (Threat Model & Assumptions) for the serialization requirement this places on callers.
///
/// # tpt-telos Contracts — Ideal Layer (QF_LRA, exact rationals)
/// requires:
///   policy.max_capacity >= 0.0 && policy.max_delta_per_call >= 0.0
/// requires:
///   state.current_load >= 0.0 && state.current_load <= policy.max_capacity
/// ensures: // State preservation on denial (pure function, implicit)
/// ensures: // Capacity invariant on allow
///   if let Decision::Allow { new_load } = result {
///     new_load == state.current_load + proposal.resource_delta &&
///     new_load <= policy.max_capacity
///   }
/// ensures: // Rate limit & non-negativity invariant on allow
///   if let Decision::Allow { new_load } = result {
///     proposal.resource_delta <= policy.max_delta_per_call &&
///     proposal.resource_delta >= 0.0
///   }
///
/// # tpt-telos Contracts — Realization Layer (f64, auto-derived ε)
/// ensures: // Bounded numerical error on allow
///   let ideal = symbolic_evaluate_ideal(state, proposal, policy);
///   (matches!(ideal, Decision::Allow { .. }) && matches!(result, Decision::Allow { .. })) ==>
///     |result.new_load - ideal.new_load| <= EPSILON_LOAD
/// ensures: // CONSERVATIVE DENIAL: NO FALSE ALLOWS
///   let ideal = symbolic_evaluate_ideal(state, proposal, policy); // re-bound: independent clause scope
///   matches!(ideal, Decision::Deny) ==> matches!(result, Decision::Deny)
///   // Achieved by subtracting EPSILON_ROUNDING from max_capacity during the f64 check.
///   // This intentionally sacrifices the top EPSILON_ROUNDING of capacity to guarantee
///   // that f64 addition rounding can never cause a false allow.
///   // Note: EPSILON_LOAD (numerical-error bound) and EPSILON_ROUNDING (capacity
///   // safety margin) are independently derived constants and are not interchangeable.
///   // `symbolic_evaluate_ideal` is the exact-rational oracle in `ideal.rs`.
pub fn validate_action(
    state: SystemState,
    proposal: ActionProposal,
    policy: PolicyBounds,
) -> Decision {
    match check_action(state, proposal, policy) {
        Ok(new_load) => Decision::Allow { new_load },
        Err(_) => Decision::Deny,
    }
}

/// The reason an action was rejected by [`check_action`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rejection {
    /// An input was NaN or infinite.
    NonFinite,
    /// The resource delta was negative.
    Negative,
    /// The resource delta exceeded `max_delta_per_call`.
    RateLimit,
    /// The projected load would exceed the capacity minus the rounding margin.
    Capacity,
}

/// [`validate_action`] with the reason for a denial. On success it returns the new load.
///
/// The decision logic lives here so that the reason and the decision cannot diverge.
pub fn check_action(
    state: SystemState,
    proposal: ActionProposal,
    policy: PolicyBounds,
) -> Result<f64, Rejection> {
    // 0. Fail closed on non-finite input (see scope note above).
    if !is_finite_input(state, proposal, policy) {
        return Err(Rejection::NonFinite);
    }

    // 1. Non-negativity check
    if proposal.resource_delta < 0.0 {
        return Err(Rejection::Negative);
    }

    // 2. Rate limit check
    if proposal.resource_delta > policy.max_delta_per_call {
        return Err(Rejection::RateLimit);
    }

    // 3. Capacity check (Conservative for f64)
    let projected_load = state.current_load + proposal.resource_delta;

    // Derive the maximum possible rounding error for this specific addition.
    // We use this to shrink the effective capacity, ensuring no false allows.
    let epsilon_rounding =
        (state.current_load.abs() + proposal.resource_delta.abs()) * f64::EPSILON;

    // If the projected load exceeds the capacity minus the rounding error, we MUST deny.
    // This prevents the scenario where ideal math says > max (Deny), but f64 rounds
    // down to <= max (Allow).
    if projected_load > policy.max_capacity - epsilon_rounding {
        return Err(Rejection::Capacity);
    }

    Ok(projected_load)
}

fn is_finite_input(state: SystemState, proposal: ActionProposal, policy: PolicyBounds) -> bool {
    state.current_load.is_finite()
        && proposal.resource_delta.is_finite()
        && policy.max_capacity.is_finite()
        && policy.max_delta_per_call.is_finite()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy(max_capacity: f64, max_delta_per_call: f64) -> PolicyBounds {
        PolicyBounds {
            max_capacity,
            max_delta_per_call,
        }
    }

    fn check(current_load: f64, resource_delta: f64, policy: PolicyBounds) -> Decision {
        validate_action(
            SystemState { current_load },
            ActionProposal { resource_delta },
            policy,
        )
    }

    #[test]
    fn allows_delta_well_within_capacity() {
        assert_eq!(
            check(0.0, 9.0, policy(10.0, 10.0)),
            Decision::Allow { new_load: 9.0 }
        );
    }

    #[test]
    fn denies_exact_capacity_match_by_conservative_margin() {
        // The ideal layer allows new_load == max_capacity, but the f64 check
        // sacrifices the top epsilon of capacity, so this is a deliberate false denial.
        assert_eq!(check(5.0, 5.0, policy(10.0, 10.0)), Decision::Deny);
    }

    #[test]
    fn denies_epsilon_sized_overshoot() {
        assert_eq!(check(0.0, 10.0 + 1e-12, policy(10.0, 20.0)), Decision::Deny);
    }

    #[test]
    fn denies_negative_delta() {
        assert_eq!(check(5.0, -1.0, policy(10.0, 10.0)), Decision::Deny);
    }

    #[test]
    fn denies_delta_exceeding_max_delta_per_call() {
        assert_eq!(check(0.0, 6.0, policy(100.0, 5.0)), Decision::Deny);
    }

    #[test]
    fn zero_bounds_allow_only_a_zero_delta() {
        assert_eq!(
            check(0.0, 0.0, policy(0.0, 0.0)),
            Decision::Allow { new_load: 0.0 }
        );
        assert_eq!(check(0.0, 1e-300, policy(0.0, 0.0)), Decision::Deny);
    }

    #[test]
    fn denies_non_finite_inputs() {
        let bounds = policy(10.0, 10.0);
        assert_eq!(check(f64::NAN, 1.0, bounds), Decision::Deny);
        assert_eq!(check(0.0, f64::NAN, bounds), Decision::Deny);
        assert_eq!(check(0.0, f64::INFINITY, bounds), Decision::Deny);
        assert_eq!(check(0.0, 1.0, policy(f64::NAN, 10.0)), Decision::Deny);
        assert_eq!(check(0.0, 1.0, policy(10.0, f64::INFINITY)), Decision::Deny);
    }
}
