# Changelog

All notable changes to `tpt-norma-audit` are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the crate follows
[Semantic Versioning](https://semver.org/) once it is released.

## [Unreleased]

### Added

- `Event` with the `Requested`, `Decided`, `Committed` and `Refused` lifecycle steps, and a
  length-prefixed deterministic encoding.
- `Entry` and `AuditLog`, a hash-chained log in which each digest covers its predecessor.
  `AuditLog::append`, `append_with` (persists through a sink before keeping the entry), `head`,
  `entries`, `len` and `is_empty`.
- `verify_chain` and `ChainError`, which recompute every digest and report the first bad entry.
- `to_hex` for displaying and anchoring digests.
- `AuditSink` trait, with `NullSink` (discards) and `FileSink` (appends and syncs each line).
- `AuditLog::to_lines` and `AuditLog::from_lines`, a line-based text format that is verified on
  load, with `LoadError`.
