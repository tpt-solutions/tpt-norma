# Conservative denial: interim argument (draft for review)

This is a hand argument, not a machine-checked proof. It replaces the earlier argument, which
did not account for the rounding of the margin itself. The deterministic test
`crates/tpt-norma-core/tests/boundary.rs` and the property tests in
`crates/tpt-norma-core/tests/cross_check.rs` are supporting evidence only.

## Claim

Let `L`, `D`, `M` be finite f64 values with `L >= 0`, `D >= 0`, `M >= 0`. If the exact sum
`s = L + D` exceeds `M`, then `check_action` returns `Rejection::Capacity`.

Negative delta and delta above `max_delta_per_call` are exact comparisons with no arithmetic,
so they match the ideal layer and are not covered here.

## What the code computes

With `L`, `D` non-negative, `|L| + |D|` is `L + D`, so both sums are the same rounded value:

- `f = fl(L + D)`, the projected load.
- `e = fl(f · EPS)`, the margin, with `EPS = 2^-52`. Here `|L| + |D|` rounds to `f` again, so the
  margin is computed from `f`, not from the exact sum.
- The code denies when `f > fl(M - e)`.

## Argument

Assume `s > M`. We show `f > fl(M - e)`.

**Case `M = 0`.** Since `s > 0` and `L`, `D` are multiples of `2^-1074`, `s` is representable, so
`f = s > 0`. The margin `e` is non-negative, so `fl(M - e) <= 0 < f`.

**Case `0 < M` subnormal.** Sums of multiples of `2^-1074` are exact while they are subnormal.
So `s > M` gives `f > M`, since `f` is at least `M + 2^-1074` when it is subnormal, and is normal
otherwise. Rounding is monotone and `e >= 0`, so `fl(M - e) <= M < f`.

**Case `M` normal (`M >= 2^-1022`).**

1. *Monotone rounding.* `M` is a double and `s > M`, so `f = fl(s) >= fl(M) = M`.
2. *Lower bound on the margin.* `f >= M` and `f · 2^-52` is exact when normal. If it is
   subnormal its error is at most `2^-1075`. So `e >= M · 2^-52 - 2^-1075`.
3. *Where `M - e` rounds.* Let `p` be the predecessor of `M` and `g = M - p`. Then
   `g / 2 <= M · 2^-53`, with equality only for `M = 2^k`, where `g / 2 = M · 2^-54`.
   Any real below the midpoint `M - g/2` rounds to at most `p < M`. So
   `fl(M - e) < M` if `M - e < M - g/2`.
4. *That inequality holds.* It reduces to `2^-1075 + g/2 < M · 2^-52`. Using `g/2 <= M · 2^-53`
   it suffices that `2^-1075 < M · 2^-53`, which is `M > 2^-1022`.
5. *The boundary `M = 2^-1022`.* Here `g = 2^-1074`, so the bound in step 4 gives equality and
   needs a direct argument. Since `f >= M = 2^-1022`, `f · 2^-52 >= 2^-1074`. Subnormal doubles are
   multiples of `2^-1074`, so rounding to nearest gives `e >= 2^-1074`. Then
   `M - e <= M - 2^-1074 = p`, and `fl(M - e) <= p < M <= f`. The deterministic test
   `boundary.rs` covers this capacity.

Combining the cases: `fl(M - e) < M <= f` whenever `M` is normal, and `fl(M - e) <= M < f`
whenever `M` is subnormal. So `f > fl(M - e)`, and the code denies. This holds for every
`s > M`, so an ideal capacity Deny is always an f64 Deny.

## Overflow

If `f` or `e` overflows to infinity, the code's comparison still denies: an infinite `f` is above
any threshold, and an infinite `e` makes the threshold `-inf`. Both directions are safe.

## Scope and gaps

- **Non-negative inputs only.** The argument uses `L, D >= 0`, and the ideal contract already
  requires `load >= 0` and `max >= 0`. Negative delta is denied by the exact check before this
  argument applies.
- **Not machine-checked.** The argument is a hand proof with explicit cases. The
  `boundary.rs` test and the property tests check the claim at the special capacities and at
  random ulp neighbours. They do not prove it. Replacing this with a verified proof is the open
  realization-layer item in the todo.
- **Earlier argument.** The previous draft treated the margin as the exact value
  `(|L| + |D|) · EPS` and did not account for the rounding that the code applies to it. This
  version uses the code's own margin `e` and covers the subnormal and power-of-two cases
  explicitly. The earlier draft's conclusion is unchanged.

## Three-valued postcondition (spec2)

spec2 adds `Indeterminate` to the outcomes, so the realization-layer postcondition is restated as:

> If the ideal-layer check denies a request, the result is `Deny` or `Indeterminate`, never `Allow`.

This follows from the argument above and from how `tpt-norma-eval` maps core results:

- **Capacity.** The exact sum `s > max` makes the f64 check deny (cases above). `tpt-norma-eval`
  maps `Rejection::Capacity` to `Deny`.
- **Negative delta and rate limit.** These are exact comparisons with no arithmetic, so the f64
  check denies exactly when the ideal check does. Both map to `Deny`.
- **Non-finite input.** Here the ideal check is not defined. The core returns
  `Rejection::NonFinite`, which `tpt-norma-eval` maps to `Indeterminate`, not `Allow`. Spec §6.1
  leaves this to the upstream schema parser, and the core guards against it as defence in depth.
- **Panic, timeout, stale state, solver budget or inconclusive answer.** Each maps to
  `Indeterminate`. An `Allow` is returned only when the core returns `Ok` and the evidence binds
  the exact request and state version.

So no path from an ideal Deny reaches `Allow`. This holds for the code as it stands, under the
scope above.
