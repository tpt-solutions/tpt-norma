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

## D8. Tool runs after commit

**Decision:** `Gate::submit` authorizes, commits, and only then runs the tool. A denied request never reaches the tool.

**Limit:** if the tool fails after the commit, the state change is not rolled back. Tools must be idempotent, or the caller must compensate. A rollback protocol is deferred.

## D9. Solver checks ground rows; realization bound is checked in big rationals

**Decision:** `tpt-norma-solver` sends the solver only the ideal-layer conditions on the exact inputs, as ground rows. The realization error bound is checked with `tpt-math-exact` big rationals.

**Why:** with the bound in the solver, its `i128` rationals overflowed during cross-multiplication, even on ordinary inputs. Fourier–Motzkin on a five-variable system also hung. The bound is still exact. Only the ideal-layer conditions are kernel-checked.

## D10. Internal evidence formats in place of external crates

**Decision:** `tpt-norma-audit` (hash chain) and `tpt-norma-proof` (certificate) are internal. They stand in for `tpt-concord` and `tpt-formal`, which are not in use here.

**Why:** the todo allowed building internally when external pieces were not available. The formats are not signed, so they detect accidental and unsophisticated edits only. An external anchor for the head digest is needed for stronger guarantees.

## D11. Allocator is closed-form, and its timing is not the FM target

**Decision:** `validate_multi` checks each dimension with `check_action`, all-or-nothing. Its benchmark measures this closed-form check, not Fourier–Motzkin composition.

**Why:** the spec's 10 ms / 50 ms targets concern FM tractability. Reporting the closed-form timings as meeting them would be misleading. The 100 ns median is at the resolution of Windows `Instant`.

## D12. `wildcards` lint is a warning

**Decision:** `deny.toml` sets `wildcards = "warn"`. The sibling `tpt-solver` crates are publishable but depend on each other by unversioned path, which cargo-deny reports as wildcards.

**Revisit when:** the sibling manifests get versions or `publish = false`.

## D13. FFI crate opts out of the workspace lints

**Decision:** `tpt-norma-ffi` does not inherit `workspace.lints`. It sets `unsafe_code = "deny"`, with one `#![allow(unsafe_code)]` at the crate root. Every other crate keeps `forbid`.

**Why:** a `forbid` cannot be relaxed by a crate that inherits it. A C ABI needs `unsafe`, and this is the only place that needs it.

**Guard:** every exported function catches panics. Every `unsafe` block has a stated safety condition.

## D14. Policy bounds are integers

**Decision:** the policy DSL takes integer bounds and coefficients.

**Why:** `tpt-telos-ir` works over `i64`. A fixed-point scale (for example micro-units) is the caller's job.

## D15. Shared gate holds the lock across admit, not across the tool

**Decision:** `SharedGate::submit` holds its mutex through authorize and commit, and releases it before the tool runs. A poisoned lock refuses the request.

**Limit:** this serializes one resource inside one process. Two processes, or two gates for one resource, are not covered.

## D16. Capacity tests use 10.5 when the test needs exactly 10 admissions

**Decision:** with capacity 10 and unit steps, the conservative margin admits only 9. The concurrency test therefore uses capacity 10.5.

**Why:** this is the margin working as designed (an exact match is a deliberate false denial), not a bug.

## Deferred

- **`tpt-concord` evidence:** not checked out. `tpt-norma-audit` uses an internal format (D10) until it is.
- **FM-based policy composition (spec §5.2) and its tractability benchmark:** not built (D11).
- **Phase 5, Go bridge and C-host compile test:** the Rust C ABI exists. A Go or C host is not built.
- **Phase 5, FFI boundary proof:** not provable as stated. `THREAT_MODEL.md` records what the bridge does and does not enforce.
- **`tpt-formal` audit-trail crate:** no crate in that repo is clearly the audit trail. Needs a choice from the maintainer.
- **`.telos` ideal-layer proof:** blocked on the `old()` soundness problem described in `THREAT_MODEL.md`.
