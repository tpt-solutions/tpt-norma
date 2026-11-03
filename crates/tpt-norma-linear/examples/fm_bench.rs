// Copyright (c) TPT Solutions
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Latency of Fourier–Motzkin composition over a 10-dimension policy (spec §9, Exit Criterion 2).
//!
//! Run with `cargo run --release -p tpt-norma-linear --example fm_bench`.
//!
//! Each call runs the same contradiction check that `tpt-norma-policy` runs: every dimension's
//! box, every coupling rule, every pair of groups, and all groups together. The space has
//! `COUPLING_RULES` rules over all `DIMENSIONS` variables with mixed coefficients. Only the
//! rule set changes between the two cases. The elimination runs over exact rationals.

use std::hint::black_box;
use std::time::Instant;

use tpt_norma_linear::{check_contradictions, Constraint, Linear, NamedConstraints, Relation};

const DIMENSIONS: usize = 10;
const RULE_COUNTS: [usize; 5] = [5, 10, 20, 40, 80];
const SAMPLES: usize = 201;

fn dim(i: usize) -> String {
    format!("x{i}")
}

/// The policy: a box per dimension, plus `rules` dense coupling rules.
///
/// `slack` is the right-hand side of every coupling rule. A large `slack` gives a satisfiable
/// space. A negative one gives an empty space, because every coefficient and every variable is
/// non-negative.
fn policy(rules: usize, slack: i64) -> Vec<NamedConstraints> {
    let mut groups = Vec::new();
    for i in 0..DIMENSIONS {
        let x = Linear::var(&dim(i));
        groups.push(NamedConstraints {
            label: format!("dim:{}", dim(i)),
            constraints: vec![
                Constraint {
                    expr: x.sub(&Linear::constant_only(0)),
                    relation: Relation::Ge,
                },
                Constraint {
                    expr: x.sub(&Linear::constant_only(100)),
                    relation: Relation::Le,
                },
            ],
        });
    }
    for r in 0..rules {
        let mut lhs = Linear::default();
        for i in 0..DIMENSIONS {
            let coeff = 1 + ((i + r) % 3) as i64;
            lhs = lhs.add(&Linear::var(&dim(i)).scale(coeff));
        }
        groups.push(NamedConstraints {
            label: format!("rule:{r}"),
            constraints: vec![Constraint {
                expr: lhs.sub(&Linear::constant_only(slack)),
                relation: Relation::Le,
            }],
        });
    }
    groups
}

fn time_case(label: &str, groups: &[NamedConstraints]) {
    // Warm up.
    for _ in 0..3 {
        black_box(check_contradictions(black_box(groups)));
    }

    let mut nanos = Vec::with_capacity(SAMPLES);
    let mut last = None;
    for _ in 0..SAMPLES {
        let start = Instant::now();
        let report = check_contradictions(black_box(groups));
        nanos.push(start.elapsed().as_nanos() as u64);
        last = Some(report);
    }
    nanos.sort_unstable();

    let percentile = |p: f64| nanos[((nanos.len() as f64 - 1.0) * p).round() as usize];
    let report = last.expect("at least one sample");
    println!("{label}");
    println!(
        "  overall_unsat: {:?}, pairs: {}, decided: {}",
        report.overall_unsat,
        report.pairs.len(),
        report.decided
    );
    println!(
        "  median: {:.3} ms, p99: {:.3} ms, max: {:.3} ms",
        percentile(0.50) as f64 / 1e6,
        percentile(0.99) as f64 / 1e6,
        nanos[nanos.len() - 1] as f64 / 1e6
    );
}

fn main() {
    println!("dimensions: {DIMENSIONS}, samples: {SAMPLES}");
    for rules in RULE_COUNTS {
        time_case(
            &format!("{rules} coupling rules, satisfiable (slack 1000)"),
            &policy(rules, 1000),
        );
        time_case(
            &format!("{rules} coupling rules, unsatisfiable (slack -1)"),
            &policy(rules, -1),
        );
    }
}
