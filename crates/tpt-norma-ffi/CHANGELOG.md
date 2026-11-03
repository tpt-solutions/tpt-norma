# Changelog

All notable changes to `tpt-norma-ffi` are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the crate follows
[Semantic Versioning](https://semver.org/) once it is released.

## [Unreleased]

### Added

- C ABI over the validator: `tn_validate_action` and `tn_validate_action_bits`.
- C ABI over the shared gate: `tn_gate_new`, `tn_gate_new_bits`, `tn_gate_admit`,
  `tn_gate_admit_bits`, `tn_gate_state` and `tn_gate_free`.
- Return codes `TN_ALLOW`, `TN_DENY`, `TN_INDETERMINATE` and `TN_ERROR`.
- Panic catching on every exported function, so no unwind crosses the C boundary.
- Hand-written header `include/tpt_norma.h`.
- `staticlib`, `cdylib` and `rlib` crate types. The `cdylib` is loaded by the Go binding.
