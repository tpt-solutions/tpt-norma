# Changelog

All notable changes to `tpt-norma-runtime` are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the crate follows
[Semantic Versioning](https://semver.org/) once it is released.

## [Unreleased]

### Added

- `authorize`, which refuses a request made against a stale state version as `Indeterminate`
  without calling the evaluator.
- `StateStore`, the committed state for one resource, with `new`, `version`, `load`, `snapshot`
  and `commit`.
- `CommitError`, the reasons a commit is refused: `NotAllowed`, `MutatedRequest` and `StateDrift`.
- `Gate`, the tool gate, with `new`, `with_sink`, `submit`, `admit`, `store` and `log`. Requests
  are recorded, authorized, committed and then run, in that order.
- `Gate::admit` for shadow mode: it commits without running the tool.
- `GuardedEvaluator`, the evaluator stack built by `Gate::new`.
- `Refusal`, the reasons a request does not reach its tool: `Denied`, `Indeterminate`, `Commit` and
  `Audit`.
- `SharedGate`, one gate behind a mutex, with validation and commit serialized per resource within
  one process.
- The `pilot_shadow` example, using the numbers in `examples/pilot/quota.policy`.
- Acceptance, gate, shared, shadow and audit-sink tests.
