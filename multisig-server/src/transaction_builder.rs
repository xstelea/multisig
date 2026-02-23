use anyhow::{anyhow, Result};
use radix_common::prelude::*;
use radix_transactions::manifest::compiler::compile_manifest;
use radix_transactions::manifest::BlobProvider;
use radix_transactions::prelude::*;
use rand::Rng;

pub struct SubintentResult {
    pub subintent_hash: String,
    pub intent_discriminator: u64,
    pub partial_transaction_bytes: Vec<u8>,
}

/// Build an unsigned subintent from raw manifest text.
///
/// Appends `YIELD_TO_PARENT;` to the manifest if not present,
/// compiles it, wraps in a PartialTransactionV2 with the given
/// epoch window and a cryptographically random discriminator.
pub fn build_unsigned_subintent(
    manifest_text: &str,
    network_id: u8,
    epoch_min: u64,
    epoch_max: u64,
) -> Result<SubintentResult> {
    let mut rng = rand::thread_rng();
    let discriminator: u64 = rng.gen();

    build_unsigned_subintent_with_discriminator(
        manifest_text,
        network_id,
        epoch_min,
        epoch_max,
        discriminator,
    )
}

/// Build an unsigned subintent with a specific discriminator (for testing).
pub fn build_unsigned_subintent_with_discriminator(
    manifest_text: &str,
    network_id: u8,
    epoch_min: u64,
    epoch_max: u64,
    discriminator: u64,
) -> Result<SubintentResult> {
    // Append YIELD_TO_PARENT if not present
    let full_manifest = if manifest_text.contains("YIELD_TO_PARENT") {
        manifest_text.to_string()
    } else {
        format!("{}\nYIELD_TO_PARENT;\n", manifest_text.trim_end())
    };

    let network = match network_id {
        0xf2 => NetworkDefinition::simulator(),
        0x02 => NetworkDefinition::stokenet(),
        0x01 => NetworkDefinition::mainnet(),
        _ => return Err(anyhow::anyhow!("Unsupported network ID: {network_id}")),
    };

    // Compile the manifest string into a SubintentManifestV2
    let manifest: SubintentManifestV2 =
        compile_manifest(&full_manifest, &network, BlobProvider::new())
            .map_err(|e| anyhow::anyhow!("Failed to compile manifest: {e:?}"))?;

    // Build the unsigned partial transaction
    let partial_tx = PartialTransactionV2Builder::new()
        .intent_header(IntentHeaderV2 {
            network_id,
            start_epoch_inclusive: Epoch::of(epoch_min),
            end_epoch_exclusive: Epoch::of(epoch_max),
            intent_discriminator: discriminator,
            min_proposer_timestamp_inclusive: None,
            max_proposer_timestamp_exclusive: None,
        })
        .manifest(manifest)
        .build();

    // Bech32-encode the subintent hash (e.g. "subtxid_...")
    let encoder = TransactionHashBech32Encoder::new(&network);
    let subintent_hash = encoder
        .encode(&partial_tx.root_subintent_hash)
        .map_err(|e| anyhow::anyhow!("Failed to encode subintent hash: {e:?}"))?;

    // Serialize for storage
    let raw = partial_tx
        .to_raw()
        .map_err(|e| anyhow::anyhow!("Failed to serialize: {e:?}"))?;
    let bytes = raw.as_slice().to_vec();

    Ok(SubintentResult {
        subintent_hash,
        intent_discriminator: discriminator,
        partial_transaction_bytes: bytes,
    })
}

/// A stored signature (public key + raw signature bytes) for reconstruction.
pub struct StoredSignature {
    pub public_key_hex: String,
    pub signature_bytes: Vec<u8>,
}

/// Result of composing a main transaction.
pub struct ComposedTransaction {
    /// Hex-encoded notarized transaction for Gateway submission.
    pub notarized_transaction_hex: String,
    /// Bech32-encoded transaction intent hash (e.g. "txid_tdx_2_1...").
    pub intent_hash: String,
}

/// Reconstruct a signed partial transaction from stored unsigned bytes + collected signatures.
///
/// Takes the original unsigned PartialTransactionV2 (serialized at proposal creation)
/// and attaches all collected Ed25519 signatures to produce a properly-signed
/// SignedPartialTransactionV2 for use as a child in the main transaction.
pub fn reconstruct_signed_partial(
    partial_transaction_bytes: &[u8],
    signatures: &[StoredSignature],
) -> Result<SignedPartialTransactionV2> {
    // Deserialize the stored unsigned partial transaction
    let raw = RawSignedPartialTransaction::from_vec(partial_transaction_bytes.to_vec());
    let unsigned = SignedPartialTransactionV2::from_raw(&raw)
        .map_err(|e| anyhow!("Failed to deserialize partial transaction: {e:?}"))?;

    // Build the signature list from stored data
    let intent_signatures: Vec<IntentSignatureV1> = signatures
        .iter()
        .map(|s| {
            let pk_bytes = hex::decode(&s.public_key_hex)
                .map_err(|e| anyhow!("Invalid public key hex: {e}"))?;
            if pk_bytes.len() != Ed25519PublicKey::LENGTH {
                return Err(anyhow!(
                    "Invalid Ed25519 key length: {} (expected {})",
                    pk_bytes.len(),
                    Ed25519PublicKey::LENGTH
                ));
            }
            let mut pk_arr = [0u8; Ed25519PublicKey::LENGTH];
            pk_arr.copy_from_slice(&pk_bytes);

            if s.signature_bytes.len() != Ed25519Signature::LENGTH {
                return Err(anyhow!(
                    "Invalid Ed25519 signature length: {} (expected {})",
                    s.signature_bytes.len(),
                    Ed25519Signature::LENGTH
                ));
            }
            let mut sig_arr = [0u8; Ed25519Signature::LENGTH];
            sig_arr.copy_from_slice(&s.signature_bytes);

            Ok(IntentSignatureV1(SignatureWithPublicKeyV1::Ed25519 {
                public_key: Ed25519PublicKey(pk_arr),
                signature: Ed25519Signature(sig_arr),
            }))
        })
        .collect::<Result<Vec<_>>>()?;

    // Reconstruct with all signatures attached
    Ok(SignedPartialTransactionV2 {
        partial_transaction: unsigned.partial_transaction,
        root_subintent_signatures: IntentSignaturesV2 {
            signatures: intent_signatures,
        },
        non_root_subintent_signatures: unsigned.non_root_subintent_signatures,
    })
}

/// Compose a complete NotarizedTransactionV2 with:
/// - Child "fee_payment": fee payer's signed subintent (lock_fee)
/// - Child "withdrawal": DAO signed subintent (with all collected signatures)
/// - Main intent: yield_to_child("fee_payment") + yield_to_child("withdrawal")
/// - Notarized by server-side ephemeral key
pub fn compose_main_transaction(
    network_id: u8,
    current_epoch: u64,
    fee_signed_partial: SignedPartialTransactionV2,
    withdrawal_signed_partial: SignedPartialTransactionV2,
) -> Result<ComposedTransaction> {
    let mut rng = rand::thread_rng();
    let discriminator: u64 = rng.gen();

    compose_main_transaction_with_discriminator(
        network_id,
        current_epoch,
        fee_signed_partial,
        withdrawal_signed_partial,
        discriminator,
    )
}

/// Compose main transaction with a specific discriminator (for testing).
pub fn compose_main_transaction_with_discriminator(
    network_id: u8,
    current_epoch: u64,
    fee_signed_partial: SignedPartialTransactionV2,
    withdrawal_signed_partial: SignedPartialTransactionV2,
    discriminator: u64,
) -> Result<ComposedTransaction> {
    let network = match network_id {
        0xf2 => NetworkDefinition::simulator(),
        0x02 => NetworkDefinition::stokenet(),
        0x01 => NetworkDefinition::mainnet(),
        _ => return Err(anyhow!("Unsupported network ID: {network_id}")),
    };

    // Generate ephemeral notary key (server-side, not controlling any account)
    let notary_private_key = Ed25519PrivateKey::from_u64(rng_u64()).unwrap();
    let notary_public_key: PublicKey = notary_private_key.public_key().into();

    // Build the main transaction with two child subintents
    let detailed = TransactionV2Builder::new()
        // Children must be added BEFORE manifest_builder
        .add_signed_child("fee_payment", fee_signed_partial)
        .add_signed_child("withdrawal", withdrawal_signed_partial)
        .transaction_header(TransactionHeaderV2 {
            notary_public_key,
            notary_is_signatory: false, // Ephemeral notary doesn't sign for any account
            tip_basis_points: 0,
        })
        .intent_header(IntentHeaderV2 {
            network_id,
            start_epoch_inclusive: Epoch::of(current_epoch),
            end_epoch_exclusive: Epoch::of(current_epoch + 100),
            min_proposer_timestamp_inclusive: None,
            max_proposer_timestamp_exclusive: None,
            intent_discriminator: discriminator,
        })
        .manifest_builder(|builder| {
            builder
                .yield_to_child("fee_payment", ())
                .yield_to_child("withdrawal", ())
        })
        .notarize(&notary_private_key)
        .build_no_validate();

    // Encode the transaction intent hash
    let encoder = TransactionHashBech32Encoder::new(&network);
    let intent_hash = encoder
        .encode(&detailed.transaction_hashes.transaction_intent_hash)
        .map_err(|e| anyhow!("Failed to encode intent hash: {e:?}"))?;

    // Serialize for submission
    let notarized_hex = hex::encode(detailed.raw.as_slice());

    Ok(ComposedTransaction {
        notarized_transaction_hex: notarized_hex,
        intent_hash,
    })
}

/// Generate a random u64 for ephemeral keys. Not cryptographically important
/// since the notary key doesn't control any account.
fn rng_u64() -> u64 {
    let mut rng = rand::thread_rng();
    // Ensure non-zero (Ed25519PrivateKey::from_u64 requires it)
    loop {
        let v: u64 = rng.gen();
        if v != 0 {
            return v;
        }
    }
}

/// Build the fee payment manifest text for the fee payer to sign via sendPreAuthorizationRequest.
pub fn build_fee_manifest(fee_payer_account: &str, lock_fee_amount: &str) -> String {
    format!(
        r#"CALL_METHOD
    Address("{fee_payer_account}")
    "lock_fee"
    Decimal("{lock_fee_amount}")
;
YIELD_TO_PARENT;
"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_NETWORK_ID: u8 = 2; // Stokenet

    fn sample_manifest() -> &'static str {
        r#"CALL_METHOD
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
;"#
    }

    #[test]
    fn builds_unsigned_subintent_from_manifest_text() {
        let result = build_unsigned_subintent_with_discriminator(
            sample_manifest(),
            TEST_NETWORK_ID,
            1000,
            1100,
            42,
        )
        .unwrap();

        assert!(!result.subintent_hash.is_empty());
        assert_eq!(result.intent_discriminator, 42);
        assert!(!result.partial_transaction_bytes.is_empty());
    }

    #[test]
    fn deterministic_with_same_discriminator() {
        let a = build_unsigned_subintent_with_discriminator(
            sample_manifest(),
            TEST_NETWORK_ID,
            1000,
            1100,
            42,
        )
        .unwrap();

        let b = build_unsigned_subintent_with_discriminator(
            sample_manifest(),
            TEST_NETWORK_ID,
            1000,
            1100,
            42,
        )
        .unwrap();

        assert_eq!(a.subintent_hash, b.subintent_hash);
        assert_eq!(a.partial_transaction_bytes, b.partial_transaction_bytes);
    }

    #[test]
    fn different_discriminators_produce_different_hashes() {
        let a = build_unsigned_subintent_with_discriminator(
            sample_manifest(),
            TEST_NETWORK_ID,
            1000,
            1100,
            42,
        )
        .unwrap();

        let b = build_unsigned_subintent_with_discriminator(
            sample_manifest(),
            TEST_NETWORK_ID,
            1000,
            1100,
            43,
        )
        .unwrap();

        assert_ne!(a.subintent_hash, b.subintent_hash);
    }

    #[test]
    fn handles_manifest_with_yield_to_parent() {
        let manifest = format!("{}\nYIELD_TO_PARENT;\n", sample_manifest());
        let result =
            build_unsigned_subintent_with_discriminator(&manifest, TEST_NETWORK_ID, 1000, 1100, 42)
                .unwrap();

        assert!(!result.subintent_hash.is_empty());
    }

    #[test]
    fn rejects_invalid_manifest() {
        let result = build_unsigned_subintent_with_discriminator(
            "THIS IS NOT VALID RTM",
            TEST_NETWORK_ID,
            1000,
            1100,
            42,
        );
        assert!(result.is_err());
    }

    // --- Transaction composition tests ---

    /// Build a signed partial transaction with known keys for testing.
    fn build_test_signed_partial(
        manifest_text: &str,
        signer_keys: &[u64],
        discriminator: u64,
    ) -> SignedPartialTransactionV2 {
        let network = NetworkDefinition::stokenet();
        let full_manifest = if manifest_text.contains("YIELD_TO_PARENT") {
            manifest_text.to_string()
        } else {
            format!("{}\nYIELD_TO_PARENT;\n", manifest_text.trim_end())
        };

        let manifest: SubintentManifestV2 =
            compile_manifest(&full_manifest, &network, BlobProvider::new()).unwrap();

        let mut builder = PartialTransactionV2Builder::new()
            .intent_header(IntentHeaderV2 {
                network_id: TEST_NETWORK_ID,
                start_epoch_inclusive: Epoch::of(1000),
                end_epoch_exclusive: Epoch::of(1100),
                intent_discriminator: discriminator,
                min_proposer_timestamp_inclusive: None,
                max_proposer_timestamp_exclusive: None,
            })
            .manifest(manifest);

        for &key_seed in signer_keys {
            let pk = Ed25519PrivateKey::from_u64(key_seed).unwrap();
            builder = builder.sign(&pk);
        }

        builder.build().partial_transaction
    }

    #[test]
    fn reconstruct_signed_partial_from_stored_data() {
        // Build an unsigned subintent and serialize it
        let subintent = build_unsigned_subintent_with_discriminator(
            sample_manifest(),
            TEST_NETWORK_ID,
            1000,
            1100,
            42,
        )
        .unwrap();

        // Simulate collecting signatures: sign the subintent hash with test keys
        let raw =
            RawSignedPartialTransaction::from_vec(subintent.partial_transaction_bytes.clone());
        let unsigned = SignedPartialTransactionV2::from_raw(&raw).unwrap();
        let prepared = unsigned.prepare(PreparationSettings::latest_ref()).unwrap();
        let subintent_hash = prepared.subintent_hash();

        // Sign with 3 test keys
        let mut stored_sigs = Vec::new();
        for key_seed in [1u64, 2, 3] {
            let pk = Ed25519PrivateKey::from_u64(key_seed).unwrap();
            let sig =
                radix_transactions::signing::Signer::sign_with_public_key(&pk, &subintent_hash);
            match sig {
                SignatureWithPublicKeyV1::Ed25519 {
                    public_key,
                    signature,
                } => {
                    stored_sigs.push(StoredSignature {
                        public_key_hex: hex::encode(public_key.0),
                        signature_bytes: signature.0.to_vec(),
                    });
                }
                _ => panic!("Expected Ed25519"),
            }
        }

        // Reconstruct
        let reconstructed =
            reconstruct_signed_partial(&subintent.partial_transaction_bytes, &stored_sigs).unwrap();

        assert_eq!(reconstructed.root_subintent_signatures.signatures.len(), 3);
    }

    #[test]
    fn compose_main_transaction_produces_valid_output() {
        // Build a fee payment subintent
        let fee_manifest = r#"CALL_METHOD
    Address("account_tdx_2_12xsvygvltz4uhsht6tdrfxktzpmnl77r0d40j8agmujgdj02el3l9v")
    "lock_fee"
    Decimal("10")
;"#;
        let fee_partial = build_test_signed_partial(fee_manifest, &[10], 100);

        // Build a withdrawal subintent with 3 signers
        let withdrawal_partial = build_test_signed_partial(sample_manifest(), &[1, 2, 3], 200);

        // Compose main transaction
        let result = compose_main_transaction_with_discriminator(
            TEST_NETWORK_ID,
            1000,
            fee_partial,
            withdrawal_partial,
            999,
        );

        assert!(
            result.is_ok(),
            "Should compose main transaction: {:?}",
            result.err()
        );

        let composed = result.unwrap();
        assert!(!composed.notarized_transaction_hex.is_empty());
        assert!(composed.intent_hash.starts_with("txid_"));
    }

    #[test]
    fn compose_main_transaction_different_discriminators_produce_different_hashes() {
        let fee_manifest = r#"CALL_METHOD
    Address("account_tdx_2_12xsvygvltz4uhsht6tdrfxktzpmnl77r0d40j8agmujgdj02el3l9v")
    "lock_fee"
    Decimal("10")
;"#;
        let fee1 = build_test_signed_partial(fee_manifest, &[10], 100);
        let fee2 = build_test_signed_partial(fee_manifest, &[10], 101);
        let withdrawal1 = build_test_signed_partial(sample_manifest(), &[1, 2, 3], 200);
        let withdrawal2 = build_test_signed_partial(sample_manifest(), &[1, 2, 3], 201);

        let a = compose_main_transaction_with_discriminator(
            TEST_NETWORK_ID,
            1000,
            fee1,
            withdrawal1,
            111,
        )
        .unwrap();

        let b = compose_main_transaction_with_discriminator(
            TEST_NETWORK_ID,
            1000,
            fee2,
            withdrawal2,
            222,
        )
        .unwrap();

        assert_ne!(a.intent_hash, b.intent_hash);
    }

    #[test]
    fn build_fee_manifest_produces_valid_rtm() {
        let manifest = build_fee_manifest(
            "account_tdx_2_12xsvygvltz4uhsht6tdrfxktzpmnl77r0d40j8agmujgdj02el3l9v",
            "10",
        );

        assert!(manifest.contains("lock_fee"));
        assert!(manifest.contains("YIELD_TO_PARENT"));

        // Should compile as a valid subintent manifest
        let network = NetworkDefinition::stokenet();
        let result: std::result::Result<SubintentManifestV2, _> =
            compile_manifest(&manifest, &network, BlobProvider::new());
        assert!(
            result.is_ok(),
            "Fee manifest should compile: {:?}",
            result.err()
        );
    }

    #[test]
    fn reconstruct_rejects_wrong_key_length() {
        let subintent = build_unsigned_subintent_with_discriminator(
            sample_manifest(),
            TEST_NETWORK_ID,
            1000,
            1100,
            42,
        )
        .unwrap();

        let bad_sig = StoredSignature {
            public_key_hex: "aabbccdd".into(), // Too short
            signature_bytes: vec![0u8; 64],
        };

        let result = reconstruct_signed_partial(&subintent.partial_transaction_bytes, &[bad_sig]);
        assert!(result.is_err());
    }
}
