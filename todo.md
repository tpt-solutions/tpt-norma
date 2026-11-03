# tpt-norma — Project Todo

Dual-licensed MIT OR Apache-2.0 · TPT Solutions

## Phase 0: Repository Scaffolding
- [x] Clone `tpt-math` and `tpt-formal` as sibling checkouts (`../tpt-math`, `../tpt-formal`), alongside the existing `../tpt-telos`
- [x] Clone `tpt-uir` and `tpt-solver` as sibling checkouts (required by spec2.txt; see Phase 2b)
- [x] Root Cargo.toml (workspace, resolver = "2", workspace.package, workspace.dependencies)
- [x] Wire sibling path dependencies (`tpt-telos-sdk`, `tpt-telos-parser` from `../tpt-telos/crates/...`; bounded-arithmetic crates from `../tpt-math/crates/...`; audit-trail crate(s) from `../tpt-formal/...`), with a rationale comment matching `tpt-nexus`'s convention (path deps because these are pre-1.0 sibling working trees, not published crates.io releases) *(Partial: `tpt-math-exact`, `tpt-solver`, `tpt-uir` are wired; no `tpt-formal` audit-trail crate is clearly the right one yet, so it is not wired.)*
- [x] `workspace.lints.rust.unsafe_code = "forbid"`, `workspace.lints.clippy.all = "warn"`
- [x] LICENSE-MIT
- [x] LICENSE-APACHE
- [x] README.md (architecture overview + license section, matching `tpt-construction/README.md`'s structure)
- [x] deny.toml (license enforcement; MIT-selectable dependency chain policy)
- [x] rustfmt.toml
- [x] clippy.toml
- [x] .github/workflows/ci.yml (fmt, clippy, test, deny)
- [x] .github/workflows/license.yml
- [x] crates/, examples/, test-data/ directory scaffolding
- [x] Source file header template (`// Copyright (c) TPT Solutions` / `// SPDX-License-Identifier: MIT OR Apache-2.0`)

## Phase 1: tpt-telos Prerequisite — IEEE-754 Rounding-Bound Verification
`tpt-telos`'s IR currently treats `Float32`/`Float64` as exact integers for verification (see `../tpt-telos/examples/float.telos`); IEEE-754 interval arithmetic and rounding-error-bound reasoning is explicitly future work there. Spec §6's realization-layer contract (machine-checked `EPSILON_ROUNDING` conservative-denial) cannot be proven by `tpt-telos` as it stands, so this upstream work is a prerequisite before `tpt-norma-core`'s realization-layer proof can be machine-checked rather than hand-argued.
- [ ] Investigate current float handling in `tpt-telos-ir` / `tpt-telos-verifier` (confirm exact scope of the gap)
- [ ] Design an IEEE-754 interval / rounding-error-bound extension to the IR and solver (auto-derivable `EPSILON_ROUNDING`/`EPSILON_LOAD`-style bounds)
- [ ] Implement the extension (in `tpt-telos-ir`/`tpt-telos-verifier`, or a new dedicated crate, e.g. `tpt-telos-float`)
- [ ] Add regression examples in `tpt-telos` (extend `examples/float.telos`, add a rounding-bound-specific fixture)
- [ ] Verify the extension's own soundness (unit tests + review) before depending on it from `tpt-norma`

## Phase 2: tpt-norma-core — Verified Stateful Action Validator (spec §6)
- [x] Scaffold crate Cargo.toml + lib.rs (`tpt_norma_core`), depending on `tpt-math` for bounded arithmetic (spec §3.5)
- [x] Define core types: `ActionProposal`, `SystemState`, `PolicyBounds`, `Decision`
- [x] Implement `validate_action` exactly per spec §6: non-negativity check, rate-limit check, conservative `epsilon_rounding` capacity check
- [x] Author a companion `.telos` twin file encoding the ideal-layer QF_LRA `requires`/`ensures` contracts from spec §6 *(Authored at `crates/tpt-norma-core/telos/action_validator.telos`; not verified, see `THREAT_MODEL.md`.)*
- [ ] Ideal-layer proof: `telos verify` the twin; confirm capacity invariant and rate-limit/non-negativity invariant hold on Allow *(Blocked: the `telos` build here does not check `old()` equality postconditions soundly; see `THREAT_MODEL.md`.)*
- [x] Implement `symbolic_evaluate_ideal`, the exact-rational reference evaluator the realization-layer contracts refer to
- [ ] Realization-layer proof (depends on Phase 1): encode `EPSILON_LOAD` (numerical-error bound on `new_load`) and `EPSILON_ROUNDING` (capacity safety margin) as independent, machine-checked bounds; prove the "no false allows" postcondition (`ideal Deny ==> result Deny`)
- [x] Until Phase 1 lands: interim written proof doc for conservative denial + exhaustive property tests (proptest) fuzzing state/proposal/policy across f64 boundary conditions, cross-checked against `symbolic_evaluate_ideal` *(Hand argument in `docs/conservative-denial.md`; 4096-case property tests in `crates/tpt-norma-core/tests/cross_check.rs`, sampled rather than exhaustive.)*
- [x] Unit tests: exact capacity match, epsilon-sized overshoot, negative delta, delta exceeding `max_delta_per_call`, zero policy bounds
- [x] Doc comments preserving spec §6.1's scope notes: NaN/finiteness is an upstream (schema-parser) obligation; single-call/non-atomicity scope (see Phase 5); `EPSILON_LOAD` vs `EPSILON_ROUNDING` are independently derived, not interchangeable
- [x] Rustdoc + SPDX header

## Phase 2b: Runtime Policy Engine & Action Bounds (spec2.txt — extends spec §6)
spec2 extends, not replaces, the spec §6 validator: `validate_action` and its proofs stay as the bounded-arithmetic core check; spec2 adds three-valued outcomes, fail-closed state handling, and evidence binding around it.
- [ ] Reconcile spec2 with spec §6 and record it as a spec.txt revision-history entry:
  - [x] Resolve the `Decision` name clash: spec §6 `Allow { new_load }` / `Deny` vs spec2 `Allow { evidence }` / `Deny { violations }` / `Indeterminate { reason }` (e.g. distinct type in `tpt-norma-eval` wrapping the core result) *(D1 in `docs/decisions.md`.)*
  - [x] Map `ActionRequest` / `StateSnapshot` onto `ActionProposal` / `SystemState` / `PolicyBounds`
  - [ ] Restate the realization-layer postcondition for three-valued outcomes (ideal Deny ⇒ result is Deny or Indeterminate, never Allow)
  - [x] Reconcile spec2's crate layout (core/eval/solver/runtime/audit) with this todo's (core/policy/runtime/proof); resolve the `tpt-norma-runtime` scope clash (state/freshness lifecycle vs FFI bridge, Phase 5) *(D2 in `docs/decisions.md`.)*
- [x] Wire new sibling dependencies `tpt-uir` (and `tpt-primitives` where types exist), `tpt-solver` (see Phase 0) *(Wired in the root `workspace.dependencies`; not yet used by any crate.)*
- [x] Define undefined types: `StateVersion`, `StateSnapshot`, `DecisionEvidence`, `Violation`, `EvaluationFailure`, `CanonicalBytes`, `ActionId`/`ActorId`/`ResourceId`
- [x] Decide the evidence-binding scheme (cryptographic signature/MAC vs strict structural binding), key ownership, and the source of truth for policy version and verifier identity *(D3 in `docs/decisions.md`: structural binding, no MAC yet.)*
- [ ] `PolicyEvaluator` trait (`tpt-norma-eval`): side-effect-free, with hard limits on CPU fuel, memory, solver work and wall-clock deadline (define units and defaults) *(Trait and deadline guard done in `tpt-norma-eval`; fuel and memory limits not implemented.)*
- [ ] Timeout / panic / resource exhaustion / solver `Inconclusive` all map to `Indeterminate`, never `Allow` *(Panic and timeout map to `Indeterminate`; resource exhaustion and solver `Inconclusive` not yet.)*
- [x] Bind `DecisionEvidence` to exact `action_id`, `actor`, `resource`, `state_version` plus policy version and verifier identity
- [x] Specify the executor and its commit-time freshness check (who owns it: `tpt-writ` side vs `tpt-norma-runtime`); align with Phase 5 per-resource serialization *(D4 in `docs/decisions.md`; `StateStore::commit` in `tpt-norma-runtime`.)*
- [ ] `tpt-norma-solver`: adapter to `tpt-solver` for SMT/certificate checking; document how it relates to the `tpt-telos` / FM-elimination path
- [ ] `tpt-norma-audit` (optional): emit `tpt-concord` evidence objects for the decision lifecycle
- [ ] Define the contract provided to `tpt-writ` (consumes `Decision` + `DecisionEvidence` to gate tool execution)
- [ ] CI acceptance tests (spec2 §5):
  - [x] Positive: valid action + current state ⇒ `Allow` with complete evidence
  - [x] Negative, violation: ⇒ `Deny` with specific `Violation` details
  - [x] Negative, missing state: ⇒ `Indeterminate`
  - [x] Negative, stale state: pick one deterministic outcome (`Indeterminate` or `Deny`; spec2 leaves both open)
  - [x] Negative, timeout: forced fuel/time limit ⇒ `Indeterminate`
  - [x] Negative, mutation: `Allow` for action A rejected when applied to a modified action B
  - [x] Negative, state drift: executor rejects evidence when state version changes between `evaluate()` and commit
- [x] Carry spec2 §6 domain caveats into README and `THREAT_MODEL.md`: healthcare (enforces hard bounds only, not clinical judgment) and aviation (macro-state transitions only, not the 100Hz control loop) *(In `THREAT_MODEL.md`.)*
- [x] Update `THREAT_MODEL.md` for the new evidence/freshness trust assumptions *(Drafted, pending review.)*
- [ ] Unit tests + rustdoc + SPDX header

## Phase 3: Phase 1 Exit Criteria (spec §9)
- [ ] Exit Criterion 1: confirm `validate_action` ideal + realization proofs pass, including the conservative-denial postcondition (cross-ref Phase 2)
- [ ] Exit Criterion 2: implement a multi-resource allocator function (synthetic 10-dimensional policy space) in `tpt-norma-core`
- [ ] Exit Criterion 2: define the reference-hardware spec (flagged "TBD" in spec §9) so latency targets are checkable
- [ ] Exit Criterion 2: FM-elimination scalability benchmark harness against the 10-dim allocator; median < 10ms, p99 < 50ms per call
- [ ] Exit Criterion 3: `tpt-norma-proof` crate scaffold, depending on `tpt-formal` for audit-trail generation (spec §3.5/§4)
- [ ] Exit Criterion 3: define the Proof Certificate format
- [ ] Exit Criterion 3: generate a first-draft certificate for `validate_action` only, mapped to EU AI Act Article 15 objectives, explicitly labeled illustrative per the spec §7 caveat (not to be represented as a completed compliance mapping)
- [ ] Exit Criterion 4: author a temporal rate-limiter `.telos` twin (adapting `../tpt-telos/examples/rate_limiter.telos`) using the two-layer f64 methodology
- [ ] Exit Criterion 4: the multi-resource allocator's proof (from Exit Criterion 2) explicitly addresses the constraint-orthogonality assumption from spec §5.2 (non-orthogonal case: an action sized as a function of a quantity another constraint also depends on)
- [x] Exit Criterion 5: promote spec §8 (Threat Model & Assumptions) into a standalone `THREAT_MODEL.md` *(Drafted as `THREAT_MODEL.md`; sign-off still open.)*
- [ ] Exit Criterion 5: written threat-model document reviewed and signed off

## Phase 4: tpt-norma-policy — N-Dimensional Policy Space (spec §4)
- [ ] Scaffold crate
- [ ] N-dimensional policy space definition (the "AABBs" of AI action)
- [ ] DSL compilation
- [ ] Prove policy definitions are internally consistent / non-contradictory, reusing `tpt-telos-sdk`'s `check_contradictions`/`NamedConstraints` entry point (no `.telos` source required)
- [ ] Unit tests + rustdoc + SPDX header

## Phase 5: tpt-norma-runtime — FFI Enforcement Bridge (spec §4, §8)
- [ ] Scaffold crate (Rust/Go FFI bridge); reconcile scope with spec2's `tpt-norma-runtime` (state validation / freshness lifecycle, Phase 2b) — split into separate crates if needed
- [ ] Intercept LLM tool calls at runtime and enforce `tpt-norma-core`
- [ ] Prove the FFI boundary cannot be bypassed or corrupted by the host environment (closes threat-model assumption 5)
- [ ] Implement per-resource serialized validation-and-commit (mutex/actor/single-writer queue), closing threat-model assumption 2 so atomicity holds in production, not just as a caller obligation
- [ ] Unit tests + Rust↔Go integration tests + rustdoc + SPDX header

## Phase 6: Compliance Roadmap Phases 2–4 (spec §7)
- [ ] Phase 2: map Proof Certificates to EU AI Act Article 15 (Accuracy & Robustness) and SOC2 CC6.1 (Logical Access Security)
- [ ] Phase 3: commission an independent third-party audit of the Conservative Denial f64 rounding argument
- [ ] Phase 4: build a reference submission package against the Phase 1 action validator, as a template for enterprise customers' AI governance submissions

## Ongoing / Cross-Cutting
- [ ] Maintain cargo-deny license checks passing on every phase
- [ ] Keep CI green (fmt, clippy, test, deny) after each crate lands
- [ ] Never cite "no false allows" externally without its accompanying `THREAT_MODEL.md` scope (spec §9 Exit Criterion 5)
- [ ] Keep a spec.txt-style revision history (per §6.1's pattern) as contracts evolve
