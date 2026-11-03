# tpt-norma-core

Verified, stateful validator for bounded AI-agent actions. Given one proposed action, one state
snapshot and one policy, it decides whether the action may go ahead, and if so what the new
state would be.

`tpt-norma-core` is the arithmetic heart of `tpt-norma`. It is a pure function with no I/O, no
clock and no shared state, so every decision can be reproduced from its three inputs.

> **Status:** `0.1.0`, unpublished (`publish = false`). It is an internal crate of the
> `tpt-norma` workspace and has no stable public API yet.

## What it does

A proposal carries one number, `resource_delta`: how much the agent wants to consume (or
add) in this call. The validator admits it only when all of these hold:

1. every input is finite (NaN and infinities are denied, never allowed);
2. the delta is non-negative;
3. the delta does not exceed `max_delta_per_call`;
4. the projected load `current_load + resource_delta` stays below `max_capacity`, less a small
   rounding margin.

If all four hold, it returns `Decision::Allow { new_load }`. Otherwise it returns `Decision::Deny`.
[`check_action`] returns the same decision with the reason for a denial, so the reason and the
decision cannot diverge.

## Usage

Inside the workspace, depend on the crate by path:

```toml
[dependencies]
tpt-norma-core = { path = "../tpt-norma-core" }
```

```rust
use tpt_norma_core::{validate_action, ActionProposal, Decision, PolicyBounds, SystemState};

let state = SystemState { current_load: 40.0 };
let policy = PolicyBounds {
    max_capacity: 100.0,
    max_delta_per_call: 10.0,
};
let proposal = ActionProposal { resource_delta: 5.0 };

match validate_action(state, proposal, policy) {
    Decision::Allow { new_load } => assert_eq!(new_load, 45.0),
    Decision::Deny => unreachable!("5 fits within both bounds"),
}
```

To validate one proposal across several independent resource dimensions, use
[`validate_multi`]. It is all-or-nothing, and it returns the first rejected dimension.

## Guarantees

The crate is written against the contracts in `spec.txt` §6. The two that matter most:

- **Conservative denial.** The f64 realization never allows what the exact-rational ideal
  would deny. A small capacity margin is reserved so that floating-point rounding cannot
  turn a denial into an allow. The reasoning is in
  [`docs/conservative-denial.md`](../../docs/conservative-denial.md).
- **Ideal-layer oracle.** [`symbolic_evaluate_ideal`] converts each `f64` input to its exact
  rational value and decides the same question with no rounding. The tests compare the f64
  validator against this oracle.

## Limits

- It validates **one proposal against one snapshot**. It does not make concurrent calls atomic.
  Callers must serialize validation and commit per resource (`spec.txt` §8). The
  `tpt-norma-runtime` crate provides that serialization.
- It trusts its state and policy inputs. Only the proposal is treated as adversarial.
- Inputs are assumed to be finite. Non-finite values are denied as defence in depth, not
  because they are a supported case.
- The `.telos` files under `telos/` describe the same contracts for an external verifier. They
  are written but not verified. See [`docs/telos-verifier-issue.md`](../../docs/telos-verifier-issue.md).

## Modules

| Module | Contents |
| :--- | :--- |
| `action_validator` | `validate_action`, `check_action`, `ActionProposal`, `SystemState`, `PolicyBounds`, `Decision`, `Rejection` |
| `allocator` | `validate_multi` and `DimensionRejection`, for independent resource dimensions |
| `ideal` | `symbolic_evaluate_ideal` and `IdealDecision`, the exact-rational reference |

## Testing

```sh
cargo test -p tpt-norma-core
```

- `tests/boundary.rs` sweeps ulp-level neighbours of special capacities (powers of two, the
  smallest normal value, subnormals) against the exact oracle.
- `tests/cross_check.rs` compares the f64 validator with the oracle on generated inputs.
- `tests/orthogonality.rs` shows concretely the non-orthogonal case from spec §5.2, where
  `validate_multi`'s composition does not hold.
- The property tests run 4096 cases each and take about a second.

The closed-form allocator is benchmarked by `examples/alloc_bench.rs`:

```sh
cargo run --release -p tpt-norma-core --example alloc_bench
```

Note that this times `validate_multi` only. It does not measure Fourier–Motzkin composition.

## Related

- Contracts: [`spec.txt`](../../spec.txt) §6 and §8
- Design decisions: [`docs/decisions.md`](../../docs/decisions.md) (D7, D16)
- Next layers: [`tpt-norma-eval`](../tpt-norma-eval/) wraps this check in a three-valued decision.

## License

MIT OR Apache-2.0. See the workspace [README](../../README.md#license).
