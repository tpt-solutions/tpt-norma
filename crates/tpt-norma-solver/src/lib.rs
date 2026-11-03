// Copyright (c) TPT Solutions
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Certificate checking of policy decisions through `tpt-solver` (spec2 §4, solver adapter).
//!
//! A claimed decision is encoded as a small linear-real-arithmetic system over the exact
//! values of the inputs. `tpt-solver` decides it, and its kernel re-checks the answer:
//!
//! - a claimed `Allow` must be satisfiable, with a model the kernel accepts. Its
//!   `new_load` must also lie within the realization error bound of the exact sum;
//! - a claimed `Deny` is certified when the system is unsatisfiable (Farkas certificate).
//!   If it is satisfiable, the ideal layer allows the action and the f64 layer denied it
//!   conservatively. That is permitted, so it is reported as [`Certification::ConservativeDenial`].
//!
//! Any answer the kernel does not accept, and any input outside the exact range of
//! `tpt-solver`'s rationals, is `Inconclusive` (or `Contradicted` for a refuted `Allow`).
//! Neither is ever read as `Allow`.

pub mod implication;

use tpt_math_exact::BigRational;
use tpt_norma_core::{
    symbolic_evaluate_ideal, ActionProposal, IdealDecision, PolicyBounds, SystemState,
};
use tpt_norma_eval::{ActionRequest, Decision, EvaluationFailure, PolicyEvaluator, StateSnapshot};
use tpt_solver::check::outcome::Outcome;
use tpt_solver::core::engine::SolveResult;
use tpt_solver::core::lra::LinConstraint;
use tpt_solver::core::rational::Rational;
use tpt_solver::reference::solve_and_check_lra;

/// The solver certifies ground constraints only: every input is a known constant, so each
/// row is a single inequality `lhs <= rhs` checked over zero variables. The solver's
/// `i128` rationals cannot hold the realization error bound without overflowing, so that
/// bound is checked separately with exact big rationals (see [`certify`]).
const N_VARS: usize = 1;

/// Fuel passed to the solver. The LRA path currently ignores it (see the solver's
/// `solve_and_check_lra`), so the real bound here is the fixed problem size.
const SOLVER_FUEL: u64 = 1_000_000;

/// The outcome of checking one claimed decision against the solver.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Certification {
    /// The claimed `Allow` is satisfiable and its model was accepted by the kernel.
    Verified,
    /// The claimed `Deny` is certified by an UNSAT answer.
    CertifiedDenial,
    /// The claimed `Deny` is satisfiable: the ideal layer allows, and the f64 layer denied
    /// conservatively. This is permitted.
    ConservativeDenial,
    /// The solver refuted the claimed `Allow`.
    Contradicted,
    /// The solver could not give a checked answer.
    Inconclusive,
}

/// Convert an `f64` to an exact `tpt-solver` rational.
///
/// Returns `None` when the value needs more than `tpt-solver`'s `i128` rationals.
fn exact(x: f64) -> Option<Rational> {
    if x == 0.0 {
        return Some(Rational::zero());
    }
    let bits = x.to_bits();
    let sign: i128 = if bits >> 63 == 1 { -1 } else { 1 };
    let biased = ((bits >> 52) & 0x7ff) as i32;
    let fraction = bits & ((1u64 << 52) - 1);
    let (mantissa, exponent) = if biased == 0 {
        (fraction, -1074)
    } else {
        (fraction | (1u64 << 52), biased - 1075)
    };

    // Strip trailing zero bits so that values such as 0.5 stay in range.
    let shift = mantissa.trailing_zeros();
    let mantissa = (mantissa >> shift) as i128 * sign;
    let exponent = exponent + shift as i32;

    if exponent >= 0 {
        if exponent > 60 {
            return None;
        }
        Rational::new(mantissa.checked_mul(1i128 << exponent)?, 1)
    } else {
        let denominator_bits = -exponent;
        if denominator_bits > 120 {
            return None;
        }
        Rational::new(mantissa, 1i128 << denominator_bits)
    }
}

/// The ground row `lhs <= rhs`, as the solver's `Σ coeffs·x <= rhs` form with no variables.
/// It is satisfiable exactly when `rhs - lhs >= 0`.
fn check_row(lhs: Rational, rhs: Rational) -> Option<LinConstraint> {
    Some(LinConstraint {
        coeffs: vec![Rational::zero(); N_VARS],
        rhs: rhs.add(lhs.neg())?,
    })
}

/// Certify a claimed decision on `(load, delta)` under `policy`.
///
/// `claim` is `Some(new_load)` for a claimed `Allow`, and `None` for a claimed `Deny`.
///
/// The solver checks the ideal-layer conditions for `Allow` on the exact inputs. A claimed
/// `Allow` also needs its `new_load` within `EPSILON * (|load| + |delta|)` of the exact sum,
/// which is checked with `tpt-math-exact` big rationals because the solver cannot hold that
/// bound. Both checks are exact. Only the first is kernel-checked.
pub fn certify(load: f64, delta: f64, policy: PolicyBounds, claim: Option<f64>) -> Certification {
    let (Some(l), Some(d), Some(cap), Some(max_delta)) = (
        exact(load),
        exact(delta),
        exact(policy.max_capacity),
        exact(policy.max_delta_per_call),
    ) else {
        return Certification::Inconclusive;
    };
    let zero = Rational::zero();

    // The ideal-layer conditions for Allow: delta >= 0, delta <= max_delta, load + delta <= cap.
    let Some(sum) = l.add(d) else {
        return Certification::Inconclusive;
    };
    let rows = [
        check_row(d.neg(), zero),
        check_row(d, max_delta),
        check_row(sum, cap),
    ];
    let Some(cs) = rows.into_iter().collect::<Option<Vec<_>>>() else {
        return Certification::Inconclusive;
    };

    let (result, verdict) = solve_and_check_lra(&cs, SOLVER_FUEL);
    match (claim, result, verdict) {
        (Some(new_load), SolveResult::Sat, Outcome::Accept) => {
            check_realization(load, delta, policy, new_load)
        }
        (Some(_), SolveResult::Unsat, Outcome::Accept) => Certification::Contradicted,
        (None, SolveResult::Unsat, Outcome::Accept) => Certification::CertifiedDenial,
        (None, SolveResult::Sat, Outcome::Accept) => Certification::ConservativeDenial,
        _ => Certification::Inconclusive,
    }
}

/// The numerical-error half of a claimed `Allow`, checked with exact big rationals.
fn check_realization(load: f64, delta: f64, policy: PolicyBounds, claimed: f64) -> Certification {
    let state = SystemState { current_load: load };
    let proposal = ActionProposal {
        resource_delta: delta,
    };
    let IdealDecision::Allow {
        new_load: exact_sum,
    } = symbolic_evaluate_ideal(state, proposal, policy)
    else {
        // The solver found the ideal conditions satisfiable, so this cannot happen.
        return Certification::Contradicted;
    };
    let Some(claimed) = BigRational::from_float(claimed) else {
        return Certification::Inconclusive;
    };
    let (Some(load), Some(delta), Some(epsilon)) = (
        BigRational::from_float(load),
        BigRational::from_float(delta),
        BigRational::from_float(f64::EPSILON),
    ) else {
        return Certification::Inconclusive;
    };
    let bound = (load.abs() + delta.abs()) * epsilon;
    if (exact_sum - claimed).abs() <= bound {
        Certification::Verified
    } else {
        Certification::Contradicted
    }
}

/// Wraps an evaluator and checks every `Allow` against the solver.
///
/// A solver refutation becomes `Indeterminate(SolverRejected)`. An unchecked answer becomes
/// `Indeterminate(SolverInconclusive)`. Only a verified `Allow` passes through. A `Deny` is
/// returned unchanged, since denying is always safe.
#[derive(Debug, Clone)]
pub struct CertifiedEvaluator<E> {
    inner: E,
    policy: PolicyBounds,
}

impl<E> CertifiedEvaluator<E> {
    pub fn new(inner: E, policy: PolicyBounds) -> Self {
        Self { inner, policy }
    }
}

impl<E: PolicyEvaluator> PolicyEvaluator for CertifiedEvaluator<E> {
    fn evaluate(&self, request: &ActionRequest, snapshot: &StateSnapshot) -> Decision {
        let decision = self.inner.evaluate(request, snapshot);
        let Decision::Allow { evidence } = &decision else {
            return decision;
        };

        let (Some(delta), Some(load)) = (request.arguments.to_delta(), snapshot.current_load)
        else {
            return Decision::Indeterminate {
                reason: EvaluationFailure::MalformedArguments,
            };
        };

        match certify(load, delta, self.policy, Some(evidence.new_load)) {
            Certification::Verified => decision,
            Certification::Contradicted => Decision::Indeterminate {
                reason: EvaluationFailure::SolverRejected,
            },
            _ => Decision::Indeterminate {
                reason: EvaluationFailure::SolverInconclusive,
            },
        }
    }
}
