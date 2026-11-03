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
*(Superseded by D19: the realization-layer work now goes into the internal crates, not `tpt-telos`. The items below remain open for the internal implementation. Upstream `tpt-telos` items are no longer blocking.)*
`tpt-telos`'s IR currently treats `Float32`/`Float64` as exact integers for verification (see `../tpt-telos/examples/float.telos`); IEEE-754 interval arithmetic and rounding-error-bound reasoning is explicitly future work there. Spec §6's realization-layer contract (machine-checked `EPSILON_ROUNDING` conservative-denial) cannot be proven by `tpt-telos` as it stands, so this upstream work is a prerequisite before `tpt-norma-core`'s realization-layer proof can be machine-checked rather than hand-argued.
- [x] Investigate current float handling in `tpt-telos-ir` / `tpt-telos-verifier` (confirm exact scope of the gap) *(Confirmed. `tpt-telos-parser` and `tpt-telos-codegen` know `Float32`/`Float64`. `tpt-telos-ir` and `tpt-telos-verifier` have no float-specific code at all. The only "interval" in the verifier is nonlinear interval bounding for products, not IEEE-754. The gap is as the note in `examples/float.telos` describes. Checked on tpt-telos at `C:\Programming\2 WIP\tpt-telos`, read-only.)*
- [x] Design an IEEE-754 interval / rounding-error-bound extension to the IR and solver (auto-derivable `EPSILON_ROUNDING`/`EPSILON_LOAD`-style bounds) *(Not an interval extension. For the capacity check the margin is a rounding-exact quantity, so the proof is a hand argument with explicit cases, in `docs/conservative-denial.md`. The earlier draft's margin step was loose and is replaced. Interval reasoning is not needed for this check.)*
- [ ] Implement the extension (in `tpt-telos-ir`/`tpt-telos-verifier`, or a new dedicated crate, e.g. `tpt-telos-float`) *(Superseded by D19. No general interval extension is built. A generic `EPSILON_LOAD` bound for `load + delta` is not needed for the capacity check, which is closed by the hand proof above.)*
- [x] Add regression examples (in `tpt-telos`, now not applicable under D19) *(Replaced by `crates/tpt-norma-core/tests/boundary.rs`: deterministic ulp sweeps around special capacities (powers of two, smallest normal, subnormals) against the exact oracle `symbolic_evaluate_ideal`.)*
- [ ] Verify the extension's own soundness (unit tests + review) before depending on it from `tpt-norma` *(Partial. The hand proof and `boundary.rs` pass. A written review of the proof is still needed, and the proof is not machine-checked.)*

## Phase 2: tpt-norma-core — Verified Stateful Action Validator (spec §6)
- [x] Scaffold crate Cargo.toml + lib.rs (`tpt_norma_core`), depending on `tpt-math` for bounded arithmetic (spec §3.5)
- [x] Define core types: `ActionProposal`, `SystemState`, `PolicyBounds`, `Decision`
- [x] Implement `validate_action` exactly per spec §6: non-negativity check, rate-limit check, conservative `epsilon_rounding` capacity check
- [x] Author a companion `.telos` twin file encoding the ideal-layer QF_LRA `requires`/`ensures` contracts from spec §6 *(Authored at `crates/tpt-norma-core/telos/action_validator.telos`; not verified, see `THREAT_MODEL.md`.)*
- [x] Ideal-layer proof: confirm capacity invariant and non-negativity hold for `admit` *(Replaced `telos verify` with exact Farkas-style certificates in `crates/tpt-norma-solver/src/implication.rs`, checked by `tests/ideal_admit.rs`. Covers `admit`'s non-strict `ensures` only. The `.telos` twin itself is still unverified by `telos`; see `THREAT_MODEL.md`.)*
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
  - [x] Restate the realization-layer postcondition for three-valued outcomes (ideal Deny ⇒ result is Deny or Indeterminate, never Allow) *(Stated and traced to the code in `docs/conservative-denial.md`, "Three-valued postcondition".)*
  - [x] Reconcile spec2's crate layout (core/eval/solver/runtime/audit) with this todo's (core/policy/runtime/proof); resolve the `tpt-norma-runtime` scope clash (state/freshness lifecycle vs FFI bridge, Phase 5) *(D2 in `docs/decisions.md`.)*
- [x] Wire new sibling dependencies `tpt-uir` (and `tpt-primitives` where types exist), `tpt-solver` (see Phase 0) *(Wired in the root `workspace.dependencies`; not yet used by any crate.)*
- [x] Define undefined types: `StateVersion`, `StateSnapshot`, `DecisionEvidence`, `Violation`, `EvaluationFailure`, `CanonicalBytes`, `ActionId`/`ActorId`/`ResourceId`
- [x] Decide the evidence-binding scheme (cryptographic signature/MAC vs strict structural binding), key ownership, and the source of truth for policy version and verifier identity *(D3 in `docs/decisions.md`: structural binding, no MAC yet.)*
- [ ] `PolicyEvaluator` trait (`tpt-norma-eval`): side-effect-free, with hard limits on CPU fuel, memory, solver work and wall-clock deadline (define units and defaults) *(Trait and deadline guard done. Memory limit done: `Guarded` refuses arguments over `DEFAULT_MAX_ARGUMENT_BYTES` (1024) with `ArgumentsTooLarge`, tests in `tpt-norma-eval/tests/limits.rs`. Fuel is not enforced. The evaluator is fixed-cost, and the solver's fuel is ignored on its LRA path (`SOLVER_FUEL` in `tpt-norma-solver`). Enforcing it needs a `tpt-solver` change. The deadline is post-hoc, see D8.)*
- [x] Timeout / panic / resource exhaustion / solver `Inconclusive` all map to `Indeterminate`, never `Allow` *(Panic, timeout, solver budget exhaustion (`ResourceExhausted`, D17) and solver `Inconclusive` all map to `Indeterminate`. Evaluator fuel and memory limits are still not enforced, see the line above.)*
- [x] Bind `DecisionEvidence` to exact `action_id`, `actor`, `resource`, `state_version` plus policy version and verifier identity
- [x] Specify the executor and its commit-time freshness check (who owns it: `tpt-writ` side vs `tpt-norma-runtime`); align with Phase 5 per-resource serialization *(D4 in `docs/decisions.md`; `StateStore::commit` in `tpt-norma-runtime`.)*
- [x] `tpt-norma-solver`: adapter to `tpt-solver` for SMT/certificate checking; document how it relates to the `tpt-telos` / FM-elimination path *(`crates/tpt-norma-solver`. Ground-row certificates; the realization bound is checked in big rationals, see D9.)*
- [x] `tpt-norma-audit` (optional): emit `tpt-concord` evidence objects for the decision lifecycle *(`crates/tpt-norma-audit`, an internal hash-chained format, D10. Not `tpt-concord` evidence.)*
- [x] Define the contract provided to `tpt-writ` (consumes `Decision` + `DecisionEvidence` to gate tool execution) *(`Gate::submit` in `tpt-norma-runtime`. `tpt-writ` itself is not checked out, so this is the internal contract. D8.)*
- [x] CI acceptance tests (spec2 §5):
  - [x] Positive: valid action + current state ⇒ `Allow` with complete evidence
  - [x] Negative, violation: ⇒ `Deny` with specific `Violation` details
  - [x] Negative, missing state: ⇒ `Indeterminate`
  - [x] Negative, stale state: pick one deterministic outcome (`Indeterminate` or `Deny`; spec2 leaves both open)
  - [x] Negative, timeout: forced fuel/time limit ⇒ `Indeterminate`
  - [x] Negative, mutation: `Allow` for action A rejected when applied to a modified action B
  - [x] Negative, state drift: executor rejects evidence when state version changes between `evaluate()` and commit
- [x] Carry spec2 §6 domain caveats into README and `THREAT_MODEL.md`: healthcare (enforces hard bounds only, not clinical judgment) and aviation (macro-state transitions only, not the 100Hz control loop) *(In `THREAT_MODEL.md`.)*
- [x] Update `THREAT_MODEL.md` for the new evidence/freshness trust assumptions *(Drafted, pending review.)*
- [x] Unit tests + rustdoc + SPDX header *(`tpt-norma-eval/tests/evaluator.rs` covers the evaluator and each guard. `tpt-norma-solver` tests `classify`. Acceptance tests are in `tpt-norma-runtime/tests/acceptance.rs`.)*

## Phase 3: Phase 1 Exit Criteria (spec §9)
- [ ] Exit Criterion 1: confirm `validate_action` ideal + realization proofs pass, including the conservative-denial postcondition (cross-ref Phase 2)
- [x] Exit Criterion 2: implement a multi-resource allocator function (synthetic 10-dimensional policy space) in `tpt-norma-core` *(`validate_multi` in `tpt-norma-core`, all-or-nothing.)*
- [x] Exit Criterion 2: define the reference-hardware spec (flagged "TBD" in spec §9) so latency targets are checkable *(Recorded for the bench run: Intel Core i7-12700, Windows 11, release build. Still not a designated reference machine.)*
- [x] Exit Criterion 2: FM-elimination scalability benchmark harness against the 10-dim allocator; median < 10ms, p99 < 50ms per call *(Harness in `crates/tpt-norma-linear/examples/fm_bench.rs`, run with `cargo run --release -p tpt-norma-linear --example fm_bench`. It times the same contradiction check `tpt-norma-policy` runs, on 10 dimensions with dense coupling rules. The check is exact over rationals. Results on the test machine: 5–40 coupling rules meet both targets (40 rules: median 4.6 ms, p99 5.4 ms). 80 rules miss the median target (13 ms). This is not a worst case. FM is exponential in general, and the harness does not search for a worst case. The closed-form `alloc_bench` still times only `validate_multi`.)*
- [x] Exit Criterion 3: `tpt-norma-proof` crate scaffold, depending on `tpt-formal` for audit-trail generation (spec §3.5/§4) *(`crates/tpt-norma-proof`.)*
- [x] Exit Criterion 3: define the Proof Certificate format *(Source-digest-pinned format in `tpt-norma-proof`.)*
- [x] Exit Criterion 3: generate a first-draft certificate for `validate_action` only, mapped to EU AI Act Article 15 objectives, explicitly labeled illustrative per the spec §7 caveat (not to be represented as a completed compliance mapping) *(`validator_draft()`, labelled illustrative.)*
- [x] Exit Criterion 4: author a temporal rate-limiter `.telos` twin (adapting `../tpt-telos/examples/rate_limiter.telos`) using the two-layer f64 methodology *(`crates/tpt-norma-core/telos/rate_limiter.telos`. Its `consume` and `refill` `ensures` are checked exactly as Farkas certificates in `crates/tpt-norma-solver/tests/rate_limiter.rs`. Not `telos verify`, and the f64 layer is not covered.)*
- [ ] Exit Criterion 4: the multi-resource allocator's proof (from Exit Criterion 2) explicitly addresses the constraint-orthogonality assumption from spec §5.2 (non-orthogonal case: an action sized as a function of a quantity another constraint also depends on) *(Partial. `crates/tpt-norma-core/tests/orthogonality.rs` shows the gap: two dimensions each pass, and the shared pool is exceeded, so `validate_multi` alone is unsound for a shared quantity. The assumption is still not checked by code, and no proof is written.)*
- [x] Exit Criterion 5: promote spec §8 (Threat Model & Assumptions) into a standalone `THREAT_MODEL.md` *(Drafted as `THREAT_MODEL.md`; sign-off still open.)*
- [ ] Exit Criterion 5: written threat-model document reviewed and signed off

## Phase 4: tpt-norma-policy — N-Dimensional Policy Space (spec §4)
- [x] Scaffold crate *(`crates/tpt-norma-policy`.)*
- [x] N-dimensional policy space definition (the "AABBs" of AI action) *(Boxes plus linear rules, in the DSL.)*
- [x] DSL compilation *(Text DSL to a checked `PolicySpace`. No codegen.)*
- [x] Prove policy definitions are internally consistent / non-contradictory, reusing `tpt-telos-sdk`'s `check_contradictions`/`NamedConstraints` entry point (no `.telos` source required) *(Now uses the internal `tpt-norma-linear::check_contradictions` after the telos replacement, D19. Integer units only, D14.)*
- [x] Unit tests + rustdoc + SPDX header

## Phase 5: tpt-norma-runtime — FFI Enforcement Bridge (spec §4, §8)
- [ ] Scaffold crate (Rust/Go FFI bridge); reconcile scope with spec2's `tpt-norma-runtime` (state validation / freshness lifecycle, Phase 2b) — split into separate crates if needed *(Rust C ABI in `crates/tpt-norma-ffi`, built as staticlib and cdylib. Go binding in `bindings/go/tpnorma`, Windows only, loads the DLL with `syscall`. No C compiler is installed here, so the C host compile test is not built.)*
- [ ] Intercept LLM tool calls at runtime and enforce `tpt-norma-core` *(In-process `Gate` and the FFI `admit` call. A host that skips them is not covered, per D8 and the FFI docs.)*
- [ ] Prove the FFI boundary cannot be bypassed or corrupted by the host environment (closes threat-model assumption 5) *(Not provable as stated. The limits are recorded in `THREAT_MODEL.md`.)*
- [x] Implement per-resource serialized validation-and-commit (mutex/actor/single-writer queue), closing threat-model assumption 2 so atomicity holds in production, not just as a caller obligation *(`SharedGate`, in-process only, D15. Concurrency test passes.)*
- [ ] Unit tests + Rust↔Go integration tests + rustdoc + SPDX header *(Rust unit tests done. Go integration tests in `bindings/go/tpnorma`, six tests, pass against the release DLL. C host tests not built.)*

## Phase 6: Compliance Roadmap Phases 2–4 (spec §7)
- [ ] Phase 2: map Proof Certificates to EU AI Act Article 15 (Accuracy & Robustness) and SOC2 CC6.1 (Logical Access Security)
- [ ] Phase 3: commission an independent third-party audit of the Conservative Denial f64 rounding argument
- [ ] Phase 4: build a reference submission package against the Phase 1 action validator, as a template for enterprise customers' AI governance submissions

## Pilot Blockers (found while scoping a pilot)
- [x] Make the guarded composition the default *(`Gate::new(policy, store, deadline)` builds `CertifiedEvaluator<Guarded<CoreEvaluator>>`, and it is the only constructor. D18.)*
- [x] Shadow mode for the pilot: the host calls `Gate::admit` and does not run the tool, and the shadow budget still counts *(`crates/tpt-norma-runtime/tests/shadow.rs`, and `examples/pilot_shadow.rs` in the runtime crate. D18.)*
- [ ] Persist the audit log and anchor its head digest outside the process. *(Partly done: the gate writes each entry through an `AuditSink` before keeping it (`AuditLog::append_with`, `FileSink`, `Gate::with_sink`). It fails closed: if the `Requested` or `Decided` entry cannot be persisted, nothing runs, and if `Committed` cannot be persisted, the tool does not run but the state has already changed (documented, see `gate.rs`). Tests in `crates/tpt-norma-runtime/tests/audit_sink.rs`. Not done: an external anchor for the head digest. The chain does not stop someone who rewrites the whole file, so the head still needs anchoring.)*
- [x] Pick a pilot resource and dimension, and write its policy in the DSL. *(Chosen: one agent's total tool-call budget, 100 calls, unit cost, no time window. Policy in `examples/pilot/quota.policy`, parse and consistency test in `crates/tpt-norma-policy/tests/pilot.rs`. Capacity is 101 because the conservative margin admits 100 of 101 (D16).)*
- [x] Settle the host language: Rust, in-process. The shadow host is `crates/tpt-norma-runtime/examples/pilot_shadow.rs`. Go and other hosts still need Phase 5.

## Ongoing / Cross-Cutting
- [ ] Maintain cargo-deny license checks passing on every phase
- [ ] Keep CI green (fmt, clippy, test, deny) after each crate lands
- [ ] Never cite "no false allows" externally without its accompanying `THREAT_MODEL.md` scope (spec §9 Exit Criterion 5)
- [ ] Keep a spec.txt-style revision history (per §6.1's pattern) as contracts evolve
