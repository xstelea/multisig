use anyhow::Result;
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
}
