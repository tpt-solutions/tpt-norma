# Changelog

All notable changes to `tpt-norma-eval` are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the crate follows
[Semantic Versioning](https://semver.org/) once it is released.

## [Unreleased]

### Added

- `Decision` with three outcomes: `Allow { evidence }`, `Deny { violations }` and
  `Indeterminate { reason }`, and `Decision::is_allow`.
- `PolicyEvaluator` trait, the contract every evaluator implements.
- `CoreEvaluator`, the reference evaluator. It delegates to `tpt-norma-core` and records the policy
  version and verifier identity as provenance.
- `Guarded`, fail-closed wrapper: a panic becomes `Indeterminate`, a call past its deadline becomes
  `Indeterminate`, and an `Allow` that does not bind its request becomes `Indeterminate`.
- `DEFAULT_MAX_ARGUMENT_BYTES` and `Guarded::with_max_argument_bytes`, a limit on encoded arguments.
- Identifier types `StateVersion`, `ActionId`, `ActorId` and `ResourceId`.
- `CanonicalBytes`, the bit-exact argument encoding for a resource delta.
- `StateSnapshot`, `ActionRequest` and `DecisionEvidence` with `binds`.
- `Violation` and `EvaluationFailure`, the reasons for `Deny` and `Indeterminate`.
