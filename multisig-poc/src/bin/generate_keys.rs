//! Generate deterministic Ed25519 keypairs for Stokenet testing.
//!
//! This utility generates test keys and derives their Stokenet account addresses
//! for use with the multisig orchestrator POC. The keys are deterministic based
//! on seed values, so running this multiple times produces the same keys.
//!
//! Usage: cargo run --bin generate_keys

use radix_common::address::AddressBech32Encoder;
use radix_common::network::NetworkDefinition;
use radix_common::prelude::*;

/// Number of signer keys to generate (for n-of-m multisig).
const NUM_SIGNERS: u64 = 4;

/// Seed for the notary/fee-payer key (distinct from signer seeds).
const NOTARY_SEED: u64 = 100;

fn main() {
    println!("Generated Stokenet Test Keys");
    println!("==============================");
    println!();

    let network = NetworkDefinition::stokenet();
    let encoder = AddressBech32Encoder::new(&network);

    // Generate signer keys (seeds 1 through NUM_SIGNERS)
    for seed in 1..=NUM_SIGNERS {
        print_key_info(&encoder, seed, &format!("Signer {}", seed));
    }

    // Generate notary key
    print_key_info(&encoder, NOTARY_SEED, "Notary (Fee Payer)");

    // Print faucet instructions
    println!();
    println!("Fund these addresses using the Stokenet faucet:");
    println!("  https://stokenet-console.radixdlt.com/faucet");
    println!();
    println!("Note: These are deterministic test keys derived from seed values.");
    println!("      Do NOT use these for real funds - private keys are public!");
}

/// Generate and print key information for a given seed.
fn print_key_info(encoder: &AddressBech32Encoder, seed: u64, label: &str) {
    // Generate deterministic Ed25519 key from seed
    let private_key = Ed25519PrivateKey::from_u64(seed)
        .expect("Failed to generate key from seed");
    let public_key = private_key.public_key();

    // Derive the preallocated account address
    let account_address = ComponentAddress::preallocated_account_from_public_key(&public_key);

    // Encode address to Bech32 for Stokenet
    let address_bech32 = encoder
        .encode(account_address.as_bytes())
        .expect("Failed to encode address");

    // Format output
    println!("{} (seed={}):", label, seed);
    println!("  Private Key: {}", hex::encode(private_key.to_bytes()));
    println!("  Public Key:  {}", hex::encode(public_key.0));
    println!("  Address:     {}", address_bech32);
    println!();
}
