//! Sub-intent creation for DAO treasury withdrawals with multisig authorization.
//!
//! This module provides functionality to create and sign sub-intents that withdraw
//! XRD from a DAO treasury account protected by 3-of-4 multisig access rules.
//!
//! Sub-intents are partial transactions that:
//! - Execute as part of a larger transaction
//! - Use `YIELD_TO_PARENT` to return control to the parent transaction
//! - Require signing with the subintent hash (not the transaction intent hash)

use anyhow::Result;
use radix_common::prelude::*;
use radix_transactions::prelude::*;
// Import the Signer trait from radix_transactions for signing functionality
use radix_transactions::signing::Signer as SignerTrait;

use crate::accounts::STOKENET_NETWORK_ID;
use crate::keys::Signer;

/// Configuration for creating a withdrawal sub-intent.
#[derive(Debug)]
pub struct WithdrawalSubintentConfig {
    /// The DAO treasury account to withdraw from (must have multisig access rule).
    pub treasury_account: ComponentAddress,
    /// The recipient account to receive the withdrawn XRD.
    pub recipient_account: ComponentAddress,
    /// The amount of XRD to withdraw.
    pub amount: Decimal,
    /// Network ID (e.g., 2 for Stokenet).
    pub network_id: u8,
    /// Epoch range for sub-intent validity.
    pub start_epoch: u64,
    pub end_epoch: u64,
    /// Intent discriminator (nonce) for uniqueness.
    pub intent_discriminator: u64,
}

impl WithdrawalSubintentConfig {
    /// Create a default withdrawal config for Stokenet.
    ///
    /// # Errors
    /// Returns an error if `amount` is zero or negative.
    pub fn new(
        treasury_account: ComponentAddress,
        recipient_account: ComponentAddress,
        amount: Decimal,
        current_epoch: u64,
    ) -> Result<Self> {
        if amount <= Decimal::ZERO {
            return Err(anyhow::anyhow!(
                "Withdrawal amount must be positive, got: {}",
                amount
            ));
        }

        Ok(Self {
            treasury_account,
            recipient_account,
            amount,
            network_id: STOKENET_NETWORK_ID,
            start_epoch: current_epoch,
            end_epoch: current_epoch + 100, // Valid for 100 epochs
            intent_discriminator: rand_intent_discriminator(),
        })
    }
}

/// Generate a random intent discriminator.
///
/// WARNING: COLLISION RISK - This uses SystemTime nanoseconds which is NOT cryptographically
/// random. Two calls within the same nanosecond will produce identical discriminators.
/// This is acceptable for POC/testing but MUST be replaced with a proper random source
/// (e.g., `rand::random::<u64>()`) for production use.
fn rand_intent_discriminator() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(1)
}

/// Build a sub-intent manifest for withdrawing XRD from a DAO treasury.
///
/// The manifest will:
/// 1. Withdraw the specified amount of XRD from the treasury account
/// 2. Take the withdrawn XRD from the worktop
/// 3. Deposit it to the recipient account
/// 4. Yield to parent to return control
///
/// Note: The withdraw_from_account call will require the treasury's access rule
/// (3-of-4 signatures) to be satisfied by the signatures on this sub-intent.
pub fn build_withdrawal_subintent_manifest(
    config: &WithdrawalSubintentConfig,
) -> SubintentManifestV2 {
    ManifestBuilder::new_subintent_v2()
        // Withdraw XRD from the treasury (requires multisig authorization)
        .withdraw_from_account(config.treasury_account, XRD, config.amount)
        // Take all withdrawn XRD from worktop into a bucket
        .take_all_from_worktop(XRD, "xrd_bucket")
        // Deposit to recipient account
        .try_deposit_or_abort(config.recipient_account, None, "xrd_bucket")
        // Return control to the parent transaction
        .yield_to_parent(())
        .build()
}

/// Result of building and preparing a withdrawal sub-intent.
pub struct PreparedWithdrawalSubintent {
    /// The signed partial transaction (contains the sub-intent and signatures).
    pub signed_partial: SignedPartialTransactionV2,
    /// The sub-intent hash (used for signing and display).
    pub subintent_hash: SubintentHash,
    /// The raw bytes of the signed partial transaction.
    pub raw: RawSignedPartialTransaction,
}

impl PreparedWithdrawalSubintent {
    /// Get the sub-intent hash as a hex string.
    pub fn hash_hex(&self) -> String {
        hex::encode(self.subintent_hash.as_hash().as_slice())
    }

    /// Get the raw bytes as a hex string for transmission/storage.
    pub fn to_hex(&self) -> String {
        hex::encode(self.raw.as_slice())
    }
}

/// Build a withdrawal sub-intent with no signatures yet.
///
/// This creates an unsigned `PartialTransactionV2` that can be passed around
/// for signature collection. Use `sign_subintent` to add signatures.
pub fn build_unsigned_withdrawal_subintent(
    config: &WithdrawalSubintentConfig,
) -> Result<(SignedPartialTransactionV2, SubintentHash)> {
    let manifest = build_withdrawal_subintent_manifest(config);

    // Build the partial transaction
    let detailed = PartialTransactionV2Builder::new()
        .intent_header(IntentHeaderV2 {
            network_id: config.network_id,
            start_epoch_inclusive: Epoch::of(config.start_epoch),
            end_epoch_exclusive: Epoch::of(config.end_epoch),
            min_proposer_timestamp_inclusive: None,
            max_proposer_timestamp_exclusive: None,
            intent_discriminator: config.intent_discriminator,
        })
        .manifest(manifest)
        // Build without signing - signatures will be added separately
        .build();

    Ok((detailed.partial_transaction, detailed.root_subintent_hash))
}

/// Build a withdrawal sub-intent and sign it with the provided signers.
///
/// For a 3-of-4 multisig treasury, you need at least 3 signers.
pub fn build_signed_withdrawal_subintent(
    config: &WithdrawalSubintentConfig,
    signers: &[&Signer],
) -> Result<PreparedWithdrawalSubintent> {
    let manifest = build_withdrawal_subintent_manifest(config);

    // Build and sign the partial transaction
    let mut builder = PartialTransactionV2Builder::new()
        .intent_header(IntentHeaderV2 {
            network_id: config.network_id,
            start_epoch_inclusive: Epoch::of(config.start_epoch),
            end_epoch_exclusive: Epoch::of(config.end_epoch),
            min_proposer_timestamp_inclusive: None,
            max_proposer_timestamp_exclusive: None,
            intent_discriminator: config.intent_discriminator,
        })
        .manifest(manifest);

    // Sign with each signer
    for signer in signers {
        builder = builder.sign(&signer.private_key);
    }

    let detailed = builder.build();

    let raw = detailed
        .partial_transaction
        .to_raw()
        .map_err(|e| anyhow::anyhow!("Failed to encode partial transaction: {:?}", e))?;

    Ok(PreparedWithdrawalSubintent {
        signed_partial: detailed.partial_transaction,
        subintent_hash: detailed.root_subintent_hash,
        raw,
    })
}

/// Sign a sub-intent hash with a single signer.
///
/// This is useful for distributed signing scenarios where each signer
/// independently signs the sub-intent hash.
pub fn sign_subintent_hash(
    subintent_hash: &SubintentHash,
    signer: &Signer,
) -> SignatureWithPublicKeyV1 {
    SignerTrait::sign_with_public_key(&signer.private_key, subintent_hash)
}

/// Add a signature to an existing signed partial transaction.
///
/// This creates a new `SignedPartialTransactionV2` with the additional signature.
/// Use this for collecting signatures from multiple parties.
pub fn add_signature_to_partial(
    partial: SignedPartialTransactionV2,
    signature: SignatureWithPublicKeyV1,
) -> SignedPartialTransactionV2 {
    let mut signatures = partial.root_subintent_signatures.signatures;
    signatures.push(IntentSignatureV1(signature));

    SignedPartialTransactionV2 {
        partial_transaction: partial.partial_transaction,
        root_subintent_signatures: IntentSignaturesV2 { signatures },
        non_root_subintent_signatures: partial.non_root_subintent_signatures,
    }
}

/// Get the subintent hash from a signed partial transaction.
pub fn get_subintent_hash(
    partial: &SignedPartialTransactionV2,
) -> Result<SubintentHash> {
    let prepared = partial
        .prepare(PreparationSettings::latest_ref())
        .map_err(|e| anyhow::anyhow!("Failed to prepare partial transaction: {:?}", e))?;
    Ok(prepared.subintent_hash())
}

/// Encode a SubintentHash for human-readable display.
///
/// Returns a string like "subintent:<hex>".
/// The format includes the network context for clarity.
pub fn format_subintent_hash(hash: &SubintentHash, network_id: u8) -> String {
    // Determine network name for context
    let network_name = match network_id {
        1 => "mainnet",
        2 => "stokenet",
        242 => "simulator",
        _ => "unknown",
    };
    format!(
        "subintent[{}]:{}",
        network_name,
        hex::encode(hash.as_hash().as_slice())
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keys::MultisigSigners;
    use radix_engine_interface::prelude::dec;

    // Create a dummy account address for testing
    fn test_account(seed: u8) -> ComponentAddress {
        ComponentAddress::preallocated_account_from_public_key(
            &Ed25519PublicKey([seed; Ed25519PublicKey::LENGTH]),
        )
    }

    #[test]
    fn test_build_withdrawal_manifest() {
        let treasury = test_account(1);
        let recipient = test_account(2);

        let config = WithdrawalSubintentConfig {
            treasury_account: treasury,
            recipient_account: recipient,
            amount: dec!(100),
            network_id: STOKENET_NETWORK_ID,
            start_epoch: 100,
            end_epoch: 200,
            intent_discriminator: 42,
        };

        let manifest = build_withdrawal_subintent_manifest(&config);

        // Manifest should have instructions
        assert!(
            !manifest.instructions.is_empty(),
            "Manifest should have instructions"
        );

        // Should be a subintent manifest (no lock_fee, ends with yield_to_parent)
        let last_instruction = manifest.instructions.last();
        assert!(last_instruction.is_some(), "Should have at least one instruction");

        // Assert the last instruction is YieldToParent
        assert!(
            matches!(last_instruction.unwrap(), InstructionV2::YieldToParent(_)),
            "Last instruction should be YieldToParent"
        );
    }

    #[test]
    fn test_build_unsigned_subintent() {
        let treasury = test_account(1);
        let recipient = test_account(2);

        let config = WithdrawalSubintentConfig {
            treasury_account: treasury,
            recipient_account: recipient,
            amount: dec!(50),
            network_id: STOKENET_NETWORK_ID,
            start_epoch: 100,
            end_epoch: 200,
            intent_discriminator: 123,
        };

        let result = build_unsigned_withdrawal_subintent(&config);
        assert!(result.is_ok(), "Should build unsigned subintent");

        let (partial, hash) = result.unwrap();

        // Should have no signatures yet
        assert!(
            partial.root_subintent_signatures.signatures.is_empty(),
            "Unsigned subintent should have no signatures"
        );

        // Hash should be valid
        assert!(!hash.as_hash().as_slice().is_empty(), "Hash should not be empty");
    }

    #[test]
    fn test_build_signed_subintent_with_3_signers() {
        let signers = MultisigSigners::new_test_set().unwrap();
        let treasury = test_account(1);
        let recipient = test_account(2);

        let config = WithdrawalSubintentConfig {
            treasury_account: treasury,
            recipient_account: recipient,
            amount: dec!(100),
            network_id: STOKENET_NETWORK_ID,
            start_epoch: 100,
            end_epoch: 200,
            intent_discriminator: 999,
        };

        // Get 3 of 4 signers
        let three_signers: Vec<&Signer> = signers.take_signers(3);

        let result = build_signed_withdrawal_subintent(&config, &three_signers);
        assert!(result.is_ok(), "Should build signed subintent: {:?}", result.err());

        let prepared = result.unwrap();

        // Should have 3 signatures
        assert_eq!(
            prepared.signed_partial.root_subintent_signatures.signatures.len(),
            3,
            "Should have 3 signatures"
        );

        // Hash hex should be valid
        let hash_hex = prepared.hash_hex();
        assert_eq!(hash_hex.len(), 64, "Hash hex should be 64 characters (32 bytes)");

        // Raw bytes should be non-empty
        let raw_hex = prepared.to_hex();
        assert!(!raw_hex.is_empty(), "Raw hex should not be empty");
    }

    #[test]
    fn test_incremental_signing() {
        let signers = MultisigSigners::new_test_set().unwrap();
        let treasury = test_account(1);
        let recipient = test_account(2);

        let config = WithdrawalSubintentConfig {
            treasury_account: treasury,
            recipient_account: recipient,
            amount: dec!(25),
            network_id: STOKENET_NETWORK_ID,
            start_epoch: 100,
            end_epoch: 200,
            intent_discriminator: 456,
        };

        // Build unsigned
        let (mut partial, hash) = build_unsigned_withdrawal_subintent(&config).unwrap();

        // Sign incrementally with each signer
        for signer in signers.take_signers(3) {
            let signature = sign_subintent_hash(&hash, signer);
            partial = add_signature_to_partial(partial, signature);
        }

        // Should have 3 signatures now
        assert_eq!(
            partial.root_subintent_signatures.signatures.len(),
            3,
            "Should have 3 signatures after incremental signing"
        );

        // Verify we can get the hash from the signed partial
        let recovered_hash = get_subintent_hash(&partial).unwrap();
        assert_eq!(
            recovered_hash.as_hash(),
            hash.as_hash(),
            "Recovered hash should match original"
        );
    }

    #[test]
    fn test_format_subintent_hash() {
        let signers = MultisigSigners::new_test_set().unwrap();
        let treasury = test_account(1);
        let recipient = test_account(2);

        let config = WithdrawalSubintentConfig {
            treasury_account: treasury,
            recipient_account: recipient,
            amount: dec!(10),
            network_id: STOKENET_NETWORK_ID,
            start_epoch: 100,
            end_epoch: 200,
            intent_discriminator: 789,
        };

        let three_signers: Vec<&Signer> = signers.take_signers(3);
        let prepared = build_signed_withdrawal_subintent(&config, &three_signers).unwrap();

        let formatted = format_subintent_hash(&prepared.subintent_hash, STOKENET_NETWORK_ID);
        assert!(formatted.starts_with("subintent[stokenet]:"), "Should have subintent prefix");
    }

    #[test]
    fn test_config_constructor() {
        let treasury = test_account(1);
        let recipient = test_account(2);

        let config = WithdrawalSubintentConfig::new(
            treasury,
            recipient,
            dec!(500),
            1000, // current epoch
        )
        .expect("Valid config should succeed");

        assert_eq!(config.treasury_account, treasury);
        assert_eq!(config.recipient_account, recipient);
        assert_eq!(config.amount, dec!(500));
        assert_eq!(config.network_id, STOKENET_NETWORK_ID);
        assert_eq!(config.start_epoch, 1000);
        assert_eq!(config.end_epoch, 1100); // current + 100
        // intent_discriminator should be non-zero (random)
        assert!(config.intent_discriminator > 0);
    }

    #[test]
    fn test_config_rejects_zero_amount() {
        let treasury = test_account(1);
        let recipient = test_account(2);

        let result = WithdrawalSubintentConfig::new(treasury, recipient, dec!(0), 1000);
        assert!(result.is_err(), "Zero amount should be rejected");
        assert!(
            result.unwrap_err().to_string().contains("must be positive"),
            "Error message should mention positive requirement"
        );
    }

    #[test]
    fn test_config_rejects_negative_amount() {
        let treasury = test_account(1);
        let recipient = test_account(2);

        let result = WithdrawalSubintentConfig::new(treasury, recipient, dec!(-100), 1000);
        assert!(result.is_err(), "Negative amount should be rejected");
        assert!(
            result.unwrap_err().to_string().contains("must be positive"),
            "Error message should mention positive requirement"
        );
    }
}
