use std::collections::HashSet;

use anyhow::Result;
use radix_common::address::AddressBech32Encoder;
use radix_common::network::NetworkDefinition;
use radix_common::prelude::ManifestGlobalAddress;
use radix_transactions::manifest::{InvocationKind, ManifestInstructionEffect, ReadableManifest};
use radix_transactions::prelude::*;

/// Account methods that require the account owner's authorization.
const AUTH_REQUIRING_METHODS: &[&str] = &[
    "withdraw",
    "withdraw_non_fungibles",
    "lock_fee",
    "lock_contingent_fee",
    "lock_fee_and_withdraw",
    "create_proof_of_amount",
    "create_proof_of_non_fungibles",
    "securify",
];

/// Iterate the instruction effects of a compiled subintent manifest and return
/// the bech32-encoded addresses of all accounts that are invoked with methods
/// requiring owner authorization.
pub fn extract_accounts_requiring_auth(
    manifest: &SubintentManifestV2,
    network_definition: &NetworkDefinition,
) -> Result<HashSet<String>> {
    let encoder = AddressBech32Encoder::new(network_definition);
    let mut accounts = HashSet::new();

    for effect in manifest.iter_instruction_effects() {
        if let ManifestInstructionEffect::Invocation { kind, .. } = effect {
            if let InvocationKind::Method {
                address, method, ..
            } = kind
            {
                if let ManifestGlobalAddress::Static(global_addr) = address {
                    let node_id = global_addr.as_node_id();
                    if node_id.is_global_account() && AUTH_REQUIRING_METHODS.contains(&method) {
                        let bech32 = encoder
                            .encode(&node_id.0)
                            .map_err(|e| anyhow::anyhow!("Failed to encode address: {e:?}"))?;
                        accounts.insert(bech32);
                    }
                }
            }
        }
    }

    Ok(accounts)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transaction_builder;

    fn stokenet() -> NetworkDefinition {
        NetworkDefinition::stokenet()
    }

    #[test]
    fn extracts_withdraw_account() {
        let manifest_text = r#"CALL_METHOD
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
;"#;
        let network = stokenet();
        let manifest = transaction_builder::compile_subintent_manifest(manifest_text, 2).unwrap();
        let accounts = extract_accounts_requiring_auth(&manifest, &network).unwrap();

        // Only the withdrawing account requires auth, deposit does not
        assert_eq!(accounts.len(), 1);
        assert!(accounts
            .contains("account_tdx_2_1cx3u3xgr9anc9fk54dxzsz6k2n6lnadludkx4mx5re5erl8jt9lpnp"));
    }

    #[test]
    fn ignores_deposit_only_accounts() {
        // Withdraw from one account, deposit to another — only the withdrawing
        // account requires auth, not the depositing one.
        let manifest_text = r#"CALL_METHOD
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
;"#;
        let network = stokenet();
        let manifest = transaction_builder::compile_subintent_manifest(manifest_text, 2).unwrap();
        let accounts = extract_accounts_requiring_auth(&manifest, &network).unwrap();

        // deposit does not require auth — only the withdrawing account is returned
        assert_eq!(accounts.len(), 1);
        assert!(!accounts
            .contains("account_tdx_2_12xsvygvltz4uhsht6tdrfxktzpmnl77r0d40j8agmujgdj02el3l9v"));
    }

    #[test]
    fn extracts_multiple_auth_accounts() {
        let manifest_text = r#"CALL_METHOD
    Address("account_tdx_2_1cx3u3xgr9anc9fk54dxzsz6k2n6lnadludkx4mx5re5erl8jt9lpnp")
    "withdraw"
    Address("resource_tdx_2_1tknxxxxxxxxxradxrdxxxxxxxxx009923554798xxxxxxxxxtfd2jc")
    Decimal("50")
;
CALL_METHOD
    Address("account_tdx_2_12xsvygvltz4uhsht6tdrfxktzpmnl77r0d40j8agmujgdj02el3l9v")
    "withdraw"
    Address("resource_tdx_2_1tknxxxxxxxxxradxrdxxxxxxxxx009923554798xxxxxxxxxtfd2jc")
    Decimal("50")
;"#;
        let network = stokenet();
        let manifest = transaction_builder::compile_subintent_manifest(manifest_text, 2).unwrap();
        let accounts = extract_accounts_requiring_auth(&manifest, &network).unwrap();

        assert_eq!(accounts.len(), 2);
    }

    #[test]
    fn deduplicates_same_account() {
        let manifest_text = r#"CALL_METHOD
    Address("account_tdx_2_1cx3u3xgr9anc9fk54dxzsz6k2n6lnadludkx4mx5re5erl8jt9lpnp")
    "withdraw"
    Address("resource_tdx_2_1tknxxxxxxxxxradxrdxxxxxxxxx009923554798xxxxxxxxxtfd2jc")
    Decimal("50")
;
CALL_METHOD
    Address("account_tdx_2_1cx3u3xgr9anc9fk54dxzsz6k2n6lnadludkx4mx5re5erl8jt9lpnp")
    "create_proof_of_amount"
    Address("resource_tdx_2_1tknxxxxxxxxxradxrdxxxxxxxxx009923554798xxxxxxxxxtfd2jc")
    Decimal("1")
;"#;
        let network = stokenet();
        let manifest = transaction_builder::compile_subintent_manifest(manifest_text, 2).unwrap();
        let accounts = extract_accounts_requiring_auth(&manifest, &network).unwrap();

        assert_eq!(accounts.len(), 1);
    }

    #[test]
    fn handles_lock_fee_method() {
        let manifest_text = r#"CALL_METHOD
    Address("account_tdx_2_1cx3u3xgr9anc9fk54dxzsz6k2n6lnadludkx4mx5re5erl8jt9lpnp")
    "lock_fee"
    Decimal("10")
;"#;
        let network = stokenet();
        let manifest = transaction_builder::compile_subintent_manifest(manifest_text, 2).unwrap();
        let accounts = extract_accounts_requiring_auth(&manifest, &network).unwrap();

        assert_eq!(accounts.len(), 1);
    }
}
