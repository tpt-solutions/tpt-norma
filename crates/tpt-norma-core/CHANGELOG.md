# Changelog

All notable changes to `tpt-norma-core` are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the crate follows
[Semantic Versioning](https://semver.org/) once it is released.

## [Unreleased]

### Added

- `validate_action` and `check_action`, the single-proposal validator. `check_action` returns the
  reason for a denial, so the reason and the decision cannot diverge.
- `ActionProposal`, `SystemState`, `PolicyBounds`, `Decision` and `Rejection`.
- Fail-closed handling of non-finite inputs: they are denied, never allowed.
- Conservative-denial margin: the f64 check never allows what the exact ideal would deny.
- `symbolic_evaluate_ideal` and `IdealDecision`, the exact-rational reference oracle.
- `validate_multi` and `DimensionRejection`, an all-or-nothing check across independent resource
  dimensions.
- Boundary, cross-check and orthogonality tests, and the `alloc_bench` example.
- `telos/` contract files for the action validator and the rate limiter. They are not verified.
