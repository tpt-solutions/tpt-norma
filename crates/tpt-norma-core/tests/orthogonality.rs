// Copyright (c) TPT Solutions
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The non-orthogonal case of spec §5.2, shown concretely.
//!
//! `validate_multi` checks each dimension in isolation. When two dimensions share a quantity,
//! it can allow a proposal that breaks the shared constraint. These tests fix that behaviour so
//! it is visible, and so a change to `validate_multi` that closes the gap is noticed.

use tpt_norma_core::{validate_multi, PolicyBounds};

const POOL_CAPACITY: f64 = 12.0;

fn bounds() -> PolicyBounds {
    PolicyBounds {
        max_capacity: 10.0,
        max_delta_per_call: 10.0,
    }
}

#[test]
fn each_dimension_passes_alone_but_the_shared_pool_is_exceeded() {
    // Two dimensions, each with capacity 10. A shared pool caps their total new load at 12.
    let loads = [4.0, 4.0];
    let deltas = [4.0, 4.0];
    let policies = [bounds(), bounds()];

    // The per-dimension check allows it: new loads 8 and 8, both within 10.
    let allowed = validate_multi(&loads, &deltas, &policies).expect("each dimension passes");
    assert_eq!(allowed, vec![8.0, 8.0]);

    // The shared pool is violated: 8 + 8 = 16 > 12.
    let total: f64 = allowed.iter().sum();
    assert!(total > POOL_CAPACITY);
}

#[test]
fn the_pool_is_not_violated_when_the_total_fits() {
    let loads = [4.0, 4.0];
    let deltas = [1.0, 1.0];
    let allowed = validate_multi(&loads, &deltas, &[bounds(), bounds()]).unwrap();
    let total: f64 = allowed.iter().sum();
    assert!(total <= POOL_CAPACITY);
}
