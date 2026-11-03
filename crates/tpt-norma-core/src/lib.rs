// Copyright (c) TPT Solutions
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Verified stateful action validator (spec §6).
//!
//! The entry point is [`validate_action`], which checks a single
//! [`ActionProposal`] against a single [`SystemState`] snapshot and a
//! [`PolicyBounds`] policy. Its guarantee is the conservative-denial
//! postcondition: an ideal-layer `Deny` is never reported as `Allow` by the
//! f64 realization.
//!
//! Scope: this crate validates one proposal against one snapshot. Atomicity
//! across concurrent calls is a caller obligation (spec §8).

mod action_validator;
mod allocator;
mod ideal;

pub use action_validator::{
    check_action, validate_action, ActionProposal, Decision, PolicyBounds, Rejection, SystemState,
};
pub use allocator::{validate_multi, DimensionRejection};
pub use ideal::{symbolic_evaluate_ideal, IdealDecision};
