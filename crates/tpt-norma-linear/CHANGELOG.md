# Changelog

All notable changes to `tpt-norma-linear` are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the crate follows
[Semantic Versioning](https://semver.org/) once it is released.

## [Unreleased]

### Added

- `Linear` expressions over named variables with `i64` coefficients, with `var`, `constant_only`,
  `scale`, `add` and `sub`.
- `Relation` (`Le`, `Lt`, `Ge`, `Gt`, `Eq`) and `Constraint`, the form `expr REL 0`.
- `NamedConstraints`, a labelled group of constraints.
- `is_unsatisfiable`, an exact Fourier–Motzkin decision over big rationals.
- `check_contradictions`, which reports the groups that cannot hold together, as a
  `ContradictionReport` of `Contradiction` pairs.
- `MAX_ROWS`, the working-set limit past which the check reports `None` rather than running on.
- The `fm_bench` example.
