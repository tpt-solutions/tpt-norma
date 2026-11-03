# Changelog

All notable changes to `tpt-norma-solver` are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the crate follows
[Semantic Versioning](https://semver.org/) once it is released.

## [Unreleased]

### Added

- `certify`, which checks a claimed `Allow` or `Deny` through `tpt-solver`, and returns a
  `Certification`.
- `Certification` with `Verified`, `CertifiedDenial`, `ConservativeDenial`, `Contradicted`,
  `Exhausted` and `Inconclusive`.
- The realization error bound for a claimed `Allow`, checked with exact big rationals.
- `CertifiedEvaluator`, which wraps any `PolicyEvaluator` and passes only verified `Allow`
  decisions through.
- `implication` module: `Row`, `int` and `check_implication`, an exact Farkas-witness checker for
  non-strict linear implications.
- Certification, ideal-layer, rate-limiter and property tests.
