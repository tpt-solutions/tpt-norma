// Copyright (c) TPT Solutions
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::panic::{self, AssertUnwindSafe};
use std::time::{Duration, Instant};

use tpt_norma_core::{check_action, ActionProposal, PolicyBounds, Rejection, SystemState};

use crate::types::{
    ActionRequest, Decision, DecisionEvidence, EvaluationFailure, StateSnapshot, Violation,
};

/// Evaluates a request against a snapshot.
///
/// Implementations must be side-effect free and must not depend on anything outside
/// `request`, `snapshot` and their own fixed policy.
pub trait PolicyEvaluator {
    fn evaluate(&self, request: &ActionRequest, snapshot: &StateSnapshot) -> Decision;
}

/// The reference evaluator. It delegates the arithmetic to `tpt-norma-core`'s
/// `check_action` and fixes the policy and verifier identity it records as provenance.
#[derive(Debug, Clone, PartialEq)]
pub struct CoreEvaluator {
    pub policy: PolicyBounds,
    pub policy_version: u64,
    pub verifier_identity: String,
}

impl PolicyEvaluator for CoreEvaluator {
    fn evaluate(&self, request: &ActionRequest, snapshot: &StateSnapshot) -> Decision {
        let Some(delta) = request.arguments.to_delta() else {
            return Decision::Indeterminate {
                reason: EvaluationFailure::MalformedArguments,
            };
        };
        let Some(load) = snapshot.current_load else {
            return Decision::Indeterminate {
                reason: EvaluationFailure::MissingState,
            };
        };

        let state = SystemState { current_load: load };
        let proposal = ActionProposal {
            resource_delta: delta,
        };
        match check_action(state, proposal, self.policy) {
            Ok(new_load) => Decision::Allow {
                evidence: DecisionEvidence {
                    request: request.clone(),
                    state_version: snapshot.version,
                    policy_version: self.policy_version,
                    verifier_identity: self.verifier_identity.clone(),
                    new_load,
                },
            },
            Err(Rejection::NonFinite) => Decision::Indeterminate {
                reason: EvaluationFailure::MalformedState,
            },
            Err(Rejection::Negative) => Decision::Deny {
                violations: vec![Violation::NegativeDelta { delta }],
            },
            Err(Rejection::RateLimit) => Decision::Deny {
                violations: vec![Violation::RateLimitExceeded {
                    delta,
                    max_delta_per_call: self.policy.max_delta_per_call,
                }],
            },
            Err(Rejection::Capacity) => Decision::Deny {
                violations: vec![Violation::CapacityExceeded {
                    projected_load: load + delta,
                    max_capacity: self.policy.max_capacity,
                }],
            },
        }
    }
}

/// Wraps an evaluator with fail-closed guards:
///
/// - a panic in the inner evaluator becomes `Indeterminate`;
/// - a call that runs past `deadline` becomes `Indeterminate`;
/// - an `Allow` whose evidence does not bind this exact request and snapshot version
///   becomes `Indeterminate`.
///
/// The deadline is checked after the call returns. It cannot interrupt a call that is
/// still running, so a real hard bound needs a separate thread or process. Fuel and
/// memory limits are not enforced here.
#[derive(Debug, Clone)]
pub struct Guarded<E> {
    inner: E,
    deadline: Duration,
}

impl<E> Guarded<E> {
    pub fn new(inner: E, deadline: Duration) -> Self {
        Self { inner, deadline }
    }
}

impl<E: PolicyEvaluator + std::panic::RefUnwindSafe> PolicyEvaluator for Guarded<E> {
    fn evaluate(&self, request: &ActionRequest, snapshot: &StateSnapshot) -> Decision {
        let start = Instant::now();
        let outcome =
            panic::catch_unwind(AssertUnwindSafe(|| self.inner.evaluate(request, snapshot)));
        let elapsed = start.elapsed();

        let decision = match outcome {
            Err(_) => {
                return Decision::Indeterminate {
                    reason: EvaluationFailure::EvaluatorPanicked,
                }
            }
            Ok(_) if elapsed > self.deadline => {
                return Decision::Indeterminate {
                    reason: EvaluationFailure::Timeout {
                        limit: self.deadline,
                    },
                }
            }
            Ok(decision) => decision,
        };

        match &decision {
            Decision::Allow { evidence }
                if !evidence.binds(request) || evidence.state_version != snapshot.version =>
            {
                Decision::Indeterminate {
                    reason: EvaluationFailure::InconsistentEvidence,
                }
            }
            _ => decision,
        }
    }
}
