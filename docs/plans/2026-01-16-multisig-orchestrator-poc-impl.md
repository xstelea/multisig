# Multisig Orchestrator POC Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build a Rust script that validates the end-to-end sub-intent multisig flow on Stokenet: create a DAO treasury with 3-of-4 access rule, create a withdrawal sub-intent, collect 3 signatures, wrap with fee payer, submit, and verify.

**Architecture:** Single-binary CLI that runs all phases sequentially. Uses `radix-transactions` for transaction building and `reqwest` for Gateway API calls. All test keys are hardcoded (pre-funded Stokenet accounts).

**Tech Stack:** Rust, radix-transactions, radix-common, reqwest, tokio, anyhow

---

## Task 1: Project Setup

**Files:**
- Create: `multisig-orchestrator-poc/Cargo.toml`
- Create: `multisig-orchestrator-poc/src/main.rs`

**Step 1: Create project directory and Cargo.toml**

```toml
[package]
name = "multisig-orchestrator-poc"
version = "0.1.0"
edition = "2021"

[dependencies]
radix-transactions = { git = "https://github.com/radixdlt/radixdlt-scrypto", tag = "v1.3.0" }
radix-common = { git = "https://github.com/radixdlt/radixdlt-scrypto", tag = "v1.3.0" }
radix-common-derive = { git = "https://github.com/radixdlt/radixdlt-scrypto", tag = "v1.3.0" }
sbor = { git = "https://github.com/radixdlt/radixdlt-scrypto", tag = "v1.3.0" }
reqwest = { version = "0.12", features = ["json"] }
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
anyhow = "1"
hex = "0.4"
rand = "0.8"
rand_chacha = "0.3"
```

**Step 2: Create minimal main.rs**

```rust
use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    println!("═══════════════════════════════════════════════════════════");
    println!("  MULTISIG ORCHESTRATOR POC — Stokenet");
    println!("═══════════════════════════════════════════════════════════");
    println!();

    Ok(())
}
```

**Step 3: Verify project compiles**

Run: `cd multisig-orchestrator-poc && cargo build`
Expected: Compiles successfully (may take time to fetch dependencies)

**Step 4: Commit**

```bash
git add multisig-orchestrator-poc/
git commit -m "feat(poc): initialize multisig orchestrator project"
```

---

## Task 2: Test Keys Module

**Files:**
- Create: `multisig-orchestrator-poc/src/keys.rs`
- Modify: `multisig-orchestrator-poc/src/main.rs`

**Step 1: Create keys.rs with hardcoded Stokenet keys**

You need to generate 6 Ed25519 keypairs for Stokenet. For now, create the structure with placeholder values that will be filled with real funded keys.

```rust
//! Hardcoded Stokenet test keys.
//!
//! IMPORTANT: These are TESTNET keys only. Never use real funds.
//! To use this POC:
//! 1. Generate 6 Ed25519 keypairs
//! 2. Derive Stokenet addresses from each
//! 3. Fund each address using the Stokenet faucet
//! 4. Replace the placeholder hex strings below

use radix_transactions::signing::PrivateKey;
use radix_common::network::NetworkDefinition;
use radix_common::crypto::{Ed25519PrivateKey, PublicKey};
use radix_common::prelude::ComponentAddress;

/// Test key with associated address
pub struct TestKey {
    pub name: &'static str,
    pub private_key: PrivateKey,
    pub address: ComponentAddress,
}

impl TestKey {
    fn from_hex(name: &'static str, hex: &str, address: &str) -> Self {
        let bytes = hex::decode(hex).expect("Invalid hex for private key");
        let ed25519_key = Ed25519PrivateKey::from_bytes(&bytes)
            .expect("Invalid Ed25519 private key");
        let private_key = PrivateKey::Ed25519(ed25519_key);

        let address = ComponentAddress::try_from_bech32(
            &NetworkDefinition::stokenet().address_encoder(),
            address
        ).expect("Invalid address");

        Self { name, private_key, address }
    }

    pub fn public_key(&self) -> PublicKey {
        self.private_key.public_key()
    }
}

/// DAO signers (need 3 of 4 to authorize actions)
pub fn dao_signers() -> [TestKey; 4] {
    [
        // TODO: Replace with real funded Stokenet keys
        TestKey::from_hex(
            "Signer1",
            "0000000000000000000000000000000000000000000000000000000000000001",
            "account_tdx_2_placeholder1"
        ),
        TestKey::from_hex(
            "Signer2",
            "0000000000000000000000000000000000000000000000000000000000000002",
            "account_tdx_2_placeholder2"
        ),
        TestKey::from_hex(
            "Signer3",
            "0000000000000000000000000000000000000000000000000000000000000003",
            "account_tdx_2_placeholder3"
        ),
        TestKey::from_hex(
            "Signer4",
            "0000000000000000000000000000000000000000000000000000000000000004",
            "account_tdx_2_placeholder4"
        ),
    ]
}

/// Fee payer account (pays transaction fees)
pub fn fee_payer() -> TestKey {
    // TODO: Replace with real funded Stokenet key
    TestKey::from_hex(
        "FeePayer",
        "0000000000000000000000000000000000000000000000000000000000000005",
        "account_tdx_2_placeholder5"
    )
}

/// Funding account (creates and funds DAO treasury)
pub fn funding_account() -> TestKey {
    // TODO: Replace with real funded Stokenet key
    // Can be same as fee_payer or one of the signers
    TestKey::from_hex(
        "Funder",
        "0000000000000000000000000000000000000000000000000000000000000006",
        "account_tdx_2_placeholder6"
    )
}
```

**Step 2: Add keys module to main.rs**

```rust
mod keys;

use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    println!("═══════════════════════════════════════════════════════════");
    println!("  MULTISIG ORCHESTRATOR POC — Stokenet");
    println!("═══════════════════════════════════════════════════════════");
    println!();

    // Load test keys
    let signers = keys::dao_signers();
    let fee_payer = keys::fee_payer();
    let funder = keys::funding_account();

    println!("[KEYS] Loaded {} DAO signers + fee payer + funder", signers.len());

    Ok(())
}
```

**Step 3: Verify it compiles**

Run: `cd multisig-orchestrator-poc && cargo build`
Expected: Compiles (will panic at runtime until real keys are added)

**Step 4: Commit**

```bash
git add multisig-orchestrator-poc/src/keys.rs
git commit -m "feat(poc): add test keys module with placeholder keys"
```

---

## Task 3: Gateway Client

**Files:**
- Create: `multisig-orchestrator-poc/src/gateway.rs`
- Modify: `multisig-orchestrator-poc/src/main.rs`

**Step 1: Create gateway.rs with Stokenet Gateway client**

```rust
//! Stokenet Gateway API client

use anyhow::{anyhow, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use radix_common::prelude::Decimal;

const STOKENET_GATEWAY: &str = "https://stokenet.radixdlt.com";

pub struct GatewayClient {
    client: Client,
    base_url: String,
}

impl GatewayClient {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
            base_url: STOKENET_GATEWAY.to_string(),
        }
    }

    /// Get current network status (epoch, state version)
    pub async fn get_network_status(&self) -> Result<NetworkStatusResponse> {
        let response = self.client
            .post(format!("{}/status/gateway-status", self.base_url))
            .json(&serde_json::json!({}))
            .send()
            .await?;

        let status: NetworkStatusResponse = response.json().await?;
        Ok(status)
    }

    /// Get account balance
    pub async fn get_account_balance(&self, address: &str) -> Result<Decimal> {
        let response = self.client
            .post(format!("{}/state/entity/details", self.base_url))
            .json(&EntityDetailsRequest {
                addresses: vec![address.to_string()],
                aggregation_level: Some("Vault".to_string()),
            })
            .send()
            .await?;

        let details: EntityDetailsResponse = response.json().await?;

        // Find XRD balance in fungible resources
        let xrd_address = "resource_tdx_2_1tknxxxxxxxxxradxrdxxxxxxxxx009923554798xxxxxxxxxtfd2jc";

        for item in &details.items {
            if let Some(resources) = &item.fungible_resources {
                for resource in &resources.items {
                    if resource.resource_address == xrd_address {
                        if let Some(vault) = resource.vaults.items.first() {
                            return Ok(Decimal::try_from(vault.amount.as_str())
                                .map_err(|_| anyhow!("Invalid decimal"))?);
                        }
                    }
                }
            }
        }

        Ok(Decimal::zero())
    }

    /// Submit transaction
    pub async fn submit_transaction(&self, compiled_tx_hex: &str) -> Result<SubmitResponse> {
        let response = self.client
            .post(format!("{}/transaction/submit", self.base_url))
            .json(&SubmitRequest {
                notarized_transaction_hex: compiled_tx_hex.to_string(),
            })
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow!("Submit failed: {}", error_text));
        }

        let result: SubmitResponse = response.json().await?;
        Ok(result)
    }

    /// Get transaction status
    pub async fn get_transaction_status(&self, intent_hash: &str) -> Result<TransactionStatusResponse> {
        let response = self.client
            .post(format!("{}/transaction/status", self.base_url))
            .json(&TransactionStatusRequest {
                intent_hash: intent_hash.to_string(),
            })
            .send()
            .await?;

        let status: TransactionStatusResponse = response.json().await?;
        Ok(status)
    }

    /// Poll until transaction is committed or failed
    pub async fn wait_for_commit(&self, intent_hash: &str) -> Result<String> {
        loop {
            let status = self.get_transaction_status(intent_hash).await?;

            match status.status.as_str() {
                "CommittedSuccess" => return Ok("CommittedSuccess".to_string()),
                "CommittedFailure" => {
                    return Err(anyhow!("Transaction failed: {:?}", status.error_message));
                }
                "Rejected" => {
                    return Err(anyhow!("Transaction rejected: {:?}", status.error_message));
                }
                "Pending" | "Unknown" => {
                    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                }
                other => {
                    return Err(anyhow!("Unexpected status: {}", other));
                }
            }
        }
    }
}

// Request/Response types

#[derive(Debug, Serialize)]
struct EntityDetailsRequest {
    addresses: Vec<String>,
    aggregation_level: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct EntityDetailsResponse {
    pub items: Vec<EntityItem>,
}

#[derive(Debug, Deserialize)]
pub struct EntityItem {
    pub address: String,
    pub fungible_resources: Option<FungibleResources>,
}

#[derive(Debug, Deserialize)]
pub struct FungibleResources {
    pub items: Vec<FungibleResource>,
}

#[derive(Debug, Deserialize)]
pub struct FungibleResource {
    pub resource_address: String,
    pub vaults: VaultCollection,
}

#[derive(Debug, Deserialize)]
pub struct VaultCollection {
    pub items: Vec<Vault>,
}

#[derive(Debug, Deserialize)]
pub struct Vault {
    pub amount: String,
}

#[derive(Debug, Deserialize)]
pub struct NetworkStatusResponse {
    pub ledger_state: LedgerState,
}

#[derive(Debug, Deserialize)]
pub struct LedgerState {
    pub epoch: u64,
    pub state_version: u64,
}

#[derive(Debug, Serialize)]
struct SubmitRequest {
    notarized_transaction_hex: String,
}

#[derive(Debug, Deserialize)]
pub struct SubmitResponse {
    pub duplicate: bool,
}

#[derive(Debug, Serialize)]
struct TransactionStatusRequest {
    intent_hash: String,
}

#[derive(Debug, Deserialize)]
pub struct TransactionStatusResponse {
    pub status: String,
    pub error_message: Option<String>,
}
```

**Step 2: Add gateway module to main.rs and test connectivity**

```rust
mod gateway;
mod keys;

use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    println!("═══════════════════════════════════════════════════════════");
    println!("  MULTISIG ORCHESTRATOR POC — Stokenet");
    println!("═══════════════════════════════════════════════════════════");
    println!();

    // Initialize gateway client
    let gateway = gateway::GatewayClient::new();

    // Test connectivity
    let status = gateway.get_network_status().await?;
    println!("[NETWORK] Connected to Stokenet");
    println!("  • Epoch: {}", status.ledger_state.epoch);
    println!("  • State version: {}", status.ledger_state.state_version);
    println!();

    // Load test keys
    let signers = keys::dao_signers();
    let fee_payer = keys::fee_payer();
    let _funder = keys::funding_account();

    println!("[KEYS] Loaded {} DAO signers + fee payer", signers.len());

    Ok(())
}
```

**Step 3: Test network connectivity**

Run: `cd multisig-orchestrator-poc && cargo run`
Expected: Prints Stokenet epoch and state version

**Step 4: Commit**

```bash
git add multisig-orchestrator-poc/src/gateway.rs
git commit -m "feat(poc): add Stokenet Gateway client"
```

---

## Task 4: Account Setup (Create DAO Treasury)

**Files:**
- Create: `multisig-orchestrator-poc/src/accounts.rs`
- Modify: `multisig-orchestrator-poc/src/main.rs`

**Step 1: Create accounts.rs for DAO treasury creation**

```rust
//! Account creation and access rule setup

use anyhow::Result;
use radix_common::prelude::*;
use radix_common::crypto::PublicKey;
use radix_transactions::prelude::*;
use radix_transactions::signing::PrivateKey;

use crate::gateway::GatewayClient;

/// Create a new account with 3-of-4 multisig access rule
pub async fn create_dao_treasury(
    gateway: &GatewayClient,
    funder_key: &PrivateKey,
    funder_address: ComponentAddress,
    signer_public_keys: [PublicKey; 4],
    initial_xrd: Decimal,
) -> Result<ComponentAddress> {
    let network = NetworkDefinition::stokenet();
    let status = gateway.get_network_status().await?;

    // Build 3-of-4 access rule
    let access_rule = rule!(require_n_of(
        3,
        signer_public_keys.iter().map(|pk| NonFungibleGlobalId::from_public_key(pk)).collect::<Vec<_>>()
    ));

    // Build manifest to create account and fund it
    let manifest = ManifestBuilder::new()
        .lock_fee(funder_address, dec!(10))
        .allocate_global_address(
            ACCOUNT_PACKAGE,
            ACCOUNT_BLUEPRINT,
            "address_reservation",
            "new_account_address",
        )
        .then(|builder| {
            let lookup = builder.name_lookup();
            let address_reservation = lookup.address_reservation("address_reservation");
            let new_account_address = lookup.named_address("new_account_address");

            builder
                .call_function(
                    ACCOUNT_PACKAGE,
                    ACCOUNT_BLUEPRINT,
                    "create_advanced",
                    manifest_args!(
                        OwnerRole::Fixed(access_rule.clone()),
                        Some(address_reservation)
                    ),
                )
                .withdraw_from_account(funder_address, XRD, initial_xrd)
                .take_all_from_worktop(XRD, "xrd_bucket")
                .call_method_with_name_lookup(
                    new_account_address,
                    "try_deposit_or_abort",
                    |lookup| (lookup.bucket("xrd_bucket"), None::<ResourceOrNonFungible>),
                )
        })
        .build();

    // Build and sign transaction
    let transaction = TransactionBuilder::new_v2()
        .transaction_header(TransactionHeaderV2 {
            notary_public_key: funder_key.public_key(),
            notary_is_signatory: true,
            tip_basis_points: 0,
        })
        .intent_header(IntentHeaderV2 {
            network_id: network.id,
            start_epoch_inclusive: Epoch::of(status.ledger_state.epoch),
            end_epoch_exclusive: Epoch::of(status.ledger_state.epoch + 10),
            intent_discriminator: rand::random(),
            min_proposer_timestamp_inclusive: None,
            max_proposer_timestamp_exclusive: None,
        })
        .manifest(manifest)
        .sign(funder_key)
        .notarize(funder_key)
        .build();

    // Submit
    let tx_hex = hex::encode(transaction.raw.as_slice());
    gateway.submit_transaction(&tx_hex).await?;

    // Wait for commit
    let intent_hash = transaction.transaction_hashes.transaction_intent_hash;
    let intent_hash_str = intent_hash.to_string(&TransactionHashBech32Encoder::new(&network));
    gateway.wait_for_commit(&intent_hash_str).await?;

    // Parse the created account address from the manifest
    // For simplicity, we'll derive it from the transaction
    // In production, you'd parse the receipt
    todo!("Extract created account address from transaction receipt")
}
```

**Note:** This task requires research into how to extract the created account address from the transaction receipt. The Gateway API has a `/transaction/committed-details` endpoint that returns receipt data.

**Step 2: Add receipt parsing to gateway.rs**

Add to `gateway.rs`:

```rust
/// Get committed transaction details (includes receipt)
pub async fn get_transaction_details(&self, intent_hash: &str) -> Result<TransactionDetailsResponse> {
    let response = self.client
        .post(format!("{}/transaction/committed-details", self.base_url))
        .json(&TransactionDetailsRequest {
            intent_hash: intent_hash.to_string(),
            opt_ins: TransactionOptIns {
                raw_hex: false,
                receipt_state_changes: true,
                receipt_events: true,
            },
        })
        .send()
        .await?;

    let details: TransactionDetailsResponse = response.json().await?;
    Ok(details)
}

#[derive(Debug, Serialize)]
struct TransactionDetailsRequest {
    intent_hash: String,
    opt_ins: TransactionOptIns,
}

#[derive(Debug, Serialize)]
struct TransactionOptIns {
    raw_hex: bool,
    receipt_state_changes: bool,
    receipt_events: bool,
}

#[derive(Debug, Deserialize)]
pub struct TransactionDetailsResponse {
    pub transaction: TransactionInfo,
}

#[derive(Debug, Deserialize)]
pub struct TransactionInfo {
    pub receipt: Option<TransactionReceipt>,
}

#[derive(Debug, Deserialize)]
pub struct TransactionReceipt {
    pub state_changes: Option<StateChanges>,
}

#[derive(Debug, Deserialize)]
pub struct StateChanges {
    pub new_global_entities: Vec<NewEntity>,
}

#[derive(Debug, Deserialize)]
pub struct NewEntity {
    pub entity_address: String,
    pub entity_type: String,
}
```

**Step 3: Complete the account creation function**

Update `accounts.rs` to extract the new account address:

```rust
// ... after wait_for_commit ...

// Get transaction receipt to find created account
let details = gateway.get_transaction_details(&intent_hash_str).await?;

let new_account = details.transaction.receipt
    .and_then(|r| r.state_changes)
    .map(|sc| sc.new_global_entities)
    .and_then(|entities| {
        entities.into_iter()
            .find(|e| e.entity_type == "GlobalAccount")
            .map(|e| e.entity_address)
    })
    .ok_or_else(|| anyhow::anyhow!("No account created in transaction"))?;

let account_address = ComponentAddress::try_from_bech32(
    &network.address_encoder(),
    &new_account
)?;

Ok(account_address)
```

**Step 4: Integrate into main.rs**

```rust
mod accounts;
mod gateway;
mod keys;

use anyhow::Result;
use radix_common::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {
    println!("═══════════════════════════════════════════════════════════");
    println!("  MULTISIG ORCHESTRATOR POC — Stokenet");
    println!("═══════════════════════════════════════════════════════════");
    println!();

    let gateway = gateway::GatewayClient::new();

    let status = gateway.get_network_status().await?;
    println!("[NETWORK] Connected to Stokenet");
    println!("  • Epoch: {}", status.ledger_state.epoch);
    println!();

    let signers = keys::dao_signers();
    let fee_payer = keys::fee_payer();
    let funder = keys::funding_account();

    println!("[PHASE 0] Account Setup");

    let signer_pks = [
        signers[0].public_key(),
        signers[1].public_key(),
        signers[2].public_key(),
        signers[3].public_key(),
    ];

    let dao_treasury = accounts::create_dao_treasury(
        &gateway,
        &funder.private_key,
        funder.address,
        signer_pks,
        dec!(500), // Fund with 500 XRD
    ).await?;

    println!("  • DAO Treasury created: {}", dao_treasury);
    println!("  • Access rule: 3 of 4 signers");
    println!("  • Funded with 500 XRD");
    println!("  ✓ Setup complete");
    println!();

    Ok(())
}
```

**Step 5: Test account creation**

Run: `cd multisig-orchestrator-poc && cargo run`
Expected: Creates DAO treasury account on Stokenet (requires real funded keys)

**Step 6: Commit**

```bash
git add multisig-orchestrator-poc/src/accounts.rs
git commit -m "feat(poc): add DAO treasury account creation with 3-of-4 access rule"
```

---

## Task 5: Sub-Intent Creation

**Files:**
- Create: `multisig-orchestrator-poc/src/subintent.rs`
- Modify: `multisig-orchestrator-poc/src/main.rs`

**Step 1: Create subintent.rs for withdrawal sub-intent**

```rust
//! Sub-intent creation and signing

use anyhow::Result;
use radix_common::prelude::*;
use radix_transactions::prelude::*;
use radix_transactions::model::{
    PartialTransactionV2, SubintentManifestV2, IntentSignaturesV2,
    SignedPartialTransactionV2, IntentSignatureV1,
};
use radix_transactions::signing::PrivateKey;

/// Create a withdrawal sub-intent (unsigned)
pub fn create_withdrawal_subintent(
    network: &NetworkDefinition,
    dao_treasury: ComponentAddress,
    recipient: ComponentAddress,
    amount: Decimal,
    current_epoch: u64,
    expiry_epochs: u64,
) -> Result<PartialTransactionV2> {
    // Build sub-intent manifest
    let manifest = ManifestBuilder::new_subintent_v2()
        .withdraw_from_account(dao_treasury, XRD, amount)
        .take_all_from_worktop(XRD, "withdrawn")
        .call_method_with_name_lookup(
            recipient,
            "try_deposit_or_abort",
            |lookup| (lookup.bucket("withdrawn"), None::<ResourceOrNonFungible>),
        )
        .yield_to_parent(())
        .build();

    let subintent_manifest = SubintentManifestV2 {
        instructions: manifest.instructions,
        blobs: manifest.blobs,
        children: manifest.children,
    };

    let partial_tx = PartialTransactionV2Builder::new()
        .intent_header(IntentHeaderV2 {
            network_id: network.id,
            start_epoch_inclusive: Epoch::of(current_epoch),
            end_epoch_exclusive: Epoch::of(current_epoch + expiry_epochs),
            intent_discriminator: rand::random(),
            min_proposer_timestamp_inclusive: None,
            max_proposer_timestamp_exclusive: None,
        })
        .manifest(subintent_manifest)
        .build()?;

    Ok(partial_tx)
}

/// Sign a partial transaction with a private key
pub fn sign_subintent(
    partial_tx: &PartialTransactionV2,
    private_key: &PrivateKey,
) -> Result<IntentSignatureV1> {
    let prepared = partial_tx.prepare(&PreparationSettings::latest())?;
    let subintent_hash = prepared.subintent_hash();

    let signature = private_key.sign_with_public_key(&subintent_hash);
    Ok(IntentSignatureV1(signature))
}

/// Combine partial transaction with multiple signatures
pub fn create_signed_partial_transaction(
    partial_tx: PartialTransactionV2,
    signatures: Vec<IntentSignatureV1>,
) -> SignedPartialTransactionV2 {
    SignedPartialTransactionV2 {
        partial_transaction: partial_tx,
        root_subintent_signatures: IntentSignaturesV2 { signatures },
        non_root_subintent_signatures: Default::default(),
    }
}

/// Get the subintent hash for display
pub fn get_subintent_hash(
    partial_tx: &PartialTransactionV2,
    network: &NetworkDefinition,
) -> Result<String> {
    let prepared = partial_tx.prepare(&PreparationSettings::latest())?;
    let hash = prepared.subintent_hash();
    Ok(hash.to_string(&TransactionHashBech32Encoder::new(network)))
}
```

**Step 2: Integrate sub-intent creation into main.rs**

Add after Phase 0:

```rust
// ... Phase 0 code ...

println!("[PHASE 1] Sub-Intent Creation");

let network = NetworkDefinition::stokenet();
let recipient = signers[0].address; // Send to first signer

let partial_tx = subintent::create_withdrawal_subintent(
    &network,
    dao_treasury,
    recipient,
    dec!(100), // Withdraw 100 XRD
    status.ledger_state.epoch,
    100, // Valid for 100 epochs
)?;

let subintent_hash = subintent::get_subintent_hash(&partial_tx, &network)?;
println!("  • Action: Withdraw 100 XRD → {}", signers[0].name);
println!("  • Sub-intent hash: {}", subintent_hash);
println!("  ✓ Sub-intent created");
println!();
```

**Step 3: Test sub-intent creation compiles**

Run: `cd multisig-orchestrator-poc && cargo build`
Expected: Compiles successfully

**Step 4: Commit**

```bash
git add multisig-orchestrator-poc/src/subintent.rs
git commit -m "feat(poc): add sub-intent creation for DAO withdrawal"
```

---

## Task 6: Signature Collection

**Files:**
- Modify: `multisig-orchestrator-poc/src/main.rs`

**Step 1: Add signature collection phase**

Add after Phase 1:

```rust
println!("[PHASE 2] Signature Collection");

let mut signatures = Vec::new();

// Collect 3 of 4 signatures (skip signer 4 to prove 3-of-4 works)
for i in 0..3 {
    let sig = subintent::sign_subintent(&partial_tx, &signers[i].private_key)?;
    signatures.push(sig);
    println!("  • {}: ✓ signed", signers[i].name);
}

println!("  • {}: (skipped — threshold already met)", signers[3].name);
println!("  ✓ 3/4 signatures collected");
println!();

// Create signed partial transaction
let signed_partial_tx = subintent::create_signed_partial_transaction(
    partial_tx,
    signatures,
);
```

**Step 2: Verify it compiles**

Run: `cd multisig-orchestrator-poc && cargo build`
Expected: Compiles successfully

**Step 3: Commit**

```bash
git commit -am "feat(poc): add signature collection phase (3 of 4)"
```

---

## Task 7: Transaction Composition and Submission

**Files:**
- Create: `multisig-orchestrator-poc/src/transaction.rs`
- Modify: `multisig-orchestrator-poc/src/main.rs`

**Step 1: Create transaction.rs for main transaction building**

```rust
//! Main transaction composition and submission

use anyhow::Result;
use radix_common::prelude::*;
use radix_transactions::prelude::*;
use radix_transactions::model::SignedPartialTransactionV2;
use radix_transactions::signing::PrivateKey;

/// Build and submit the main transaction with fee payer
pub fn build_main_transaction(
    network: &NetworkDefinition,
    current_epoch: u64,
    fee_payer_address: ComponentAddress,
    fee_payer_key: &PrivateKey,
    signed_subintent: SignedPartialTransactionV2,
    lock_fee_amount: Decimal,
) -> Result<(NotarizedTransactionV2, String)> {
    let transaction = TransactionBuilder::new_v2()
        .transaction_header(TransactionHeaderV2 {
            notary_public_key: fee_payer_key.public_key(),
            notary_is_signatory: true, // Fee payer is also a signer
            tip_basis_points: 0,
        })
        .intent_header(IntentHeaderV2 {
            network_id: network.id,
            start_epoch_inclusive: Epoch::of(current_epoch),
            end_epoch_exclusive: Epoch::of(current_epoch + 10),
            intent_discriminator: rand::random(),
            min_proposer_timestamp_inclusive: None,
            max_proposer_timestamp_exclusive: None,
        })
        .add_signed_child("dao_withdraw", signed_subintent)
        .manifest_builder(|builder| {
            builder
                .lock_fee(fee_payer_address, lock_fee_amount)
                .yield_to_child("dao_withdraw", ())
        })
        .sign(fee_payer_key)
        .notarize(fee_payer_key)
        .build();

    let intent_hash = transaction.transaction_hashes.transaction_intent_hash
        .to_string(&TransactionHashBech32Encoder::new(network));

    Ok((transaction, intent_hash))
}

/// Get hex-encoded compiled transaction for submission
pub fn to_hex(transaction: &NotarizedTransactionV2) -> String {
    hex::encode(transaction.raw.as_slice())
}
```

**Step 2: Complete main.rs with submission**

```rust
mod accounts;
mod gateway;
mod keys;
mod subintent;
mod transaction;

use anyhow::Result;
use radix_common::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {
    println!("═══════════════════════════════════════════════════════════");
    println!("  MULTISIG ORCHESTRATOR POC — Stokenet");
    println!("═══════════════════════════════════════════════════════════");
    println!();

    let gateway = gateway::GatewayClient::new();
    let network = NetworkDefinition::stokenet();

    let status = gateway.get_network_status().await?;
    println!("[NETWORK] Connected to Stokenet");
    println!("  • Epoch: {}", status.ledger_state.epoch);
    println!();

    // Load keys
    let signers = keys::dao_signers();
    let fee_payer = keys::fee_payer();
    let funder = keys::funding_account();

    // === PHASE 0: Account Setup ===
    println!("[PHASE 0] Account Setup");

    let signer_pks = [
        signers[0].public_key(),
        signers[1].public_key(),
        signers[2].public_key(),
        signers[3].public_key(),
    ];

    let dao_treasury = accounts::create_dao_treasury(
        &gateway,
        &funder.private_key,
        funder.address,
        signer_pks,
        dec!(500),
    ).await?;

    println!("  • DAO Treasury created: {}", dao_treasury);
    println!("  • Access rule: 3 of 4 signers");
    println!("  • Funded with 500 XRD");
    println!("  ✓ Setup complete");
    println!();

    // === PHASE 1: Sub-Intent Creation ===
    println!("[PHASE 1] Sub-Intent Creation");

    let recipient = signers[0].address;

    let partial_tx = subintent::create_withdrawal_subintent(
        &network,
        dao_treasury,
        recipient,
        dec!(100),
        status.ledger_state.epoch,
        100,
    )?;

    let subintent_hash = subintent::get_subintent_hash(&partial_tx, &network)?;
    println!("  • Action: Withdraw 100 XRD → {}", signers[0].name);
    println!("  • Sub-intent hash: {}", subintent_hash);
    println!("  ✓ Sub-intent created");
    println!();

    // === PHASE 2: Signature Collection ===
    println!("[PHASE 2] Signature Collection");

    let mut signatures = Vec::new();
    for i in 0..3 {
        let sig = subintent::sign_subintent(&partial_tx, &signers[i].private_key)?;
        signatures.push(sig);
        println!("  • {}: ✓ signed", signers[i].name);
    }
    println!("  • {}: (skipped — threshold already met)", signers[3].name);
    println!("  ✓ 3/4 signatures collected");
    println!();

    let signed_partial_tx = subintent::create_signed_partial_transaction(partial_tx, signatures);

    // === PHASE 3: Transaction Composition ===
    println!("[PHASE 3] Transaction Composition");

    let (notarized_tx, intent_hash) = transaction::build_main_transaction(
        &network,
        status.ledger_state.epoch,
        fee_payer.address,
        &fee_payer.private_key,
        signed_partial_tx,
        dec!(10),
    )?;

    println!("  • Fee payer: {}", fee_payer.name);
    println!("  • Lock fee: 10 XRD");
    println!("  ✓ Main intent built");
    println!();

    // === PHASE 4: Submission ===
    println!("[PHASE 4] Submission");

    let tx_hex = transaction::to_hex(&notarized_tx);
    gateway.submit_transaction(&tx_hex).await?;
    println!("  • Submitted: {}", intent_hash);

    let final_status = gateway.wait_for_commit(&intent_hash).await?;
    println!("  • Status: {}", final_status);
    println!("  ✓ Transaction committed");
    println!();

    // === RESULT ===
    let treasury_balance = gateway.get_account_balance(
        &dao_treasury.to_string(&network.address_encoder())
    ).await?;
    let recipient_balance = gateway.get_account_balance(
        &recipient.to_string(&network.address_encoder())
    ).await?;

    println!("[RESULT] ══════════════════════════════════════════════════");
    println!("  DAO Treasury: {} XRD", treasury_balance);
    println!("  Recipient:    {} XRD", recipient_balance);
    println!("  SUCCESS — Multisig withdrawal completed");
    println!("═══════════════════════════════════════════════════════════");

    Ok(())
}
```

**Step 3: Test full flow compiles**

Run: `cd multisig-orchestrator-poc && cargo build`
Expected: Compiles successfully

**Step 4: Commit**

```bash
git add multisig-orchestrator-poc/src/transaction.rs
git commit -am "feat(poc): add transaction composition and submission"
```

---

## Task 8: Generate and Fund Test Keys

**Files:**
- Modify: `multisig-orchestrator-poc/src/keys.rs`

**Step 1: Create a key generation utility**

Create a separate binary to generate keys:

```rust
// src/bin/generate_keys.rs
use radix_common::crypto::Ed25519PrivateKey;
use radix_common::network::NetworkDefinition;
use radix_transactions::signing::PrivateKey;

fn main() {
    let network = NetworkDefinition::stokenet();

    println!("Generated Stokenet test keys:");
    println!("==============================");
    println!();

    for i in 1..=6 {
        let private_key = Ed25519PrivateKey::from_u64(i as u64).unwrap();
        let public_key = PrivateKey::Ed25519(private_key.clone()).public_key();

        // Derive account address from public key
        let address = radix_common::prelude::ComponentAddress::virtual_account_from_public_key(
            &public_key
        );

        println!("Key {}:", i);
        println!("  Private: {}", hex::encode(private_key.to_bytes()));
        println!("  Address: {}", address.to_string(&network.address_encoder()));
        println!();
    }

    println!("Fund these addresses using the Stokenet faucet:");
    println!("https://stokenet-console.radixdlt.com/faucet");
}
```

**Step 2: Add binary to Cargo.toml**

```toml
[[bin]]
name = "generate_keys"
path = "src/bin/generate_keys.rs"

[[bin]]
name = "multisig_poc"
path = "src/main.rs"
```

**Step 3: Generate keys and fund them**

Run: `cd multisig-orchestrator-poc && cargo run --bin generate_keys`
Expected: Prints 6 keypairs with Stokenet addresses

Then:
1. Go to https://stokenet-console.radixdlt.com/faucet
2. Fund each address with XRD
3. Update `keys.rs` with the real values

**Step 4: Commit**

```bash
git add multisig-orchestrator-poc/src/bin/
git commit -am "feat(poc): add key generation utility"
```

---

## Task 9: End-to-End Test

**Files:**
- None (uses existing code)

**Step 1: Run the full POC**

Run: `cd multisig-orchestrator-poc && cargo run --bin multisig_poc`

Expected output:

```
═══════════════════════════════════════════════════════════
  MULTISIG ORCHESTRATOR POC — Stokenet
═══════════════════════════════════════════════════════════

[NETWORK] Connected to Stokenet
  • Epoch: 12345

[PHASE 0] Account Setup
  • DAO Treasury created: account_tdx_2_...
  • Access rule: 3 of 4 signers
  • Funded with 500 XRD
  ✓ Setup complete

[PHASE 1] Sub-Intent Creation
  • Action: Withdraw 100 XRD → Signer1
  • Sub-intent hash: subtxid_tdx_2_...
  ✓ Sub-intent created

[PHASE 2] Signature Collection
  • Signer1: ✓ signed
  • Signer2: ✓ signed
  • Signer3: ✓ signed
  • Signer4: (skipped — threshold already met)
  ✓ 3/4 signatures collected

[PHASE 3] Transaction Composition
  • Fee payer: FeePayer
  • Lock fee: 10 XRD
  ✓ Main intent built

[PHASE 4] Submission
  • Submitted: txid_tdx_2_...
  • Status: CommittedSuccess
  ✓ Transaction committed

[RESULT] ══════════════════════════════════════════════════
  DAO Treasury: 400 XRD
  Recipient:    +100 XRD
  SUCCESS — Multisig withdrawal completed
═══════════════════════════════════════════════════════════
```

**Step 2: Commit final state**

```bash
git commit -am "feat(poc): complete multisig orchestrator POC"
```

---

## Notes for Implementation

1. **Radix crate versions:** The `radix-transactions` and related crates are from the `radixdlt-scrypto` monorepo. The tag `v1.3.0` corresponds to the Cuttlefish update. Check the latest tag if this doesn't compile.

2. **API type mismatches:** The Gateway API response types may differ slightly from what's documented. Adjust the serde structs as needed based on actual responses.

3. **Address encoding:** Stokenet uses `account_tdx_2_...` prefix. Mainnet uses `account_rdx1_...`.

4. **Epoch handling:** Re-fetch the current epoch before building transactions if the POC takes a while to run.

5. **Error messages:** The Gateway returns detailed error messages. Use them to debug access rule issues.
