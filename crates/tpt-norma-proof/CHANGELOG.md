# Changelog

All notable changes to `tpt-norma-proof` are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the crate follows
[Semantic Versioning](https://semver.org/) once it is released.

## [Unreleased]

### Added

- `Certificate`, which binds a claim to its artifact by SHA-256 digest, and lists its evidence and
  framework mappings.
- `Evidence`, `Mapping` and `Status` (`Evidenced` or `Open`).
- `digest_text`, lower-case hex SHA-256 of a string.
- `Certificate::is_current_for`, which reports a certificate as stale when its source has changed.
- `Certificate::render`, plain-text output for a file or a review.
- `validator_draft`, an illustrative certificate for `validate_action`. It is not a compliance
  mapping, and its Article 15 mapping is marked `Open`.
