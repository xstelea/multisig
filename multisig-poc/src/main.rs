//! Multisig Orchestrator POC - End-to-End Demonstration
//!
//! This demonstrates the complete multi-party signing workflow for a DAO treasury
//! withdrawal using 3-of-4 multisig access rules on Radix. The flow covers:
//! 1. Setup: Generate test keys
//! 2. Sub-Intent Creation: Build withdrawal sub-intent
//! 3. Signature Collection: Collect 3-of-4 signatures
//! 4. Transaction Composition: Build main transaction wrapping the sub-intent
//! 5. Ready for Submission: Show the transaction is ready (hex encoded)

use anyhow::Result;
use radix_common::prelude::*;
use radix_engine_interface::prelude::dec;
use radix_transactions::prelude::*;

use multisig_poc::accounts::STOKENET_NETWORK_ID;
use multisig_poc::keys::MultisigSigners;
use multisig_poc::subintent::{
    add_signature_to_partial, build_unsigned_withdrawal_subintent, format_subintent_hash,
    sign_subintent_hash, WithdrawalSubintentConfig,
};
use multisig_poc::transaction::build_stokenet_main_transaction;

fn main() -> Result<()> {
    println!("Multisig Orchestrator POC");
    println!("=========================\n");

    // =========================================================================
    // PHASE 1: Setup - Generate test keys and create withdrawal sub-intent
    // =========================================================================
    println!("[PHASE 1] Setup");
    println!("---------------");

    // Generate test signers (4 signers + notary)
    let signers = MultisigSigners::new_test_set()?;
    println!("  Generated {} signers for 3-of-4 multisig:", signers.signers.len());
    for signer in &signers.signers {
        println!("    - {} (pubkey: {}...)", signer.name, &hex::encode(signer.public_key.0)[..16]);
    }
    println!("  Notary: {} (pubkey: {}...)", signers.notary.name, &hex::encode(signers.notary.public_key.0)[..16]);

    // Create dummy account addresses for demonstration
    // In a real scenario, these would be actual deployed accounts
    let treasury_account = ComponentAddress::preallocated_account_from_public_key(
        &Ed25519PublicKey([1; Ed25519PublicKey::LENGTH]),
    );
    let recipient_account = ComponentAddress::preallocated_account_from_public_key(
        &Ed25519PublicKey([2; Ed25519PublicKey::LENGTH]),
    );

    // Create the withdrawal sub-intent configuration
    let current_epoch = 1000u64; // Simulated current epoch
    let withdrawal_amount = dec!(500);

    let config = WithdrawalSubintentConfig::new(
        treasury_account,
        recipient_account,
        withdrawal_amount,
        current_epoch,
    )?;

    println!("\n  Withdrawal sub-intent configuration:");
    println!("    - Amount: {} XRD", config.amount);
    println!("    - Valid epochs: {} to {}", config.start_epoch, config.end_epoch);
    println!("    - Intent discriminator: {}", config.intent_discriminator);

    // Build unsigned sub-intent
    let (unsigned_partial, subintent_hash) = build_unsigned_withdrawal_subintent(&config)?;

    let formatted_hash = format_subintent_hash(&subintent_hash, STOKENET_NETWORK_ID);
    println!("\n  Built unsigned withdrawal sub-intent");
    println!("    Hash: {}", formatted_hash);
    println!("    Initial signatures: {}", unsigned_partial.root_subintent_signatures.signatures.len());

    // =========================================================================
    // PHASE 2: Signature Collection - Collect 3 of 4 signatures
    // =========================================================================
    println!("\n[PHASE 2] Signature Collection");
    println!("-------------------------------");
    println!("  Collecting signatures for 3-of-4 threshold...\n");

    let mut signed_partial = unsigned_partial;
    let threshold = 3;
    let total_signers = signers.signers.len();

    // Collect signatures from the first 3 signers (skip signer 4 to prove threshold works)
    for (i, signer) in signers.signers.iter().enumerate() {
        if i < threshold {
            // Sign the subintent hash with this signer
            let signature = sign_subintent_hash(&subintent_hash, signer);

            // Add the signature to the partial transaction
            signed_partial = add_signature_to_partial(signed_partial, signature);

            let sig_count = signed_partial.root_subintent_signatures.signatures.len();
            println!(
                "  [{}] {}: signed (signatures: {}/{})",
                i + 1,
                signer.name,
                sig_count,
                threshold
            );
        } else {
            // Skip this signer to demonstrate threshold behavior
            println!(
                "  [{}] {}: SKIPPED (threshold already met)",
                i + 1,
                signer.name
            );
        }
    }

    // =========================================================================
    // PHASE 3: Verification - Confirm signature collection
    // =========================================================================
    println!("\n[PHASE 3] Signature Verification");
    println!("---------------------------------");

    let final_sig_count = signed_partial.root_subintent_signatures.signatures.len();
    println!("  Total signatures collected: {}/{}", final_sig_count, total_signers);
    println!("  Threshold requirement: {}/{}", threshold, total_signers);

    if final_sig_count >= threshold {
        println!("  Status: THRESHOLD MET - Sub-intent is ready for composition");
    } else {
        println!("  Status: THRESHOLD NOT MET - Need {} more signature(s)", threshold - final_sig_count);
        return Err(anyhow::anyhow!("Insufficient signatures"));
    }

    // Show signature details
    println!("\n  Signature details:");
    for (i, sig) in signed_partial.root_subintent_signatures.signatures.iter().enumerate() {
        // Extract pubkey from Ed25519 signatures (our test signers use Ed25519)
        if let SignatureWithPublicKeyV1::Ed25519 { public_key, .. } = &sig.0 {
            println!("    [{}] Ed25519 pubkey: {}...", i + 1, &hex::encode(public_key.0)[..16]);
        } else {
            println!("    [{}] Secp256k1 signature", i + 1);
        }
    }

    // Encode the signed partial transaction for storage/transmission
    let raw_partial = signed_partial
        .to_raw()
        .map_err(|e| anyhow::anyhow!("Failed to encode signed partial: {:?}", e))?;
    let partial_hex = hex::encode(raw_partial.as_slice());

    println!("\n  Signed partial transaction:");
    println!("    Size: {} bytes", raw_partial.as_slice().len());
    println!("    Hex (first 64 chars): {}...", &partial_hex[..64.min(partial_hex.len())]);

    // =========================================================================
    // PHASE 4: Transaction Composition - Build main transaction with sub-intent
    // =========================================================================
    println!("\n[PHASE 4] Transaction Composition");
    println!("----------------------------------");

    // The fee payer account is derived from the notary's public key
    let fee_payer_account = ComponentAddress::preallocated_account_from_public_key(
        &signers.notary.public_key,
    );

    println!("  Building main transaction...");
    println!("    Fee payer: {} (notary)", &hex::encode(signers.notary.public_key.0)[..16]);
    println!("    Network: Stokenet (ID: {})", STOKENET_NETWORK_ID);

    // Build the main transaction wrapping the signed sub-intent
    let main_tx = build_stokenet_main_transaction(
        current_epoch,
        fee_payer_account,
        &signers.notary,
        signed_partial,
    )?;

    println!("\n  Main transaction built successfully:");
    println!("    Transaction intent hash: {}", main_tx.intent_hash_hex());
    println!("    Transaction size: {} bytes", main_tx.raw.as_slice().len());

    // =========================================================================
    // PHASE 5: Ready for Submission
    // =========================================================================
    println!("\n[PHASE 5] Ready for Submission");
    println!("-------------------------------");

    let tx_hex = main_tx.to_hex();
    println!("  Transaction hex (first 64 chars): {}...", &tx_hex[..64.min(tx_hex.len())]);
    println!("  Transaction hex length: {} chars", tx_hex.len());

    println!("\n  NOTE: Actual submission requires:");
    println!("    1. Fund test accounts via Stokenet faucet");
    println!("    2. Create DAO treasury account on-chain first");
    println!("    3. Then submit this transaction via Gateway API");

    // =========================================================================
    // Summary
    // =========================================================================
    println!("\n[SUMMARY]");
    println!("---------");
    println!("  Successfully demonstrated full multisig sub-intent flow:");
    println!("    1. Generated 4 test signers + 1 notary");
    println!("    2. Created withdrawal sub-intent for {} XRD", withdrawal_amount);
    println!("    3. Collected {}/{} signatures (threshold: {})", final_sig_count, total_signers, threshold);
    println!("    4. Built main transaction with child sub-intent");
    println!("    5. Transaction ready for submission ({} bytes)", main_tx.raw.as_slice().len());

    Ok(())
}
