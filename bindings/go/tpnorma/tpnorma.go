// Copyright (c) TPT Solutions
// SPDX-License-Identifier: MIT OR Apache-2.0

//go:build windows

// Package tpnorma is a Go binding to the tpt-norma FFI bridge (spec §4, Phase 5).
//
// It loads tpt_norma_ffi.dll and calls the bit-pattern entry points, so no C compiler is needed.
// Floating-point values are passed as their IEEE-754 bit patterns, because syscall.Proc.Call
// passes integer arguments only. This package works on Windows only. The tests need the DLL,
// built with `cargo build -p tpt-norma-ffi --release`.
//
// Every call that can refuse returns one of Allow, Deny, Indeterminate or Error. Treat anything
// but Allow as a refusal. Deny means a proven violation. Indeterminate means safety could not be
// established, which includes a stale version, a non-finite value, and an audit or commit failure.
package tpnorma

import (
	"errors"
	"math"
	"runtime"
	"syscall"
	"unsafe"
)

// Return codes, matching tpt_norma.h.
const (
	Allow         int32 = 1
	Deny          int32 = 0
	Indeterminate int32 = -1
	Error         int32 = -2
)

// ErrNotLoaded is returned when Load has not been called successfully.
var ErrNotLoaded = errors.New("tpnorma: DLL not loaded; call Load first")

// ErrGateClosed is returned for a gate that is nil or already closed.
var ErrGateClosed = errors.New("tpnorma: gate is closed")

var (
	dll          *syscall.LazyDLL
	procValidate *syscall.LazyProc
	procNew      *syscall.LazyProc
	procFree     *syscall.LazyProc
	procAdmit    *syscall.LazyProc
	procState    *syscall.LazyProc
)

// Load points the binding at tpt_norma_ffi.dll. Call it once before any other function.
func Load(path string) error {
	dll = syscall.NewLazyDLL(path)
	procValidate = dll.NewProc("tn_validate_action_bits")
	procNew = dll.NewProc("tn_gate_new_bits")
	procFree = dll.NewProc("tn_gate_free")
	procAdmit = dll.NewProc("tn_gate_admit_bits")
	procState = dll.NewProc("tn_gate_state")
	return dll.Load()
}

func bits(x float64) uintptr { return uintptr(math.Float64bits(x)) }

// code converts the C int32 return value. The upper half of the register is not defined by the
// callee, so only the low 32 bits are kept.
func code(r uintptr) int32 { return int32(uint32(r)) }

// Validate checks one proposal against one load, without changing any state. On Allow it
// returns the new load.
func Validate(load, delta, maxCapacity, maxDeltaPerCall float64) (int32, float64, error) {
	if dll == nil {
		return Error, 0, ErrNotLoaded
	}
	newLoad := new(float64)
	r, _, _ := procValidate.Call(
		bits(load), bits(delta), bits(maxCapacity), bits(maxDeltaPerCall),
		uintptr(unsafe.Pointer(newLoad)),
	)
	runtime.KeepAlive(newLoad)
	if code(r) != Allow {
		return code(r), 0, nil
	}
	return code(r), *newLoad, nil
}

// Gate is one resource's state, held on the Rust side. Close it when it is no longer needed.
type Gate struct {
	handle uintptr
}

// NewGate creates a gate with the given starting load and policy.
func NewGate(load, maxCapacity, maxDeltaPerCall float64) (*Gate, error) {
	if dll == nil {
		return nil, ErrNotLoaded
	}
	h, _, _ := procNew.Call(bits(load), bits(maxCapacity), bits(maxDeltaPerCall))
	if h == 0 {
		return nil, errors.New("tpnorma: tn_gate_new returned null")
	}
	g := &Gate{handle: h}
	runtime.SetFinalizer(g, (*Gate).Close)
	return g, nil
}

// Close releases the gate. It is safe to call more than once.
func (g *Gate) Close() {
	if g == nil || g.handle == 0 {
		return
	}
	procFree.Call(g.handle)
	g.handle = 0
	runtime.SetFinalizer(g, nil)
}

// Admit authorizes and commits a delta at expectedVersion, without running a tool. On Allow the
// state has advanced and the caller may run its tool.
func (g *Gate) Admit(action, resource string, delta float64, expectedVersion uint64) (int32, error) {
	if g == nil || g.handle == 0 {
		return Error, ErrGateClosed
	}
	a := cString(action)
	r := cString(resource)
	ret, _, _ := procAdmit.Call(
		g.handle,
		uintptr(unsafe.Pointer(&a[0])),
		uintptr(unsafe.Pointer(&r[0])),
		bits(delta),
		uintptr(expectedVersion),
	)
	runtime.KeepAlive(a)
	runtime.KeepAlive(r)
	runtime.KeepAlive(g)
	return code(ret), nil
}

// State reads the committed load and version.
func (g *Gate) State() (load float64, version uint64, err error) {
	if g == nil || g.handle == 0 {
		return 0, 0, ErrGateClosed
	}
	l := new(float64)
	v := new(uint64)
	ret, _, _ := procState.Call(
		g.handle,
		uintptr(unsafe.Pointer(l)),
		uintptr(unsafe.Pointer(v)),
	)
	runtime.KeepAlive(g)
	if code(ret) != Allow {
		return 0, 0, errors.New("tpnorma: tn_gate_state failed")
	}
	return *l, *v, nil
}

// cString returns s as a NUL-terminated byte slice, which is what the C side expects.
func cString(s string) []byte {
	b := make([]byte, len(s)+1)
	copy(b, s)
	return b
}
