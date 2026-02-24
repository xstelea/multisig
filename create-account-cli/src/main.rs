mod manifest;

use anyhow::{Context, Result};
use inquire::{Confirm, CustomType, Select, Text};
use radix_common::address::AddressBech32Decoder;
use radix_common::network::NetworkDefinition;
use radix_common::prelude::*;

fn main() -> Result<()> {
    // 1. Network selection
    let network_name = Select::new("Select network:", vec!["Stokenet", "Mainnet"])
        .prompt()
        .context("Network selection cancelled")?;
    let network = match network_name {
        "Mainnet" => NetworkDefinition::mainnet(),
        _ => NetworkDefinition::stokenet(),
    };
    let decoder = AddressBech32Decoder::new(&network);

    // 2. Collect signers
    let mut badges: Vec<NonFungibleGlobalId> = Vec::new();

    loop {
        let label = format!("Signer {} — enter public key (hex) or badge ID:", badges.len() + 1);
        let input = Text::new(&label)
            .with_help_message("64 hex chars = Ed25519 pubkey, resource_...:[] = badge")
            .prompt()
            .context("Signer input cancelled")?;
        let input = input.trim().to_string();

        match parse_signer_input(&input, &decoder) {
            Ok(badge) => {
                let badge_display = format_badge(&badge, &network);
                println!("  ✓ Badge: {badge_display}");
                badges.push(badge);
            }
            Err(e) => {
                println!("  ✗ {e}");
                continue;
            }
        }

        if badges.len() >= 2 {
            let add_more = Confirm::new("Add another signer?")
                .with_default(false)
                .prompt()
                .context("Prompt cancelled")?;
            if !add_more {
                break;
            }
        } else {
            let add_more = Confirm::new("Add another signer?")
                .with_default(true)
                .prompt()
                .context("Prompt cancelled")?;
            if !add_more {
                break;
            }
        }
    }

    if badges.is_empty() {
        anyhow::bail!("At least one signer is required");
    }

    // 3. Threshold
    let max = badges.len() as u8;
    let threshold: u8 = if max == 1 {
        println!("  Threshold: 1 of 1 (single signer)");
        1
    } else {
        CustomType::<u8>::new(&format!("Signature threshold (1-{max}):"))
            .with_default(max)
            .with_error_message(&format!("Enter a number between 1 and {max}"))
            .with_parser(&move |s: &str| {
                s.parse::<u8>()
                    .ok()
                    .filter(|&n| n >= 1 && n <= max)
                    .ok_or(())
            })
            .prompt()
            .context("Threshold input cancelled")?
    };

    // 4. Fee payer
    let expected_prefix = if network.id == NetworkDefinition::mainnet().id {
        "account_rdx"
    } else {
        "account_tdx"
    };
    let fee_payer_str = Text::new("Fee payer account address:")
        .with_validator(move |s: &str| {
            if s.trim().starts_with(expected_prefix) {
                Ok(inquire::validator::Validation::Valid)
            } else {
                Ok(inquire::validator::Validation::Invalid(
                    format!("Address must start with '{expected_prefix}'").into(),
                ))
            }
        })
        .prompt()
        .context("Fee payer input cancelled")?;
    let fee_payer = decode_component_address(fee_payer_str.trim(), &network)?;

    // 5. Funding amount
    let fund_input = Text::new("Initial XRD funding amount (0 to skip):")
        .with_default("0")
        .prompt()
        .context("Fund amount cancelled")?;
    let fund_amount: Decimal = fund_input
        .trim()
        .parse()
        .context("Invalid decimal for funding amount")?;

    // 6. Fee amount
    let fee_input = Text::new("Fee amount in XRD:")
        .with_default("10")
        .prompt()
        .context("Fee amount cancelled")?;
    let fee_amount: Decimal = fee_input
        .trim()
        .parse()
        .context("Invalid decimal for fee amount")?;

    // 7. Summary
    let fee_payer_display = format_address(fee_payer, &network);
    println!();
    println!("── Summary ──────────────────────────");
    println!("  Network:    {network_name}");
    println!("  Signers:    {}", badges.len());
    println!("  Threshold:  {threshold} of {}", badges.len());
    println!("  Fee payer:  {fee_payer_display}");
    if fund_amount > Decimal::ZERO {
        println!("  Funding:    {fund_amount} XRD");
    }
    println!("  Fee:        {fee_amount} XRD");
    println!("─────────────────────────────────────");
    println!();

    // 8. Confirm
    let go = Confirm::new("Generate manifest?")
        .with_default(true)
        .prompt()
        .context("Confirmation cancelled")?;
    if !go {
        println!("Aborted.");
        return Ok(());
    }

    // 9. Build and print
    let config = manifest::ManifestConfig {
        network,
        badges,
        threshold,
        fee_payer,
        fee_amount,
        fund_amount,
    };
    let rtm = manifest::build_and_decompile(&config)?;
    println!();
    println!("{rtm}");

    Ok(())
}

/// Parse signer input: auto-detect hex pubkey vs badge ID.
fn parse_signer_input(
    input: &str,
    decoder: &AddressBech32Decoder,
) -> Result<NonFungibleGlobalId> {
    // Strip potential 0x prefix
    let cleaned = input.strip_prefix("0x").unwrap_or(input);

    // 64 hex chars = Ed25519 public key
    if cleaned.len() == 64 && cleaned.chars().all(|c| c.is_ascii_hexdigit()) {
        let bytes = hex::decode(cleaned).context("Invalid hex")?;
        let pubkey = Ed25519PublicKey::try_from(bytes.as_slice())
            .map_err(|e| anyhow::anyhow!("Invalid Ed25519 public key: {e:?}"))?;
        return Ok(NonFungibleGlobalId::from_public_key(&pubkey));
    }

    // Starts with resource_ and contains :[ → parse as NonFungibleGlobalId
    if input.starts_with("resource_") && input.contains(":[") {
        return NonFungibleGlobalId::try_from_canonical_string(decoder, input)
            .map_err(|e| anyhow::anyhow!("Invalid badge ID: {e:?}"));
    }

    anyhow::bail!(
        "Unrecognized format. Enter a 64-char hex public key or a badge ID (resource_...:[...])"
    )
}

/// Decode a bech32m account address to ComponentAddress.
fn decode_component_address(bech32: &str, network: &NetworkDefinition) -> Result<ComponentAddress> {
    let decoder = AddressBech32Decoder::new(network);
    let (_entity_type, bytes) = decoder
        .validate_and_decode(bech32)
        .map_err(|e| anyhow::anyhow!("Invalid address: {e:?}"))?;
    let node_id: [u8; NodeId::LENGTH] = bytes
        .try_into()
        .map_err(|_| anyhow::anyhow!("Invalid address byte length"))?;
    ComponentAddress::try_from(node_id)
        .map_err(|e| anyhow::anyhow!("Not a valid component address: {e:?}"))
}

/// Format a NonFungibleGlobalId for display using the network encoder.
fn format_badge(badge: &NonFungibleGlobalId, network: &NetworkDefinition) -> String {
    let encoder = radix_common::address::AddressBech32Encoder::new(network);
    badge.to_canonical_string(&encoder)
}

/// Format a ComponentAddress for display.
fn format_address(address: ComponentAddress, network: &NetworkDefinition) -> String {
    let encoder = radix_common::address::AddressBech32Encoder::new(network);
    encoder.encode(address.as_bytes()).unwrap_or_else(|_| format!("{address:?}"))
}
