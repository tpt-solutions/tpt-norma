// Copyright (c) TPT Solutions
// SPDX-License-Identifier: MIT OR Apache-2.0

//go:build windows

package tpnorma

import (
	"math"
	"os"
	"testing"
)

// dllPath is TPT_NORMA_DLL if set, otherwise the release build from the repository root.
func dllPath(t *testing.T) string {
	t.Helper()
	if p := os.Getenv("TPT_NORMA_DLL"); p != "" {
		return p
	}
	return "../../../target/release/tpt_norma_ffi.dll"
}

func load(t *testing.T) {
	t.Helper()
	if err := Load(dllPath(t)); err != nil {
		t.Fatalf("load DLL: %v (build with cargo build -p tpt-norma-ffi --release)", err)
	}
}

func TestValidateAllowsWithinCapacityAndReturnsNewLoad(t *testing.T) {
	load(t)
	code, newLoad, err := Validate(2, 3, 10, 5)
	if err != nil || code != Allow || newLoad != 5 {
		t.Fatalf("got code %d load %v err %v; want Allow 5", code, newLoad, err)
	}
}

func TestValidateDeniesProvenViolations(t *testing.T) {
	load(t)
	cases := []struct {
		name                       string
		load, delta, cap, maxDelta float64
	}{
		{"capacity", 8, 4, 10, 5},
		{"rate limit", 0, 6, 10, 5},
		{"negative delta", 0, -1, 10, 5},
	}
	for _, c := range cases {
		code, _, err := Validate(c.load, c.delta, c.cap, c.maxDelta)
		if err != nil || code != Deny {
			t.Errorf("%s: got %d err %v; want Deny", c.name, code, err)
		}
	}
}

func TestValidateNonFiniteIsIndeterminateNotDeny(t *testing.T) {
	load(t)
	for _, v := range []float64{math.NaN(), math.Inf(1)} {
		code, _, err := Validate(v, 1, 10, 5)
		if err != nil || code != Indeterminate {
			t.Errorf("load %v: got %d err %v; want Indeterminate", v, code, err)
		}
	}
}

func TestGateAdmitsAdvancesStateAndRefusesCapacity(t *testing.T) {
	load(t)
	g, err := NewGate(2, 10, 5)
	if err != nil {
		t.Fatal(err)
	}
	defer g.Close()

	code, err := g.Admit("tx", "pool", 3, 0)
	if err != nil || code != Allow {
		t.Fatalf("first admit: got %d err %v; want Allow", code, err)
	}
	l, v, err := g.State()
	if err != nil || l != 5 || v != 1 {
		t.Fatalf("state: load %v version %d err %v; want 5, 1", l, v, err)
	}

	code, _ = g.Admit("tx", "pool", 5, 1)
	if code != Deny {
		t.Fatalf("over capacity: got %d; want Deny", code)
	}
	if l, v, _ := g.State(); l != 5 || v != 1 {
		t.Fatalf("a refused admit changed the state: load %v version %d", l, v)
	}
}

func TestGateStaleVersionIsIndeterminate(t *testing.T) {
	load(t)
	g, err := NewGate(0, 10, 5)
	if err != nil {
		t.Fatal(err)
	}
	defer g.Close()
	code, _ := g.Admit("tx", "pool", 1, 7)
	if code != Indeterminate {
		t.Fatalf("stale version: got %d; want Indeterminate", code)
	}
}

func TestClosedGateReportsItIsClosed(t *testing.T) {
	load(t)
	g, err := NewGate(0, 10, 5)
	if err != nil {
		t.Fatal(err)
	}
	g.Close()
	g.Close() // a second close is harmless
	if _, err := g.Admit("tx", "pool", 1, 0); err != ErrGateClosed {
		t.Fatalf("admit on closed gate: err %v; want ErrGateClosed", err)
	}
}
