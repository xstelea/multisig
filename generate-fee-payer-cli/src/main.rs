use anyhow::{anyhow, Context, Result};
use inquire::{Confirm, Select};
use radix_common::network::NetworkDefinition;
use radix_common::prelude::*;
use radix_transactions::prelude::*;
use rand::RngCore;
use serde::Deserialize;

fn main() -> Result<()> {
    // 1. Network selection
    let network_name = Select::new("Select network:", vec!["Stokenet", "Mainnet"])
        .prompt()
        .context("Network selection cancelled")?;
    let network = match network_name {
        "Mainnet" => NetworkDefinition::mainnet(),
        _ => NetworkDefinition::stokenet(),
    };
    let is_stokenet = network.id == NetworkDefinition::stokenet().id;

    // 2. Generate random Ed25519 private key
    let mut key_bytes = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut key_bytes);
    let private_key = Ed25519PrivateKey::from_bytes(&key_bytes)
        .map_err(|e| anyhow!("Failed to create private key: {e:?}"))?;
    let public_key = private_key.public_key();

    // 3. Derive account address
    let pub_key_generic: PublicKey = public_key.into();
    let account_address =
        ComponentAddress::preallocated_account_from_public_key(&pub_key_generic);
    let encoder = radix_common::address::AddressBech32Encoder::new(&network);
    let account_bech32 = encoder
        .encode(account_address.as_bytes())
        .map_err(|e| anyhow!("Address encode failed: {e:?}"))?;

    // 4. Print key info
    let private_key_hex = hex::encode(&key_bytes);
    let public_key_hex = hex::encode(public_key.to_vec());
    println!();
    println!("── Generated Fee Payer ──────────────");
    println!("  Network:     {network_name}");
    println!("  Private key: {private_key_hex}");
    println!("  Public key:  {public_key_hex}");
    println!("  Address:     {account_bech32}");
    println!("─────────────────────────────────────");
    println!();

    // 5. If Stokenet, offer to fund from faucet
    if is_stokenet {
        let fund = Confirm::new("Fund this account from the Stokenet faucet?")
            .with_default(true)
            .prompt()
            .context("Faucet prompt cancelled")?;

        if fund {
            fund_from_faucet(account_address, &private_key)?;
        }
    }

    // 6. Print .env snippet
    println!();
    println!("── Add to your .env ─────────────────");
    println!("FEE_PAYER_PRIVATE_KEY_HEX={private_key_hex}");
    println!("─────────────────────────────────────");
    println!();

    Ok(())
}

// ============================================================================
// Stokenet faucet funding
// ============================================================================

const STOKENET_GATEWAY: &str = "https://babylon-stokenet-gateway.radixdlt.com";

fn fund_from_faucet(
    target: ComponentAddress,
    private_key: &Ed25519PrivateKey,
) -> Result<()> {
    let client = reqwest::blocking::Client::new();

    // Get current epoch
    print!("  Fetching current epoch...");
    let epoch = get_current_epoch(&client)?;
    println!(" epoch {epoch}");

    // Build faucet transaction
    print!("  Building faucet transaction...");
    let manifest = ManifestBuilder::new_v2()
        .lock_fee_from_faucet()
        .get_free_xrd_from_faucet()
        .try_deposit_entire_worktop_or_abort(target, None)
        .build();

    let notary_public_key: PublicKey = private_key.public_key().into();
    let intent_discriminator = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(1);

    let detailed = TransactionV2Builder::new()
        .intent_header(IntentHeaderV2 {
            network_id: NetworkDefinition::stokenet().id,
            start_epoch_inclusive: Epoch::of(epoch),
            end_epoch_exclusive: Epoch::of(epoch + 100),
            min_proposer_timestamp_inclusive: None,
            max_proposer_timestamp_exclusive: None,
            intent_discriminator,
        })
        .transaction_header(TransactionHeaderV2 {
            notary_public_key,
            notary_is_signatory: false,
            tip_basis_points: 0,
        })
        .manifest(manifest)
        .notarize(private_key)
        .build_no_validate();

    let tx_hex = hex::encode(detailed.raw.as_slice());
    let intent_hash = {
        let hash_encoder =
            TransactionHashBech32Encoder::new(&NetworkDefinition::stokenet());
        hash_encoder
            .encode(&detailed.transaction_hashes.transaction_intent_hash)
            .map_err(|e| anyhow!("Hash encode failed: {e:?}"))?
    };
    println!(" done");

    // Submit
    print!("  Submitting transaction...");
    submit_transaction(&client, &tx_hex)?;
    println!(" submitted");

    // Poll for commit
    print!("  Waiting for commit");
    wait_for_commit(&client, &intent_hash, 30)?;
    println!();

    println!(
        "  Funded! https://stokenet-dashboard.radixdlt.com/transaction/{}/summary",
        intent_hash
    );

    Ok(())
}

// ============================================================================
// Gateway helpers (inline, stokenet only)
// ============================================================================

fn get_current_epoch(client: &reqwest::blocking::Client) -> Result<u64> {
    let resp = client
        .post(format!("{STOKENET_GATEWAY}/status/gateway-status"))
        .json(&serde_json::json!({}))
        .send()?;

    if !resp.status().is_success() {
        let text = resp.text()?;
        return Err(anyhow!("Gateway status failed: {text}"));
    }

    let status: GatewayStatusResponse = resp.json()?;
    Ok(status.ledger_state.epoch)
}

fn submit_transaction(client: &reqwest::blocking::Client, tx_hex: &str) -> Result<()> {
    let resp = client
        .post(format!("{STOKENET_GATEWAY}/transaction/submit"))
        .json(&serde_json::json!({
            "notarized_transaction_hex": tx_hex
        }))
        .send()?;

    if !resp.status().is_success() {
        let text = resp.text()?;
        return Err(anyhow!("Submit failed: {text}"));
    }

    Ok(())
}

fn wait_for_commit(
    client: &reqwest::blocking::Client,
    intent_hash: &str,
    max_attempts: u32,
) -> Result<()> {
    for attempt in 0..max_attempts {
        let resp = client
            .post(format!("{STOKENET_GATEWAY}/transaction/status"))
            .json(&serde_json::json!({ "intent_hash": intent_hash }))
            .send()?;

        if !resp.status().is_success() {
            let text = resp.text()?;
            return Err(anyhow!("Status query failed: {text}"));
        }

        let status: TxStatusResponse = resp.json()?;
        match status.status.as_str() {
            "CommittedSuccess" => return Ok(()),
            "CommittedFailure" => {
                return Err(anyhow!(
                    "Transaction failed: {:?}",
                    status.error_message
                ));
            }
            "Rejected" => {
                return Err(anyhow!(
                    "Transaction rejected: {:?}",
                    status.error_message
                ));
            }
            _ => {
                print!(".");
                if attempt < max_attempts - 1 {
                    std::thread::sleep(std::time::Duration::from_secs(2));
                }
            }
        }
    }
    Err(anyhow!("Timeout waiting for commit after {max_attempts} attempts"))
}

// ============================================================================
// Gateway response types
// ============================================================================

#[derive(Deserialize)]
struct GatewayStatusResponse {
    ledger_state: LedgerState,
}

#[derive(Deserialize)]
struct LedgerState {
    epoch: u64,
}

#[derive(Deserialize)]
struct TxStatusResponse {
    status: String,
    error_message: Option<String>,
}
