// Copyright (c) TPT Solutions
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The tool gate: the one entry point a tool executor calls (the contract for `tpt-writ`).
//!
//! [`Gate::submit`] runs the lifecycle in order: record the request, authorize it against the
//! current snapshot, commit the state change, and only then run the tool. A request that is not
//! allowed never reaches the tool. Each step is written to the audit log.
//!
//! The tool runs after the commit. If it then fails, the state change has already been applied,
//! and the gate does not roll it back. Tools that can fail should be idempotent, or the caller
//! should compensate. This is a design decision recorded in `docs/decisions.md`.

//! The only public constructor, [`Gate::new`], builds the guarded stack: `CoreEvaluator` inside
//! `Guarded` (deadline, panic and evidence checks), inside `CertifiedEvaluator` (solver check of
//! every `Allow`). A gate cannot be built around a bare evaluator outside this crate, so a host
//! cannot skip those checks by accident.
//!
//! Shadow mode for a pilot is [`Gate::admit`] with the tool left out: the host calls `admit`,
//! records the result, and does not run the tool. Admissions still commit, so the shadow budget
//! counts calls the way enforcement would.
//!
//! Audit entries are written through an [`AuditSink`] before they are kept in memory. The gate
//! fails closed on audit: if the `Requested` or `Decided` entry cannot be persisted, the request
//! is refused and no state changes. If the final `Committed` entry cannot be persisted, the state
//! has already changed, so the gate refuses without running the tool, and the caller must
//! reconcile the state from the store.

use std::time::Duration;

use tpt_norma_audit::{AuditLog, AuditSink, Event, NullSink};
use tpt_norma_core::PolicyBounds;
use tpt_norma_eval::{
    ActionRequest, CoreEvaluator, Decision, EvaluationFailure, Guarded, PolicyEvaluator, Violation,
};
use tpt_norma_solver::CertifiedEvaluator;

use crate::{authorize, CommitError, StateStore};

/// The evaluator stack that [`Gate::new`] builds.
pub type GuardedEvaluator = CertifiedEvaluator<Guarded<CoreEvaluator>>;

/// Why a request did not reach its tool.
#[derive(Debug, Clone, PartialEq)]
pub enum Refusal {
    /// The evaluator proved a violation.
    Denied { violations: Vec<Violation> },
    /// Safety could not be established.
    Indeterminate { reason: EvaluationFailure },
    /// The decision was `Allow`, but the commit was refused.
    Commit(CommitError),
    /// An audit entry could not be persisted. Nothing has been run. See the module docs for
    /// whether the state has changed.
    Audit { message: String },
}

/// Authorizes, commits and runs tool calls, and keeps an audit trail of each one.
#[derive(Debug)]
pub struct Gate<E> {
    evaluator: E,
    store: StateStore,
    log: AuditLog,
    sink: Box<dyn AuditSink>,
}

impl Gate<GuardedEvaluator> {
    /// Build a gate for `policy` over `store`.
    ///
    /// `deadline` is the wall-clock limit on one evaluation. It is checked after the call
    /// returns, so it cannot interrupt a call that is still running (see `Guarded`).
    pub fn new(policy: CoreEvaluator, store: StateStore, deadline: Duration) -> Self {
        Self::with_sink(policy, store, deadline, Box::new(NullSink))
    }

    /// As [`Gate::new`], but every audit entry is written to `sink` before it is kept.
    pub fn with_sink(
        policy: CoreEvaluator,
        store: StateStore,
        deadline: Duration,
        sink: Box<dyn AuditSink>,
    ) -> Self {
        let bounds: PolicyBounds = policy.policy;
        let evaluator = CertifiedEvaluator::new(Guarded::new(policy, deadline), bounds);
        Self {
            evaluator,
            store,
            log: AuditLog::new(),
            sink,
        }
    }
}

impl<E: PolicyEvaluator> Gate<E> {
    pub fn store(&self) -> &StateStore {
        &self.store
    }

    pub fn log(&self) -> &AuditLog {
        &self.log
    }

    /// Run `tool` if and only if `request` is authorized and its commit succeeds.
    pub fn submit<T>(
        &mut self,
        request: &ActionRequest,
        tool: impl FnOnce(&ActionRequest) -> T,
    ) -> Result<T, Refusal> {
        self.admit(request)?;
        Ok(tool(request))
    }

    /// Authorize `request` and commit its state change, without running a tool.
    ///
    /// Each step is logged. On `Ok`, the state has advanced and the caller may run the tool.
    pub fn admit(&mut self, request: &ActionRequest) -> Result<(), Refusal> {
        self.record(Event::Requested {
            action_id: request.action_id.0.clone(),
            actor: request.actor.0.clone(),
            resource: request.resource.0.clone(),
            expected_version: request.expected_state_version.0,
        })?;

        let decision = authorize(&self.evaluator, request, &self.store.snapshot());
        self.record(decided_event(&decision, self.store.version().0))?;

        match &decision {
            Decision::Allow { .. } => {}
            Decision::Deny { violations } => {
                let refusal = Refusal::Denied {
                    violations: violations.clone(),
                };
                self.record(Event::Refused {
                    reason: format!("denied: {violations:?}"),
                })?;
                return Err(refusal);
            }
            Decision::Indeterminate { reason } => {
                let refusal = Refusal::Indeterminate { reason: *reason };
                self.record(Event::Refused {
                    reason: format!("indeterminate: {reason:?}"),
                })?;
                return Err(refusal);
            }
        }

        if let Err(error) = self.store.commit(request, &decision) {
            self.record(Event::Refused {
                reason: format!("commit refused: {error:?}"),
            })?;
            return Err(Refusal::Commit(error));
        }
        self.record(Event::Committed {
            new_version: self.store.version().0,
            new_load_bits: self.store.load().to_bits(),
        })
    }

    /// Append `event` to the log through the sink. A persistence failure is a refusal.
    fn record(&mut self, event: Event) -> Result<(), Refusal> {
        self.log
            .append_with(event, self.sink.as_mut())
            .map(|_| ())
            .map_err(|error| Refusal::Audit {
                message: error.to_string(),
            })
    }
}

/// The `Decided` event for `decision`. `current_version` is the store's version, used for the
/// outcomes that do not carry their own.
fn decided_event(decision: &Decision, current_version: u64) -> Event {
    let (outcome, state_version, policy_version, verifier_identity, detail) = match decision {
        Decision::Allow { evidence } => (
            "allow",
            evidence.state_version.0,
            evidence.policy_version,
            evidence.verifier_identity.clone(),
            format!("new_load={}", evidence.new_load),
        ),
        Decision::Deny { violations } => (
            "deny",
            current_version,
            0,
            String::new(),
            format!("{violations:?}"),
        ),
        Decision::Indeterminate { reason } => (
            "indeterminate",
            current_version,
            0,
            String::new(),
            format!("{reason:?}"),
        ),
    };
    Event::Decided {
        outcome: outcome.into(),
        state_version,
        policy_version,
        verifier_identity,
        detail,
    }
}
