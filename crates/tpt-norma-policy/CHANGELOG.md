# Changelog

All notable changes to `tpt-norma-policy` are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the crate follows
[Semantic Versioning](https://semver.org/) once it is released.

## [Unreleased]

### Added

- A line-based policy DSL with `dim NAME in [MIN, MAX]` and `rule NAME: LHS OP RHS`, where `OP` is
  one of `<=`, `<`, `>=`, `>` or `==`.
- `PolicySpace::parse`, which returns the first `PolicyError` with its line number.
- `PolicySpace::validate_names`, which rejects undeclared and duplicate dimension names.
- `PolicySpace::admits`, the integer membership test for a point.
- `PolicySpace::check_consistency`, which reports through `tpt-norma-linear` whether the policy is
  satisfiable, and which pairs of rules or dimension bounds contradict each other.
- The types `Dimension`, `Rule`, `Op`, `Consistency` and `PolicyError`.
- Pilot policy test against `examples/pilot/quota.policy`.
