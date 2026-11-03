// Copyright (c) TPT Solutions
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Ideal-layer proof of the `admit` contracts in `crates/tpt-norma-core/telos/action_validator.telos`,
//! discharged with exact certificates instead of the `tpt-telos` verifier (todo Phase 2).
//!
//! Variables, in order: `load`, `delta`, `cap`, `max` (the `max_delta_per_call` bound).

use tpt_math_exact::BigRational;
use tpt_norma_solver::implication::{check_implication, int, Row};

/// The `requires` clauses of `admit`, plus the `Ledger` invariant on the input state.
fn premises() -> Vec<Row> {
    vec![
        // requires max_delta_per_call >= 0
        Row::from_ints(&[0, 0, 0, -1], 0),
        // requires delta >= 0
        Row::from_ints(&[0, -1, 0, 0], 0),
        // requires delta <= max_delta_per_call
        Row::from_ints(&[0, 1, 0, -1], 0),
        // requires ledger.load + delta <= ledger.capacity
        Row::from_ints(&[1, 1, -1, 0], 0),
        // invariant load >= 0
        Row::from_ints(&[-1, 0, 0, 0], 0),
        // invariant load <= capacity
        Row::from_ints(&[1, 0, -1, 0], 0),
    ]
}

/// The post-state load is `load + delta`, by the `mutate` assignment. The `ensures` clauses
/// then become these rows over the pre-state variables.
fn new_load_at_most_capacity() -> Row {
    // ensures ledger.load <= ledger.capacity
    Row::from_ints(&[1, 1, -1, 0], 0)
}

fn new_load_non_negative() -> Row {
    // ensures ledger.load >= 0, i.e. -(load + delta) <= 0
    Row::from_ints(&[-1, -1, 0, 0], 0)
}

#[test]
fn ensures_capacity_is_implied_by_the_requires_clause() {
    // The witness uses premise 3 (`load + delta <= cap`) with multiplier 1.
    let mut lambda = vec![int(0); premises().len()];
    lambda[3] = int(1);
    assert!(check_implication(
        &premises(),
        &new_load_at_most_capacity(),
        &lambda
    ));
}

#[test]
fn ensures_non_negativity_is_implied_by_the_requires_clauses() {
    // load >= 0 (premise 4) and delta >= 0 (premise 1), each with multiplier 1.
    let mut lambda = vec![int(0); premises().len()];
    lambda[1] = int(1);
    lambda[4] = int(1);
    assert!(check_implication(
        &premises(),
        &new_load_non_negative(),
        &lambda
    ));
}

#[test]
fn premises_are_satisfiable_so_the_proof_is_not_vacuous() {
    // load = delta = cap = max = 0 satisfies every requires clause and the invariant.
    let point: Vec<BigRational> = vec![int(0); 4];
    assert!(premises().iter().all(|p| p.holds_at(&point)));
}

#[test]
fn a_tighter_capacity_ensures_is_not_certified() {
    // ensures ledger.load <= ledger.capacity - 1 is false at load = delta = cap = max = 0.
    // No multiplier vector should certify it, so the witness below must be rejected.
    let tighter = Row::from_ints(&[1, 1, -1, 0], -1);
    let mut lambda = vec![int(0); premises().len()];
    lambda[3] = int(1);
    assert!(!check_implication(&premises(), &tighter, &lambda));

    let point: Vec<BigRational> = vec![int(0); 4];
    assert!(!tighter.holds_at(&point));
}

#[test]
fn a_positive_floor_ensures_is_not_certified() {
    // ensures ledger.load >= 1 does not follow: load = delta = 0 is a counterexample.
    let floor = Row::from_ints(&[-1, -1, 0, 0], -1);
    let mut lambda = vec![int(0); premises().len()];
    lambda[1] = int(1);
    lambda[4] = int(1);
    assert!(!check_implication(&premises(), &floor, &lambda));
}
