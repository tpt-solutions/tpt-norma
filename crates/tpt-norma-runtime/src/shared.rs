// Copyright (c) TPT Solutions
// SPDX-License-Identifier: MIT OR Apache-2.0

//! A gate that many threads can share, with validation and commit serialized per resource.
//!
//! This closes spec §8 assumption 2 for one resource inside one process. The lock is held across
//! [`Gate::admit`], which covers authorization and commit. It is released before the tool runs, so
//! slow tools do not block other requests.
//!
//! The lock does not cover a separate process or a separate host. Each resource needs its own
//! gate, and a deployment must not run two gates for the same resource.

use std::sync::Mutex;

use tpt_norma_eval::{ActionRequest, EvaluationFailure, PolicyEvaluator, StateVersion};

use crate::{Gate, Refusal};

/// A [`Gate`] behind a mutex.
#[derive(Debug)]
pub struct SharedGate<E> {
    inner: Mutex<Gate<E>>,
}

impl<E: PolicyEvaluator> SharedGate<E> {
    pub fn new(gate: Gate<E>) -> Self {
        Self {
            inner: Mutex::new(gate),
        }
    }

    /// Admit `request` under the lock, then run `tool` outside it.
    ///
    /// If the lock is poisoned, a previous call panicked while holding it, and the state can no
    /// longer be trusted. The request is refused as `Indeterminate` rather than admitted.
    pub fn submit<T>(
        &self,
        request: &ActionRequest,
        tool: impl FnOnce(&ActionRequest) -> T,
    ) -> Result<T, Refusal> {
        {
            let mut gate = self.inner.lock().map_err(|_| Refusal::Indeterminate {
                reason: EvaluationFailure::EvaluatorPanicked,
            })?;
            gate.admit(request)?;
        }
        Ok(tool(request))
    }

    /// The committed state version, read under the lock.
    pub fn version(&self) -> Option<StateVersion> {
        self.inner.lock().ok().map(|g| g.store().version())
    }

    /// The committed load, read under the lock.
    pub fn load(&self) -> Option<f64> {
        self.inner.lock().ok().map(|g| g.store().load())
    }
}
