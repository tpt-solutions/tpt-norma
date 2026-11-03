// Copyright (c) TPT Solutions
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Three-valued policy evaluation for `tpt-norma` (spec2 §3-4).
//!
//! [`Decision`] is a separate type from `tpt-norma-core`'s two-valued result. It wraps the
//! core check and adds `Indeterminate`, so that a missing, stale, malformed or
//! unverifiable input is never reported as `Allow`.

mod evaluator;
mod types;

pub use evaluator::{CoreEvaluator, Guarded, PolicyEvaluator};
pub use types::{
    ActionId, ActionRequest, ActorId, CanonicalBytes, Decision, DecisionEvidence,
    EvaluationFailure, ResourceId, StateSnapshot, StateVersion, Violation,
};
