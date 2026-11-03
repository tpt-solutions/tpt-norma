/* Copyright (c) TPT Solutions
 * SPDX-License-Identifier: MIT OR Apache-2.0
 *
 * C interface to tpt-norma. Hand-written to match crates/tpt-norma-ffi/src/lib.rs.
 * Not yet compiled against a C host: the Rust tests call the same exported functions.
 *
 * Every function returns one of the codes below. Treat anything other than TN_ALLOW as a refusal.
 */
#ifndef TPT_NORMA_H
#define TPT_NORMA_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

#define TN_ALLOW         1   /* admitted; out-parameters have been written */
#define TN_DENY          0   /* proven to violate policy */
#define TN_INDETERMINATE (-1) /* safety could not be established */
#define TN_ERROR         (-2) /* bad argument, null pointer, or caught panic */

typedef struct TnGate TnGate;

/* Validate one proposal against one state. No state is changed.
 * out_new_load may be NULL. On TN_ALLOW it receives the new load. */
int32_t tn_validate_action(double load, double delta, double max_capacity,
                           double max_delta_per_call, double *out_new_load);

/* Create a gate for one resource. Returns NULL on failure.
 * Release with tn_gate_free exactly once. */
TnGate *tn_gate_new(double load, double max_capacity, double max_delta_per_call);

/* Release a gate. NULL is accepted and does nothing. */
void tn_gate_free(TnGate *gate);

/* Authorize and commit a delta at expected_version, without running a tool.
 * On TN_ALLOW the state has advanced, and the host then runs its tool.
 * action and resource must be NUL-terminated UTF-8 strings. */
int32_t tn_gate_admit(const TnGate *gate, const char *action, const char *resource,
                      double delta, uint64_t expected_version);

/* Read the committed load and version. out_load and out_version may be NULL. */
int32_t tn_gate_state(const TnGate *gate, double *out_load, uint64_t *out_version);

#ifdef __cplusplus
}
#endif

#endif /* TPT_NORMA_H */
