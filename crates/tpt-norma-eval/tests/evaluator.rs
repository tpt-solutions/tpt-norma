// Copyright (c) TPT Solutions
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Tests for the reference evaluator and the fail-closed guards around it (spec2 §4).

use std::thread;
use std::time::Duration;

use tpt_norma_core::PolicyBounds;
use tpt_norma_eval::{
    ActionId, ActionRequest, ActorId, CanonicalBytes, CoreEvaluator, Decision, DecisionEvidence,
    EvaluationFailure, Guarded, PolicyEvaluator, ResourceId, StateSnapshot, StateVersion,
    Violation,
};

fn evaluator() -> CoreEvaluator {
    CoreEvaluator {
        policy: PolicyBounds {
            max_capacity: 10.0,
            max_delta_per_call: 5.0,
        },
        policy_version: 1,
        verifier_identity: "test".to_string(),
    }
}

fn request(delta: f64) -> ActionRequest {
    ActionRequest {
        action_id: ActionId("a".into()),
        actor: ActorId("agent".into()),
        resource: ResourceId("r".into()),
        expected_state_version: StateVersion(3),
        arguments: CanonicalBytes::from_delta(delta),
    }
}

fn snapshot(load: Option<f64>) -> StateSnapshot {
    StateSnapshot {
        version: StateVersion(3),
        current_load: load,
    }
}

fn allow_for(req: &ActionRequest, version: StateVersion) -> Decision {
    Decision::Allow {
        evidence: DecisionEvidence {
            request: req.clone(),
            state_version: version,
            policy_version: 1,
            verifier_identity: "test".to_string(),
            new_load: 5.0,
        },
    }
}

/// Returns a fixed decision, so the guards can be tested in isolation.
struct Fixed(Decision);

impl PolicyEvaluator for Fixed {
    fn evaluate(&self, _: &ActionRequest, _: &StateSnapshot) -> Decision {
        self.0.clone()
    }
}

/// Returns a fixed decision after a delay.
struct Delayed {
    decision: Decision,
    delay: Duration,
}

impl PolicyEvaluator for Delayed {
    fn evaluate(&self, _: &ActionRequest, _: &StateSnapshot) -> Decision {
        thread::sleep(self.delay);
        self.decision.clone()
    }
}

struct Panics;

impl PolicyEvaluator for Panics {
    fn evaluate(&self, _: &ActionRequest, _: &StateSnapshot) -> Decision {
        panic!("evaluator bug");
    }
}

#[test]
fn an_admitted_action_allows_with_evidence_for_this_request() {
    let req = request(3.0);
    let decision = evaluator().evaluate(&req, &snapshot(Some(2.0)));

    let Decision::Allow { evidence } = decision else {
        panic!("expected Allow, got {decision:?}");
    };
    assert!(evidence.binds(&req));
    assert_eq!(evidence.new_load, 5.0);
    assert_eq!(evidence.state_version, StateVersion(3));
    assert_eq!(evidence.policy_version, 1);
    assert_eq!(evidence.verifier_identity, "test");
}

#[test]
fn a_negative_delta_is_denied() {
    assert_eq!(
        evaluator().evaluate(&request(-1.0), &snapshot(Some(2.0))),
        Decision::Deny {
            violations: vec![Violation::NegativeDelta { delta: -1.0 }],
        }
    );
}

#[test]
fn a_delta_over_the_rate_limit_is_denied() {
    assert_eq!(
        evaluator().evaluate(&request(6.0), &snapshot(Some(0.0))),
        Decision::Deny {
            violations: vec![Violation::RateLimitExceeded {
                delta: 6.0,
                max_delta_per_call: 5.0,
            }],
        }
    );
}

#[test]
fn an_action_over_capacity_is_denied_with_the_projected_load() {
    assert_eq!(
        evaluator().evaluate(&request(4.0), &snapshot(Some(9.0))),
        Decision::Deny {
            violations: vec![Violation::CapacityExceeded {
                projected_load: 13.0,
                max_capacity: 10.0,
            }],
        }
    );
}

#[test]
fn a_missing_load_is_indeterminate_and_never_read_as_zero() {
    assert_eq!(
        evaluator().evaluate(&request(1.0), &snapshot(None)),
        Decision::Indeterminate {
            reason: EvaluationFailure::MissingState,
        }
    );
}

#[test]
fn a_non_finite_load_is_indeterminate() {
    assert_eq!(
        evaluator().evaluate(&request(1.0), &snapshot(Some(f64::NAN))),
        Decision::Indeterminate {
            reason: EvaluationFailure::MalformedState,
        }
    );
}

#[test]
fn a_non_finite_delta_is_malformed_arguments() {
    assert_eq!(
        evaluator().evaluate(&request(f64::INFINITY), &snapshot(Some(0.0))),
        Decision::Indeterminate {
            reason: EvaluationFailure::MalformedArguments,
        }
    );
}

#[test]
fn a_panic_in_the_inner_evaluator_is_indeterminate() {
    let guarded = Guarded::new(Panics, Duration::from_secs(1));
    assert_eq!(
        guarded.evaluate(&request(1.0), &snapshot(Some(0.0))),
        Decision::Indeterminate {
            reason: EvaluationFailure::EvaluatorPanicked,
        }
    );
}

#[test]
fn a_call_past_its_deadline_is_indeterminate_even_if_it_would_allow() {
    let req = request(1.0);
    let limit = Duration::from_millis(1);
    let guarded = Guarded::new(
        Delayed {
            decision: allow_for(&req, StateVersion(3)),
            delay: Duration::from_millis(50),
        },
        limit,
    );
    assert_eq!(
        guarded.evaluate(&req, &snapshot(Some(0.0))),
        Decision::Indeterminate {
            reason: EvaluationFailure::Timeout { limit },
        }
    );
}

#[test]
fn a_call_within_its_deadline_is_returned_unchanged() {
    let deny = Decision::Deny { violations: vec![] };
    let guarded = Guarded::new(
        Delayed {
            decision: deny.clone(),
            delay: Duration::from_millis(1),
        },
        Duration::from_secs(10),
    );
    assert_eq!(guarded.evaluate(&request(1.0), &snapshot(Some(0.0))), deny);
}

#[test]
fn an_allow_for_a_different_request_is_inconsistent() {
    let other = request(2.0);
    let guarded = Guarded::new(
        Fixed(allow_for(&other, StateVersion(3))),
        Duration::from_secs(1),
    );
    assert_eq!(
        guarded.evaluate(&request(1.0), &snapshot(Some(0.0))),
        Decision::Indeterminate {
            reason: EvaluationFailure::InconsistentEvidence,
        }
    );
}

#[test]
fn an_allow_for_a_different_state_version_is_inconsistent() {
    let req = request(1.0);
    let guarded = Guarded::new(
        Fixed(allow_for(&req, StateVersion(2))),
        Duration::from_secs(1),
    );
    assert_eq!(
        guarded.evaluate(&req, &snapshot(Some(0.0))),
        Decision::Indeterminate {
            reason: EvaluationFailure::InconsistentEvidence,
        }
    );
}

#[test]
fn a_deny_is_passed_through_the_guards() {
    let req = request(1.0);
    let deny = Decision::Deny {
        violations: vec![Violation::NegativeDelta { delta: -1.0 }],
    };
    let guarded = Guarded::new(Fixed(deny.clone()), Duration::from_secs(1));
    assert_eq!(guarded.evaluate(&req, &snapshot(Some(0.0))), deny);
}
