# tpt-norma-runtime

Snapshot freshness and the commit-time executor for `tpt-norma`. This crate turns a decision
into a state change, and runs a tool only after that change has been committed.

It holds the contract a tool executor calls. In the wider design that caller is `tpt-writ`.

> **Status:** `0.1.0`, unpublished (`publish = false`). Internal to the `tpt-norma` workspace.

## Lifecycle

```text
snapshot ──► authorize(request, snapshot) ──► Decision
                                                 │
                              Allow ─────────────┤
                                                 ▼
                                  StateStore::commit(request, decision)
                                                 │
                                                 ▼
                                     tool(request)   (runs only after commit)
```

Every step is written to the audit log. A request that is not allowed never reaches the tool.

- [`authorize`] rejects a request made against a stale state version as `Indeterminate`, without
  calling the evaluator.
- [`StateStore::commit`] rejects evidence made for a different request, or against a version
  that has since changed, and advances the version on success.
- [`Gate`] runs the whole sequence and records it.

## Usage

```toml
[dependencies]
tpt-norma-runtime = { path = "../tpt-norma-runtime" }
```

```rust
use std::time::Duration;
use tpt_norma_core::PolicyBounds;
use tpt_norma_eval::{
    ActionId, ActionRequest, ActorId, CanonicalBytes, CoreEvaluator, ResourceId, StateVersion,
};
use tpt_norma_runtime::{Gate, Refusal, StateStore};

let policy = CoreEvaluator {
    policy: PolicyBounds {
        max_capacity: 101.0,
        max_delta_per_call: 1.0,
    },
    policy_version: 1,
    verifier_identity: "core-v1".into(),
};
let mut gate = Gate::new(policy, StateStore::new(0.0), Duration::from_millis(50));

let request = ActionRequest {
    action_id: ActionId("call-1".into()),
    actor: ActorId("agent-7".into()),
    resource: ResourceId("quota".into()),
    expected_state_version: StateVersion(0),
    arguments: CanonicalBytes::from_delta(1.0),
};

match gate.submit(&request, |_| "tool output") {
    Ok(output) => println!("ran: {output}"),
    Err(Refusal::Denied { .. }) => println!("denied by policy"),
    Err(other) => println!("refused: {other:?}"),
}
```

[`Gate::new`] builds the guarded evaluator stack: `CoreEvaluator`, inside `Guarded`, inside
`CertifiedEvaluator` from [`tpt-norma-solver`](../tpt-norma-solver/). A gate cannot be built
around a bare evaluator from outside this crate, so a host cannot skip those checks by
accident.

## Choosing a call

| Method | Runs the tool | Commits the state | Use for |
| :--- | :---: | :---: | :--- |
| [`Gate::submit`] | yes, after commit | yes | Normal enforcement |
| [`Gate::admit`] | no | yes | Shadow mode for a pilot, or a host that runs its own tool |
| [`SharedGate::submit`] | yes, outside the lock | yes | Several threads sharing one resource in one process |

Shadow mode admits without running the tool. Admissions still commit, so the shadow budget
counts calls the way enforcement would.

## Refusals

`Refusal` is why a request did not reach its tool:

- `Denied { violations }`: the evaluator proved a violation.
- `Indeterminate { reason }`: safety could not be established.
- `Commit(CommitError)`: the decision was `Allow`, but the commit was refused.
- `Audit { message }`: an audit entry could not be persisted. Nothing has run.

## Failure semantics

- **Tool failure after commit.** The state change has already happened and the gate does not
  roll it back. Tools that can fail should be idempotent, or the caller should compensate
  (decision D8).
- **Audit failure.** If the `Requested` or `Decided` entry cannot be persisted, the request is
  refused and no state changes. If the final `Committed` entry cannot be persisted, the state
  has already changed. The gate refuses without running the tool, and the caller must reconcile
  the state from the store.
- **Lock poisoning.** If a previous call panicked while holding a `SharedGate` lock, the state
  can no longer be trusted. Requests are refused as `Indeterminate` rather than admitted.

## Limits

- `SharedGate` serializes requests for one resource inside one process only. Two processes, or
  two hosts, must not run gates for the same resource. Each resource needs one gate.
- `StateStore` is in memory. Persistence is the caller's job, through an `AuditSink` for the log
  and whatever store the host uses for state.
- The deadline in `Gate::new` is checked after the evaluation returns (see `tpt-norma-eval`).

## Modules

| Module | Contents |
| :--- | :--- |
| crate root | `authorize`, `StateStore`, `CommitError` |
| `gate` | `Gate`, `GuardedEvaluator`, `Refusal` |
| `shared` | `SharedGate` |

## Testing

```sh
cargo test -p tpt-norma-runtime
```

- `tests/acceptance.rs`: the acceptance tests from `spec2.txt` §5, plus the guards from §4.
- `tests/gate.rs`: the gate.
- `tests/shared.rs`: the shared gate.
- `tests/shadow.rs`: shadow admission, which commits but does not run the tool.
- `tests/audit_sink.rs`: the gate writes each audit entry through its sink, and fails closed.

Pilot host, using the numbers in `examples/pilot/quota.policy`:

```sh
cargo run -p tpt-norma-runtime --example pilot_shadow
```

## Related

- Evaluation and evidence: [`tpt-norma-eval`](../tpt-norma-eval/)
- Audit log: [`tpt-norma-audit`](../tpt-norma-audit/)
- C binding of the gate: [`tpt-norma-ffi`](../tpt-norma-ffi/)
- Design decisions: [`docs/decisions.md`](../../docs/decisions.md) (D4, D8, D15, D18)

## License

MIT OR Apache-2.0. See the workspace [README](../../README.md#license).
