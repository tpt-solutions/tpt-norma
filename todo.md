# tpt-norma — Project Todo

Dual-licensed MIT OR Apache-2.0 · TPT Solutions

## Phase 0: Repository Scaffolding
- [ ] Clone `tpt-math` and `tpt-formal` as sibling checkouts (`../tpt-math`, `../tpt-formal`), alongside the existing `../tpt-telos`
- [ ] Root Cargo.toml (workspace, resolver = "2", workspace.package, workspace.dependencies)
- [ ] Wire sibling path dependencies (`tpt-telos-sdk`, `tpt-telos-parser` from `../tpt-telos/crates/...`; bounded-arithmetic crates from `../tpt-math/crates/...`; audit-trail crate(s) from `../tpt-formal/...`), with a rationale comment matching `tpt-nexus`'s convention (path deps because these are pre-1.0 sibling working trees, not published crates.io releases)
- [ ] `workspace.lints.rust.unsafe_code = "forbid"`, `workspace.lints.clippy.all = "warn"`
- [ ] LICENSE-MIT
- [ ] LICENSE-APACHE
- [ ] README.md (architecture overview + license section, matching `tpt-construction/README.md`'s structure)
- [ ] deny.toml (license enforcement; MIT-selectable dependency chain policy)
- [ ] rustfmt.toml
- [ ] clippy.toml
- [ ] .github/workflows/ci.yml (fmt, clippy, test, deny)
- [ ] .github/workflows/license.yml
- [ ] crates/, examples/, test-data/ directory scaffolding
- [ ] Source file header template (`// Copyright (c) TPT Solutions` / `// SPDX-License-Identifier: MIT OR Apache-2.0`)

## Phase 1: tpt-telos Prerequisite — IEEE-754 Rounding-Bound Verification
`tpt-telos`'s IR currently treats `Float32`/`Float64` as exact integers for verification (see `../tpt-telos/examples/float.telos`); IEEE-754 interval arithmetic and rounding-error-bound reasoning is explicitly future work there. Spec §6's realization-layer contract (machine-checked `EPSILON_ROUNDING` conservative-denial) cannot be proven by `tpt-telos` as it stands, so this upstream work is a prerequisite before `tpt-norma-core`'s realization-layer proof can be machine-checked rather than hand-argued.
- [ ] Investigate current float handling in `tpt-telos-ir` / `tpt-telos-verifier` (confirm exact scope of the gap)
- [ ] Design an IEEE-754 interval / rounding-error-bound extension to the IR and solver (auto-derivable `EPSILON_ROUNDING`/`EPSILON_LOAD`-style bounds)
- [ ] Implement the extension (in `tpt-telos-ir`/`tpt-telos-verifier`, or a new dedicated crate, e.g. `tpt-telos-float`)
- [ ] Add regression examples in `tpt-telos` (extend `examples/float.telos`, add a rounding-bound-specific fixture)
- [ ] Verify the extension's own soundness (unit tests + review) before depending on it from `tpt-norma`

## Phase 2: tpt-norma-core — Verified Stateful Action Validator (spec §6)
- [ ] Scaffold crate Cargo.toml + lib.rs (`tpt_norma_core`), depending on `tpt-math` for bounded arithmetic (spec §3.5)
- [ ] Define core types: `ActionProposal`, `SystemState`, `PolicyBounds`, `Decision`
- [ ] Implement `validate_action` exactly per spec §6: non-negativity check, rate-limit check, conservative `epsilon_rounding` capacity check
- [ ] Author a companion `.telos` twin file encoding the ideal-layer QF_LRA `requires`/`ensures` contracts from spec §6
- [ ] Ideal-layer proof: `telos verify` the twin; confirm capacity invariant and rate-limit/non-negativity invariant hold on Allow
- [ ] Implement `symbolic_evaluate_ideal`, the exact-rational reference evaluator the realization-layer contracts refer to
- [ ] Realization-layer proof (depends on Phase 1): encode `EPSILON_LOAD` (numerical-error bound on `new_load`) and `EPSILON_ROUNDING` (capacity safety margin) as independent, machine-checked bounds; prove the "no false allows" postcondition (`ideal Deny ==> result Deny`)
- [ ] Until Phase 1 lands: interim written proof doc for conservative denial + exhaustive property tests (proptest) fuzzing state/proposal/policy across f64 boundary conditions, cross-checked against `symbolic_evaluate_ideal`
- [ ] Unit tests: exact capacity match, epsilon-sized overshoot, negative delta, delta exceeding `max_delta_per_call`, zero policy bounds
- [ ] Doc comments preserving spec §6.1's scope notes: NaN/finiteness is an upstream (schema-parser) obligation; single-call/non-atomicity scope (see Phase 5); `EPSILON_LOAD` vs `EPSILON_ROUNDING` are independently derived, not interchangeable
- [ ] Rustdoc + SPDX header

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
- [ ] Exit Criterion 5: promote spec §8 (Threat Model & Assumptions) into a standalone `THREAT_MODEL.md`
- [ ] Exit Criterion 5: written threat-model document reviewed and signed off

## Phase 4: tpt-norma-policy — N-Dimensional Policy Space (spec §4)
- [ ] Scaffold crate
- [ ] N-dimensional policy space definition (the "AABBs" of AI action)
- [ ] DSL compilation
- [ ] Prove policy definitions are internally consistent / non-contradictory, reusing `tpt-telos-sdk`'s `check_contradictions`/`NamedConstraints` entry point (no `.telos` source required)
- [ ] Unit tests + rustdoc + SPDX header

## Phase 5: tpt-norma-runtime — FFI Enforcement Bridge (spec §4, §8)
- [ ] Scaffold crate (Rust/Go FFI bridge)
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
