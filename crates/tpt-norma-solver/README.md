# tpt-norma-solver

Certificate checking of `tpt-norma` policy decisions through
[`tpt-solver`](../../../tpt-solver/tpt-solver/). Each claimed decision is turned into a small
linear-real-arithmetic problem, and the solver's kernel checks the answer before it is trusted.

It also provides an exact checker for linear implications, which can replace the external
`tpt-telos` verifier on the ideal layer.

> **Status:** `0.1.0`, unpublished (`publish = false`). It depends on the unpublished
> `tpt-solver` sibling checkout.

## Certification

[`certify`] takes the load, the delta, the policy and a claim, and returns a [`Certification`]:

| Claim | Solver result | Certification | Meaning |
| :--- | :--- | :--- | :--- |
| `Allow` (`Some(new_load)`) | satisfiable, model accepted | `Verified` | The claimed admission holds on the exact inputs |
| `Allow` | unsatisfiable | `Contradicted` | The solver refutes the admission |
| `Allow`, `new_load` off by more than the error bound | satisfiable | `Contradicted` | The f64 result is not within rounding error of the exact sum |
| `Deny` (`None`) | unsatisfiable (Farkas certificate) | `CertifiedDenial` | The denial is proven |
| `Deny` | satisfiable | `ConservativeDenial` | The ideal layer allows, and the f64 layer denied conservatively. This is permitted |
| any | resource budget exhausted | `Exhausted` | Undecided. Treated as a refusal |
| any | input outside the solver's exact range | `Inconclusive` | Undecided. Treated as a refusal |

Only `Verified` and `CertifiedDenial` / `ConservativeDenial` are acceptable outcomes for the
host. An `Allow` is never accepted on `Inconclusive` or `Exhausted`.

```rust
use tpt_norma_core::PolicyBounds;
use tpt_norma_solver::{certify, Certification};

let policy = PolicyBounds {
    max_capacity: 10.0,
    max_delta_per_call: 5.0,
};

// 2 + 3 = 5 is within capacity and within the per-call cap.
assert_eq!(certify(2.0, 3.0, policy, Some(5.0)), Certification::Verified);

// 9 + 4 = 13 exceeds capacity 10, so this denial is proven.
assert_eq!(certify(9.0, 4.0, policy, None), Certification::CertifiedDenial);
```

## CertifiedEvaluator

`CertifiedEvaluator<E>` wraps any `PolicyEvaluator` and checks every `Allow` against the solver:

- a refuted `Allow` becomes `Indeterminate` (`SolverRejected`);
- an unchecked `Allow` becomes `Indeterminate` (`SolverInconclusive`);
- only a verified `Allow` passes through;
- a `Deny` is returned unchanged, since denying is always safe.

[`tpt-norma-runtime`](../tpt-norma-runtime/)'s `Gate::new` puts this wrapper around the stack it
builds, so the solver check is on every path that can admit a request.

## Implication checking

The `implication` module checks that some premises imply a conclusion, using exact rationals.

```rust
use tpt_norma_solver::implication::{check_implication, int, Row};

// x <= 4 implies x <= 5. The multiplier 1 on the premise is the witness.
let premises = [Row::from_ints(&[1], 4)];
let conclusion = Row::from_ints(&[1], 5);
assert!(check_implication(&premises, &conclusion, &[int(1)]));
```

[`check_implication`] only verifies a witness. It never searches, so a `true` result is sound.
Strict conclusions are not handled. Every `ensures` in the `.telos` twins is non-strict, so this
does not limit them.

## Limits

- The solver receives ground constraints only. Each row is a single inequality over known
  constants, because the solver's `i128` rationals cannot hold the realization error bound.
  That bound is checked separately with `tpt-math-exact` big rationals, so only the ideal-layer
  conditions go through the kernel.
- The solver's fuel argument is not honoured on its linear-real-arithmetic path. The real bound
  is the fixed problem size.
- Any input that needs more than the solver's `i128` rationals cannot be converted exactly. Such
  inputs are `Inconclusive`.

## Testing

```sh
cargo test -p tpt-norma-solver
```

- `tests/certify.rs`: each row of the table above, plus property tests against the f64 validator.
- `tests/ideal_admit.rs`: exact certificates for the `admit` postconditions in
  `crates/tpt-norma-core/telos/action_validator.telos`, in place of the `tpt-telos` verifier.
- `tests/rate_limiter.rs`: exact checks of the postconditions in
  `crates/tpt-norma-core/telos/rate_limiter.telos`.
- `certify.proptest-regressions` records failing cases found by earlier property runs.

## Related

- What it certifies: [`tpt-norma-core`](../tpt-norma-core/)
- The evaluator it wraps: [`tpt-norma-eval`](../tpt-norma-eval/)
- Design decisions: [`docs/decisions.md`](../../docs/decisions.md) (D9, D17)

## License

MIT OR Apache-2.0. See the workspace [README](../../README.md#license).
