# tpt-norma-proof

A proof-certificate format for `tpt-norma`. A certificate states a claim, binds it to the exact
source it describes by SHA-256 digest, and lists the evidence the claim rests on and any mapping
to an external framework.

Because the certificate is pinned to a digest, it goes stale as soon as its source changes.

> **Status:** `0.1.0`, unpublished (`publish = false`). The format is internal, and the only
> certificate this crate produces is an **illustrative draft**.

## Important: the draft is not a compliance claim

[`validator_draft`] builds a certificate for `validate_action` with an EU AI Act Article 15
mapping. That mapping is marked `Open`, and the certificate label says it is **not a compliance
mapping**. Spec §7 forbids presenting it as a completed Article 15 mapping. Do not cite it as
evidence of compliance.

## Usage

```toml
[dependencies]
tpt-norma-proof = { path = "../tpt-norma-proof" }
```

```rust
use tpt_norma_proof::validator_draft;

let cert = validator_draft();
let source = include_str!("../../tpt-norma-core/src/action_validator.rs");

// The draft is pinned to the validator's current source.
assert!(cert.is_current_for(source));

// Any change to the source makes it stale.
assert!(!cert.is_current_for(&format!("{source}\n// edited")));

println!("{}", cert.render());
```

To build a certificate for something else, fill in the public fields directly:

```rust
use tpt_norma_proof::{digest_text, Certificate};

let source = "fn main() {}";
let cert = Certificate {
    label: "Example".into(),
    claim: "The program does nothing.".into(),
    artifact: "main.rs".into(),
    source_digest: digest_text(source),
    evidence: vec![],
    mappings: vec![],
};
assert!(cert.is_current_for(source));
```

## Types

| Type | Role |
| :--- | :--- |
| `Certificate` | The claim, its artifact, the pinned digest, the evidence and the mappings |
| `Evidence` | One item the claim rests on, with a `Status` |
| `Mapping` | A claim mapped to a framework reference, with a `Status` |
| `Status` | `Evidenced` (backed by evidence in the certificate) or `Open` (stated, with no evidence yet) |

## Functions

| Function | Purpose |
| :--- | :--- |
| `digest_text(&str)` | Lower-case hex SHA-256 of a string |
| `Certificate::is_current_for(&str)` | `false` if the source has changed since the certificate was made |
| `Certificate::render()` | Plain text, for a file or a review |
| `validator_draft()` | The illustrative certificate for `validate_action` |

The validator source is pinned at compile time with `include_str!`, so the draft certificate's
digest is taken from the file in `tpt-norma-core`.

## Limits

- A digest proves that the source text is unchanged. It does not prove that the claim is true.
- The evidence list records what this repository tests. It does not say that the tests are
  sufficient for any claim.
- Certificates are not signed.

## Testing

```sh
cargo test -p tpt-norma-proof
```

Unit tests live in `src/lib.rs`.

## Related

- The certified source: [`tpt-norma-core`](../tpt-norma-core/)
- Spec §7 (what may be claimed) and Exit Criterion 3: [`spec.txt`](../../spec.txt)
- Design decisions: [`docs/decisions.md`](../../docs/decisions.md)

## License

MIT OR Apache-2.0. See the workspace [README](../../README.md#license).
