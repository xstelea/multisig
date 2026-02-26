use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use radix_common::prelude::*;
use radix_transactions::prelude::*;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::gateway::{AccessRuleInfo, SignerInfo};
use crate::proposal_store::{ProposalStatus, ProposalStore};

/// Compute the bech32-encoded root subintent hash from a signed partial transaction hex.
///
/// Deserializes the wallet's signed partial, prepares it (which computes all
/// internal hashes), and returns the root subintent hash in bech32 format
/// (e.g. `subtxid_tdx_2_1...`). Used to verify the wallet signed over the
/// same subintent the server expected.
pub fn compute_subintent_hash_from_signed_partial_hex(
    signed_partial_hex: &str,
    network_id: u8,
) -> Result<String> {
    let bytes = hex::decode(signed_partial_hex).map_err(|e| anyhow!("Invalid hex: {e}"))?;

    let raw = RawSignedPartialTransaction::from_vec(bytes);
    let signed_partial = SignedPartialTransactionV2::from_raw(&raw)
        .map_err(|e| anyhow!("Failed to decode signed partial transaction: {e:?}"))?;

    let prepared = signed_partial
        .prepare(PreparationSettings::latest_ref())
        .map_err(|e| anyhow!("Failed to prepare signed partial transaction: {e:?}"))?;

    let hash = prepared.subintent_hash();

    let network = match network_id {
        0x01 => NetworkDefinition::mainnet(),
        0x02 => NetworkDefinition::stokenet(),
        0xf2 => NetworkDefinition::simulator(),
        _ => return Err(anyhow!("Unsupported network ID: {network_id}")),
    };
    let encoder = TransactionHashBech32Encoder::new(&network);
    let encoded = encoder
        .encode(&hash)
        .map_err(|e| anyhow!("Failed to encode subintent hash: {e:?}"))?;

    Ok(encoded)
}

/// A stored signature from a signer.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Signature {
    pub id: Uuid,
    pub proposal_id: Uuid,
    pub signer_public_key: String,
    pub signer_key_hash: String,
    pub signature_bytes: Vec<u8>,
    pub signed_partial_transaction_hex: String,
    pub created_at: DateTime<Utc>,
    pub is_valid: bool,
}

/// Summary of signature collection progress.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignatureStatus {
    pub proposal_id: Uuid,
    pub signatures: Vec<SignatureSummary>,
    pub threshold: u8,
    pub collected: usize,
    pub remaining: usize,
    pub signers: Vec<SignerStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignatureSummary {
    pub signer_public_key: String,
    pub signer_key_hash: String,
    pub created_at: DateTime<Utc>,
}

/// Per-signer status: have they signed or not, and is their signature still valid?
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignerStatus {
    pub key_hash: String,
    pub key_type: String,
    pub has_signed: bool,
    pub is_valid: bool,
}

pub struct SignatureCollector {
    pool: PgPool,
}

/// Extract signature + public key from a signed partial transaction hex string.
///
/// The wallet's `sendPreAuthorizationRequest` returns a hex-encoded
/// `SignedPartialTransactionV2`. We decode it and pull out the first
/// signature (which includes the Ed25519 public key).
pub fn extract_signature_from_hex(
    signed_partial_hex: &str,
) -> Result<(SignatureWithPublicKeyV1, String)> {
    let bytes = hex::decode(signed_partial_hex).map_err(|e| anyhow!("Invalid hex: {e}"))?;

    let raw = RawSignedPartialTransaction::from_vec(bytes);
    let signed_partial = SignedPartialTransactionV2::from_raw(&raw)
        .map_err(|e| anyhow!("Failed to decode signed partial transaction: {e:?}"))?;

    let signatures = &signed_partial.root_subintent_signatures.signatures;
    if signatures.is_empty() {
        return Err(anyhow!("Signed partial transaction has no signatures"));
    }

    let sig = &signatures[0].0;
    let public_key_hex = match sig {
        SignatureWithPublicKeyV1::Ed25519 { public_key, .. } => hex::encode(public_key.0),
        SignatureWithPublicKeyV1::Secp256k1 { .. } => {
            return Err(anyhow!(
                "Secp256k1 signatures not yet supported (public key recovery needed)"
            ));
        }
    };

    Ok((sig.clone(), public_key_hex))
}

/// Compute the key hash from an Ed25519 public key hex string.
///
/// Returns the hex-encoded hash that matches what the Gateway API returns
/// in the access rule's NonFungibleGlobalId local_id simple_rep.
pub fn compute_key_hash(public_key_hex: &str) -> Result<String> {
    let pk_bytes =
        hex::decode(public_key_hex).map_err(|e| anyhow!("Invalid public key hex: {e}"))?;

    if pk_bytes.len() != Ed25519PublicKey::LENGTH {
        return Err(anyhow!(
            "Invalid Ed25519 public key length: {} (expected {})",
            pk_bytes.len(),
            Ed25519PublicKey::LENGTH
        ));
    }

    let mut arr = [0u8; Ed25519PublicKey::LENGTH];
    arr.copy_from_slice(&pk_bytes);
    let pk = Ed25519PublicKey(arr);
    let hash = pk.get_hash();

    Ok(hex::encode(hash.get_hash_bytes()))
}

/// Check whether a key hash matches any signer in the access rule.
pub fn find_signer_by_hash<'a>(
    access_rule: &'a AccessRuleInfo,
    key_hash: &str,
) -> Option<&'a SignerInfo> {
    access_rule.signers.iter().find(|s| s.key_hash == key_hash)
}

/// Encode signature bytes for storage.
fn encode_signature_bytes(sig: &SignatureWithPublicKeyV1) -> Vec<u8> {
    match sig {
        SignatureWithPublicKeyV1::Ed25519 { signature, .. } => signature.0.to_vec(),
        SignatureWithPublicKeyV1::Secp256k1 { signature } => signature.0.to_vec(),
    }
}

impl SignatureCollector {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Add a signature for a proposal. Returns the updated signature status.
    ///
    /// Validates:
    /// 1. Wallet signed over the correct subintent (hash match)
    /// 2. Signer is in the current access rule
    /// 3. No duplicate signature from the same signer
    /// 4. Proposal is in a valid state (Created or Signing)
    ///
    /// Transitions: Created→Signing on first sig, Signing→Ready when threshold met.
    pub async fn add_signature(
        &self,
        proposal_id: Uuid,
        signed_partial_hex: &str,
        access_rule: &AccessRuleInfo,
        proposal_store: &ProposalStore,
        expected_subintent_hash: &str,
        network_id: u8,
    ) -> Result<SignatureStatus> {
        // Validate the wallet signed over the correct subintent
        let wallet_subintent_hash =
            compute_subintent_hash_from_signed_partial_hex(signed_partial_hex, network_id)?;

        if wallet_subintent_hash != expected_subintent_hash {
            return Err(anyhow!(
                "Wallet produced a different subintent hash (expected {expected_subintent_hash}, \
                 got {wallet_subintent_hash}). Your wallet may not support custom subintent \
                 headers — please update your Radix Wallet."
            ));
        }

        // Extract signature + public key from the wallet's response
        let (sig, public_key_hex) = extract_signature_from_hex(signed_partial_hex)?;
        let key_hash = compute_key_hash(&public_key_hex)?;

        // Validate signer is in the access rule
        if find_signer_by_hash(access_rule, &key_hash).is_none() {
            return Err(anyhow!(
                "Signer with key hash {key_hash} is not in the current access rule"
            ));
        }

        // Check proposal exists and is in valid state
        let proposal = proposal_store
            .get(proposal_id)
            .await?
            .ok_or_else(|| anyhow!("Proposal {proposal_id} not found"))?;

        if proposal.status != ProposalStatus::Created && proposal.status != ProposalStatus::Signing
        {
            return Err(anyhow!(
                "Proposal is in {:?} status; signatures can only be added in Created or Signing",
                proposal.status
            ));
        }

        // Store the signature (UNIQUE constraint prevents duplicates)
        let sig_bytes = encode_signature_bytes(&sig);
        let result = sqlx::query(
            r#"
            INSERT INTO signatures (proposal_id, signer_public_key, signer_key_hash, signature_bytes, signed_partial_transaction_hex)
            VALUES ($1, $2, $3, $4, $5)
            "#,
        )
        .bind(proposal_id)
        .bind(&public_key_hex)
        .bind(&key_hash)
        .bind(&sig_bytes)
        .bind(signed_partial_hex)
        .execute(&self.pool)
        .await;

        match result {
            Ok(_) => {}
            Err(sqlx::Error::Database(db_err)) if db_err.is_unique_violation() => {
                return Err(anyhow!(
                    "Signer {key_hash} has already signed this proposal"
                ));
            }
            Err(e) => return Err(e.into()),
        }

        // Count signatures and handle state transitions
        let sig_count = self.count_signatures(proposal_id).await?;

        // Created → Signing on first signature
        if proposal.status == ProposalStatus::Created {
            proposal_store
                .transition_status(
                    proposal_id,
                    ProposalStatus::Created,
                    ProposalStatus::Signing,
                )
                .await?;
        }

        // Signing → Ready when threshold met
        if sig_count >= access_rule.threshold as i64 {
            proposal_store
                .transition_status(proposal_id, ProposalStatus::Signing, ProposalStatus::Ready)
                .await?;
        }

        self.get_signature_status(proposal_id, access_rule).await
    }

    /// Get the current signature status for a proposal.
    pub async fn get_signature_status(
        &self,
        proposal_id: Uuid,
        access_rule: &AccessRuleInfo,
    ) -> Result<SignatureStatus> {
        let signatures = self.list_signatures(proposal_id).await?;

        // Build a map of key_hash → (has_signed, is_valid)
        let sig_map: std::collections::HashMap<&str, bool> = signatures
            .iter()
            .map(|s| (s.signer_key_hash.as_str(), s.is_valid))
            .collect();

        let signers: Vec<SignerStatus> = access_rule
            .signers
            .iter()
            .map(|s| {
                let (has_signed, is_valid) = match sig_map.get(s.key_hash.as_str()) {
                    Some(&valid) => (true, valid),
                    None => (false, true), // Not signed yet, validity N/A
                };
                SignerStatus {
                    key_hash: s.key_hash.clone(),
                    key_type: s.key_type.clone(),
                    has_signed,
                    is_valid,
                }
            })
            .collect();

        let collected = signatures.len();
        let threshold = access_rule.threshold as usize;
        let remaining = threshold.saturating_sub(collected);

        Ok(SignatureStatus {
            proposal_id,
            signatures: signatures
                .into_iter()
                .map(|s| SignatureSummary {
                    signer_public_key: s.signer_public_key,
                    signer_key_hash: s.signer_key_hash,
                    created_at: s.created_at,
                })
                .collect(),
            threshold: access_rule.threshold,
            collected,
            remaining,
            signers,
        })
    }

    /// Get raw signature data for transaction reconstruction.
    ///
    /// Returns (public_key_hex, signature_bytes) pairs for all signatures on a proposal.
    pub async fn get_raw_signatures(&self, proposal_id: Uuid) -> Result<Vec<(String, Vec<u8>)>> {
        let rows: Vec<(String, Vec<u8>)> = sqlx::query_as(
            "SELECT signer_public_key, signature_bytes FROM signatures WHERE proposal_id = $1 ORDER BY created_at ASC",
        )
        .bind(proposal_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }

    async fn list_signatures(&self, proposal_id: Uuid) -> Result<Vec<Signature>> {
        let rows = sqlx::query_as::<_, Signature>(
            r#"
            SELECT id, proposal_id, signer_public_key, signer_key_hash, signature_bytes,
                   signed_partial_transaction_hex, created_at, is_valid
            FROM signatures
            WHERE proposal_id = $1
            ORDER BY created_at ASC
            "#,
        )
        .bind(proposal_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }

    async fn count_signatures(&self, proposal_id: Uuid) -> Result<i64> {
        let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM signatures WHERE proposal_id = $1")
            .bind(proposal_id)
            .fetch_one(&self.pool)
            .await?;
        Ok(row.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    /// Build a signed partial transaction hex for testing.
    /// Uses the Radix SDK to create a real signed partial, mimicking what the wallet returns.
    fn build_test_signed_partial(
        signer_private_key: &Ed25519PrivateKey,
    ) -> (String, Ed25519PublicKey) {
        let public_key = signer_private_key.public_key();

        let manifest_text = r#"CALL_METHOD
    Address("account_tdx_2_1cx3u3xgr9anc9fk54dxzsz6k2n6lnadludkx4mx5re5erl8jt9lpnp")
    "withdraw"
    Address("resource_tdx_2_1tknxxxxxxxxxradxrdxxxxxxxxx009923554798xxxxxxxxxtfd2jc")
    Decimal("100")
;
TAKE_ALL_FROM_WORKTOP
    Address("resource_tdx_2_1tknxxxxxxxxxradxrdxxxxxxxxx009923554798xxxxxxxxxtfd2jc")
    Bucket("xrd_bucket")
;
CALL_METHOD
    Address("account_tdx_2_12xsvygvltz4uhsht6tdrfxktzpmnl77r0d40j8agmujgdj02el3l9v")
    "deposit"
    Bucket("xrd_bucket")
;
YIELD_TO_PARENT;
"#;

        let network = NetworkDefinition::stokenet();
        let manifest: SubintentManifestV2 =
            radix_transactions::manifest::compiler::compile_manifest(
                manifest_text,
                &network,
                radix_transactions::manifest::BlobProvider::new(),
            )
            .unwrap();

        let detailed = PartialTransactionV2Builder::new()
            .intent_header(IntentHeaderV2 {
                network_id: 0x02,
                start_epoch_inclusive: Epoch::of(1000),
                end_epoch_exclusive: Epoch::of(1100),
                intent_discriminator: 12345,
                min_proposer_timestamp_inclusive: None,
                max_proposer_timestamp_exclusive: None,
            })
            .manifest(manifest)
            .sign(signer_private_key)
            .build();

        let raw = detailed.partial_transaction.to_raw().unwrap();
        let hex_str = hex::encode(raw.as_slice());

        (hex_str, public_key)
    }

    #[test]
    fn compute_ed25519_key_hash_produces_29_byte_hex() {
        let pk_hex = hex::encode([1u8; Ed25519PublicKey::LENGTH]);
        let hash = compute_key_hash(&pk_hex).unwrap();
        // 29 bytes = 58 hex chars
        assert_eq!(hash.len(), 58, "Key hash should be 58 hex chars (29 bytes)");
    }

    #[test]
    fn compute_key_hash_deterministic() {
        let pk_hex = hex::encode([42u8; Ed25519PublicKey::LENGTH]);
        let a = compute_key_hash(&pk_hex).unwrap();
        let b = compute_key_hash(&pk_hex).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn compute_key_hash_rejects_wrong_length() {
        let short_hex = hex::encode([1u8; 16]);
        assert!(compute_key_hash(&short_hex).is_err());
    }

    #[test]
    fn find_signer_by_hash_finds_match() {
        let access_rule = AccessRuleInfo {
            signers: vec![SignerInfo {
                key_hash: "aabbccdd".into(),
                key_type: "EddsaEd25519".into(),
                badge_resource: "resource_test".into(),
                badge_local_id: "[aabbccdd]".into(),
            }],
            threshold: 1,
        };
        assert!(find_signer_by_hash(&access_rule, "aabbccdd").is_some());
        assert!(find_signer_by_hash(&access_rule, "00000000").is_none());
    }

    #[test]
    fn extract_signature_rejects_invalid_hex() {
        assert!(extract_signature_from_hex("not-hex!").is_err());
    }

    #[test]
    fn extract_signature_rejects_invalid_payload() {
        assert!(extract_signature_from_hex("deadbeef").is_err());
    }

    #[test]
    fn extract_signature_from_real_signed_partial() {
        let private_key = Ed25519PrivateKey::from_u64(1).unwrap();
        let (hex_str, expected_pk) = build_test_signed_partial(&private_key);

        let (sig, pk_hex) = extract_signature_from_hex(&hex_str).unwrap();

        // Verify we got an Ed25519 signature
        assert!(matches!(sig, SignatureWithPublicKeyV1::Ed25519 { .. }));

        // Verify the public key matches
        assert_eq!(pk_hex, hex::encode(expected_pk.0));
    }

    #[test]
    fn extracted_public_key_hash_matches_computed() {
        let private_key = Ed25519PrivateKey::from_u64(42).unwrap();
        let (hex_str, expected_pk) = build_test_signed_partial(&private_key);

        let (_sig, pk_hex) = extract_signature_from_hex(&hex_str).unwrap();
        let hash = compute_key_hash(&pk_hex).unwrap();

        // Verify hash matches what Radix SDK computes
        let expected_hash = expected_pk.get_hash();
        let expected_hash_hex = hex::encode(expected_hash.get_hash_bytes());
        assert_eq!(hash, expected_hash_hex);
    }

    #[test]
    fn signer_validation_accepts_valid_signer() {
        let private_key = Ed25519PrivateKey::from_u64(7).unwrap();
        let public_key = private_key.public_key();
        let key_hash = hex::encode(public_key.get_hash().get_hash_bytes());

        let access_rule = AccessRuleInfo {
            signers: vec![SignerInfo {
                key_hash: key_hash.clone(),
                key_type: "EddsaEd25519".into(),
                badge_resource: "resource_test".into(),
                badge_local_id: format!("[{key_hash}]"),
            }],
            threshold: 1,
        };

        let (hex_str, _) = build_test_signed_partial(&private_key);
        let (_sig, pk_hex) = extract_signature_from_hex(&hex_str).unwrap();
        let computed_hash = compute_key_hash(&pk_hex).unwrap();

        assert!(find_signer_by_hash(&access_rule, &computed_hash).is_some());
    }

    #[test]
    fn signer_validation_rejects_unknown_signer() {
        let private_key = Ed25519PrivateKey::from_u64(99).unwrap();

        // Access rule has a different signer
        let access_rule = AccessRuleInfo {
            signers: vec![SignerInfo {
                key_hash: "0000000000000000000000000000000000000000000000000000000000".into(),
                key_type: "EddsaEd25519".into(),
                badge_resource: "resource_test".into(),
                badge_local_id: "[0000...]".into(),
            }],
            threshold: 1,
        };

        let (hex_str, _) = build_test_signed_partial(&private_key);
        let (_sig, pk_hex) = extract_signature_from_hex(&hex_str).unwrap();
        let computed_hash = compute_key_hash(&pk_hex).unwrap();

        assert!(find_signer_by_hash(&access_rule, &computed_hash).is_none());
    }

    #[test]
    fn compute_subintent_hash_from_signed_partial_matches_expected() {
        let private_key = Ed25519PrivateKey::from_u64(1).unwrap();
        let (hex_str, _) = build_test_signed_partial(&private_key);

        let hash = compute_subintent_hash_from_signed_partial_hex(&hex_str, 0x02).unwrap();

        // Should produce a bech32-encoded subintent hash
        assert!(
            hash.starts_with("subtxid_"),
            "Subintent hash should be bech32-encoded, got: {hash}"
        );
    }

    #[test]
    fn compute_subintent_hash_deterministic() {
        let private_key = Ed25519PrivateKey::from_u64(1).unwrap();
        let (hex_str, _) = build_test_signed_partial(&private_key);

        let a = compute_subintent_hash_from_signed_partial_hex(&hex_str, 0x02).unwrap();
        let b = compute_subintent_hash_from_signed_partial_hex(&hex_str, 0x02).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn different_headers_produce_different_subintent_hashes() {
        let private_key = Ed25519PrivateKey::from_u64(1).unwrap();

        // Build two signed partials with different discriminators
        let manifest_text = r#"CALL_METHOD
    Address("account_tdx_2_1cx3u3xgr9anc9fk54dxzsz6k2n6lnadludkx4mx5re5erl8jt9lpnp")
    "withdraw"
    Address("resource_tdx_2_1tknxxxxxxxxxradxrdxxxxxxxxx009923554798xxxxxxxxxtfd2jc")
    Decimal("100")
;
TAKE_ALL_FROM_WORKTOP
    Address("resource_tdx_2_1tknxxxxxxxxxradxrdxxxxxxxxx009923554798xxxxxxxxxtfd2jc")
    Bucket("xrd_bucket")
;
CALL_METHOD
    Address("account_tdx_2_12xsvygvltz4uhsht6tdrfxktzpmnl77r0d40j8agmujgdj02el3l9v")
    "deposit"
    Bucket("xrd_bucket")
;
YIELD_TO_PARENT;
"#;
        let network = NetworkDefinition::stokenet();
        let manifest: SubintentManifestV2 =
            radix_transactions::manifest::compiler::compile_manifest(
                manifest_text,
                &network,
                radix_transactions::manifest::BlobProvider::new(),
            )
            .unwrap();

        // Discriminator 11111
        let detailed_a = PartialTransactionV2Builder::new()
            .intent_header(IntentHeaderV2 {
                network_id: 0x02,
                start_epoch_inclusive: Epoch::of(1000),
                end_epoch_exclusive: Epoch::of(1100),
                intent_discriminator: 11111,
                min_proposer_timestamp_inclusive: None,
                max_proposer_timestamp_exclusive: None,
            })
            .manifest(manifest.clone())
            .sign(&private_key)
            .build();
        let hex_a = hex::encode(detailed_a.partial_transaction.to_raw().unwrap().as_slice());

        // Discriminator 22222
        let detailed_b = PartialTransactionV2Builder::new()
            .intent_header(IntentHeaderV2 {
                network_id: 0x02,
                start_epoch_inclusive: Epoch::of(1000),
                end_epoch_exclusive: Epoch::of(1100),
                intent_discriminator: 22222,
                min_proposer_timestamp_inclusive: None,
                max_proposer_timestamp_exclusive: None,
            })
            .manifest(manifest)
            .sign(&private_key)
            .build();
        let hex_b = hex::encode(detailed_b.partial_transaction.to_raw().unwrap().as_slice());

        let hash_a = compute_subintent_hash_from_signed_partial_hex(&hex_a, 0x02).unwrap();
        let hash_b = compute_subintent_hash_from_signed_partial_hex(&hex_b, 0x02).unwrap();

        assert_ne!(
            hash_a, hash_b,
            "Different discriminators should produce different subintent hashes"
        );
    }
}
