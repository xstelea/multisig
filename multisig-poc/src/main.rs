//! Multisig Orchestrator POC — End-to-End on Stokenet
//!
//! Executes a complete 3-of-4 multisig treasury withdrawal against live Stokenet:
//! 1. Setup: connect Gateway, get epoch, generate keys
//! 2. Create Treasury: build & submit multisig account creation TX
//! 3. Fund Accounts: fund treasury + fee payer via faucet
//! 4. Build Subintent: withdrawal subintent with real addresses/epoch
//! 5. Collect Signatures: 3-of-4 threshold signing
//! 6. Build Main TX: wrap subintent with fee lock
//! 7. Submit: submit withdrawal TX, wait for commit
//! 8. Verify: print summary with dashboard links

use anyhow::Result;
use radix_common::address::AddressBech32Encoder;
use radix_common::network::NetworkDefinition;
use radix_common::prelude::*;
use radix_engine_interface::prelude::dec;

use multisig_poc::accounts::{
    build_create_multisig_account_transaction, build_fund_from_faucet_transaction,
    decode_component_address, transaction_to_hex, MultisigAccountConfig,
};
use multisig_poc::gateway::{
    encode_intent_hash, extract_created_account_address, GatewayClient,
};
use multisig_poc::keys::MultisigSigners;
use multisig_poc::subintent::{
    add_signature_to_partial, build_unsigned_withdrawal_subintent, sign_subintent_hash,
    WithdrawalSubintentConfig,
};
use multisig_poc::transaction::build_stokenet_main_transaction;

fn main() -> Result<()> {
    println!("Multisig Orchestrator POC — E2E on Stokenet");
    println!("=============================================\n");

    let network = NetworkDefinition::stokenet();
    let addr_encoder = AddressBech32Encoder::new(&network);

    // =========================================================================
    // PHASE 1: Setup
    // =========================================================================
    println!("[PHASE 1] Setup");
    println!("----------------");

    let gateway = GatewayClient::new();
    let epoch = gateway.get_current_epoch()?;
    println!("  Connected to Stokenet — epoch {}", epoch);

    let signers = MultisigSigners::new_test_set()?;
    println!("  Generated {} signers + 1 notary", signers.signers.len());

    // Derive notary's virtual account (used as fee payer and recipient)
    let notary_account = ComponentAddress::preallocated_account_from_public_key(
        &signers.notary.public_key,
    );
    let notary_account_bech32 = addr_encoder
        .encode(notary_account.as_bytes())
        .expect("encode notary address");
    println!("  Notary account: {}", notary_account_bech32);

    // =========================================================================
    // PHASE 2: Create Treasury Account
    // =========================================================================
    println!("\n[PHASE 2] Create Treasury Account");
    println!("-----------------------------------");

    let treasury_config = MultisigAccountConfig::dao_treasury_3_of_4(&signers, epoch);
    let create_tx = build_create_multisig_account_transaction(&treasury_config, &signers)?;
    let create_tx_hex = transaction_to_hex(&create_tx.raw);
    let create_hash = encode_intent_hash(&create_tx.intent_hash)?;

    println!("  TX size: {} bytes", create_tx.raw.as_slice().len());
    println!("  Intent hash: {}", create_hash);
    println!("  Submitting...");

    let submit_result = gateway.submit_transaction(&create_tx_hex)?;
    println!("  Submitted (duplicate={})", submit_result.duplicate);

    println!("  Waiting for commit...");
    let status = gateway.wait_for_commit(&create_hash, 30)?;
    println!("  Status: {}", status);

    // Extract created account address
    let details = gateway.get_committed_details(&create_hash)?;
    let treasury_bech32 = extract_created_account_address(&details)?;
    let treasury_address = decode_component_address(&treasury_bech32)?;
    println!("  Treasury account: {}", treasury_bech32);

    // =========================================================================
    // PHASE 3: Fund Accounts via Faucet
    // =========================================================================
    println!("\n[PHASE 3] Fund Accounts");
    println!("------------------------");

    // Fund treasury
    println!("  Funding treasury...");
    let fund_treasury_tx =
        build_fund_from_faucet_transaction(treasury_address, &signers.notary, epoch)?;
    let fund_treasury_hex = transaction_to_hex(&fund_treasury_tx.raw);
    let fund_treasury_hash = encode_intent_hash(&fund_treasury_tx.intent_hash)?;
    gateway.submit_transaction(&fund_treasury_hex)?;
    let status = gateway.wait_for_commit(&fund_treasury_hash, 30)?;
    println!("  Treasury funded: {}", status);

    // Fund notary account (fee payer)
    println!("  Funding fee payer (notary account)...");
    let fund_notary_tx =
        build_fund_from_faucet_transaction(notary_account, &signers.notary, epoch)?;
    let fund_notary_hex = transaction_to_hex(&fund_notary_tx.raw);
    let fund_notary_hash = encode_intent_hash(&fund_notary_tx.intent_hash)?;
    gateway.submit_transaction(&fund_notary_hex)?;
    let status = gateway.wait_for_commit(&fund_notary_hash, 30)?;
    println!("  Fee payer funded: {}", status);

    // =========================================================================
    // PHASE 4: Build Withdrawal Subintent
    // =========================================================================
    println!("\n[PHASE 4] Build Withdrawal Subintent");
    println!("--------------------------------------");

    // Re-fetch epoch for freshness
    let epoch = gateway.get_current_epoch()?;
    println!("  Current epoch: {}", epoch);

    let withdrawal_amount = dec!(500);
    let config = WithdrawalSubintentConfig::new(
        treasury_address,
        notary_account, // recipient = notary's account
        withdrawal_amount,
        epoch,
    )?;

    let (unsigned_partial, subintent_hash) = build_unsigned_withdrawal_subintent(&config)?;
    println!("  Withdrawal: {} XRD from treasury → notary account", withdrawal_amount);
    println!(
        "  Subintent hash: {}",
        hex::encode(subintent_hash.as_hash().as_slice())
    );

    // =========================================================================
    // PHASE 5: Collect Signatures (3-of-4)
    // =========================================================================
    println!("\n[PHASE 5] Collect Signatures");
    println!("-----------------------------");

    let mut signed_partial = unsigned_partial;
    let threshold = 3;

    for (i, signer) in signers.signers.iter().enumerate() {
        if i < threshold {
            let signature = sign_subintent_hash(&subintent_hash, signer);
            signed_partial = add_signature_to_partial(signed_partial, signature);
            println!("  [{}] {} — signed", i + 1, signer.name);
        } else {
            println!("  [{}] {} — SKIPPED (threshold met)", i + 1, signer.name);
        }
    }

    println!(
        "  Signatures: {}/{}",
        signed_partial.root_subintent_signatures.signatures.len(),
        signers.signers.len()
    );

    // =========================================================================
    // PHASE 6: Build Main Transaction
    // =========================================================================
    println!("\n[PHASE 6] Build Main Transaction");
    println!("----------------------------------");

    let main_tx = build_stokenet_main_transaction(
        epoch,
        notary_account,
        &signers.notary,
        signed_partial,
    )?;

    println!("  TX size: {} bytes", main_tx.raw.as_slice().len());
    let main_hash = encode_intent_hash(&main_tx.intent_hash)?;
    println!("  Intent hash: {}", main_hash);

    // =========================================================================
    // PHASE 7: Submit Withdrawal TX
    // =========================================================================
    println!("\n[PHASE 7] Submit Withdrawal");
    println!("----------------------------");

    let main_tx_hex = main_tx.to_hex();
    println!("  Submitting...");
    let submit_result = gateway.submit_transaction(&main_tx_hex)?;
    println!("  Submitted (duplicate={})", submit_result.duplicate);

    println!("  Waiting for commit...");
    let status = gateway.wait_for_commit(&main_hash, 30)?;
    println!("  Status: {}", status);

    // =========================================================================
    // PHASE 8: Summary
    // =========================================================================
    println!("\n[PHASE 8] Summary");
    println!("==================");
    println!("  Treasury account:  {}", treasury_bech32);
    println!("  Fee payer:         {}", notary_account_bech32);
    println!("  Recipient:         {}", notary_account_bech32);
    println!("  Withdrawal amount: {} XRD", withdrawal_amount);
    println!();
    println!("  Create treasury TX: {}", create_hash);
    println!("  Fund treasury TX:   {}", fund_treasury_hash);
    println!("  Fund fee payer TX:  {}", fund_notary_hash);
    println!("  Withdrawal TX:      {}", main_hash);
    println!();
    println!(
        "  Dashboard: https://stokenet-dashboard.radixdlt.com/transaction/{}/summary",
        main_hash
    );

    println!("\nDone.");
    Ok(())
}
