// Copyright (c) TPT Solutions
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Latency of the 10-dimension allocator (spec §9, Exit Criterion 2).
//!
//! Run with `cargo run --release -p tpt-norma-core --example alloc_bench`.
//!
//! This times the closed-form `validate_multi`. It does not time Fourier–Motzkin elimination,
//! which is what the spec's 10 ms / 50 ms targets were written for. Results therefore do not
//! establish that FM-based policy composition meets those targets.

use std::hint::black_box;
use std::time::Instant;

use tpt_norma_core::{validate_multi, PolicyBounds};

const DIMENSIONS: usize = 10;
const SAMPLES: usize = 200_000;

fn main() {
    let policies = vec![
        PolicyBounds {
            max_capacity: 1_000.0,
            max_delta_per_call: 50.0,
        };
        DIMENSIONS
    ];
    let loads: Vec<f64> = (0..DIMENSIONS).map(|i| 10.0 * i as f64).collect();
    let deltas: Vec<f64> = (0..DIMENSIONS).map(|i| 1.0 + i as f64).collect();

    // Warm up.
    for _ in 0..10_000 {
        black_box(
            validate_multi(black_box(&loads), black_box(&deltas), black_box(&policies)).is_ok(),
        );
    }

    let mut nanos = Vec::with_capacity(SAMPLES);
    for _ in 0..SAMPLES {
        let start = Instant::now();
        let ok =
            validate_multi(black_box(&loads), black_box(&deltas), black_box(&policies)).is_ok();
        nanos.push(start.elapsed().as_nanos() as u64);
        black_box(ok);
    }
    nanos.sort_unstable();

    let percentile = |p: f64| nanos[((nanos.len() as f64 - 1.0) * p).round() as usize];
    println!("dimensions: {DIMENSIONS}, samples: {SAMPLES}");
    println!("median: {} ns", percentile(0.50));
    println!("p99:    {} ns", percentile(0.99));
    println!("max:    {} ns", nanos[nanos.len() - 1]);
}
