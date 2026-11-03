// Copyright (c) TPT Solutions
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Proof-certificate format for `tpt-norma` (spec §4 `tpt-norma-proof`, Exit Criterion 3).
//!
//! A certificate binds a claim to the exact source it describes, by SHA-256 digest, so a
//! certificate goes stale when the source changes. It lists the evidence the claim rests on,
//! and any mapping to a regulatory framework.
//!
//! The certificate produced by [`validator_draft`] is illustrative. Spec §7 forbids presenting
//! it as a completed EU AI Act Article 15 mapping, and the certificate says so in its label.

use sha2::{Digest, Sha256};

/// Source of the validator, pinned into the certificate.
const VALIDATOR_SOURCE: &str = include_str!("../../tpt-norma-core/src/action_validator.rs");

/// The status a mapping or evidence item can have.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// Backed by evidence in this certificate.
    Evidenced,
    /// Stated, with no evidence yet.
    Open,
}

/// One item of evidence the claim rests on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Evidence {
    pub description: String,
    pub status: Status,
}

/// A mapping of a claim to an external framework's requirement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mapping {
    pub framework: String,
    pub reference: String,
    pub status: Status,
}

/// A proof certificate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Certificate {
    pub label: String,
    pub claim: String,
    pub artifact: String,
    /// Lower-case hex SHA-256 of the artifact's source at the time it was certified.
    pub source_digest: String,
    pub evidence: Vec<Evidence>,
    pub mappings: Vec<Mapping>,
}

/// Lower-case hex SHA-256 of `text`.
pub fn digest_text(text: &str) -> String {
    Sha256::digest(text.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

impl Certificate {
    /// Check the certificate against the source it claims to describe. Returns `false` if the
    /// source has changed since the certificate was made.
    pub fn is_current_for(&self, source: &str) -> bool {
        self.source_digest == digest_text(source)
    }

    /// Render the certificate as plain text, suitable for a file or a review.
    pub fn render(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("{}\n\n", self.label));
        out.push_str(&format!("Claim:    {}\n", self.claim));
        out.push_str(&format!("Artifact: {}\n", self.artifact));
        out.push_str(&format!("Source SHA-256: {}\n\n", self.source_digest));
        out.push_str("Evidence:\n");
        for item in &self.evidence {
            out.push_str(&format!(
                "  [{}] {}\n",
                status_text(item.status),
                item.description
            ));
        }
        out.push_str("\nMappings:\n");
        for mapping in &self.mappings {
            out.push_str(&format!(
                "  [{}] {} {}\n",
                status_text(mapping.status),
                mapping.framework,
                mapping.reference
            ));
        }
        out
    }
}

fn status_text(status: Status) -> &'static str {
    match status {
        Status::Evidenced => "evidenced",
        Status::Open => "open",
    }
}

/// The first-draft certificate for `validate_action`, mapped illustratively to EU AI Act
/// Article 15 (spec §9, Exit Criterion 3).
///
/// The evidence list records what this repository does test. The Article 15 mapping is marked
/// `Open` and is not a compliance claim.
pub fn validator_draft() -> Certificate {
    Certificate {
        label:
            "ILLUSTRATIVE DRAFT. NOT A COMPLIANCE MAPPING. Spec §7 forbids presenting this as a \
                completed EU AI Act Article 15 mapping."
                .into(),
        claim: "validate_action admits an action only when its delta is non-negative, within the \
                per-call rate limit, and keeps the projected load within capacity minus the \
                f64 rounding margin."
            .into(),
        artifact: "tpt-norma-core::validate_action".into(),
        source_digest: digest_text(VALIDATOR_SOURCE),
        evidence: vec![
            Evidence {
                description: "unit tests: capacity boundary, epsilon overshoot, negative delta, \
                              rate limit, zero bounds, non-finite inputs"
                    .into(),
                status: Status::Evidenced,
            },
            Evidence {
                description:
                    "property tests: f64 realization never allows what the exact-rational \
                              reference denies, 4096 cases per property (sampled, not exhaustive)"
                        .into(),
                status: Status::Evidenced,
            },
            Evidence {
                description: "machine-checked ideal-layer proof (.telos twin)".into(),
                status: Status::Open,
            },
            Evidence {
                description: "interim written argument for conservative denial".into(),
                status: Status::Evidenced,
            },
        ],
        mappings: vec![Mapping {
            framework: "EU AI Act".into(),
            reference: "Article 15 (accuracy and robustness)".into(),
            status: Status::Open,
        }],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_draft_is_labelled_and_pinned_to_the_current_source() {
        let cert = validator_draft();
        assert!(cert.label.starts_with("ILLUSTRATIVE DRAFT"));
        assert!(cert.is_current_for(VALIDATOR_SOURCE));
    }

    #[test]
    fn a_changed_source_makes_the_certificate_stale() {
        let cert = validator_draft();
        assert!(!cert.is_current_for("pub fn validate_action() {}"));
    }

    #[test]
    fn the_regulatory_mapping_stays_open() {
        assert!(validator_draft()
            .mappings
            .iter()
            .all(|m| m.status == Status::Open));
    }
}
