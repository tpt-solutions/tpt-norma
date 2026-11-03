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

use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::Path;

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
        let entry = self.next_entry(event);
        let digest = entry.digest;
        self.entries.push(entry);
        digest
    }

    /// Append an event only after `sink` has persisted it.
    ///
    /// If the sink fails, the entry is not added and the error is returned, so the in-memory log
    /// never runs ahead of what is on disk.
    pub fn append_with(&mut self, event: Event, sink: &mut dyn AuditSink) -> io::Result<[u8; 32]> {
        let entry = self.next_entry(event);
        sink.persist(&entry)?;
        let digest = entry.digest;
        self.entries.push(entry);
        Ok(digest)
    }

    fn next_entry(&self, event: Event) -> Entry {
        let seq = self.entries.len() as u64;
        let prev = self.head();
        let digest = digest_of(seq, &prev, &event);
        Entry {
            seq,
            prev,
            digest,
            event,
        }
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

/// Somewhere an entry is written before it is kept in memory.
///
/// Implementations must not return `Ok` until the entry is durable to the degree they promise.
pub trait AuditSink: std::fmt::Debug + Send {
    fn persist(&mut self, entry: &Entry) -> io::Result<()>;
}

/// Discards entries. Use it only where no persistence is wanted, such as tests.
#[derive(Debug, Default, Clone, Copy)]
pub struct NullSink;

impl AuditSink for NullSink {
    fn persist(&mut self, _entry: &Entry) -> io::Result<()> {
        Ok(())
    }
}

/// Appends each entry as one line to a file, and syncs it before returning.
#[derive(Debug)]
pub struct FileSink {
    file: File,
}

impl FileSink {
    /// Open `path` for appending, creating it if needed. Existing lines are kept, so the file
    /// should be loaded with [`AuditLog::from_lines`] before new entries are added.
    pub fn open(path: &Path) -> io::Result<Self> {
        let file = OpenOptions::new().create(true).append(true).open(path)?;
        Ok(Self { file })
    }
}

impl AuditSink for FileSink {
    fn persist(&mut self, entry: &Entry) -> io::Result<()> {
        writeln!(self.file, "{}", entry_line(entry))?;
        self.file.sync_data()
    }
}

/// Why a persisted log could not be loaded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadError {
    /// The line at `line` (1-indexed) is not a valid entry.
    Malformed { line: usize },
    /// Every line parsed, but the chain does not verify.
    Chain(ChainError),
}

impl AuditLog {
    /// The log as text: one entry per line, `seq prev digest event`, with binary fields in hex.
    ///
    /// Lines are written in order, so a log can be appended to a file as it grows and loaded
    /// back with [`AuditLog::from_lines`].
    pub fn to_lines(&self) -> String {
        self.entries
            .iter()
            .map(|e| format!("{}\n", entry_line(e)))
            .collect()
    }

    /// Load a log written by [`AuditLog::to_lines`]. The chain is verified before the log is
    /// returned, so a truncated, reordered or edited file is refused.
    pub fn from_lines(text: &str) -> Result<Self, LoadError> {
        let mut entries = Vec::new();
        for (index, line) in text.lines().enumerate() {
            let entry = parse_entry(line).ok_or(LoadError::Malformed { line: index + 1 })?;
            entries.push(entry);
        }
        verify_chain(&entries).map_err(LoadError::Chain)?;
        Ok(Self { entries })
    }
}

fn entry_line(entry: &Entry) -> String {
    let mut buf = Vec::new();
    entry.event.encode(&mut buf);
    format!(
        "{} {} {} {}",
        entry.seq,
        to_hex(&entry.prev),
        to_hex(&entry.digest),
        bytes_to_hex(&buf)
    )
}

fn parse_entry(line: &str) -> Option<Entry> {
    let mut fields = line.split(' ');
    let seq = fields.next()?.parse().ok()?;
    let prev = hex_to_digest(fields.next()?)?;
    let digest = hex_to_digest(fields.next()?)?;
    let bytes = hex_to_bytes(fields.next()?)?;
    if fields.next().is_some() {
        return None;
    }
    let event = Event::decode(&bytes)?;
    Some(Entry {
        seq,
        prev,
        digest,
        event,
    })
}

fn bytes_to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn hex_to_bytes(hex: &str) -> Option<Vec<u8>> {
    if hex.len() % 2 != 0 || !hex.is_ascii() {
        return None;
    }
    (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).ok())
        .collect()
}

fn hex_to_digest(hex: &str) -> Option<[u8; 32]> {
    hex_to_bytes(hex)?.try_into().ok()
}

/// Reads back what [`Event::encode`] wrote. Any leftover or missing byte is an error.
struct Reader<'a> {
    bytes: &'a [u8],
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        if self.bytes.len() < n {
            return None;
        }
        let (head, tail) = self.bytes.split_at(n);
        self.bytes = tail;
        Some(head)
    }

    fn u8(&mut self) -> Option<u8> {
        Some(self.take(1)?[0])
    }

    fn u64(&mut self) -> Option<u64> {
        Some(u64::from_be_bytes(self.take(8)?.try_into().ok()?))
    }

    fn string(&mut self) -> Option<String> {
        let len = usize::try_from(self.u64()?).ok()?;
        String::from_utf8(self.take(len)?.to_vec()).ok()
    }

    fn finished(&self) -> bool {
        self.bytes.is_empty()
    }
}

impl Event {
    fn decode(bytes: &[u8]) -> Option<Self> {
        let mut r = Reader { bytes };
        let event = match r.u8()? {
            0 => Event::Requested {
                action_id: r.string()?,
                actor: r.string()?,
                resource: r.string()?,
                expected_version: r.u64()?,
            },
            1 => Event::Decided {
                outcome: r.string()?,
                state_version: r.u64()?,
                policy_version: r.u64()?,
                verifier_identity: r.string()?,
                detail: r.string()?,
            },
            2 => Event::Committed {
                new_version: r.u64()?,
                new_load_bits: r.u64()?,
            },
            3 => Event::Refused {
                reason: r.string()?,
            },
            _ => return None,
        };
        r.finished().then_some(event)
    }
}

#[cfg(test)]
mod persistence_tests {
    use super::*;

    fn sample() -> AuditLog {
        let mut log = AuditLog::new();
        log.append(Event::Requested {
            action_id: "tx 1".into(),
            actor: "agent".into(),
            resource: "pool".into(),
            expected_version: 0,
        });
        log.append(Event::Decided {
            outcome: "allow".into(),
            state_version: 0,
            policy_version: 1,
            verifier_identity: "core".into(),
            detail: "new_load=5".into(),
        });
        log.append(Event::Committed {
            new_version: 1,
            new_load_bits: 5.0f64.to_bits(),
        });
        log.append(Event::Refused {
            reason: "denied: []".into(),
        });
        log
    }

    #[test]
    fn a_log_round_trips_through_its_text_form() {
        let log = sample();
        let loaded = AuditLog::from_lines(&log.to_lines()).expect("loads");
        assert_eq!(loaded, log);
        assert_eq!(loaded.head(), log.head());
    }

    #[test]
    fn an_empty_log_round_trips() {
        assert_eq!(AuditLog::from_lines("").expect("loads"), AuditLog::new());
    }

    #[test]
    fn an_edited_line_fails_the_chain() {
        // Flip the last hex digit of the second line's event. The event still decodes, so only
        // the digest check can catch it.
        let text = sample().to_lines();
        let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
        let line = &mut lines[1];
        let last = line.pop().expect("line is not empty");
        line.push(if last == '0' { '1' } else { '0' });
        let text = lines.join("\n");
        assert!(matches!(
            AuditLog::from_lines(&text),
            Err(LoadError::Chain(_))
        ));
    }

    #[test]
    fn a_dropped_middle_line_fails_the_chain() {
        let text = sample().to_lines();
        let mut lines: Vec<&str> = text.lines().collect();
        lines.remove(1);
        let text = lines.join("\n");
        assert!(matches!(
            AuditLog::from_lines(&text),
            Err(LoadError::Chain(_))
        ));
    }

    #[test]
    fn a_truncated_tail_is_a_valid_shorter_log_so_the_head_must_be_anchored() {
        // Dropping the last line leaves a valid chain. Only an external head anchor can catch it.
        let log = sample();
        let text = log.to_lines();
        let mut lines: Vec<&str> = text.lines().collect();
        lines.pop();
        let loaded = AuditLog::from_lines(&lines.join("\n")).expect("still a valid chain");
        assert_ne!(loaded.head(), log.head());
    }

    #[test]
    fn a_garbled_line_is_malformed_with_its_line_number() {
        let mut text = sample().to_lines();
        text.push_str("not an entry\n");
        assert_eq!(
            AuditLog::from_lines(&text),
            Err(LoadError::Malformed { line: 5 })
        );
    }

    #[test]
    fn an_unknown_event_tag_is_malformed() {
        let text = format!("0 {} {} 09\n", to_hex(&[0; 32]), to_hex(&[0; 32]));
        assert_eq!(
            AuditLog::from_lines(&text),
            Err(LoadError::Malformed { line: 1 })
        );
    }
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
