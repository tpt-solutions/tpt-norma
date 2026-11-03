// Copyright (c) TPT Solutions
// SPDX-License-Identifier: MIT OR Apache-2.0

//! C ABI over the `tpt-norma` validator and shared gate (spec §4, FFI enforcement bridge).
//!
//! Every exported function catches panics, so no unwind crosses the C boundary. A panic is
//! reported as [`TN_ERROR`], which a host must treat as a refusal.
//!
//! Return codes shared by all functions:
//!
//! - [`TN_ALLOW`] (`1`): the action is admitted, and any out-parameter has been written.
//! - [`TN_DENY`] (`0`): the action is proven to violate policy.
//! - [`TN_INDETERMINATE`] (`-1`): safety could not be established. Treat as a refusal.
//! - [`TN_ERROR`] (`-2`): invalid arguments, a null pointer, or a caught panic. Treat as a refusal.
//!
//! Limits of this bridge, stated in `THREAT_MODEL.md`:
//!
//! - It can enforce only what a host routes through it. A host that calls its tool without first
//!   calling [`tn_gate_admit`] is not covered.
//! - [`tn_gate_admit`] authorizes and commits. It does not run the tool. The host runs the tool
//!   after a `TN_ALLOW`, and must treat a tool failure after the commit as a state change that has
//!   already happened.
//! - The boundary itself is not proven. Its safety rests on the argument checks below, and on the
//!   host passing pointers it owns.

#![allow(unsafe_code)]

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::time::Duration;

use tpt_norma_core::{check_action, ActionProposal, PolicyBounds, SystemState};
use tpt_norma_eval::{
    ActionId, ActionRequest, ActorId, CanonicalBytes, CoreEvaluator, ResourceId, StateVersion,
};
use tpt_norma_runtime::{Gate, GuardedEvaluator, Refusal, SharedGate, StateStore};

/// Wall-clock limit on one evaluation through the FFI. The check is post-hoc (see `Guarded`).
const FFI_DEADLINE: Duration = Duration::from_secs(1);

pub const TN_ALLOW: i32 = 1;
pub const TN_DENY: i32 = 0;
pub const TN_INDETERMINATE: i32 = -1;
pub const TN_ERROR: i32 = -2;

/// An opaque gate handle. Hosts receive a pointer to this and never look inside it.
pub struct TnGate {
    inner: SharedGate<GuardedEvaluator>,
}

fn run<F: FnOnce() -> i32>(f: F) -> i32 {
    catch_unwind(AssertUnwindSafe(f)).unwrap_or(TN_ERROR)
}

/// Validate one proposal against one state, without any state change.
///
/// On `TN_ALLOW`, the new load is written to `out_new_load`. `out_new_load` may be null when the
/// host does not need the value.
///
/// # Safety
/// If `out_new_load` is non-null, it must be valid for writes of one `f64`.
#[no_mangle]
pub unsafe extern "C" fn tn_validate_action(
    load: f64,
    delta: f64,
    max_capacity: f64,
    max_delta_per_call: f64,
    out_new_load: *mut f64,
) -> i32 {
    run(|| {
        let policy = PolicyBounds {
            max_capacity,
            max_delta_per_call,
        };
        match check_action(
            SystemState { current_load: load },
            ActionProposal {
                resource_delta: delta,
            },
            policy,
        ) {
            Ok(new_load) => {
                if !out_new_load.is_null() {
                    // SAFETY: the caller guarantees `out_new_load` is valid for one f64 write
                    // when it is non-null (the function contract).
                    unsafe { out_new_load.write(new_load) };
                }
                TN_ALLOW
            }
            Err(_) => TN_DENY,
        }
    })
}

/// Create a gate for one resource. Returns null if the arguments cannot be used.
///
/// The returned pointer must be released exactly once with [`tn_gate_free`].
#[no_mangle]
pub extern "C" fn tn_gate_new(
    load: f64,
    max_capacity: f64,
    max_delta_per_call: f64,
) -> *mut TnGate {
    catch_unwind(AssertUnwindSafe(|| {
        let evaluator = CoreEvaluator {
            policy: PolicyBounds {
                max_capacity,
                max_delta_per_call,
            },
            policy_version: 1,
            verifier_identity: "tpt-norma-core/validate_action".to_string(),
        };
        let gate = Gate::new(evaluator, StateStore::new(load), FFI_DEADLINE);
        Box::into_raw(Box::new(TnGate {
            inner: SharedGate::new(gate),
        }))
    }))
    .unwrap_or(std::ptr::null_mut())
}

/// Release a gate created by [`tn_gate_new`]. Null is accepted and does nothing.
///
/// # Safety
/// `gate` must be null or a pointer returned by [`tn_gate_new`] that has not been freed.
#[no_mangle]
pub unsafe extern "C" fn tn_gate_free(gate: *mut TnGate) {
    if gate.is_null() {
        return;
    }
    // SAFETY: the caller guarantees `gate` came from `tn_gate_new` and is freed only once.
    drop(unsafe { Box::from_raw(gate) });
}

/// Authorize and commit a delta for `resource` at `expected_version`, without running a tool.
///
/// On `TN_ALLOW`, the state has advanced. The host then runs its tool.
///
/// # Safety
/// `gate` must be a live pointer from [`tn_gate_new`]. `resource` and `action` must each be null
/// or point to a NUL-terminated UTF-8 string.
#[no_mangle]
pub unsafe extern "C" fn tn_gate_admit(
    gate: *const TnGate,
    action: *const std::ffi::c_char,
    resource: *const std::ffi::c_char,
    delta: f64,
    expected_version: u64,
) -> i32 {
    run(|| {
        if gate.is_null() {
            return TN_ERROR;
        }
        let (Some(action), Some(resource)) = (c_str(action), c_str(resource)) else {
            return TN_ERROR;
        };
        // SAFETY: the caller guarantees `gate` is a live pointer from `tn_gate_new`.
        let gate = unsafe { &*gate };
        let request = ActionRequest {
            action_id: ActionId(action),
            actor: ActorId("ffi".to_string()),
            resource: ResourceId(resource),
            expected_state_version: StateVersion(expected_version),
            arguments: CanonicalBytes::from_delta(delta),
        };
        match gate.inner.submit(&request, |_| ()) {
            Ok(()) => TN_ALLOW,
            Err(Refusal::Denied { .. }) => TN_DENY,
            Err(Refusal::Indeterminate { .. }) | Err(Refusal::Commit(_)) | Err(Refusal::Audit { .. }) => TN_INDETERMINATE,
        }
    })
}

/// Read the committed load and version. Returns [`TN_ALLOW`] on success.
///
/// # Safety
/// `gate` must be a live pointer from [`tn_gate_new`]. `out_load` and `out_version` must each be
/// null or valid for one write.
#[no_mangle]
pub unsafe extern "C" fn tn_gate_state(
    gate: *const TnGate,
    out_load: *mut f64,
    out_version: *mut u64,
) -> i32 {
    run(|| {
        if gate.is_null() {
            return TN_ERROR;
        }
        // SAFETY: the caller guarantees `gate` is a live pointer from `tn_gate_new`.
        let gate = unsafe { &*gate };
        let (Some(load), Some(version)) = (gate.inner.load(), gate.inner.version()) else {
            return TN_ERROR;
        };
        if !out_load.is_null() {
            // SAFETY: the caller guarantees `out_load` is valid for one f64 write when non-null.
            unsafe { out_load.write(load) };
        }
        if !out_version.is_null() {
            // SAFETY: the caller guarantees `out_version` is valid for one u64 write when non-null.
            unsafe { out_version.write(version.0) };
        }
        TN_ALLOW
    })
}

/// Read a NUL-terminated UTF-8 string. Returns `None` for a null pointer or invalid UTF-8.
fn c_str(ptr: *const std::ffi::c_char) -> Option<String> {
    if ptr.is_null() {
        return None;
    }
    // SAFETY: the caller guarantees `ptr` is NUL-terminated when non-null (the function contract).
    let bytes = unsafe { std::ffi::CStr::from_ptr(ptr) };
    bytes.to_str().ok().map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CString;
    use std::ptr;

    #[test]
    fn validate_allows_and_writes_the_new_load() {
        let mut out = 0.0;
        // SAFETY: `out` is a valid f64 for the duration of the call.
        let code = unsafe { tn_validate_action(2.0, 3.0, 10.0, 5.0, &mut out) };
        assert_eq!(code, TN_ALLOW);
        assert_eq!(out, 5.0);
    }

    #[test]
    fn validate_denies_and_leaves_the_output_alone() {
        let mut out = -1.0;
        // SAFETY: `out` is a valid f64 for the duration of the call.
        let code = unsafe { tn_validate_action(2.0, 6.0, 10.0, 5.0, &mut out) };
        assert_eq!(code, TN_DENY);
        assert_eq!(out, -1.0);
    }

    #[test]
    fn validate_accepts_a_null_output_pointer() {
        // SAFETY: a null output pointer is allowed by the contract.
        let code = unsafe { tn_validate_action(2.0, 3.0, 10.0, 5.0, ptr::null_mut()) };
        assert_eq!(code, TN_ALLOW);
    }

    #[test]
    fn validate_refuses_non_finite_input() {
        // SAFETY: a null output pointer is allowed by the contract.
        let code = unsafe { tn_validate_action(f64::NAN, 1.0, 10.0, 5.0, ptr::null_mut()) };
        assert_eq!(code, TN_DENY);
    }

    #[test]
    fn gate_admits_then_refuses_past_capacity_and_reports_state() {
        let gate = tn_gate_new(0.0, 10.5, 1.0);
        assert!(!gate.is_null());
        let action = CString::new("tx").expect("no NUL");
        let resource = CString::new("pool").expect("no NUL");

        let mut admitted = 0;
        for version in 0..20u64 {
            // SAFETY: `gate` is live, and both strings are NUL-terminated and outlive the call.
            let code =
                unsafe { tn_gate_admit(gate, action.as_ptr(), resource.as_ptr(), 1.0, version) };
            if code == TN_ALLOW {
                admitted += 1;
            } else {
                assert_eq!(code, TN_DENY, "version {version}");
                break;
            }
        }
        assert_eq!(admitted, 10);

        let (mut load, mut version) = (0.0, 0u64);
        // SAFETY: `gate` is live, and both out-pointers are valid for one write.
        let code = unsafe { tn_gate_state(gate, &mut load, &mut version) };
        assert_eq!(code, TN_ALLOW);
        assert_eq!(load, 10.0);
        assert_eq!(version, 10);

        // SAFETY: `gate` came from `tn_gate_new` and is freed once.
        unsafe { tn_gate_free(gate) };
    }

    #[test]
    fn a_stale_version_is_indeterminate_not_allow() {
        let gate = tn_gate_new(0.0, 10.5, 1.0);
        let action = CString::new("tx").expect("no NUL");
        let resource = CString::new("pool").expect("no NUL");
        // SAFETY: `gate` is live, and both strings are NUL-terminated and outlive the call.
        let code = unsafe { tn_gate_admit(gate, action.as_ptr(), resource.as_ptr(), 1.0, 7) };
        assert_eq!(code, TN_INDETERMINATE);
        // SAFETY: `gate` came from `tn_gate_new` and is freed once.
        unsafe { tn_gate_free(gate) };
    }

    #[test]
    fn null_and_invalid_arguments_are_errors() {
        // SAFETY: a null gate is allowed by the contract and must not be dereferenced.
        let code = unsafe { tn_gate_admit(ptr::null(), ptr::null(), ptr::null(), 1.0, 0) };
        assert_eq!(code, TN_ERROR);

        let gate = tn_gate_new(0.0, 10.5, 1.0);
        // SAFETY: `gate` is live. A null string is allowed by the contract and is rejected.
        let code = unsafe { tn_gate_admit(gate, ptr::null(), ptr::null(), 1.0, 0) };
        assert_eq!(code, TN_ERROR);
        // SAFETY: `gate` came from `tn_gate_new` and is freed once.
        unsafe { tn_gate_free(gate) };

        // SAFETY: freeing null is a no-op by contract.
        unsafe { tn_gate_free(ptr::null_mut()) };
    }
}
