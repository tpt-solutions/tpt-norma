# tpt-norma-linear

Linear constraints and an exact contradiction check over the rationals. It decides whether a set
of linear constraints has a common solution, using Fourier–Motzkin elimination with big
rationals, and it reports which named groups of constraints contradict each other.

It replaces the `tpt-telos-ir` / `tpt-telos-sdk` check that `tpt-norma-policy` used before, so
the workspace no longer depends on the `tpt-telos` checkout.

> **Status:** `0.1.0`, unpublished (`publish = false`). Internal to the `tpt-norma` workspace.

## Model

A [`Constraint`] is `expr REL 0`:

- `expr` is a [`Linear`] expression, `Σ coeff·var + constant`, over named variables with `i64`
  coefficients;
- `REL` is one of `Le`, `Lt`, `Ge`, `Gt` or `Eq`.

Constraints are grouped into [`NamedConstraints`], for example one policy rule or one dimension's
bounds. [`check_contradictions`] checks each group, every pair of groups, and all groups together.

## Usage

```toml
[dependencies]
tpt-norma-linear = { path = "../tpt-norma-linear" }
```

```rust
use tpt_norma_linear::{is_unsatisfiable, Constraint, Linear, Relation};

// x <= 0 and x >= 1 cannot both hold.
let x = Linear::var("x");
let constraints = [
    Constraint { expr: x.clone(), relation: Relation::Le },
    Constraint {
        expr: Linear::constant_only(1).sub(&x),
        relation: Relation::Le,
    },
];

assert_eq!(is_unsatisfiable(&constraints), Some(true));
```

`is_unsatisfiable` returns `Some(true)` for unsatisfiable, `Some(false)` for satisfiable, and `None`
when it gives up.

## Reading the result

| Result | Meaning |
| :--- | :--- |
| `Some(true)` | Unsatisfiable. This is sound: no rational solution means no integer solution either |
| `Some(false)` | Satisfiable over the rationals only. An integer-only policy can still be empty, so this is not an integer witness |
| `None` | Elimination grew past `MAX_ROWS` (10,000) and stopped. Undecided |

[`ContradictionReport`] carries the same three-way answer as `overall_unsat`, plus `pairs`: each
[`Contradiction`] names two groups that cannot hold together, and `decided`, which is false when
any check in the report gave up.

## Algorithm

1. Each constraint is rewritten as one or two rows of the form `Σ coeff·var + constant ≤ 0`
   (or `< 0` when strict).
2. Variables are eliminated one at a time. For each variable, every upper bound is paired with
   every lower bound, and the variable's coefficient cancels.
3. When no variables remain, a constant row that is false is a contradiction. If every constant
   row holds, the system is satisfiable.

All arithmetic is exact (`tpt-math-exact`'s `BigInt` and `BigRational`), so the result has no
rounding.

## Limits

- Decisions are over the rationals. Integer-only reasoning is out of scope.
- Fourier–Motzkin can grow doubly exponentially in the number of variables. `MAX_ROWS` bounds the
  working set, and the answer is `None` past that. Policies with many interacting rules may hit
  this, so the caller must treat `None` as unverified.
- Coefficients and constants are `i64`. Policies in fixed-point units must be scaled first.

## Testing

```sh
cargo test -p tpt-norma-linear
```

Unit tests live in `src/lib.rs`. To run the Fourier–Motzkin benchmark:

```sh
cargo run --release -p tpt-norma-linear --example fm_bench
```

## Related

- Consumer: [`tpt-norma-policy`](../tpt-norma-policy/), which uses it for consistency checking
- Design decisions: [`docs/decisions.md`](../../docs/decisions.md) (D19)

## License

MIT OR Apache-2.0. See the workspace [README](../../README.md#license).
