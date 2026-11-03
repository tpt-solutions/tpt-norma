// Copyright (c) TPT Solutions
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::time::Duration;

/// Version of a committed state snapshot. A decision is bound to the version it was made against.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StateVersion(pub u64);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ActionId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ActorId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ResourceId(pub String);

/// The canonical encoding of an action's arguments.
///
/// For this action set, the argument is the proposed resource delta, encoded as the
/// 8 little-endian bytes of its IEEE-754 bit pattern. Equality is therefore exact:
/// two requests match only if their arguments are bit-identical.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CanonicalBytes(Vec<u8>);

impl CanonicalBytes {
    pub fn from_delta(delta: f64) -> Self {
        Self(delta.to_bits().to_le_bytes().to_vec())
    }

    /// Wrap raw bytes as arguments. Their meaning is checked by the evaluator, not here.
    pub fn from_bytes(bytes: Vec<u8>) -> Self {
        Self(bytes)
    }

    /// The encoded length in bytes.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Decode the delta. Returns `None` for a wrong length or a non-finite value.
    pub fn to_delta(&self) -> Option<f64> {
        let bytes: [u8; 8] = self.0.as_slice().try_into().ok()?;
        let delta = f64::from_bits(u64::from_le_bytes(bytes));
        delta.is_finite().then_some(delta)
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

/// A committed view of the state that a decision is made against.
///
/// `current_load` is `None` when the state layer has no value. That is a missing
/// field, and it must never be read as zero.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StateSnapshot {
    pub version: StateVersion,
    pub current_load: Option<f64>,
}

/// An action an AI agent proposes, with its identifiers and canonical arguments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionRequest {
    pub action_id: ActionId,
    pub actor: ActorId,
    pub resource: ResourceId,
    pub expected_state_version: StateVersion,
    pub arguments: CanonicalBytes,
}

/// Evidence that a request was allowed. It is bound to that exact request and state version.
#[derive(Debug, Clone, PartialEq)]
pub struct DecisionEvidence {
    /// The request this evidence was produced for. Binding is an exact structural match.
    pub request: ActionRequest,
    pub state_version: StateVersion,
    /// The policy version that justified the decision.
    pub policy_version: u64,
    /// The verifier that produced the decision.
    pub verifier_identity: String,
    pub new_load: f64,
}

impl DecisionEvidence {
    /// True only if this evidence was produced for exactly `request`.
    pub fn binds(&self, request: &ActionRequest) -> bool {
        self.request == *request
    }
}

/// A proven reason that a request violates policy.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Violation {
    NegativeDelta {
        delta: f64,
    },
    RateLimitExceeded {
        delta: f64,
        max_delta_per_call: f64,
    },
    CapacityExceeded {
        projected_load: f64,
        max_capacity: f64,
    },
}

/// Why safety could not be determined. Each reason is `Indeterminate`, never `Allow`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EvaluationFailure {
    /// The snapshot has no load value.
    MissingState,
    /// The snapshot's load or the policy is NaN or infinite.
    MalformedState,
    /// The arguments are not a valid canonical encoding.
    MalformedArguments,
    /// The request was made against a version that is not the current snapshot.
    StaleState {
        expected: StateVersion,
        actual: StateVersion,
    },
    /// The evaluator exceeded its wall-clock deadline.
    Timeout { limit: Duration },
    /// The request's arguments are larger than the memory limit allows.
    ArgumentsTooLarge { limit: usize, actual: usize },
    /// The evaluator panicked.
    EvaluatorPanicked,
    /// An `Allow` did not bind the request and snapshot version it was asked about.
    InconsistentEvidence,
    /// The solver refuted the claimed `Allow` (for example a Farkas certificate for UNSAT).
    SolverRejected,
    /// The solver could not decide, or the inputs are outside its exact range.
    SolverInconclusive,
    /// The solver ran out of its resource budget before it could decide.
    ResourceExhausted,
}

/// The three-valued outcome of a policy evaluation.
#[derive(Debug, Clone, PartialEq)]
pub enum Decision {
    /// The action is proven safe against the snapshot and policy.
    Allow { evidence: DecisionEvidence },
    /// The action is proven to violate a constraint.
    Deny { violations: Vec<Violation> },
    /// Safety cannot be proven. The executor must treat this as a denial.
    Indeterminate { reason: EvaluationFailure },
}

impl Decision {
    pub fn is_allow(&self) -> bool {
        matches!(self, Decision::Allow { .. })
    }
}
