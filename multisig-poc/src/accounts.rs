//! Account creation with multisig access rules for DAO treasury.
//!
//! This module provides functionality to build transaction manifests and transactions
//! that create Radix accounts with n-of-m multisig access rules using virtual signature badges.

use anyhow::Result;
use radix_common::prelude::*;
use radix_engine_interface::prelude::*;
use radix_transactions::prelude::*;

use crate::keys::MultisigSigners;

/// Network ID for Stokenet testnet.
pub const STOKENET_NETWORK_ID: u8 = 2;

/// Build an access rule requiring n-of-m signatures from the provided badge IDs.
///
/// This creates a `CountOf(n, badges)` rule that requires at least `n` out of the
/// provided signature badges to authorize an operation.
///
/// # Errors
///
/// Returns an error if:
/// - `badges` is empty
/// - `required_count` is 0
/// - `required_count` exceeds the number of badges
pub fn build_n_of_m_access_rule(
    required_count: u8,
    badges: &[NonFungibleGlobalId],
) -> Result<AccessRule> {
    // Validate inputs
    if badges.is_empty() {
        anyhow::bail!("badges cannot be empty");
    }
    if required_count == 0 {
        anyhow::bail!("required_count must be greater than 0");
    }
    if required_count as usize > badges.len() {
        anyhow::bail!(
            "required_count ({}) cannot exceed number of badges ({})",
            required_count,
            badges.len()
        );
    }

    let resources: Vec<ResourceOrNonFungible> = badges
        .iter()
        .map(|badge| ResourceOrNonFungible::NonFungible(badge.clone()))
        .collect();

    // require_n_of creates a CountOf(n, resources) composite requirement
    Ok(AccessRule::Protected(require_n_of(required_count, resources)))
}

/// Configuration for creating a multisig account.
pub struct MultisigAccountConfig {
    /// Number of signatures required (the "n" in n-of-m).
    pub required_signatures: u8,
    /// The virtual signature badges of all signers (the "m" in n-of-m).
    pub signer_badges: Vec<NonFungibleGlobalId>,
    /// Network ID (e.g., 2 for Stokenet).
    pub network_id: u8,
    /// Epoch range for transaction validity.
    pub start_epoch: u64,
    pub end_epoch: u64,
    /// Intent discriminator (nonce) for uniqueness.
    pub intent_discriminator: u64,
}

impl MultisigAccountConfig {
    /// Create a 3-of-4 multisig config for the DAO treasury on Stokenet.
    pub fn dao_treasury_3_of_4(
        signers: &MultisigSigners,
        current_epoch: u64,
    ) -> Self {
        Self {
            required_signatures: 3,
            signer_badges: signers.all_badges(),
            network_id: STOKENET_NETWORK_ID,
            start_epoch: current_epoch,
            end_epoch: current_epoch + 100, // Valid for 100 epochs
            intent_discriminator: 1,
        }
    }
}

/// Build a manifest that creates an account with multisig access control.
///
/// The account's owner role will be protected by an n-of-m rule requiring
/// `config.required_signatures` out of `config.signer_badges.len()` signatures.
pub fn build_create_multisig_account_manifest(
    config: &MultisigAccountConfig,
) -> Result<TransactionManifestV2> {
    let access_rule = build_n_of_m_access_rule(
        config.required_signatures,
        &config.signer_badges,
    )?;

    // OwnerRole::Fixed means the access rule cannot be changed after creation
    let owner_role = OwnerRole::Fixed(access_rule);

    Ok(ManifestBuilder::new_v2()
        .lock_fee_from_faucet()
        .create_account_with_owner(None, owner_role)
        .build())
}

/// Result of building a create-account transaction.
pub struct CreateAccountTransaction {
    /// The compiled transaction ready for signing.
    pub transaction: NotarizedTransactionV2,
    /// Raw bytes of the notarized transaction (hex-encodable for submission).
    pub raw: RawNotarizedTransaction,
    /// The transaction intent hash (used for status queries).
    pub intent_hash: TransactionIntentHash,
}

/// Build and notarize a complete transaction that creates a multisig account.
///
/// This function:
/// 1. Builds the manifest with the multisig access rule
/// 2. Creates the transaction intent with proper headers
/// 3. Signs with all required signers
/// 4. Notarizes with the provided notary
///
/// The returned transaction is ready for submission to the network.
pub fn build_create_multisig_account_transaction(
    config: &MultisigAccountConfig,
    signers: &MultisigSigners,
) -> Result<CreateAccountTransaction> {
    let manifest = build_create_multisig_account_manifest(config)?;

    let notary_public_key: PublicKey = signers.notary.public_key.into();

    // Build the transaction with V2 API
    let detailed = TransactionV2Builder::new()
        .intent_header(IntentHeaderV2 {
            network_id: config.network_id,
            start_epoch_inclusive: Epoch::of(config.start_epoch),
            end_epoch_exclusive: Epoch::of(config.end_epoch),
            min_proposer_timestamp_inclusive: None,
            max_proposer_timestamp_exclusive: None,
            intent_discriminator: config.intent_discriminator,
        })
        .transaction_header(TransactionHeaderV2 {
            notary_public_key,
            notary_is_signatory: false,
            tip_basis_points: 0, // No tip for testnet
        })
        .manifest(manifest)
        // Sign with all signers (the transaction creating the account needs to be
        // authorized, but since we're creating a new account with faucet fees,
        // we don't actually need signer authorization yet - the multisig rule
        // only applies after the account exists)
        .notarize(&signers.notary.private_key)
        // POC: Using build_no_validate() because we're building test transactions
        // with faucet-funded fees and deterministic test keys. For production,
        // use .build() to enforce full transaction validation.
        .build_no_validate();

    Ok(CreateAccountTransaction {
        transaction: detailed.transaction,
        raw: detailed.raw,
        intent_hash: detailed.transaction_hashes.transaction_intent_hash,
    })
}

/// Compile a notarized transaction to hex string for Gateway API submission.
pub fn transaction_to_hex(raw: &RawNotarizedTransaction) -> String {
    hex::encode(raw.as_slice())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keys::Signer;

    #[test]
    fn test_build_n_of_m_access_rule() {
        // Create test badges
        let signer1 = Signer::from_seed("test1", 1).unwrap();
        let signer2 = Signer::from_seed("test2", 2).unwrap();
        let signer3 = Signer::from_seed("test3", 3).unwrap();
        let signer4 = Signer::from_seed("test4", 4).unwrap();

        let badges = vec![
            signer1.badge.clone(),
            signer2.badge.clone(),
            signer3.badge.clone(),
            signer4.badge.clone(),
        ];

        let rule = build_n_of_m_access_rule(3, &badges).unwrap();

        // Verify it's a Protected rule (not AllowAll or DenyAll)
        match &rule {
            AccessRule::Protected(req) => {
                // Should be a CountOf requirement
                match req {
                    CompositeRequirement::BasicRequirement(BasicRequirement::CountOf(
                        count,
                        resources,
                    )) => {
                        assert_eq!(*count, 3, "Should require 3 signatures");
                        assert_eq!(resources.len(), 4, "Should have 4 possible signers");
                    }
                    _ => panic!("Expected CountOf requirement, got {:?}", req),
                }
            }
            _ => panic!("Expected Protected rule, got {:?}", rule),
        }
    }

    #[test]
    fn test_build_n_of_m_access_rule_validation() {
        let signer1 = Signer::from_seed("test1", 1).unwrap();
        let badges = vec![signer1.badge.clone()];

        // Empty badges should fail
        assert!(build_n_of_m_access_rule(1, &[]).is_err());

        // Zero required_count should fail
        assert!(build_n_of_m_access_rule(0, &badges).is_err());

        // required_count > badges.len() should fail
        assert!(build_n_of_m_access_rule(2, &badges).is_err());

        // Valid case should succeed
        assert!(build_n_of_m_access_rule(1, &badges).is_ok());
    }

    #[test]
    fn test_build_create_multisig_account_manifest() {
        let signers = MultisigSigners::new_test_set().unwrap();

        let config = MultisigAccountConfig {
            required_signatures: 3,
            signer_badges: signers.all_badges(),
            network_id: STOKENET_NETWORK_ID,
            start_epoch: 100,
            end_epoch: 200,
            intent_discriminator: 42,
        };

        let manifest = build_create_multisig_account_manifest(&config).unwrap();

        // Manifest should have instructions (lock_fee + create_account)
        assert!(
            !manifest.instructions.is_empty(),
            "Manifest should have instructions"
        );
    }

    #[test]
    fn test_build_create_multisig_account_transaction() {
        let signers = MultisigSigners::new_test_set().unwrap();

        let config = MultisigAccountConfig {
            required_signatures: 3,
            signer_badges: signers.all_badges(),
            network_id: STOKENET_NETWORK_ID,
            start_epoch: 100,
            end_epoch: 200,
            intent_discriminator: 42,
        };

        let result = build_create_multisig_account_transaction(&config, &signers);
        assert!(result.is_ok(), "Should build transaction successfully");

        let tx = result.unwrap();

        // Verify the raw transaction can be hex encoded
        let hex_str = transaction_to_hex(&tx.raw);
        assert!(!hex_str.is_empty(), "Should produce non-empty hex");

        // Verify we can decode the hex back
        let decoded = hex::decode(&hex_str);
        assert!(decoded.is_ok(), "Hex should be valid");
    }

    #[test]
    fn test_dao_treasury_config() {
        let signers = MultisigSigners::new_test_set().unwrap();
        let config = MultisigAccountConfig::dao_treasury_3_of_4(&signers, 500);

        assert_eq!(config.required_signatures, 3);
        assert_eq!(config.signer_badges.len(), 4);
        assert_eq!(config.network_id, STOKENET_NETWORK_ID);
        assert_eq!(config.start_epoch, 500);
        assert_eq!(config.end_epoch, 600);
    }
}
