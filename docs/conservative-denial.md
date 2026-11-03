# Conservative denial: interim argument (draft for review)

This is a hand argument, not a machine-checked proof. It stands in for the realization-layer proof until Phase 1 lands. The property tests in `crates/tpt-norma-core/tests/cross_check.rs` are supporting evidence only.

## Claim

If the exact sum `s = load + delta` exceeds `max`, then `validate_action` denies, where `max` is the policy capacity and `load`, `delta` are finite f64 inputs with `0 <= delta <= max_delta_per_call`.

## Setup

Let `f = fl(load + delta)` be the f64 sum, and `E = (|load| + |delta|) * EPS`, with `EPS = f64::EPSILON`. The code denies when `f > fl(max - E)`.

## Argument

1. **Rounding error of one addition.** Round-to-nearest gives `|f - s| <= ulp(s) / 2 <= |s| * EPS / 2`. Since `|s| <= |load| + |delta|`, this is at most `E / 2`.

2. **The exact sum is above the capacity.** `s > max` gives `f >= s - E/2 > max - E/2`.

3. **Where the threshold lies.** The code's threshold is `fl(max - E)`, which is at most `max - E + ulp(max - E)/2`. Rounding can move it up by half an ulp. We need `f > fl(max - E)`, and step 2 gives a lower bound for `f`, so it is enough to show that `max - E/2 >= max - E + ulp(max)/2`. That reduces to `E >= ulp(max)`.

4. **`E >= ulp(max)` in the case that matters.** The argument only needs to cover `s > max`. In that case `|load| + |delta| >= |s| > max`, so `E = (|load| + |delta|) * EPS > max * EPS >= ulp(max)`. The last step uses `ulp(x) <= |x| * EPS` for normal `x`.

5. **Conclusion.** `f > fl(max - E)`, so the capacity check fires and the code denies. This holds for every `s > max`, so an ideal Deny for capacity is always an f64 Deny.

## Scope and gaps

- **Normal range only.** Step 4 uses `ulp(x) <= |x| * EPS`, which holds for normal `x`. Subnormal values and the zero boundary need a separate case check. The property tests cover them (`f64::MIN_POSITIVE`, zero and negative zero), but the hand argument does not.
- **Negative capacity is out of contract.** Policy requires `max >= 0`.
- **Rate-limit and non-negativity checks are exact.** They compare the inputs directly, with no arithmetic, so they add no rounding error.
- **Not machine-checked.** Replacing this argument with a verified proof is the open Phase 1 / realization-layer item in the todo.
