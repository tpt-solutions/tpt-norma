# tpt-norma-eval

Three-valued policy evaluation for `tpt-norma`. It wraps the two-valued check from
[`tpt-norma-core`](../tpt-norma-core/) in a decision type with a third outcome, `Indeterminate`,
so that a missing, stale, malformed or unverifiable input is never reported as `Allow`.

> **Status:** `0.1.0`, unpublished (`publish = false`). Internal to the `tpt-norma` workspace.

## Why three values

`tpt-norma-core` answers Allow or Deny. That is not enough at the boundary between an LLM and
a tool. If the state has no load value, the arguments fail to decode, or the evaluator panics,
a two-valued answer forces a guess. Here the guess is always "not proven safe":

| Outcome | Meaning | What the executor does |
| :--- | :--- | :--- |
| `Allow { evidence }` | Proven safe against this snapshot and policy | Commits the state change |
| `Deny { violations }` | Proven to violate a constraint | Refuses, and records why |
| `Indeterminate { reason }` | Safety could not be established | Refuses |

Only `Allow` is an admission. `Decision::is_allow` is the only helper, and it returns true for
`Allow` alone.

## Usage

```toml
[dependencies]
tpt-norma-eval = { path = "../tpt-norma-eval" }
```

`CoreEvaluator` is the reference evaluator. It delegates the arithmetic to `check_action` and
records the policy version and verifier identity as provenance. `Guarded` adds fail-closed
checks around any evaluator: a deadline, a panic guard, a size limit on arguments, and a
binding check on the evidence.

```rust
use std::time::Duration;
use tpt_norma_core::PolicyBounds;
use tpt_norma_eval::{
    ActionId, ActionRequest, ActorId, CanonicalBytes, CoreEvaluator, Guarded, PolicyEvaluator,
    ResourceId, StateSnapshot, StateVersion,
};

let evaluator = Guarded::new(
    CoreEvaluator {
        policy: PolicyBounds {
            max_capacity: 101.0,
            max_delta_per_call: 1.0,
        },
        policy_version: 1,
        verifier_identity: "core-v1".into(),
    },
    Duration::from_millis(50),
);

let request = ActionRequest {
    action_id: ActionId("call-1".into()),
    actor: ActorId("agent-7".into()),
    resource: ResourceId("quota".into()),
    expected_state_version: StateVersion(0),
    arguments: CanonicalBytes::from_delta(1.0),
};
let snapshot = StateSnapshot {
    version: StateVersion(0),
    current_load: Some(0.0),
};

assert!(evaluator.evaluate(&request, &snapshot).is_allow());
```

## Types

| Type | Role |
| :--- | :--- |
| `StateVersion`, `ActionId`, `ActorId`, `ResourceId` | Identifiers. Each is a plain newtype |
| `CanonicalBytes` | Canonical encoding of the arguments. For this action set, an 8-byte IEEE-754 delta. Equality is bit-exact |
| `StateSnapshot` | A versioned view of the load. `current_load: None` means "no value", and is never read as zero |
| `ActionRequest` | One proposal: identifiers, the state version it was made against, and its arguments |
| `DecisionEvidence` | What an `Allow` rests on. `binds(&request)` checks that it was produced for exactly this request |
| `Violation` | A proven reason for a denial |
| `EvaluationFailure` | Why a decision is `Indeterminate` |

## Guards

`Guarded<E>` turns these into `Indeterminate`, never `Allow`:

- a panic in the inner evaluator;
- a call that runs past its deadline;
- an `Allow` whose evidence does not bind this request and state version;
- an argument encoding larger than the limit (`DEFAULT_MAX_ARGUMENT_BYTES`, 1024 bytes, or
  `with_max_argument_bytes`).

The deadline is checked after the call returns. It cannot interrupt a call that is still
running, so a hard bound needs a separate thread or process. The crate documents this in
`Guarded`.

## Limits

- The evaluator must be side-effect free and depend only on its inputs and its fixed policy
  (see the `PolicyEvaluator` trait docs).
- Fuel is not enforced here. The evaluator is a closed-form check, and the solver's fuel is
  not honoured on its linear-real-arithmetic path (see `tpt-norma-solver`).
- Argument encoding is currently defined for one action only: a single resource delta.

## Testing

```sh
cargo test -p tpt-norma-eval
```

- `tests/evaluator.rs`: the reference evaluator and the fail-closed guards around it.
- `tests/limits.rs`: the limits on argument size.

## Related

- Underlying check: [`tpt-norma-core`](../tpt-norma-core/)
- Certification of `Allow` through the solver: [`tpt-norma-solver`](../tpt-norma-solver/)
- Consumer of decisions: [`tpt-norma-runtime`](../tpt-norma-runtime/)
- Design decisions: [`docs/decisions.md`](../../docs/decisions.md) (D3, D5, D6, D7)

## License

MIT OR Apache-2.0. See the workspace [README](../../README.md#license).
