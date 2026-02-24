use anyhow::Result;
use radix_common::prelude::*;
use radix_engine_interface::blueprints::account::ACCOUNT_BLUEPRINT;
use radix_engine_interface::prelude::*;
use radix_transactions::manifest::decompiler::decompile;
use radix_transactions::prelude::*;

/// All the info needed to build a create-multisig-account manifest.
pub struct ManifestConfig {
    pub network: NetworkDefinition,
    pub badges: Vec<NonFungibleGlobalId>,
    pub threshold: u8,
    pub fee_payer: ComponentAddress,
    pub fee_amount: Decimal,
    /// XRD to deposit into the new account (0 = skip).
    pub fund_amount: Decimal,
}

/// Build the transaction manifest and decompile it to an RTM string.
pub fn build_and_decompile(config: &ManifestConfig) -> Result<String> {
    let access_rule = build_n_of_m_access_rule(config.threshold, &config.badges)?;
    let owner_role = OwnerRole::Fixed(access_rule);

    let manifest = if config.fund_amount > Decimal::ZERO {
        build_funded_manifest(config, owner_role)
    } else {
        build_simple_manifest(config, owner_role)
    };

    decompile(&manifest, &config.network).map_err(|e| anyhow::anyhow!("Decompile failed: {e:?}"))
}

/// Manifest without funding: just lock_fee + create_account.
fn build_simple_manifest(config: &ManifestConfig, owner_role: OwnerRole) -> TransactionManifestV2 {
    ManifestBuilder::new_v2()
        .lock_fee(config.fee_payer, config.fee_amount)
        .create_account_with_owner(None, owner_role)
        .build()
}

/// Manifest with funding: allocate address, create account, withdraw XRD, deposit.
fn build_funded_manifest(config: &ManifestConfig, owner_role: OwnerRole) -> TransactionManifestV2 {
    ManifestBuilder::new_v2()
        .lock_fee(config.fee_payer, config.fee_amount)
        .allocate_global_address(
            ACCOUNT_PACKAGE,
            ACCOUNT_BLUEPRINT,
            "account_reservation",
            "account_address",
        )
        .create_account_with_owner("account_reservation", owner_role)
        .withdraw_from_account(config.fee_payer, XRD, config.fund_amount)
        .take_all_from_worktop(XRD, "xrd_bucket")
        .deposit("account_address", "xrd_bucket")
        .build()
}

/// Build an access rule requiring `n` of `m` signature badges.
fn build_n_of_m_access_rule(
    required_count: u8,
    badges: &[NonFungibleGlobalId],
) -> Result<AccessRule> {
    if badges.is_empty() {
        anyhow::bail!("At least one signer is required");
    }
    if required_count == 0 {
        anyhow::bail!("Threshold must be at least 1");
    }
    if required_count as usize > badges.len() {
        anyhow::bail!(
            "Threshold ({}) cannot exceed number of signers ({})",
            required_count,
            badges.len()
        );
    }

    let resources: Vec<ResourceOrNonFungible> = badges
        .iter()
        .map(|b| ResourceOrNonFungible::NonFungible(b.clone()))
        .collect();

    Ok(AccessRule::Protected(require_n_of(required_count, resources)))
}
