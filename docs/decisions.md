# Design decisions

Records the choices made for the open questions in todo Phase 2b. Each entry is the recommendation from the todo, taken as the decision. Revisit an entry if its premise changes.

## D1. `Decision` name clash (spec §6 vs spec2)

**Decision:** keep the two-valued `tpt_norma_core::Decision` for spec §6, and give the three-valued `Decision` (`Allow` / `Deny` / `Indeterminate`) its own type in `tpt-norma-eval`. The eval type wraps the core result.

**Why:** spec §6 defines the core as a two-valued check, and its proofs refer to that type. spec2 needs three values. Keeping both avoids changing the proven contract.

## D2. Crate layout

**Decision:** adopt spec2's layout: `tpt-norma-core`, `tpt-norma-eval`, `tpt-norma-runtime`, and later `tpt-norma-solver` and `tpt-norma-audit`. Keep spec §4's `tpt-norma-policy` and `tpt-norma-proof` as later additions.

**Why:** spec2 is the newer design. Its layout maps onto the lifecycle directly (evaluate, then runtime authorization and commit). The todo's core/policy/runtime/proof names are kept for the policy DSL and certificates, which are not built yet.

**Scope clash resolved:** `tpt-norma-runtime` owns the state and freshness lifecycle (`authorize`, `StateStore::commit`). The FFI bridge from spec §4 is a separate crate to be added later. It is not part of this one.

## D3. Evidence binding

**Decision:** structural binding. `DecisionEvidence` stores the full `ActionRequest`, and a commit requires an exact match. No MAC or signature yet.

**Why:** the threat in spec2 §5 is a host that reuses evidence for a modified action or a different state. Structural equality closes that inside one process. A MAC or signature matters once evidence crosses a trust boundary, such as to `tpt-writ` in another process. That is when key ownership needs deciding.

**Revisit when:** evidence is passed out of process.

## D4. Freshness and commit

**Decision:** the commit-time check lives in `tpt-norma-runtime` (`StateStore::commit`). A stale request is `Indeterminate` at authorization, and a version change between evaluation and commit is a `StateDrift` error. The store takes `&mut self`, so one owner serializes commits. Multiple owners need external serialization.

**Why:** this is the choice that puts the check at the point where state is actually written.

## D5. Stale-state outcome

**Decision:** `Indeterminate { StaleState }`, not `Deny`.

**Why:** the request may be valid against the current state, so the correct outcome is "retry with a fresh snapshot", not "this action is unsafe".

## D6. Timeouts and panics

**Decision:** a panic becomes `Indeterminate(EvaluatorPanicked)`. A call that exceeds its deadline becomes `Indeterminate(Timeout)`. Both are checked after the call returns.

**Limit:** this cannot interrupt a running call. A hard deadline needs a thread or process boundary, which is deferred. Fuel and memory limits are deferred as well.

## D7. Non-finite inputs

**Decision:** the core denies non-finite inputs (defence in depth, beyond spec §6's upstream obligation). Non-finite state or policy values give `Indeterminate`.

**Why:** every comparison with NaN is false, so without the guard a NaN would fall through to `Allow`.

## Deferred

- **`tpt-norma-solver` and `tpt-norma-audit`:** need `tpt-solver`'s API and `tpt-concord`, which is not checked out. Not started.
- **`tpt-formal` audit-trail crate:** no crate in that repo is clearly the audit trail. Needs a choice from the maintainer.
- **`.telos` ideal-layer proof:** blocked on the `old()` soundness problem described in `THREAT_MODEL.md`.
