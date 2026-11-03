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

use tpt_norma_core::PolicyBounds;
use tpt_norma_eval::{ActionRequest, Decision, EvaluationFailure, PolicyEvaluator, StateSnapshot};
use tpt_solver::core::lra::LinConstraint;
use tpt_solver::core::rational::Rational;
use tpt_solver::core::engine::SolveResult;
use tpt_solver::check::outcome::Outcome;
use tpt_solver::reference::solve_and_check_lra;

/// The solver's only variable: the claimed new load.
///
/// The inputs are known constants, so they are folded into each constraint's right-hand
/// side rather than pinned as variables. Pinning them made Fourier–Motzkin elimination
/// blow up on a five-variable system, even for trivial inputs.
const NEW_LOAD: usize = 0;
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

/// `|x|` for an exact rational.
fn abs(x: Rational) -> Rational {
    if x.is_negative() {
        x.neg()
    } else {
        x
    }
}

/// A term of a constraint: a known input value, or the claimed new load.
#[derive(Clone, Copy)]
enum Term {
    Known(Rational),
    NewLoad,
}

/// Build `Σ coeff·term <= rhs`, folding every known term into the right-hand side.
/// Returns `None` if an intermediate value overflows the solver's rationals.
fn constraint(terms: &[(Term, i64)], rhs: Rational) -> Option<LinConstraint> {
    let mut known = Rational::zero();
    let mut new_load_coeff = 0i64;
    for &(term, coeff) in terms {
        match term {
            Term::Known(value) => {
                known = known.add(value.mul(Rational::from_i64(coeff))?)?;
            }
            Term::NewLoad => new_load_coeff += coeff,
        }
    }
    let mut coeffs = vec![Rational::zero(); N_VARS];
    coeffs[NEW_LOAD] = Rational::from_i64(new_load_coeff);
    Some(LinConstraint {
        coeffs,
        rhs: rhs.add(known.neg())?,
    })
}

/// Certify a claimed decision on `(load, delta)` under `policy`.
///
/// `claim` is `Some(new_load)` for a claimed `Allow`, and `None` for a claimed `Deny`.
pub fn certify(
    load: f64,
    delta: f64,
    policy: PolicyBounds,
    claim: Option<f64>,
) -> Certification {
    let (Some(l), Some(d), Some(cap), Some(max_delta)) = (
        exact(load),
        exact(delta),
        exact(policy.max_capacity),
        exact(policy.max_delta_per_call),
    ) else {
        return Certification::Inconclusive;
    };
    let zero = Rational::zero();
    let known = Term::Known;

    // The ideal-layer conditions for Allow: delta >= 0, delta <= max_delta, load + delta <= cap.
    let mut rows = vec![
        constraint(&[(known(d), -1)], zero),
        constraint(&[(known(d), 1), (known(max_delta), -1)], zero),
        constraint(&[(known(l), 1), (known(d), 1), (known(cap), -1)], zero),
    ];

    if let Some(new_load) = claim {
        let Some(claimed) = exact(new_load) else {
            return Certification::Inconclusive;
        };
        // The realization error bound: EPSILON * (|load| + |delta|), exactly.
        let (Some(scale), Some(epsilon)) = (abs(l).add(abs(d)), Rational::new(1, 1i128 << 52))
        else {
            return Certification::Inconclusive;
        };
        let Some(bound) = scale.mul(epsilon) else {
            return Certification::Inconclusive;
        };
        let (Some(upper), Some(lower)) = (claimed.add(bound), claimed.add(bound.neg())) else {
            return Certification::Inconclusive;
        };
        rows.extend([
            // new_load is the exact sum: new_load <= l + d and new_load >= l + d.
            constraint(&[(Term::NewLoad, 1), (known(l), -1), (known(d), -1)], zero),
            constraint(&[(Term::NewLoad, -1), (known(l), 1), (known(d), 1)], zero),
            // |new_load - claimed| <= bound, as two inequalities.
            constraint(&[(Term::NewLoad, 1)], upper),
            constraint(&[(Term::NewLoad, -1)], lower.neg()),
        ]);
    }

    let Some(cs) = rows.into_iter().collect::<Option<Vec<_>>>() else {
        return Certification::Inconclusive;
    };

    let (result, verdict) = solve_and_check_lra(&cs, SOLVER_FUEL);
    match (claim.is_some(), result, verdict) {
        (true, SolveResult::Sat, Outcome::Accept) => Certification::Verified,
        (true, SolveResult::Unsat, Outcome::Accept) => Certification::Contradicted,
        (false, SolveResult::Unsat, Outcome::Accept) => Certification::CertifiedDenial,
        (false, SolveResult::Sat, Outcome::Accept) => Certification::ConservativeDenial,
        _ => Certification::Inconclusive,
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

        let (Some(delta), Some(load)) = (request.arguments.to_delta(), snapshot.current_load) else {
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
