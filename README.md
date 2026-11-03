# tpt-norma

[![CI](https://github.com/tpt-solutions/tpt-norma/actions/workflows/ci.yml/badge.svg)](https://github.com/tpt-solutions/tpt-norma/actions/workflows/ci.yml)
[![License headers](https://github.com/tpt-solutions/tpt-norma/actions/workflows/license.yml/badge.svg)](https://github.com/tpt-solutions/tpt-norma/actions/workflows/license.yml)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)

Dual-licensed **MIT OR Apache-2.0** · TPT Solutions

A pure-Rust runtime policy engine for bounding AI-agent actions. `tpt-norma`
treats each proposed action as a state-transition problem and validates it
against resource, rate and capacity policies before anything is executed.

- Specification: [spec.txt](spec.txt) (the formal contract, §6 for the validator)
- Runtime policy extension: [spec2.txt](spec2.txt) (three-valued outcomes, fail-closed state, evidence binding)
- Threat model and scope: [THREAT_MODEL.md](THREAT_MODEL.md) (draft, pending review)
- Design decisions: [docs/decisions.md](docs/decisions.md)

## Architecture

`tpt-norma` sits between an LLM tool call and the system it would act on. The
LLM is treated as untrusted; every proposal is denied unless it is proven safe.

```text
tpt-math          bounded / exact arithmetic
tpt-telos         verification of the ideal-layer contracts
tpt-solver        SMT / certificate checking        (planned adapter)
tpt-uir           shared intermediate representation (planned)
        ↑
tpt-norma-core    validate_action: non-negativity, rate limit, conservative capacity check
```

### Crates

| Crate | Status | Purpose |
| :--- | :--- | :--- |
| `tpt-norma-core` | Implemented | `validate_action`, the exact-rational reference `symbolic_evaluate_ideal`, and the conservative-denial f64 check |
| `tpt-norma-eval` | Implemented | Three-valued `Decision` (`Allow` / `Deny` / `Indeterminate`), evidence binding, and fail-closed guards |
| `tpt-norma-runtime` | Implemented | State-version freshness (`authorize`) and the commit-time executor (`StateStore::commit`) |
| `tpt-norma-solver` | Planned | Adapter to `tpt-solver` for SMT and certificate checking |
| `tpt-norma-audit` | Planned | Emits `tpt-concord` evidence objects for the decision lifecycle |
| `tpt-norma-policy`, `tpt-norma-proof` | Planned | Policy DSL and proof certificates (spec §4) |

### Scope

- `validate_action` checks one proposal against one state snapshot. It does not
  make concurrent calls atomic; callers must serialize validation and commit per
  resource (spec §8).
- Only the proposal is treated as adversarial. State and policy are assumed to
  come from a trusted layer.
- Every guarantee is stated under these assumptions. Do not cite "no false
  allows" externally without this scope.
- The `.telos` ideal-layer twin is written but not verified. See
  [THREAT_MODEL.md](THREAT_MODEL.md) for the verifier issue.

### Domain caveats

- **Healthcare:** enforces hard numeric bounds only. Clinical judgment, data
  quality and human oversight stay with the application layer.
- **Aviation:** used for mission planning and macro-state transitions. Not for
  hard real-time control loops (for example 100 Hz).

## Quick start

```sh
cargo test --workspace
```

The property tests in `tpt-norma-core` run 4096 cases each and take about a
second.

The workspace depends on sibling checkouts by relative path, so they must sit
next to this repository:

```text
../tpt-telos    ../tpt-math    ../tpt-solver    ../tpt-uir
```

## Development

- `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`
  and `cargo test --workspace` must pass.
- `cargo deny check` enforces the license and dependency policy (`deny.toml`).
- The `unsafe_code = "forbid"` and `clippy::all` lints are set at the workspace level.

### Continuous integration

- [`ci.yml`](.github/workflows/ci.yml): format, clippy, tests, and cargo-deny.
- [`license.yml`](.github/workflows/license.yml): every `.rs` file carries the TPT copyright and SPDX header.

### Source file headers

Every `.rs` file starts with:

```rust
// Copyright (c) TPT Solutions
// SPDX-License-Identifier: MIT OR Apache-2.0
```

## License

Licensed under either of

- MIT license ([LICENSE-MIT](LICENSE-MIT))
- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in this repository shall be dual licensed as MIT OR Apache-2.0,
without any additional terms or conditions.
