# tpt-telos verifier: postconditions that pass when they should fail

Found while building the ideal-layer twin `crates/tpt-norma-core/telos/action_validator.telos`.
Reproduced with `tpt-telos` at commit `23d53eb` (master), built with `cargo build --release -p tpt-telos`,
run from the `tpt-telos` directory (the CLI prints nothing for relative-path runs made from elsewhere).

## 1. `old()` equality postconditions on struct fields pass when false

```telos
module R {
    invariant Box { v >= 0 }
    func bump(mut b: Box, d: Int) -> Int
        requires d == 1
        ensures b.v == old(b.v) + 5      // false: post-state is old + 1
    {
        mutate state { b.v = old(b.v) + d; }
        b.v
    }
}
```

`telos verify` reports `[PASS] ensures b.v == old(b.v) + 5`. The inequality `ensures b.v <= old(b.v)`
also passes, although it is false for the same reason. The same claim with a constant, `ensures b.v == 12345`,
does FAIL, so the problem is specific to `old()` in the postcondition.

## 2. Postconditions on scalar `mut` parameters are not checked

```telos
module R {
    func bump(mut b: Int, d: Int) -> Int
        requires d == 1
        ensures b == 999                 // false
    {
        mutate state { b = old(b) + d; }
        b
    }
}
```

This reports `[PASS] ensures b == 999`, and it is not an `old()` problem. The verifier appears to
produce no obligations for scalar `mut` parameters, so their postconditions are never checked.

## Impact

Any `.telos` file that relies on `old()` equality or on scalar `mut` postconditions can report PASS for
false claims. `tpt-norma` does not treat its ideal-layer twin as evidence until this is fixed.

## Suggested next step

Add regression tests to `tpt-telos` for both cases above, expecting FAIL.
