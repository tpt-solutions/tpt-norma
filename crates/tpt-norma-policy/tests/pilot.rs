// Copyright (c) TPT Solutions
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The pilot policy file parses and is consistent.

use tpt_norma_policy::PolicySpace;

const PILOT: &str = include_str!("../../../examples/pilot/quota.policy");

#[test]
fn the_pilot_policy_parses_and_is_consistent() {
    let space = PolicySpace::parse(PILOT).expect("pilot policy parses");
    let consistency = space.check_consistency().expect("consistency check runs");
    assert!(consistency.decided, "solver could not decide");
    assert!(
        consistency.consistent,
        "{:?}",
        consistency.contradicting_pairs
    );
}
