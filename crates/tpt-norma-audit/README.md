# tpt-norma-audit

A hash-chained, tamper-evident log of the authorization lifecycle. Each entry carries the
SHA-256 digest of its predecessor, so any edit, reordering, insertion or deletion breaks the
chain at the point where it happened.

It records what the gate did: a request arrived, a decision was made, a state change was
committed, or a request was refused.

> **Status:** `0.1.0`, unpublished (`publish = false`). This is an internal format. It is not
> `tpt-concord` evidence, which is not checked out in this workspace.

## What it protects against

The chain detects **accidental and unsophisticated edits**: a line changed in a text editor, a
record deleted, two records swapped.

It does **not** stop someone who can rewrite the whole log and recompute its digests. Entries
are not signed. To close that gap, the head digest must be anchored outside the process that
writes the log. This crate exposes the head ([`AuditLog::head`], [`to_hex`]) for that purpose,
but does not do the anchoring.

## Usage

```toml
[dependencies]
tpt-norma-audit = { path = "../tpt-norma-audit" }
```

```rust
use tpt_norma_audit::{verify_chain, AuditLog, Event};

let mut log = AuditLog::new();
log.append(Event::Refused {
    reason: "budget exhausted".into(),
});
log.append(Event::Committed {
    new_version: 1,
    new_load_bits: 1.0_f64.to_bits(),
});

assert!(verify_chain(log.entries()).is_ok());
```

To persist entries as they are made, use [`AuditLog::append_with`] with a sink. If the sink
fails, the entry is not added, so the in-memory log never runs ahead of the file.

```rust
use std::path::Path;
use tpt_norma_audit::{AuditLog, Event, FileSink};

let mut sink = FileSink::open(Path::new("audit.log"))?;
let mut log = AuditLog::new();
log.append_with(Event::Refused { reason: "budget exhausted".into() }, &mut sink)?;
# Ok::<(), std::io::Error>(())
```

## Events

| Variant | Recorded when |
| :--- | :--- |
| `Requested { action_id, actor, resource, expected_version }` | A request arrives at the gate |
| `Decided { outcome, state_version, policy_version, verifier_identity, detail }` | The evaluator returns a three-valued decision |
| `Committed { new_version, new_load_bits }` | The executor applies an `Allow` and advances the version |
| `Refused { reason }` | The request does not go ahead, and the reason is recorded |

`new_load_bits` is the IEEE-754 bit pattern of the load, so the log stores the exact value.

## Format

Each entry is a record with a sequence number, the previous digest, the digest of this record,
and the event.

- The digest is SHA-256 over the sequence number, the previous digest and a deterministic
  encoding of the event. Every field is length-prefixed, so two different events never share
  an encoding.
- The first entry's `prev` is 32 zero bytes.
- [`AuditLog::to_lines`] writes one entry per line as `seq prev digest event`, with binary
  fields in hex. Lines can be appended to a file as the log grows.
- [`AuditLog::from_lines`] reads that format back, and verifies the chain before returning. A
  truncated, reordered or edited file is refused with a `LoadError`.

## Sinks

| Sink | Behaviour |
| :--- | :--- |
| `NullSink` | Discards entries. For tests, or where no persistence is wanted |
| `FileSink` | Appends each entry as one line, and syncs it before returning |

Implement `AuditSink` to write elsewhere. An implementation must not return `Ok` until the
entry is durable to the degree it promises.

## Verification

[`verify_chain`] recomputes every digest and checks every link. On failure it returns a
`ChainError` with the index of the first bad entry. An auditor can run it on a copy of the log
without trusting the process that wrote it.

## Limits

- Entries are not signed. See *What it protects against*.
- `FileSink` syncs each entry, which is slow on a busy path. Batch at the caller if throughput
  matters, and keep the fail-closed behaviour in mind.
- The format is internal and may change before a release.

## Testing

```sh
cargo test -p tpt-norma-audit
```

Unit tests live in `src/lib.rs`. The gate-level behaviour, including what happens when a sink
fails, is tested in [`tpt-norma-runtime`](../tpt-norma-runtime/) (`tests/audit_sink.rs`).

## Related

- Written by: [`tpt-norma-runtime`](../tpt-norma-runtime/)
- Design decisions: [`docs/decisions.md`](../../docs/decisions.md) (D10)
- Threat model, including what the chain does not cover: [`THREAT_MODEL.md`](../../THREAT_MODEL.md)

## License

MIT OR Apache-2.0. See the workspace [README](../../README.md#license).
