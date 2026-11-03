// Copyright (c) TPT Solutions
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Linear constraints and an exact contradiction check, internal to tpt-norma (spec §4).
//!
//! This crate replaces the `tpt-telos-ir` and `tpt-telos-sdk` contradiction check that
//! `tpt-norma-policy` used before. It does not depend on the `tpt-telos` checkout.
//!
//! A [`Constraint`] is `expr REL 0`, with `expr` a linear expression over named variables with
//! `i64` coefficients. A set of constraints is decided by Fourier–Motzkin elimination over exact
//! big rationals. The decision is therefore exact over the rationals:
//!
//! - `Some(true)` (unsatisfiable) is sound. A set that has no rational solution has no integer
//!   solution either.
//! - `Some(false)` (satisfiable) is over the rationals only. An integer-only policy can still be
//!   empty, and the caller must not read it as an integer witness.
//! - `None` means the elimination grew past [`MAX_ROWS`] and gave up. The result is undecided.

use std::collections::{BTreeMap, BTreeSet};

use tpt_math_exact::{BigInt, BigRational};

/// Elimination stops with `None` once the working set grows past this many rows.
pub const MAX_ROWS: usize = 10_000;

/// A linear expression `Σ coeff·var + constant`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Linear {
    pub terms: BTreeMap<String, i64>,
    pub constant: i64,
}

impl Linear {
    /// The expression `1·name`.
    pub fn var(name: &str) -> Self {
        Linear {
            terms: BTreeMap::from([(name.to_string(), 1)]),
            constant: 0,
        }
    }

    /// The constant expression `value`.
    pub fn constant_only(value: i64) -> Self {
        Linear {
            terms: BTreeMap::new(),
            constant: value,
        }
    }

    /// `k · self`.
    pub fn scale(&self, k: i64) -> Linear {
        Linear {
            terms: self
                .terms
                .iter()
                .map(|(var, coeff)| (var.clone(), coeff * k))
                .filter(|(_, coeff)| *coeff != 0)
                .collect(),
            constant: self.constant * k,
        }
    }

    /// `self + other`, with zero coefficients dropped.
    pub fn add(&self, other: &Linear) -> Linear {
        let mut terms = self.terms.clone();
        for (var, coeff) in &other.terms {
            *terms.entry(var.clone()).or_insert(0) += coeff;
        }
        terms.retain(|_, coeff| *coeff != 0);
        Linear {
            terms,
            constant: self.constant + other.constant,
        }
    }

    /// `self - other`, with zero coefficients dropped.
    pub fn sub(&self, other: &Linear) -> Linear {
        let mut terms = self.terms.clone();
        for (var, coeff) in &other.terms {
            let entry = terms.entry(var.clone()).or_insert(0);
            *entry -= coeff;
        }
        terms.retain(|_, coeff| *coeff != 0);
        Linear {
            terms,
            constant: self.constant - other.constant,
        }
    }
}

/// The relation in `expr REL 0`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Relation {
    Le,
    Lt,
    Ge,
    Gt,
    Eq,
}

/// One constraint `expr REL 0`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Constraint {
    pub expr: Linear,
    pub relation: Relation,
}

/// A named group of constraints, such as one policy rule or one dimension's bounds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NamedConstraints {
    pub label: String,
    pub constraints: Vec<Constraint>,
}

/// A pair of groups that cannot hold together.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Contradiction {
    pub a: String,
    pub b: String,
}

/// The result of [`check_contradictions`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContradictionReport {
    /// `Some(true)` if all groups together are unsatisfiable, `Some(false)` if they are
    /// satisfiable over the rationals, and `None` if the check gave up.
    pub overall_unsat: Option<bool>,
    /// Every pair of groups that is unsatisfiable together.
    pub pairs: Vec<Contradiction>,
    /// `false` if any check in this report gave up.
    pub decided: bool,
}

/// A row `Σ coeffs[v]·v + constant <= 0`, or `< 0` when `strict`.
#[derive(Debug, Clone)]
struct Row {
    coeffs: BTreeMap<String, BigRational>,
    constant: BigRational,
    strict: bool,
}

/// Decide whether the constraints have a common rational solution.
///
/// `Some(true)` means unsatisfiable, `Some(false)` satisfiable, `None` gave up at [`MAX_ROWS`].
pub fn is_unsatisfiable(constraints: &[Constraint]) -> Option<bool> {
    let mut rows = Vec::new();
    for constraint in constraints {
        push_rows(constraint, &mut rows);
    }
    eliminate_all(rows)
}

/// Check each group, every pair of groups, and all groups together.
pub fn check_contradictions(groups: &[NamedConstraints]) -> ContradictionReport {
    let all: Vec<Constraint> = groups
        .iter()
        .flat_map(|g| g.constraints.iter().cloned())
        .collect();
    let overall_unsat = is_unsatisfiable(&all);
    let mut decided = overall_unsat.is_some();

    let mut pairs = Vec::new();
    for (i, a) in groups.iter().enumerate() {
        for b in &groups[i + 1..] {
            let union: Vec<Constraint> = a
                .constraints
                .iter()
                .chain(b.constraints.iter())
                .cloned()
                .collect();
            match is_unsatisfiable(&union) {
                Some(true) => pairs.push(Contradiction {
                    a: a.label.clone(),
                    b: b.label.clone(),
                }),
                Some(false) => {}
                None => decided = false,
            }
        }
    }

    ContradictionReport {
        overall_unsat,
        pairs,
        decided,
    }
}

/// Turn `expr REL 0` into rows of the form `<= 0` or `< 0`.
fn push_rows(constraint: &Constraint, rows: &mut Vec<Row>) {
    let base = to_row(&constraint.expr, false);
    match constraint.relation {
        Relation::Le => rows.push(base),
        Relation::Lt => rows.push(Row {
            strict: true,
            ..base
        }),
        Relation::Ge => rows.push(to_row(&negate(&constraint.expr), false)),
        Relation::Gt => rows.push(to_row(&negate(&constraint.expr), true)),
        Relation::Eq => {
            rows.push(base);
            rows.push(to_row(&negate(&constraint.expr), false));
        }
    }
}

fn negate(expr: &Linear) -> Linear {
    Linear {
        terms: expr.terms.iter().map(|(v, c)| (v.clone(), -c)).collect(),
        constant: -expr.constant,
    }
}

fn to_row(expr: &Linear, strict: bool) -> Row {
    Row {
        coeffs: expr
            .terms
            .iter()
            .filter(|(_, c)| **c != 0)
            .map(|(v, c)| (v.clone(), int(*c)))
            .collect(),
        constant: int(expr.constant),
        strict,
    }
}

fn int(n: i64) -> BigRational {
    BigRational::from(BigInt::from(n))
}

fn zero() -> BigRational {
    int(0)
}

/// Eliminate variables one at a time. Returns `Some(true)` on a contradiction, `Some(false)`
/// when every constant row holds, and `None` if the row count passes [`MAX_ROWS`].
fn eliminate_all(mut rows: Vec<Row>) -> Option<bool> {
    loop {
        if let Some(unsat) = constant_rows_contradict(&rows) {
            return Some(unsat);
        }
        let Some(var) = pick_variable(&rows) else {
            return Some(false);
        };
        rows = eliminate(&rows, &var);
        if rows.len() > MAX_ROWS {
            return None;
        }
    }
}

/// `Some(true)` if a row with no variables is false, `Some(false)` if all such rows hold,
/// and `None` if some rows still have variables.
fn constant_rows_contradict(rows: &[Row]) -> Option<bool> {
    let mut any_variable = false;
    for row in rows {
        if row.coeffs.is_empty() {
            let violated = if row.strict {
                row.constant >= zero()
            } else {
                row.constant > zero()
            };
            if violated {
                return Some(true);
            }
        } else {
            any_variable = true;
        }
    }
    if any_variable {
        None
    } else {
        Some(false)
    }
}

fn pick_variable(rows: &[Row]) -> Option<String> {
    let vars: BTreeSet<&String> = rows.iter().flat_map(|r| r.coeffs.keys()).collect();
    vars.into_iter().next().cloned()
}

/// Fourier–Motzkin step for one variable: pair every upper bound with every lower bound.
fn eliminate(rows: &[Row], var: &str) -> Vec<Row> {
    let mut upper = Vec::new();
    let mut lower = Vec::new();
    let mut kept = Vec::new();
    for row in rows {
        match row.coeffs.get(var) {
            Some(c) if *c > zero() => upper.push((row, c.clone())),
            Some(c) if *c < zero() => lower.push((row, c.clone())),
            _ => kept.push(row.clone()),
        }
    }

    for (p, a) in &upper {
        for (n, b) in &lower {
            // (-b)·p + a·n has a zero coefficient on `var`, since a > 0 and b < 0.
            let mult_p = -b.clone();
            let mult_n = a.clone();
            let mut coeffs: BTreeMap<String, BigRational> = BTreeMap::new();
            for (v, c) in &p.coeffs {
                *coeffs.entry(v.clone()).or_insert_with(zero) += &mult_p * c;
            }
            for (v, c) in &n.coeffs {
                *coeffs.entry(v.clone()).or_insert_with(zero) += &mult_n * c;
            }
            coeffs.retain(|_, c| *c != zero());
            let constant = &mult_p * &p.constant + &mult_n * &n.constant;
            kept.push(Row {
                coeffs,
                constant,
                strict: p.strict || n.strict,
            });
        }
    }
    kept
}

#[cfg(test)]
mod tests {
    use super::*;

    fn le(expr: Linear) -> Constraint {
        Constraint {
            expr,
            relation: Relation::Le,
        }
    }

    fn ge(expr: Linear) -> Constraint {
        Constraint {
            expr,
            relation: Relation::Ge,
        }
    }

    #[test]
    fn a_box_is_satisfiable() {
        let x = Linear::var("x");
        let set = [
            ge(x.sub(&Linear::constant_only(0))),
            le(x.sub(&Linear::constant_only(10))),
        ];
        assert_eq!(is_unsatisfiable(&set), Some(false));
    }

    #[test]
    fn disjoint_bounds_are_unsatisfiable() {
        let x = Linear::var("x");
        let set = [
            ge(x.sub(&Linear::constant_only(5))),
            le(x.sub(&Linear::constant_only(3))),
        ];
        assert_eq!(is_unsatisfiable(&set), Some(true));
    }

    #[test]
    fn strict_bounds_meeting_at_a_point_are_unsatisfiable() {
        let x = Linear::var("x");
        let set = [
            Constraint {
                expr: x.sub(&Linear::constant_only(3)),
                relation: Relation::Lt,
            },
            ge(x.sub(&Linear::constant_only(3))),
        ];
        assert_eq!(is_unsatisfiable(&set), Some(true));
    }

    #[test]
    fn non_strict_bounds_meeting_at_a_point_are_satisfiable() {
        let x = Linear::var("x");
        let set = [
            le(x.sub(&Linear::constant_only(3))),
            ge(x.sub(&Linear::constant_only(3))),
        ];
        assert_eq!(is_unsatisfiable(&set), Some(false));
    }

    #[test]
    fn coupled_rules_are_checked_across_variables() {
        // x + y <= 4, x >= 3, y >= 2 is unsatisfiable.
        let x = Linear::var("x");
        let y = Linear::var("y");
        let sum = x.sub(&Linear::constant_only(0));
        let mut total = sum.clone();
        total.terms.insert("y".into(), 1);
        total.constant = -4;
        let set = [
            le(total),
            ge(x.sub(&Linear::constant_only(3))),
            ge(y.sub(&Linear::constant_only(2))),
        ];
        assert_eq!(is_unsatisfiable(&set), Some(true));
    }

    #[test]
    fn pairwise_contradictions_are_reported_by_label() {
        let x = Linear::var("x");
        let groups = [
            NamedConstraints {
                label: "low".into(),
                constraints: vec![le(x.sub(&Linear::constant_only(2)))],
            },
            NamedConstraints {
                label: "high".into(),
                constraints: vec![ge(x.sub(&Linear::constant_only(5)))],
            },
            NamedConstraints {
                label: "any".into(),
                constraints: vec![ge(x.sub(&Linear::constant_only(0)))],
            },
        ];
        let report = check_contradictions(&groups);
        assert_eq!(report.overall_unsat, Some(true));
        assert!(report.decided);
        assert_eq!(
            report.pairs,
            vec![Contradiction {
                a: "low".into(),
                b: "high".into()
            }]
        );
    }
}
