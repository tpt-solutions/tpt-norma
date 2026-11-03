// Copyright (c) TPT Solutions
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Acceptance tests from spec2 §5, plus the fail-closed guards from spec2 §4.

use std::cell::Cell;
use std::thread;
use std::time::Duration;

use tpt_norma_core::PolicyBounds;
use tpt_norma_eval::{
    ActionId, ActionRequest, ActorId, CanonicalBytes, CoreEvaluator, Decision, EvaluationFailure,
    Guarded, PolicyEvaluator, ResourceId, StateSnapshot, StateVersion, Violation,
};
use tpt_norma_runtime::{authorize, CommitError, StateStore};

const POLICY_VERSION: u64 = 7;
const VERIFIER: &str = "tpt-norma-core/validate_action";

fn evaluator() -> CoreEvaluator {
    CoreEvaluator {
        policy: PolicyBounds {
            max_capacity: 10.0,
            max_delta_per_call: 5.0,
        },
        policy_version: POLICY_VERSION,
        verifier_identity: VERIFIER.to_string(),
    }
}

fn request(delta: f64, version: u64) -> ActionRequest {
    ActionRequest {
        action_id: ActionId("transfer-1".into()),
        actor: ActorId("agent-42".into()),
        resource: ResourceId("pool-a".into()),
        expected_state_version: StateVersion(version),
        arguments: CanonicalBytes::from_delta(delta),
    }
}

#[test]
fn positive_valid_action_against_current_state_allows_with_complete_evidence() {
    let mut store = StateStore::new(2.0);
    let req = request(3.0, 0);
    let decision = authorize(&evaluator(), &req, &store.snapshot());

    let Decision::Allow { evidence } = &decision else {
        panic!("expected Allow, got {decision:?}");
    };
    assert!(evidence.binds(&req));
    assert_eq!(evidence.state_version, StateVersion(0));
    assert_eq!(evidence.policy_version, POLICY_VERSION);
    assert_eq!(evidence.verifier_identity, VERIFIER);
    assert_eq!(evidence.new_load, 5.0);

    assert_eq!(store.commit(&req, &decision), Ok(()));
    assert_eq!(store.load(), 5.0);
    assert_eq!(store.version(), StateVersion(1));
}

#[test]
fn negative_violation_denies_with_specific_violation() {
    let store = StateStore::new(2.0);
    let decision = authorize(&evaluator(), &request(6.0, 0), &store.snapshot());
    assert_eq!(
        decision,
        Decision::Deny {
            violations: vec![Violation::RateLimitExceeded {
                delta: 6.0,
                max_delta_per_call: 5.0,
            }],
        }
    );

    let decision = authorize(
        &evaluator(),
        &request(4.0, 0),
        &StateStore::new(9.0).snapshot(),
    );
    assert_eq!(
        decision,
        Decision::Deny {
            violations: vec![Violation::CapacityExceeded {
                projected_load: 13.0,
                max_capacity: 10.0,
            }],
        }
    );
}

#[test]
fn negative_missing_state_is_indeterminate_not_allow() {
    let snapshot = StateSnapshot {
        version: StateVersion(0),
        current_load: None,
    };
    let decision = authorize(&evaluator(), &request(1.0, 0), &snapshot);
    assert_eq!(
        decision,
        Decision::Indeterminate {
            reason: EvaluationFailure::MissingState,
        }
    );
}

#[test]
fn negative_stale_state_is_indeterminate_and_skips_the_evaluator() {
    struct Counting(Cell<u32>);
    impl PolicyEvaluator for Counting {
        fn evaluate(&self, _: &ActionRequest, _: &StateSnapshot) -> Decision {
            self.0.set(self.0.get() + 1);
            Decision::Deny { violations: vec![] }
        }
    }

    let evaluator = Counting(Cell::new(0));
    let snapshot = StateStore::new(0.0).snapshot();
    let mut stale_snapshot = snapshot;
    stale_snapshot.version = StateVersion(3);

    let decision = authorize(&evaluator, &request(1.0, 0), &stale_snapshot);
    assert_eq!(
        decision,
        Decision::Indeterminate {
            reason: EvaluationFailure::StaleState {
                expected: StateVersion(0),
                actual: StateVersion(3),
            },
        }
    );
    assert_eq!(
        evaluator.0.get(),
        0,
        "evaluator must not run on stale state"
    );
}

/// Sleeps past the deadline, then delegates to the real evaluator.
struct Slow(CoreEvaluator);
impl PolicyEvaluator for Slow {
    fn evaluate(&self, request: &ActionRequest, snapshot: &StateSnapshot) -> Decision {
        thread::sleep(Duration::from_millis(30));
        self.0.evaluate(request, snapshot)
    }
}

#[test]
fn negative_timeout_is_indeterminate_never_allow() {
    let limit = Duration::from_millis(1);
    let guarded = Guarded::new(Slow(evaluator()), limit);
    let store = StateStore::new(0.0);

    let decision = authorize(&guarded, &request(1.0, 0), &store.snapshot());
    assert_eq!(
        decision,
        Decision::Indeterminate {
            reason: EvaluationFailure::Timeout { limit },
        }
    );
}

#[test]
fn negative_panic_is_indeterminate_never_allow() {
    struct Panicking;
    impl PolicyEvaluator for Panicking {
        fn evaluate(&self, _: &ActionRequest, _: &StateSnapshot) -> Decision {
            panic!("evaluator failed");
        }
    }

    let guarded = Guarded::new(Panicking, Duration::from_secs(1));
    let decision = authorize(&guarded, &request(1.0, 0), &StateStore::new(0.0).snapshot());
    assert_eq!(
        decision,
        Decision::Indeterminate {
            reason: EvaluationFailure::EvaluatorPanicked,
        }
    );
}

#[test]
fn negative_mutation_allow_for_action_a_is_rejected_for_modified_action_b() {
    let mut store = StateStore::new(0.0);
    let action_a = request(1.0, 0);
    let decision = authorize(&evaluator(), &action_a, &store.snapshot());
    assert!(decision.is_allow());

    // B differs from A by a single ulp in the delta, so it is a near-identical action.
    let action_b = request(f64::from_bits(1.0f64.to_bits() + 1), 0);
    assert_eq!(
        store.commit(&action_b, &decision),
        Err(CommitError::MutatedRequest)
    );
    assert_eq!(
        store.version(),
        StateVersion(0),
        "a rejected commit must not change state"
    );
}

#[test]
fn negative_state_drift_rejects_evidence_after_the_version_changes() {
    let mut store = StateStore::new(0.0);
    let req = request(1.0, 0);
    let decision = authorize(&evaluator(), &req, &store.snapshot());
    assert!(decision.is_allow());

    // Another commit lands between evaluate() and the executor's commit.
    let other = request(1.0, 0);
    let other_decision = authorize(&evaluator(), &other, &store.snapshot());
    store
        .commit(&other, &other_decision)
        .expect("first commit succeeds");

    assert_eq!(
        store.commit(&req, &decision),
        Err(CommitError::StateDrift {
            evidence: StateVersion(0),
            current: StateVersion(1),
        })
    );
}

#[test]
fn indeterminate_and_deny_are_never_committed() {
    let mut store = StateStore::new(0.0);
    let req = request(6.0, 0);
    let deny = authorize(&evaluator(), &req, &store.snapshot());
    assert_eq!(store.commit(&req, &deny), Err(CommitError::NotAllowed));

    let stale = authorize(&evaluator(), &request(1.0, 9), &store.snapshot());
    assert_eq!(
        store.commit(&request(1.0, 9), &stale),
        Err(CommitError::NotAllowed)
    );
    assert_eq!(store.load(), 0.0);
}

#[test]
fn malformed_arguments_are_indeterminate() {
    let store = StateStore::new(0.0);
    let mut req = request(1.0, 0);
    req.arguments = CanonicalBytes::from_delta(f64::NAN);
    assert_eq!(
        authorize(&evaluator(), &req, &store.snapshot()),
        Decision::Indeterminate {
            reason: EvaluationFailure::MalformedArguments,
        }
    );
}

#[test]
fn guard_rejects_an_allow_whose_evidence_is_for_a_different_request() {
    struct Lying(CoreEvaluator);
    impl PolicyEvaluator for Lying {
        fn evaluate(&self, _: &ActionRequest, snapshot: &StateSnapshot) -> Decision {
            // Returns evidence for an unrelated request.
            self.0.evaluate(&request(1.0, 0), snapshot)
        }
    }

    let guarded = Guarded::new(Lying(evaluator()), Duration::from_secs(1));
    let decision = authorize(&guarded, &request(2.0, 0), &StateStore::new(0.0).snapshot());
    assert_eq!(
        decision,
        Decision::Indeterminate {
            reason: EvaluationFailure::InconsistentEvidence,
        }
    );
}
