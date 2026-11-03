// Copyright (c) TPT Solutions
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The gate writes each audit entry through its sink before keeping it, and fails closed.

use std::cell::Cell;
use std::io;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tpt_norma_audit::{verify_chain, AuditLog, AuditSink, Entry, FileSink};
use tpt_norma_core::PolicyBounds;
use tpt_norma_eval::{
    ActionId, ActionRequest, ActorId, CanonicalBytes, CoreEvaluator, ResourceId, StateVersion,
};
use tpt_norma_runtime::{Gate, GuardedEvaluator, Refusal, StateStore};

fn gate_with(load: f64, sink: Box<dyn AuditSink>) -> Gate<GuardedEvaluator> {
    Gate::with_sink(
        CoreEvaluator {
            policy: PolicyBounds {
                max_capacity: 10.0,
                max_delta_per_call: 5.0,
            },
            policy_version: 2,
            verifier_identity: "core".into(),
        },
        StateStore::new(load),
        Duration::from_secs(1),
        sink,
    )
}

fn request(delta: f64, version: u64) -> ActionRequest {
    ActionRequest {
        action_id: ActionId("tx".into()),
        actor: ActorId("agent".into()),
        resource: ResourceId("pool".into()),
        expected_state_version: StateVersion(version),
        arguments: CanonicalBytes::from_delta(delta),
    }
}

/// Keeps what it is given, and fails once `budget` persists have succeeded.
#[derive(Debug)]
struct BudgetSink {
    budget: usize,
    kept: Arc<Mutex<Vec<Entry>>>,
}

impl AuditSink for BudgetSink {
    fn persist(&mut self, entry: &Entry) -> io::Result<()> {
        if self.budget == 0 {
            return Err(io::Error::other("disk full"));
        }
        self.budget -= 1;
        self.kept.lock().unwrap().push(entry.clone());
        Ok(())
    }
}

#[test]
fn a_failed_request_entry_refuses_before_any_state_change_or_tool_run() {
    let mut gate = gate_with(
        2.0,
        Box::new(BudgetSink {
            budget: 0,
            kept: Arc::default(),
        }),
    );
    let ran = Cell::new(false);
    let out = gate.submit(&request(1.0, 0), |_| ran.set(true));
    assert!(matches!(out, Err(Refusal::Audit { .. })));
    assert!(!ran.get());
    assert_eq!(gate.store().load(), 2.0);
    assert_eq!(gate.store().version().0, 0);
    assert!(gate.log().is_empty());
}

#[test]
fn a_failed_commit_entry_refuses_and_the_tool_does_not_run() {
    // Requested and Decided persist; the Committed entry does not.
    let kept = Arc::new(Mutex::new(Vec::new()));
    let mut gate = gate_with(
        2.0,
        Box::new(BudgetSink {
            budget: 2,
            kept: kept.clone(),
        }),
    );
    let ran = Cell::new(false);
    let out = gate.submit(&request(1.0, 0), |_| ran.set(true));
    assert!(matches!(out, Err(Refusal::Audit { .. })));
    assert!(!ran.get());
    // The state change has been applied. The module docs say the caller must reconcile it.
    assert_eq!(gate.store().load(), 3.0);
    assert_eq!(kept.lock().unwrap().len(), 2);
    assert_eq!(gate.log().len(), 2);
}

#[test]
fn every_persisted_entry_matches_the_in_memory_log() {
    let kept = Arc::new(Mutex::new(Vec::new()));
    let mut gate = gate_with(
        2.0,
        Box::new(BudgetSink {
            budget: 100,
            kept: kept.clone(),
        }),
    );
    gate.submit(&request(1.0, 0), |_| ()).unwrap();
    let _ = gate.submit(&request(50.0, 1), |_| ());
    let kept = kept.lock().unwrap();
    assert_eq!(kept.as_slice(), gate.log().entries());
    assert_eq!(verify_chain(&kept), Ok(()));
}

#[test]
fn a_file_sink_survives_a_restart_with_a_verified_chain() {
    let dir = std::env::temp_dir().join(format!("tpt-norma-audit-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path: PathBuf = dir.join("audit.log");
    let _ = std::fs::remove_file(&path);

    let mut gate = gate_with(2.0, Box::new(FileSink::open(&path).unwrap()));
    gate.submit(&request(1.0, 0), |_| ()).unwrap();
    let _ = gate.submit(&request(50.0, 1), |_| ());
    let in_memory = gate.log().to_lines();
    drop(gate);

    let text = std::fs::read_to_string(&path).unwrap();
    let reloaded = AuditLog::from_lines(&text).expect("chain verifies after restart");
    assert_eq!(reloaded.to_lines(), in_memory);
    assert_eq!(reloaded.len(), 6);

    std::fs::remove_dir_all(&dir).unwrap();
}
