# tpt-norma-policy

A small text language for describing an agent's policy: named integer dimensions with inclusive
ranges, and named linear rules over them. It parses that text, checks that the policy is
consistent, and tests whether a given point is admitted.

> **Status:** `0.1.0`, unpublished (`publish = false`). Internal to the `tpt-norma` workspace.

## Language

The DSL is line based. `#` starts a comment, and blank lines are ignored.

```text
# comments start with '#'
dim delta in [0, 50]
dim load  in [0, 1000]
rule per_call_cap: delta <= 50
rule headroom:     load + 2 * delta <= 1000
```

- `dim NAME in [MIN, MAX]` declares a dimension. Both bounds are inclusive `i64` values.
- `rule NAME: LHS OP RHS` declares a rule. `OP` is one of `<=`, `<`, `>=`, `>` or `==`.
- An expression is a sum of terms, each a variable, `k * var`, or an integer constant. Example:
  `load + 2 * delta - 3`.
- Every rule may use only declared dimensions, and no dimension may be declared twice.

Coefficients and bounds are `i64`. A policy in fixed-point units, such as micro-units, must be
scaled before it is written here.

The pilot policy is [`examples/pilot/quota.policy`](../../examples/pilot/quota.policy). It caps
each call at a delta of 1 and a total budget of 101, which admits 100 calls because the capacity
margin in `docs/decisions.md` (D16) denies an exact fit.

## Usage

```toml
[dependencies]
tpt-norma-policy = { path = "../tpt-norma-policy" }
```

```rust
use std::collections::BTreeMap;
use tpt_norma_policy::PolicySpace;

let space = PolicySpace::parse(
    "dim delta in [0, 50]\n\
     dim load in [0, 1000]\n\
     rule per_call_cap: delta <= 50\n\
     rule headroom: load + 2 * delta <= 1000\n",
)?;

let consistency = space.check_consistency()?;
assert!(consistency.decided && consistency.consistent);

let mut point = BTreeMap::new();
point.insert("delta".to_string(), 10);
point.insert("load".to_string(), 500);
assert!(space.admits(&point));
# Ok::<(), Box<dyn std::error::Error>>(())
```

## Operations

| Method | Purpose |
| :--- | :--- |
| `PolicySpace::parse` | Parse the DSL. Returns the first `PolicyError`, with its line number |
| `validate_names` | Check that rules use declared dimensions only, and that no name is declared twice |
| `admits` | Whether a point lies inside every dimension's range and satisfies every rule. A dimension missing from the point is not admitted |
| `check_consistency` | Whether the policy can be satisfied at all, and which groups contradict each other |

## Consistency

`check_consistency` hands each rule, and each dimension's bounds, to
[`tpt-norma-linear`](../tpt-norma-linear/) as a named group. The returned `Consistency` says:

- `consistent`: no two groups contradict, and the space is not empty;
- `decided`: whether the linear check finished. If it is `false`, treat the policy as unverified;
- `contradicting_pairs`: pairs of labels that can never hold together. A label is a rule name, or
  `dim:NAME` for a dimension's bounds.

Consistency is checked over the rationals. A policy can be consistent over the rationals and still
have no integer point. `admits` is the integer test.

## Errors

`PolicyError` carries a 1-based `line` and a `message`. Errors from `validate_names` report line
`0`, because the check runs after parsing.

## Limits

- Only linear rules are supported. There are no disjunctions, no non-integer constants and no
  variables outside the declared dimensions.
- The language describes boxes and linear rules. It does not describe time windows or sequences
  of actions. The pilot policy notes that it has no time window.
- Consistency inherits the row limit of `tpt-norma-linear`.

## Testing

```sh
cargo test -p tpt-norma-policy
```

- Unit tests live in `src/lib.rs`.
- `tests/pilot.rs` checks that the pilot policy parses and is consistent.

## Related

- Consistency engine: [`tpt-norma-linear`](../tpt-norma-linear/)
- Pilot policy: [`examples/pilot/quota.policy`](../../examples/pilot/quota.policy)
- Pilot host: [`tpt-norma-runtime`](../tpt-norma-runtime/) (`examples/pilot_shadow.rs`)

## License

MIT OR Apache-2.0. See the workspace [README](../../README.md#license).
