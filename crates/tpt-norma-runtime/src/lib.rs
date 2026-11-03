// Copyright (c) TPT Solutions
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Snapshot freshness and the commit-time executor for authorized actions (spec2 §4-5).
//!
//! The lifecycle is: take a snapshot, [`authorize`] a request against it, then
//! [`StateStore::commit`] the resulting decision. The commit rejects evidence that was
//! made for a different request, or against a state version that has since changed.

mod gate;
mod shared;

pub use gate::{Gate, Refusal};
pub use shared::SharedGate;

use tpt_norma_eval::{
    ActionRequest, Decision, EvaluationFailure, PolicyEvaluator, StateSnapshot, StateVersion,
};

/// Evaluate `request` against `snapshot`, after checking that the request was made
/// against the snapshot's current version.
///
/// A stale request is `Indeterminate`, not `Allow`. The evaluator is not called.
pub fn authorize<E: PolicyEvaluator>(
    evaluator: &E,
    request: &ActionRequest,
    snapshot: &StateSnapshot,
) -> Decision {
    if request.expected_state_version != snapshot.version {
        return Decision::Indeterminate {
            reason: EvaluationFailure::StaleState {
                expected: request.expected_state_version,
                actual: snapshot.version,
            },
        };
    }
    evaluator.evaluate(request, snapshot)
}

/// Why a commit was refused.
#[derive(Debug, Clone, PartialEq)]
pub enum CommitError {
    /// The decision was `Deny` or `Indeterminate`, so there is nothing to commit.
    NotAllowed,
    /// The evidence was produced for a different request than the one being committed.
    MutatedRequest,
    /// The state version changed between the decision and the commit.
    StateDrift {
        evidence: StateVersion,
        current: StateVersion,
    },
}

/// The committed state for one resource.
///
/// Commits take `&mut self`, so they are serialized by the borrow checker within one
/// owner. Concurrent owners still need external serialization (spec §8).
#[derive(Debug, Clone, PartialEq)]
pub struct StateStore {
    version: StateVersion,
    load: f64,
}

impl StateStore {
    pub fn new(load: f64) -> Self {
        Self {
            version: StateVersion(0),
            load,
        }
    }

    pub fn version(&self) -> StateVersion {
        self.version
    }

    pub fn load(&self) -> f64 {
        self.load
    }

    /// The current state, to be passed to [`authorize`].
    pub fn snapshot(&self) -> StateSnapshot {
        StateSnapshot {
            version: self.version,
            current_load: Some(self.load),
        }
    }

    /// Apply an `Allow` decision for `request` and advance the state version.
    pub fn commit(
        &mut self,
        request: &ActionRequest,
        decision: &Decision,
    ) -> Result<(), CommitError> {
        let Decision::Allow { evidence } = decision else {
            return Err(CommitError::NotAllowed);
        };
        if !evidence.binds(request) {
            return Err(CommitError::MutatedRequest);
        }
        if evidence.state_version != self.version {
            return Err(CommitError::StateDrift {
                evidence: evidence.state_version,
                current: self.version,
            });
        }

        self.load = evidence.new_load;
        self.version = StateVersion(self.version.0 + 1);
        Ok(())
    }
}
