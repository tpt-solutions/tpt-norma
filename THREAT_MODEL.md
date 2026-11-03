# Threat Model & Assumptions (draft)

**Status: draft, not reviewed or signed off.** This is the standalone form of spec §8 plus the spec2 additions. Spec §9 Exit Criterion 5 requires independent review before any "no false allows" claim is made externally.

## Scope of the guarantees

The "no false allow" claim covers `tpt-norma-core::validate_action` and `symbolic_evaluate_ideal`, and the decision and commit path in `tpt-norma-eval` and `tpt-norma-runtime`. It holds only under the assumptions below. Outside them, it is not claimed.

## Assumptions

1. **Finite, non-NaN inputs.** The schema parser is expected to reject NaN and infinite values before they reach the engine. As defence in depth, `check_action` denies non-finite inputs, and `CanonicalBytes::to_delta` rejects them. A non-finite *state* value yields `Indeterminate`, not `Deny`.
2. **Serialized validation and commit per resource.** A proof covers one call against one snapshot. `StateStore::commit` takes `&mut self`, so one owner serializes commits. Concurrent owners of the same resource still need an external mutex, actor or single-writer queue. This is not enforced by the engine.
3. **Single resource dimension.** The validator checks one delta against one load and capacity. Multi-dimensional policies depend on the orthogonality argument in spec §5.2, which is not yet verified.
4. **Trusted policy and state.** `PolicyBounds`, the policy version, the verifier identity and `StateSnapshot` come from a trusted layer. Only the `ActionRequest` arguments are adversarial.
5. **No enforcement bypass, unless the host is wired correctly.** The engine cannot stop a host from calling `commit` without `authorize`, or from ignoring `Deny` or `Indeterminate`. Only the commit path, which requires `Allow` evidence, is covered. The host's FFI boundary is not verified (spec §4, `tpt-norma-runtime` FFI bridge, not yet built).

## Runtime guarantees (spec2)

- **Fail-closed.** Missing state, stale state, malformed arguments, a panic, a timeout and an inconsistent `Allow` all produce `Indeterminate`. Only `Allow` permits commit.
- **Three-valued outcomes.** `Deny` is a proven violation. `Indeterminate` means safety was not established. Both are treated as refusal by the executor.
- **Structural evidence binding.** `DecisionEvidence` stores the full `ActionRequest`. A commit compares it exactly, so an `Allow` for action A cannot be applied to a request B that differs in any field, including the argument bit pattern. A cryptographic MAC is deferred (see `docs/decisions.md`).
- **State-version freshness.** `authorize` rejects a request whose expected version differs from the snapshot. `StateStore::commit` rejects evidence whose version differs from the current one.
- **Provenance.** Each `Allow` records the policy version and verifier identity.

## Limits that are not yet enforced

- **Wall-clock deadline is post-hoc.** `Guarded` checks elapsed time after the evaluator returns. It cannot interrupt a call that is still running. A hard deadline needs a separate thread or process.
- **No fuel or memory limits.** The engine has no CPU-fuel or allocation budget. The core check is O(1), so this is low risk today. It becomes a real requirement with `tpt-norma-solver`.
- **No solver integration yet.** Constraints requiring SMT or certificate checking are not handled, so no `Inconclusive` path exists.
- **Persistence and audit.** The commit log is in memory. Durable, tamper-evident audit records (`tpt-norma-audit`) are not built.

## Verification status

- **Realization layer:** property-tested in `crates/tpt-norma-core/tests/cross_check.rs` (4096 cases per property, including boundary-biased inputs). This is evidence, not a machine-checked proof. The interim written argument for conservative denial is still owed (todo, Phase 2).
- **Ideal layer (`.telos` twin):** `crates/tpt-norma-core/telos/action_validator.telos` is written but **not verified**. The `telos` build from `tpt-telos` at the time of writing does not check `old()` equality postconditions correctly. A false postcondition (`ensures ledger.load == old(ledger.load)` with `delta` pinned to 1) was reported as PASS. Plain (non-`mutate`) functions produce no output at all. Until this is fixed upstream, the twin is not evidence.

## Domain caveats

- **Healthcare.** `tpt-norma` enforces hard numeric bounds, for example a maximum dose. It does not make clinical judgment, assure data quality or provide human oversight. Those stay with the application layer.
- **Aviation.** `tpt-norma` is for mission planning and macro-state transitions. It must not sit in a hard real-time control loop (for example 100 Hz).

## Reviewers' checklist (for sign-off)

- [ ] Assumptions 1–5 accepted as the deployment contract.
- [ ] Post-hoc deadline and absent fuel limits accepted, or a hard bound designed.
- [ ] `.telos` twin re-verified after the `old()` issue is fixed upstream.
- [ ] Interim conservative-denial argument written and reviewed.
- [ ] Independent third-party review (spec §7 Phase 3).
