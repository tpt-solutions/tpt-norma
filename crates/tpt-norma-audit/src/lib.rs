// Copyright (c) TPT Solutions
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Hash-chained, tamper-evident audit log of the authorization lifecycle.
//!
//! Each [`Entry`] stores the SHA-256 digest of its predecessor's digest together with its own
//! event, so editing, reordering, inserting or dropping an entry breaks the chain at that point.
//! [`verify_chain`] recomputes every digest, and an external auditor can run it on a copy of
//! the log without trusting the process that wrote it.
//!
//! This is an internal format. It is not `tpt-concord` evidence, which is not checked out here.
//! Entries are not signed, so the chain detects accidental and unsophisticated edits. It does
//! not stop someone who can rewrite the whole log and its digests. Anchoring the head digest
//! externally is needed for that, and is not done here.

use sha2::{Digest, Sha256};

/// One step in the authorization lifecycle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// A request arrived at the gate.
    Requested {
        action_id: String,
        actor: String,
        resource: String,
        expected_version: u64,
    },
    /// The evaluator produced a three-valued decision.
    Decided {
        outcome: String,
        state_version: u64,
        policy_version: u64,
        verifier_identity: String,
        detail: String,
    },
    /// The executor applied an `Allow` and advanced the state version.
    Committed {
        new_version: u64,
        new_load_bits: u64,
    },
    /// The request did not go ahead, and the reason is recorded.
    Refused { reason: String },
}

impl Event {
    /// A deterministic encoding of the event. Each field is length-prefixed, so no two distinct
    /// events share an encoding.
    fn encode(&self, out: &mut Vec<u8>) {
        match self {
            Event::Requested {
                action_id,
                actor,
                resource,
                expected_version,
            } => {
                out.push(0);
                push_str(out, action_id);
                push_str(out, actor);
                push_str(out, resource);
                out.extend_from_slice(&expected_version.to_be_bytes());
            }
            Event::Decided {
                outcome,
                state_version,
                policy_version,
                verifier_identity,
                detail,
            } => {
                out.push(1);
                push_str(out, outcome);
                out.extend_from_slice(&state_version.to_be_bytes());
                out.extend_from_slice(&policy_version.to_be_bytes());
                push_str(out, verifier_identity);
                push_str(out, detail);
            }
            Event::Committed {
                new_version,
                new_load_bits,
            } => {
                out.push(2);
                out.extend_from_slice(&new_version.to_be_bytes());
                out.extend_from_slice(&new_load_bits.to_be_bytes());
            }
            Event::Refused { reason } => {
                out.push(3);
                push_str(out, reason);
            }
        }
    }
}

fn push_str(out: &mut Vec<u8>, s: &str) {
    out.extend_from_slice(&(s.len() as u64).to_be_bytes());
    out.extend_from_slice(s.as_bytes());
}

/// One record in the chain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub seq: u64,
    /// Digest of the previous entry, or 32 zero bytes for the first entry.
    pub prev: [u8; 32],
    /// SHA-256 over `seq`, `prev` and the encoded event.
    pub digest: [u8; 32],
    pub event: Event,
}

fn digest_of(seq: u64, prev: &[u8; 32], event: &Event) -> [u8; 32] {
    let mut buf = Vec::new();
    buf.extend_from_slice(&seq.to_be_bytes());
    buf.extend_from_slice(prev);
    event.encode(&mut buf);
    Sha256::digest(&buf).into()
}

/// An append-only log.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AuditLog {
    entries: Vec<Entry>,
}

impl AuditLog {
    pub fn new() -> Self {
        Self::default()
    }

    /// Append an event and return its digest.
    pub fn append(&mut self, event: Event) -> [u8; 32] {
        let seq = self.entries.len() as u64;
        let prev = self.head();
        let digest = digest_of(seq, &prev, &event);
        self.entries.push(Entry {
            seq,
            prev,
            digest,
            event,
        });
        digest
    }

    /// The digest of the latest entry, or 32 zero bytes for an empty log.
    pub fn head(&self) -> [u8; 32] {
        self.entries.last().map_or([0; 32], |e| e.digest)
    }

    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// The first point at which a chain fails verification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChainError {
    /// Index of the first entry that does not verify.
    pub index: usize,
}

/// Recompute every digest and check every link. Returns the index of the first bad entry.
pub fn verify_chain(entries: &[Entry]) -> Result<(), ChainError> {
    let mut expected_prev = [0u8; 32];
    for (index, entry) in entries.iter().enumerate() {
        let bad = entry.seq != index as u64
            || entry.prev != expected_prev
            || entry.digest != digest_of(entry.seq, &entry.prev, &entry.event);
        if bad {
            return Err(ChainError { index });
        }
        expected_prev = entry.digest;
    }
    Ok(())
}

/// Lower-case hex of a digest, for display and for anchoring the head externally.
pub fn to_hex(digest: &[u8; 32]) -> String {
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> AuditLog {
        let mut log = AuditLog::new();
        log.append(Event::Requested {
            action_id: "a".into(),
            actor: "agent".into(),
            resource: "pool".into(),
            expected_version: 0,
        });
        log.append(Event::Committed {
            new_version: 1,
            new_load_bits: 5.0f64.to_bits(),
        });
        log
    }

    #[test]
    fn an_untouched_chain_verifies() {
        assert_eq!(verify_chain(sample().entries()), Ok(()));
        assert_eq!(verify_chain(&[]), Ok(()));
    }

    #[test]
    fn editing_an_event_is_detected_at_that_entry() {
        let mut entries = sample().entries().to_vec();
        entries[1].event = Event::Committed {
            new_version: 1,
            new_load_bits: 9.0f64.to_bits(),
        };
        assert_eq!(verify_chain(&entries), Err(ChainError { index: 1 }));
    }

    #[test]
    fn dropping_an_entry_is_detected() {
        let mut entries = sample().entries().to_vec();
        entries.remove(0);
        assert_eq!(verify_chain(&entries), Err(ChainError { index: 0 }));
    }

    #[test]
    fn reordering_entries_is_detected() {
        let mut entries = sample().entries().to_vec();
        entries.swap(0, 1);
        assert!(verify_chain(&entries).is_err());
    }

    #[test]
    fn length_prefixing_separates_fields() {
        // Without length prefixes, these two events would encode to the same bytes.
        let a = Event::Refused {
            reason: "ab".into(),
        };
        let b = Event::Refused { reason: "a".into() };
        let (mut ea, mut eb) = (Vec::new(), Vec::new());
        a.encode(&mut ea);
        b.encode(&mut eb);
        assert_ne!(ea, eb);
    }
}
