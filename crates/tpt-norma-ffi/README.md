# tpt-norma-ffi

A C ABI over the `tpt-norma` validator and shared gate. It lets a non-Rust host call the
validator, or admit and commit a tool call through the gate, using a hand-written header.

Built as `staticlib`, `cdylib` and `rlib`. The `cdylib` is what the Go binding in
[`bindings/go/tpnorma`](../../bindings/go/tpnorma/) loads.

> **Status:** `0.1.0`, unpublished (`publish = false`). The Rust side is implemented. The Go
> bridge is written, and the C-host compile test is not yet built.

## Return codes

Every function returns one of these. **Treat anything other than `TN_ALLOW` as a refusal.**

| Constant | Value | Meaning |
| :--- | :---: | :--- |
| `TN_ALLOW` | `1` | Admitted. Any out-parameter has been written |
| `TN_DENY` | `0` | Proven to violate policy |
| `TN_INDETERMINATE` | `-1` | Safety could not be established |
| `TN_ERROR` | `-2` | Bad argument, a null pointer, or a caught panic |

Every exported function catches panics, so no unwind crosses the C boundary.

## Functions

| Function | Changes state | Purpose |
| :--- | :---: | :--- |
| `tn_validate_action(load, delta, max_capacity, max_delta_per_call, out_new_load)` | no | Validate one proposal. `out_new_load` may be null |
| `tn_validate_action_bits(...)` | no | As above, with each `f64` passed as its IEEE-754 bit pattern |
| `tn_gate_new(load, max_capacity, max_delta_per_call)` | n/a | Create a gate for one resource. Returns null if the arguments cannot be used |
| `tn_gate_new_bits(...)` | n/a | As above, with bit patterns |
| `tn_gate_admit(gate, action, resource, delta, expected_version)` | yes, on `TN_ALLOW` | Authorize and commit, without running a tool |
| `tn_gate_admit_bits(...)` | yes, on `TN_ALLOW` | As above, with bit patterns |
| `tn_gate_state(gate, out_load, out_version)` | no | Read the committed load and version |
| `tn_gate_free(gate)` | n/a | Release a gate. Null is accepted |

Hosts that cannot pass `double` arguments directly, such as Go's `syscall` on Windows, use the
`_bits` forms. They behave exactly as the `f64` versions.

## Usage from C

```c
#include "tpt_norma.h"

TnGate *gate = tn_gate_new(0.0, 101.0, 1.0);
if (gate == NULL) {
    /* refuse: the arguments cannot be used */
}

int32_t rc = tn_gate_admit(gate, "call-1", "quota", 1.0, 0);
if (rc == TN_ALLOW) {
    /* run the tool. The state has already advanced. */
} else {
    /* refuse */
}

tn_gate_free(gate);
```

Every pointer returned by `tn_gate_new` must be freed exactly once with `tn_gate_free`.
Strings passed to `tn_gate_admit` must be NUL-terminated UTF-8.

The header is [`include/tpt_norma.h`](include/tpt_norma.h). It is hand-written to match
`src/lib.rs`, and the two must be kept in step.

## What the bridge does and does not enforce

- **Coverage.** It enforces only what the host routes through it. A host that calls its tool
  without first calling `tn_gate_admit` is not covered.
- **Commit, then run.** `tn_gate_admit` authorizes and commits. It does not run the tool. If the
  tool fails after `TN_ALLOW`, the state change has already happened.
- **The boundary itself.** It is not proven. Its safety rests on the argument checks, which
  reject null pointers and invalid UTF-8, and on the host passing pointers it owns.

These limits are stated in [`THREAT_MODEL.md`](../../THREAT_MODEL.md).

## Safety

`tpt-norma-ffi` opts out of the workspace's `unsafe_code = "forbid"`, because it must dereference
raw pointers. It sets `unsafe_code = "deny"` instead, and allows `unsafe` only in the FFI module.
Each `unsafe` function documents its contract under `# Safety`.

The crate also sets `unused_must_use = "deny"`, so a dropped `Result` in the bridge is a build
error.

## Limits

- Evaluation has a deadline, but it is checked after the call returns. See
  [`tpt-norma-eval`](../tpt-norma-eval/).
- The API covers one action type, a single resource delta, and one resource per gate.
- The gate is in memory. A process that holds the gate must be the only writer for that resource.

## Testing

```sh
cargo test -p tpt-norma-ffi
```

The Rust tests call the exported functions directly. The C-host compile test is not built yet.

## Related

- Validator: [`tpt-norma-core`](../tpt-norma-core/)
- Gate: [`tpt-norma-runtime`](../tpt-norma-runtime/)
- Go binding: [`bindings/go/tpnorma`](../../bindings/go/tpnorma/)
- Design decisions: [`docs/decisions.md`](../../docs/decisions.md) (D13)

## License

MIT OR Apache-2.0. See the workspace [README](../../README.md#license).
