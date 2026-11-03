// Copyright (c) TPT Solutions
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Exact checking of linear implications, as a replacement for the `tpt-telos` verifier on
//! the ideal layer (todo Phase 2).
//!
//! A [`Row`] is the non-strict inequality `Σ coeffs[i]·x_i <= rhs`. A conclusion is implied by a
//! set of premises when a nonnegative combination of the premises gives the conclusion's left
//! side, with a right side no larger than the conclusion's. That is the Farkas lemma for
//! non-strict rows. The prover supplies the multipliers as a witness. [`check_implication`]
//! verifies them with exact big rationals and never searches. It is therefore sound
//! whenever it returns `true`.
//!
//! Strict conclusions are not handled. Their negations are strict, and the witness form cannot
//! express them without a further argument. Every `ensures` in the `.telos` twins is
//! non-strict, so this does not limit them.

use tpt_math_exact::BigRational;

/// The non-strict row `Σ coeffs[i]·x_i <= rhs`.
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    pub coeffs: Vec<BigRational>,
    pub rhs: BigRational,
}

/// An exact rational from a small integer.
pub fn int(n: i64) -> BigRational {
    BigRational::from_float(n as f64).expect("small integers are finite")
}

impl Row {
    /// Build a row from integer coefficients and a right-hand side.
    pub fn from_ints(coeffs: &[i64], rhs: i64) -> Self {
        Self {
            coeffs: coeffs.iter().map(|&c| int(c)).collect(),
            rhs: int(rhs),
        }
    }

    /// Whether `point` satisfies this row.
    pub fn holds_at(&self, point: &[BigRational]) -> bool {
        self.lhs_at(point) <= self.rhs
    }

    fn lhs_at(&self, point: &[BigRational]) -> BigRational {
        self.coeffs
            .iter()
            .zip(point)
            .fold(int(0), |acc, (c, x)| acc + c.clone() * x.clone())
    }
}

/// Check that `premises` imply `conclusion`, using the nonnegative `multipliers` as a witness.
///
/// Returns `true` only when there is one multiplier per premise, every multiplier is
/// nonnegative, the weighted coefficients equal the conclusion's coefficients exactly, and the
/// weighted right-hand sides sum to at most the conclusion's right-hand side.
pub fn check_implication(premises: &[Row], conclusion: &Row, multipliers: &[BigRational]) -> bool {
    if multipliers.len() != premises.len() {
        return false;
    }
    if multipliers.iter().any(|m| *m < int(0)) {
        return false;
    }
    let width = conclusion.coeffs.len();
    if premises.iter().any(|p| p.coeffs.len() != width) {
        return false;
    }

    let mut lhs = vec![int(0); width];
    let mut rhs = int(0);
    for (premise, lambda) in premises.iter().zip(multipliers) {
        for (total, c) in lhs.iter_mut().zip(&premise.coeffs) {
            *total = total.clone() + lambda.clone() * c.clone();
        }
        rhs += lambda.clone() * premise.rhs.clone();
    }

    lhs == conclusion.coeffs && rhs <= conclusion.rhs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sums_nonnegative_premises_into_a_bound() {
        // x >= 0 and y >= 0 imply x + y >= 0, i.e. -x - y <= 0.
        let premises = [Row::from_ints(&[-1, 0], 0), Row::from_ints(&[0, -1], 0)];
        let conclusion = Row::from_ints(&[-1, -1], 0);
        assert!(check_implication(&premises, &conclusion, &[int(1), int(1)]));
    }

    #[test]
    fn rejects_a_wrong_multiplier() {
        let premises = [Row::from_ints(&[-1, 0], 0), Row::from_ints(&[0, -1], 0)];
        let conclusion = Row::from_ints(&[-1, -1], 0);
        // Coefficients no longer match.
        assert!(!check_implication(
            &premises,
            &conclusion,
            &[int(1), int(2)]
        ));
    }

    #[test]
    fn rejects_negative_multiplier() {
        let premises = [Row::from_ints(&[-1], 0)];
        let conclusion = Row::from_ints(&[1], 0);
        assert!(!check_implication(&premises, &conclusion, &[int(-1)]));
    }

    #[test]
    fn rejects_a_conclusion_with_a_tighter_bound() {
        // x <= 0 does not imply x <= -1.
        let premises = [Row::from_ints(&[1], 0)];
        let conclusion = Row::from_ints(&[1], -1);
        assert!(!check_implication(&premises, &conclusion, &[int(1)]));
    }

    #[test]
    fn rejects_mismatched_witness_length() {
        let premises = [Row::from_ints(&[1], 0)];
        let conclusion = Row::from_ints(&[1], 0);
        assert!(!check_implication(&premises, &conclusion, &[]));
    }
}
