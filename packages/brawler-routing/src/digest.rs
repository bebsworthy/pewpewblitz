//! SHA-256 helpers for the canonical M01 records.

use sha2::{Digest, Sha256};

/// Domain separator used for v1 worker manifests.
pub const MANIFEST_DIGEST_DOMAIN: &[u8] = b"BRAWLER-MANIFEST-V1";
/// Domain separator used for v1 match results.
pub const RESULT_DIGEST_DOMAIN: &[u8] = b"BRAWLER-RESULT-V1";

/// Compute the v1 manifest digest over the domain separator and the canonical bytes preceding
/// the digest field.
#[must_use]
pub fn manifest_digest(canonical_prefix: &[u8]) -> [u8; 32] {
    digest(MANIFEST_DIGEST_DOMAIN, canonical_prefix)
}

/// Compute the v1 result digest over the domain separator and canonical result bytes.
#[must_use]
pub fn result_digest(canonical_result: &[u8]) -> [u8; 32] {
    digest(RESULT_DIGEST_DOMAIN, canonical_result)
}

fn digest(domain: &[u8], bytes: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update(bytes);
    hasher.finalize().into()
}
