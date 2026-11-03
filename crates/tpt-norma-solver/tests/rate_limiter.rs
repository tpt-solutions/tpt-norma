// Copyright (c) TPT Solutions
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Exact check of the ideal-layer postconditions of `crates/tpt-norma-core/telos/rate_limiter.telos`.
//!
//! Each `ensures` is a linear conclusion. It is replayed as a nonnegative combination of the
//! premises, with the witness given by hand. `check_implication` verifies that witness with
//! exact rationals and never searches.
//!
//! The `cost > 0` and `amount > 0` preconditions are relaxed to `>= 0`. That relaxation is
//! weaker, so any implication proved from it also holds for the strict form.

use tpt_norma_solver::implication::{check_implication, int, Row};

/// `consume`, over the variables `(x, c, y)`: `x` the old tokens, `c` the cost, `y` the new tokens.
fn consume_premises() -> Vec<Row> {
    vec![
        Row::from_ints(&[0, -1, 0], 0),  // cost >= 0
        Row::from_ints(&[-1, 1, 0], 0),  // cost <= old tokens
        Row::from_ints(&[-1, 0, 0], 0),  // old tokens >= 0 (invariant)
        Row::from_ints(&[-1, 1, 1], 0),  // y - x + c <= 0 (y = x - c, upper half)
        Row::from_ints(&[1, -1, -1], 0), // x - c - y <= 0 (y = x - c, lower half)
    ]
}

#[test]
fn consume_ensures_new_tokens_are_nonnegative() {
    // y >= 0, i.e. -y <= 0. Witness: (x - c - y) + (c - x) = -y.
    let premises = consume_premises();
    let conclusion = Row::from_ints(&[0, 0, -1], 0);
    let witness = [int(0), int(1), int(0), int(0), int(1)];
    assert!(check_implication(&premises, &conclusion, &witness));
}

#[test]
fn consume_ensures_new_tokens_equal_old_minus_cost() {
    // y - x + c <= 0 and x - c - y <= 0 are the two halves of y = x - c. They are premises.
    let premises = consume_premises();
    let upper = Row::from_ints(&[-1, 1, 1], 0);
    let lower = Row::from_ints(&[1, -1, -1], 0);
    let upper_witness = [int(0), int(0), int(0), int(1), int(0)];
    let lower_witness = [int(0), int(0), int(0), int(0), int(1)];
    assert!(check_implication(&premises, &upper, &upper_witness));
    assert!(check_implication(&premises, &lower, &lower_witness));
}

#[test]
fn consume_without_the_affordability_precondition_is_not_implied() {
    // Drop `cost <= old tokens`. The nonnegativity conclusion no longer follows, so the same
    // witness must fail.
    let premises = [
        Row::from_ints(&[0, -1, 0], 0),
        Row::from_ints(&[-1, 0, 0], 0),
        Row::from_ints(&[-1, 1, 1], 0),
        Row::from_ints(&[1, -1, -1], 0),
    ];
    let conclusion = Row::from_ints(&[0, 0, -1], 0);
    let witness = [int(1), int(0), int(0), int(1)];
    assert!(!check_implication(&premises, &conclusion, &witness));
}

/// `refill`, over the variables `(x, a, cap, y)`: `x` the old tokens, `a` the amount,
/// `cap` the capacity, `y` the new tokens.
fn refill_premises() -> Vec<Row> {
    vec![
        Row::from_ints(&[0, -1, 0, 0], 0),  // amount >= 0
        Row::from_ints(&[1, 1, -1, 0], 0),  // x + a <= cap (precondition)
        Row::from_ints(&[-1, 0, 0, 0], 0),  // x >= 0 (invariant)
        Row::from_ints(&[1, 0, -1, 0], 0),  // x <= cap (invariant)
        Row::from_ints(&[-1, -1, 0, 1], 0), // y - x - a <= 0
        Row::from_ints(&[1, 1, 0, -1], 0),  // x + a - y <= 0
    ]
}

#[test]
fn refill_ensures_new_tokens_stay_within_capacity() {
    // y <= cap, i.e. y - cap <= 0. Witness: (y - x - a) + (x + a - cap).
    let premises = refill_premises();
    let conclusion = Row::from_ints(&[0, 0, -1, 1], 0);
    let witness = [int(0), int(1), int(0), int(0), int(1), int(0)];
    assert!(check_implication(&premises, &conclusion, &witness));
}

#[test]
fn refill_ensures_new_tokens_are_nonnegative() {
    // y >= 0, i.e. -y <= 0. Witness: (x + a - y) + (-x) + (-a) = -y.
    let premises = refill_premises();
    let conclusion = Row::from_ints(&[0, 0, 0, -1], 0);
    let witness = [int(1), int(0), int(1), int(0), int(0), int(1)];
    assert!(check_implication(&premises, &conclusion, &witness));
}

#[test]
fn refill_equals_old_plus_amount() {
    let premises = refill_premises();
    let upper = Row::from_ints(&[-1, -1, 0, 1], 0);
    let lower = Row::from_ints(&[1, 1, 0, -1], 0);
    assert!(check_implication(
        &premises,
        &upper,
        &[int(0), int(0), int(0), int(0), int(1), int(0)]
    ));
    assert!(check_implication(
        &premises,
        &lower,
        &[int(0), int(0), int(0), int(0), int(0), int(1)]
    ));
}
