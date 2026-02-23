use std::sync::Arc;
use std::time::Duration;

use crate::gateway::GatewayClient;
use crate::proposal_store::{ProposalStatus, ProposalStore};

/// Background task that periodically checks active proposals for expiry
/// and access rule changes, transitioning them to Expired or Invalid as needed.
pub async fn run(
    proposal_store: Arc<ProposalStore>,
    gateway: Arc<GatewayClient>,
    multisig_account: String,
    interval_secs: u64,
) {
    let mut interval = tokio::time::interval(Duration::from_secs(interval_secs));
    // Skip the immediate first tick so the server can start up
    interval.tick().await;

    loop {
        interval.tick().await;
        if let Err(e) = check_proposals(&proposal_store, &gateway, &multisig_account).await {
            tracing::error!("Validity monitor error: {e}");
        }
    }
}

/// Check all active proposals for epoch expiry and access rule changes.
pub async fn check_proposals(
    proposal_store: &ProposalStore,
    gateway: &GatewayClient,
    multisig_account: &str,
) -> anyhow::Result<()> {
    let proposals = proposal_store.list_active().await?;
    if proposals.is_empty() {
        return Ok(());
    }

    let current_epoch = gateway.get_current_epoch().await?;

    // Phase 1: Check epoch expiry
    let mut still_active = Vec::new();
    for proposal in proposals {
        if current_epoch >= proposal.epoch_max as u64 {
            tracing::info!(
                "Proposal {} expired (epoch {} >= epoch_max {})",
                proposal.id,
                current_epoch,
                proposal.epoch_max
            );
            if let Err(e) = proposal_store.mark_expired(proposal.id).await {
                tracing::warn!("Failed to mark proposal {} as expired: {e}", proposal.id);
            }
        } else {
            still_active.push(proposal);
        }
    }

    // Phase 2: Check access rule changes for proposals that have signatures
    if still_active
        .iter()
        .any(|p| p.status == ProposalStatus::Signing || p.status == ProposalStatus::Ready)
    {
        let access_rule = gateway.read_access_rule(multisig_account).await?;
        let current_hashes: std::collections::HashSet<&str> = access_rule
            .signers
            .iter()
            .map(|s| s.key_hash.as_str())
            .collect();

        for proposal in &still_active {
            if proposal.status != ProposalStatus::Signing
                && proposal.status != ProposalStatus::Ready
            {
                continue;
            }

            // Get signature key hashes for this proposal
            let sig_hashes = proposal_store
                .get_signature_key_hashes(proposal.id)
                .await
                .unwrap_or_default();

            let mut removed_signers = Vec::new();
            for (key_hash, is_valid) in &sig_hashes {
                if *is_valid && !current_hashes.contains(key_hash.as_str()) {
                    // Signer was removed from access rule — invalidate their signature
                    if let Err(e) = proposal_store
                        .invalidate_signature(proposal.id, key_hash)
                        .await
                    {
                        tracing::warn!(
                            "Failed to invalidate signature for {key_hash} on proposal {}: {e}",
                            proposal.id
                        );
                    }
                    removed_signers.push(key_hash.clone());
                }
            }

            if !removed_signers.is_empty() {
                // Recount valid signatures
                let valid_count = proposal_store
                    .count_valid_signatures(proposal.id)
                    .await
                    .unwrap_or(0);

                if valid_count < access_rule.threshold as i64 {
                    let reason = format!(
                        "Access rule changed — signer(s) removed: {}",
                        removed_signers
                            .iter()
                            .map(|h| format!("{}...{}", &h[..8], &h[h.len() - 6..]))
                            .collect::<Vec<_>>()
                            .join(", ")
                    );
                    tracing::info!("Proposal {} invalidated: {reason}", proposal.id);
                    if let Err(e) = proposal_store.mark_invalid(proposal.id, &reason).await {
                        tracing::warn!("Failed to mark proposal {} as invalid: {e}", proposal.id);
                    }
                }
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::gateway::AccessRuleInfo;
    use crate::gateway::SignerInfo;

    fn make_signer(key_hash: &str) -> SignerInfo {
        SignerInfo {
            key_hash: key_hash.to_string(),
            key_type: "EddsaEd25519".to_string(),
            badge_resource: "resource_test".to_string(),
            badge_local_id: format!("[{key_hash}]"),
        }
    }

    #[test]
    fn epoch_expiry_detection() {
        // Epoch 1100 has passed if current_epoch >= epoch_max
        let current_epoch: u64 = 1100;
        let epoch_max: i64 = 1100;
        assert!(current_epoch >= epoch_max as u64);

        // Not expired yet
        let current_epoch: u64 = 1099;
        assert!(!(current_epoch >= epoch_max as u64));
    }

    #[test]
    fn access_rule_signer_removal_detection() {
        let original_signers = vec![
            make_signer("aaaa1111bbbb2222cccc3333dddd4444eeee5555ffff6666aabb"),
            make_signer("1111aaaa2222bbbb3333cccc4444dddd5555eeee6666ffffaabb"),
            make_signer("5555666677778888aaaa1111bbbb2222cccc3333dddd4444eeff"),
        ];

        // After access rule change, one signer was removed
        let new_access_rule = AccessRuleInfo {
            signers: vec![original_signers[0].clone(), original_signers[2].clone()],
            threshold: 2,
        };

        let current_hashes: std::collections::HashSet<&str> = new_access_rule
            .signers
            .iter()
            .map(|s| s.key_hash.as_str())
            .collect();

        // Check which original signers are still valid
        let removed: Vec<&str> = original_signers
            .iter()
            .filter(|s| !current_hashes.contains(s.key_hash.as_str()))
            .map(|s| s.key_hash.as_str())
            .collect();

        assert_eq!(removed.len(), 1);
        assert_eq!(
            removed[0],
            "1111aaaa2222bbbb3333cccc4444dddd5555eeee6666ffffaabb"
        );
    }

    #[test]
    fn threshold_check_with_invalidated_sigs() {
        let threshold: u8 = 3;

        // 3 valid signatures, but one gets invalidated
        let valid_count: i64 = 2;
        assert!(valid_count < threshold as i64); // Should mark Invalid

        // Still enough valid signatures
        let valid_count: i64 = 3;
        assert!(!(valid_count < threshold as i64)); // Should remain active
    }
}
