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

use tpt_norma_audit::{AuditLog, Event};
use tpt_norma_eval::{ActionRequest, Decision, EvaluationFailure, PolicyEvaluator, Violation};

use crate::{authorize, CommitError, StateStore};

/// Why a request did not reach its tool.
#[derive(Debug, Clone, PartialEq)]
pub enum Refusal {
    /// The evaluator proved a violation.
    Denied { violations: Vec<Violation> },
    /// Safety could not be established.
    Indeterminate { reason: EvaluationFailure },
    /// The decision was `Allow`, but the commit was refused.
    Commit(CommitError),
}

/// Authorizes, commits and runs tool calls, and keeps an audit trail of each one.
#[derive(Debug)]
pub struct Gate<E> {
    evaluator: E,
    store: StateStore,
    log: AuditLog,
}

impl<E: PolicyEvaluator> Gate<E> {
    pub fn new(evaluator: E, store: StateStore) -> Self {
        Self {
            evaluator,
            store,
            log: AuditLog::new(),
        }
    }

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
        self.log.append(Event::Requested {
            action_id: request.action_id.0.clone(),
            actor: request.actor.0.clone(),
            resource: request.resource.0.clone(),
            expected_version: request.expected_state_version.0,
        });

        let decision = authorize(&self.evaluator, request, &self.store.snapshot());
        self.log_decision(&decision);

        match &decision {
            Decision::Allow { .. } => {}
            Decision::Deny { violations } => {
                let refusal = Refusal::Denied {
                    violations: violations.clone(),
                };
                self.log.append(Event::Refused {
                    reason: format!("denied: {violations:?}"),
                });
                return Err(refusal);
            }
            Decision::Indeterminate { reason } => {
                let refusal = Refusal::Indeterminate { reason: *reason };
                self.log.append(Event::Refused {
                    reason: format!("indeterminate: {reason:?}"),
                });
                return Err(refusal);
            }
        }

        if let Err(error) = self.store.commit(request, &decision) {
            self.log.append(Event::Refused {
                reason: format!("commit refused: {error:?}"),
            });
            return Err(Refusal::Commit(error));
        }
        self.log.append(Event::Committed {
            new_version: self.store.version().0,
            new_load_bits: self.store.load().to_bits(),
        });
        Ok(())
    }

    fn log_decision(&mut self, decision: &Decision) {
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
                self.store.version().0,
                0,
                String::new(),
                format!("{violations:?}"),
            ),
            Decision::Indeterminate { reason } => (
                "indeterminate",
                self.store.version().0,
                0,
                String::new(),
                format!("{reason:?}"),
            ),
        };
        self.log.append(Event::Decided {
            outcome: outcome.into(),
            state_version,
            policy_version,
            verifier_identity,
            detail,
        });
    }
}
