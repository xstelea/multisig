//! Transaction composition and submission for multisig DAO operations.
//!
//! This module provides functionality to build complete transactions (TransactionV2) that:
//! - Include signed sub-intents as children
//! - Pay fees from a designated fee payer account
//! - Execute sub-intents via `yield_to_child`
//! - Are notarized by the fee payer for submission
//!
//! The main transaction wraps a signed partial transaction (sub-intent) and handles
//! fee payment and notarization, making it ready for Gateway API submission.

use anyhow::Result;
use radix_common::prelude::*;
use radix_transactions::prelude::*;

use crate::accounts::STOKENET_NETWORK_ID;
use crate::keys::Signer;

/// Result of building a main transaction with a child sub-intent.
pub struct PreparedMainTransaction {
    /// The notarized transaction ready for submission.
    pub transaction: NotarizedTransactionV2,
    /// Raw bytes of the notarized transaction.
    pub raw: RawNotarizedTransaction,
    /// The transaction intent hash (for status queries).
    pub intent_hash: TransactionIntentHash,
}

impl PreparedMainTransaction {
    /// Get the transaction intent hash as a hex string.
    pub fn intent_hash_hex(&self) -> String {
        hex::encode(self.intent_hash.as_hash().as_slice())
    }

    /// Get the raw transaction bytes as a hex string for Gateway submission.
    pub fn to_hex(&self) -> String {
        hex::encode(self.raw.as_slice())
    }
}

/// Build a main transaction that wraps and executes a signed sub-intent.
///
/// This creates a complete `NotarizedTransactionV2` that:
/// 1. Locks fees from the fee payer account
/// 2. Includes the signed sub-intent as a child
/// 3. Yields to the child to execute it
/// 4. Is signed and notarized by the fee payer
///
/// The fee payer acts as both the signer (for fee authorization) and the notary.
///
/// # Arguments
/// * `network_id` - Network ID (e.g., 2 for Stokenet)
/// * `current_epoch` - Current network epoch for validity window
/// * `fee_payer_account` - Account address to pay transaction fees
/// * `fee_payer` - Signer with private key for the fee payer (also notarizes)
/// * `signed_subintent` - The signed partial transaction to execute
/// * `lock_fee_amount` - Amount of XRD to lock for fees
///
/// # Returns
/// A `PreparedMainTransaction` containing the notarized transaction and its hex encoding.
pub fn build_main_transaction(
    network_id: u8,
    current_epoch: u64,
    fee_payer_account: ComponentAddress,
    fee_payer: &Signer,
    signed_subintent: SignedPartialTransactionV2,
    lock_fee_amount: Decimal,
) -> Result<PreparedMainTransaction> {
    build_main_transaction_with_discriminator(
        network_id,
        current_epoch,
        fee_payer_account,
        fee_payer,
        signed_subintent,
        lock_fee_amount,
        rand_intent_discriminator(),
    )
}

/// Build a main transaction with a specific intent discriminator.
///
/// Same as `build_main_transaction` but allows specifying the intent discriminator
/// for deterministic transaction building (useful for testing).
pub fn build_main_transaction_with_discriminator(
    network_id: u8,
    current_epoch: u64,
    fee_payer_account: ComponentAddress,
    fee_payer: &Signer,
    signed_subintent: SignedPartialTransactionV2,
    lock_fee_amount: Decimal,
    intent_discriminator: u64,
) -> Result<PreparedMainTransaction> {
    let notary_public_key: PublicKey = fee_payer.public_key.into();

    // Build the main transaction with the signed sub-intent as a child
    let detailed = TransactionV2Builder::new()
        // Add the signed sub-intent as a child (must be done before manifest_builder)
        .add_signed_child("withdrawal", signed_subintent)
        // Set the transaction header (notary info)
        .transaction_header(TransactionHeaderV2 {
            notary_public_key,
            notary_is_signatory: true, // Fee payer is also a signatory
            tip_basis_points: 0,       // No tip for testnet
        })
        // Set the intent header (epoch range, network)
        .intent_header(IntentHeaderV2 {
            network_id,
            start_epoch_inclusive: Epoch::of(current_epoch),
            end_epoch_exclusive: Epoch::of(current_epoch + 100), // Valid for 100 epochs
            min_proposer_timestamp_inclusive: None,
            max_proposer_timestamp_exclusive: None,
            intent_discriminator,
        })
        // Build the manifest inline - lock fee and execute child
        .manifest_builder(|builder| {
            builder
                // Lock fees from the fee payer account
                .lock_fee(fee_payer_account, lock_fee_amount)
                // Execute the child sub-intent (the withdrawal)
                .yield_to_child("withdrawal", ())
        })
        // Sign as fee payer (authorizes the lock_fee)
        .sign(&fee_payer.private_key)
        // Notarize with the fee payer's key
        .notarize(&fee_payer.private_key)
        // Build without validation for POC (use .build() for production)
        .build_no_validate();

    Ok(PreparedMainTransaction {
        transaction: detailed.transaction,
        raw: detailed.raw,
        intent_hash: detailed.transaction_hashes.transaction_intent_hash,
    })
}

/// Build a main transaction for Stokenet with default settings.
///
/// Convenience function that uses Stokenet network ID and a default fee amount.
pub fn build_stokenet_main_transaction(
    current_epoch: u64,
    fee_payer_account: ComponentAddress,
    fee_payer: &Signer,
    signed_subintent: SignedPartialTransactionV2,
) -> Result<PreparedMainTransaction> {
    // Use a generous fee for testnet (10 XRD should be plenty)
    let lock_fee_amount = Decimal::from(10);

    build_main_transaction(
        STOKENET_NETWORK_ID,
        current_epoch,
        fee_payer_account,
        fee_payer,
        signed_subintent,
        lock_fee_amount,
    )
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

/// Convert any raw notarized transaction to hex for Gateway submission.
pub fn notarized_transaction_to_hex(raw: &RawNotarizedTransaction) -> String {
    hex::encode(raw.as_slice())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keys::MultisigSigners;
    use crate::subintent::{build_signed_withdrawal_subintent, WithdrawalSubintentConfig};
    use radix_engine_interface::prelude::dec;

    // Create a dummy account address for testing
    fn test_account(seed: u8) -> ComponentAddress {
        ComponentAddress::preallocated_account_from_public_key(&Ed25519PublicKey(
            [seed; Ed25519PublicKey::LENGTH],
        ))
    }

    #[test]
    fn test_build_main_transaction_with_subintent() {
        let signers = MultisigSigners::new_test_set().unwrap();
        let treasury = test_account(1);
        let recipient = test_account(2);
        let fee_payer_account = test_account(3);

        // Create a withdrawal sub-intent
        let subintent_config = WithdrawalSubintentConfig {
            treasury_account: treasury,
            recipient_account: recipient,
            amount: dec!(100),
            network_id: STOKENET_NETWORK_ID,
            start_epoch: 100,
            end_epoch: 200,
            intent_discriminator: 999,
        };

        let three_signers: Vec<&Signer> = signers.take_signers(3);
        let prepared_subintent =
            build_signed_withdrawal_subintent(&subintent_config, &three_signers)
                .expect("Should build signed subintent");

        // Build the main transaction
        let result = build_main_transaction_with_discriminator(
            STOKENET_NETWORK_ID,
            100,
            fee_payer_account,
            &signers.notary,
            prepared_subintent.signed_partial,
            dec!(10),
            12345, // Deterministic discriminator for test
        );

        assert!(result.is_ok(), "Should build main transaction: {:?}", result.err());

        let main_tx = result.unwrap();

        // Verify intent hash is valid
        let hash_hex = main_tx.intent_hash_hex();
        assert_eq!(hash_hex.len(), 64, "Intent hash should be 64 hex chars (32 bytes)");

        // Verify raw transaction produces valid hex
        let tx_hex = main_tx.to_hex();
        assert!(!tx_hex.is_empty(), "Transaction hex should not be empty");

        // Verify hex is valid
        let decoded = hex::decode(&tx_hex);
        assert!(decoded.is_ok(), "Transaction hex should be valid");
    }

    #[test]
    fn test_build_stokenet_main_transaction() {
        let signers = MultisigSigners::new_test_set().unwrap();
        let treasury = test_account(1);
        let recipient = test_account(2);
        let fee_payer_account = test_account(3);

        // Create a withdrawal sub-intent
        let subintent_config = WithdrawalSubintentConfig {
            treasury_account: treasury,
            recipient_account: recipient,
            amount: dec!(50),
            network_id: STOKENET_NETWORK_ID,
            start_epoch: 100,
            end_epoch: 200,
            intent_discriminator: 888,
        };

        let three_signers: Vec<&Signer> = signers.take_signers(3);
        let prepared_subintent =
            build_signed_withdrawal_subintent(&subintent_config, &three_signers)
                .expect("Should build signed subintent");

        // Use the convenience function
        let result = build_stokenet_main_transaction(
            100,
            fee_payer_account,
            &signers.notary,
            prepared_subintent.signed_partial,
        );

        assert!(
            result.is_ok(),
            "Should build Stokenet main transaction: {:?}",
            result.err()
        );

        let main_tx = result.unwrap();
        assert!(!main_tx.to_hex().is_empty(), "Should produce valid hex");
    }

    #[test]
    fn test_transaction_hex_roundtrip() {
        let signers = MultisigSigners::new_test_set().unwrap();
        let treasury = test_account(1);
        let recipient = test_account(2);
        let fee_payer_account = test_account(3);

        let subintent_config = WithdrawalSubintentConfig {
            treasury_account: treasury,
            recipient_account: recipient,
            amount: dec!(25),
            network_id: STOKENET_NETWORK_ID,
            start_epoch: 100,
            end_epoch: 200,
            intent_discriminator: 777,
        };

        let three_signers: Vec<&Signer> = signers.take_signers(3);
        let prepared_subintent =
            build_signed_withdrawal_subintent(&subintent_config, &three_signers).unwrap();

        let main_tx = build_main_transaction_with_discriminator(
            STOKENET_NETWORK_ID,
            100,
            fee_payer_account,
            &signers.notary,
            prepared_subintent.signed_partial,
            dec!(5),
            11111,
        )
        .unwrap();

        // Get hex
        let tx_hex = main_tx.to_hex();

        // Decode hex back to bytes
        let bytes = hex::decode(&tx_hex).expect("Should decode hex");

        // Verify bytes match original
        assert_eq!(
            bytes,
            main_tx.raw.as_slice(),
            "Decoded bytes should match original"
        );
    }

    #[test]
    fn test_notarized_transaction_to_hex() {
        let signers = MultisigSigners::new_test_set().unwrap();
        let treasury = test_account(1);
        let recipient = test_account(2);
        let fee_payer_account = test_account(3);

        let subintent_config = WithdrawalSubintentConfig {
            treasury_account: treasury,
            recipient_account: recipient,
            amount: dec!(10),
            network_id: STOKENET_NETWORK_ID,
            start_epoch: 100,
            end_epoch: 200,
            intent_discriminator: 666,
        };

        let three_signers: Vec<&Signer> = signers.take_signers(3);
        let prepared_subintent =
            build_signed_withdrawal_subintent(&subintent_config, &three_signers).unwrap();

        let main_tx = build_main_transaction_with_discriminator(
            STOKENET_NETWORK_ID,
            100,
            fee_payer_account,
            &signers.notary,
            prepared_subintent.signed_partial,
            dec!(5),
            22222,
        )
        .unwrap();

        // Use the standalone helper function
        let hex1 = notarized_transaction_to_hex(&main_tx.raw);
        let hex2 = main_tx.to_hex();

        assert_eq!(hex1, hex2, "Both hex methods should produce same output");
    }

    #[test]
    fn test_intent_hash_uniqueness() {
        let signers = MultisigSigners::new_test_set().unwrap();
        let treasury = test_account(1);
        let recipient = test_account(2);
        let fee_payer_account = test_account(3);

        let subintent_config = WithdrawalSubintentConfig {
            treasury_account: treasury,
            recipient_account: recipient,
            amount: dec!(100),
            network_id: STOKENET_NETWORK_ID,
            start_epoch: 100,
            end_epoch: 200,
            intent_discriminator: 555,
        };

        let three_signers: Vec<&Signer> = signers.take_signers(3);

        // Build two subintents with the same config
        let prepared_subintent1 =
            build_signed_withdrawal_subintent(&subintent_config, &three_signers).unwrap();
        let prepared_subintent2 =
            build_signed_withdrawal_subintent(&subintent_config, &three_signers).unwrap();

        // Build two main transactions with DIFFERENT discriminators
        let main_tx1 = build_main_transaction_with_discriminator(
            STOKENET_NETWORK_ID,
            100,
            fee_payer_account,
            &signers.notary,
            prepared_subintent1.signed_partial,
            dec!(5),
            33333,
        )
        .unwrap();

        let main_tx2 = build_main_transaction_with_discriminator(
            STOKENET_NETWORK_ID,
            100,
            fee_payer_account,
            &signers.notary,
            prepared_subintent2.signed_partial,
            dec!(5),
            44444, // Different discriminator
        )
        .unwrap();

        // Intent hashes should be different
        assert_ne!(
            main_tx1.intent_hash_hex(),
            main_tx2.intent_hash_hex(),
            "Different discriminators should produce different intent hashes"
        );
    }
}
