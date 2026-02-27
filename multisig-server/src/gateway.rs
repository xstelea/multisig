use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Information about a signer extracted from the access rule.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
pub struct SignerInfo {
    /// Hash of the signer's public key (from NonFungibleGlobalId local_id).
    pub key_hash: String,
    /// Key type: "EddsaEd25519" or "EcdsaSecp256k1".
    pub key_type: String,
    /// Resource address of the virtual badge (identifies key type on-chain).
    pub badge_resource: String,
    /// Full local_id simple_rep, e.g. "[a0c2219f...]".
    pub badge_local_id: String,
}

/// Parsed access rule: signers and threshold.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
pub struct AccessRuleInfo {
    pub signers: Vec<SignerInfo>,
    pub threshold: u8,
    pub is_updatable: bool,
}

pub struct GatewayClient {
    client: reqwest::Client,
    base_url: String,
}

// --- Gateway API response types (subset needed for access rule parsing) ---

#[derive(Debug, Deserialize)]
struct EntityDetailsResponse {
    items: Vec<EntityDetailsItem>,
}

#[derive(Debug, Deserialize)]
struct EntityDetailsItem {
    details: Option<EntityDetails>,
}

#[derive(Debug, Deserialize)]
struct EntityDetails {
    role_assignments: Option<RoleAssignments>,
}

#[derive(Debug, Deserialize)]
struct RoleAssignments {
    owner: Option<OwnerRole>,
}

#[derive(Debug, Deserialize)]
struct OwnerRole {
    rule: serde_json::Value,
    updater: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GatewayStatusResponse {
    ledger_state: LedgerState,
}

#[derive(Debug, Deserialize)]
struct LedgerState {
    epoch: u64,
}

// --- Request types ---

#[derive(Debug, Serialize)]
struct EntityDetailsRequest {
    addresses: Vec<String>,
}

impl GatewayClient {
    pub fn new(base_url: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            base_url,
        }
    }

    /// Read the access rule (signers + threshold) for a multisig account.
    pub async fn read_access_rule(&self, account_address: &str) -> Result<AccessRuleInfo> {
        let url = format!("{}/state/entity/details", self.base_url);
        let body = EntityDetailsRequest {
            addresses: vec![account_address.to_string()],
        };

        let response = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .context("Failed to send request to Gateway API")?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow!("Gateway API returned {status}: {error_text}"));
        }

        let details: EntityDetailsResponse = response
            .json()
            .await
            .context("Failed to parse Gateway API response")?;

        let item = details
            .items
            .into_iter()
            .next()
            .ok_or_else(|| anyhow!("No items in entity details response"))?;

        let role_assignments = item
            .details
            .ok_or_else(|| anyhow!("No details in entity response"))?
            .role_assignments
            .ok_or_else(|| anyhow!("No role_assignments in entity details"))?;

        let owner = role_assignments
            .owner
            .ok_or_else(|| anyhow!("No owner in role_assignments"))?;

        let is_updatable = owner.updater.as_deref() == Some("Owner");

        let mut info = parse_access_rule(&owner.rule)?;
        info.is_updatable = is_updatable;
        Ok(info)
    }

    /// Submit a notarized transaction to the network.
    pub async fn submit_transaction(&self, notarized_transaction_hex: &str) -> Result<bool> {
        let url = format!("{}/transaction/submit", self.base_url);

        let response = self
            .client
            .post(&url)
            .json(&serde_json::json!({
                "notarized_transaction_hex": notarized_transaction_hex,
            }))
            .send()
            .await
            .context("Failed to submit transaction")?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow!("Submit failed ({status}): {error_text}"));
        }

        let result: SubmitTransactionResponse = response
            .json()
            .await
            .context("Failed to parse submit response")?;

        Ok(result.duplicate)
    }

    /// Get transaction status by intent hash (bech32-encoded, e.g. "txid_tdx_2_1...").
    pub async fn get_transaction_status(
        &self,
        intent_hash: &str,
    ) -> Result<TransactionStatusResponse> {
        let url = format!("{}/transaction/status", self.base_url);

        let response = self
            .client
            .post(&url)
            .json(&serde_json::json!({
                "intent_hash": intent_hash,
            }))
            .send()
            .await
            .context("Failed to query transaction status")?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow!("Status query failed ({status}): {error_text}"));
        }

        let status: TransactionStatusResponse = response
            .json()
            .await
            .context("Failed to parse transaction status")?;

        Ok(status)
    }

    /// Poll until a transaction is committed or fails.
    ///
    /// Returns the final status string ("CommittedSuccess") or an error.
    pub async fn wait_for_commit(&self, intent_hash: &str, max_attempts: u32) -> Result<String> {
        for attempt in 0..max_attempts {
            let status = self.get_transaction_status(intent_hash).await?;

            match status.status.as_str() {
                "CommittedSuccess" => return Ok("CommittedSuccess".to_string()),
                "CommittedFailure" => {
                    return Err(anyhow!(
                        "Transaction failed: {}",
                        status.error_message.unwrap_or_default()
                    ));
                }
                "Rejected" => {
                    return Err(anyhow!(
                        "Transaction rejected: {}",
                        status.error_message.unwrap_or_default()
                    ));
                }
                "Pending" | "Unknown" => {
                    if attempt < max_attempts - 1 {
                        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                    }
                }
                other => {
                    return Err(anyhow!("Unexpected transaction status: {other}"));
                }
            }
        }
        Err(anyhow!(
            "Timeout waiting for commit after {max_attempts} attempts"
        ))
    }

    /// Get the current epoch from the Gateway.
    pub async fn get_current_epoch(&self) -> Result<u64> {
        let url = format!("{}/status/gateway-status", self.base_url);

        let response = self
            .client
            .post(&url)
            .send()
            .await
            .context("Failed to send request to Gateway API")?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow!("Gateway API returned {status}: {error_text}"));
        }

        let status: GatewayStatusResponse = response
            .json()
            .await
            .context("Failed to parse gateway status response")?;

        Ok(status.ledger_state.epoch)
    }
}

/// Parse the owner rule JSON into AccessRuleInfo.
///
/// Expected structure for a multisig (CountOf) account:
/// ```json
/// {
///   "type": "Protected",
///   "access_rule": {
///     "type": "ProofRule",
///     "proof_rule": {
///       "type": "CountOf",
///       "count": 3,
///       "list": [
///         { "type": "NonFungible", "non_fungible": { "local_id": { "simple_rep": "..." }, "resource_address": "..." } }
///       ]
///     }
///   }
/// }
/// ```
///
/// Also handles single-signer (Require) accounts:
/// ```json
/// {
///   "type": "Protected",
///   "access_rule": {
///     "type": "ProofRule",
///     "proof_rule": {
///       "type": "Require",
///       "requirement": { "type": "NonFungible", "non_fungible": { ... } }
///     }
///   }
/// }
/// ```
fn parse_access_rule(rule_json: &serde_json::Value) -> Result<AccessRuleInfo> {
    let rule_type = rule_json["type"]
        .as_str()
        .ok_or_else(|| anyhow!("Missing 'type' in owner rule"))?;

    match rule_type {
        "Protected" => {
            let proof_rule = &rule_json["access_rule"]["proof_rule"];
            let proof_rule_type = proof_rule["type"]
                .as_str()
                .ok_or_else(|| anyhow!("Missing 'type' in proof_rule"))?;

            match proof_rule_type {
                "CountOf" => parse_count_of(proof_rule),
                "Require" => parse_require(proof_rule),
                "AllOf" => parse_all_of(proof_rule),
                "AnyOf" => parse_any_of(proof_rule),
                other => Err(anyhow!("Unsupported proof_rule type: {other}")),
            }
        }
        "AllowAll" => Ok(AccessRuleInfo {
            signers: vec![],
            threshold: 0,
            is_updatable: false,
        }),
        "DenyAll" => Err(anyhow!("Account has DenyAll access rule")),
        other => Err(anyhow!("Unsupported rule type: {other}")),
    }
}

/// Parse a CountOf proof rule (N-of-M multisig).
fn parse_count_of(proof_rule: &serde_json::Value) -> Result<AccessRuleInfo> {
    let count = proof_rule["count"]
        .as_u64()
        .ok_or_else(|| anyhow!("Missing 'count' in CountOf rule"))? as u8;

    let list = proof_rule["list"]
        .as_array()
        .ok_or_else(|| anyhow!("Missing 'list' in CountOf rule"))?;

    let signers = list
        .iter()
        .map(parse_non_fungible_requirement)
        .collect::<Result<Vec<_>>>()?;

    Ok(AccessRuleInfo {
        signers,
        threshold: count,
        is_updatable: false,
    })
}

/// Parse a Require proof rule (single signer).
fn parse_require(proof_rule: &serde_json::Value) -> Result<AccessRuleInfo> {
    let signer = parse_non_fungible_requirement(&proof_rule["requirement"])?;

    Ok(AccessRuleInfo {
        signers: vec![signer],
        threshold: 1,
        is_updatable: false,
    })
}

/// Parse AllOf proof rule (all must sign).
fn parse_all_of(proof_rule: &serde_json::Value) -> Result<AccessRuleInfo> {
    let list = proof_rule["list"]
        .as_array()
        .ok_or_else(|| anyhow!("Missing 'list' in AllOf rule"))?;

    let signers = list
        .iter()
        .map(parse_non_fungible_requirement)
        .collect::<Result<Vec<_>>>()?;

    let threshold = signers.len() as u8;
    Ok(AccessRuleInfo {
        signers,
        threshold,
        is_updatable: false,
    })
}

/// Parse AnyOf proof rule (any one can sign).
fn parse_any_of(proof_rule: &serde_json::Value) -> Result<AccessRuleInfo> {
    let list = proof_rule["list"]
        .as_array()
        .ok_or_else(|| anyhow!("Missing 'list' in AnyOf rule"))?;

    let signers = list
        .iter()
        .map(parse_non_fungible_requirement)
        .collect::<Result<Vec<_>>>()?;

    Ok(AccessRuleInfo {
        signers,
        threshold: 1,
        is_updatable: false,
    })
}

/// Parse a NonFungible requirement into SignerInfo.
fn parse_non_fungible_requirement(req: &serde_json::Value) -> Result<SignerInfo> {
    let req_type = req["type"]
        .as_str()
        .ok_or_else(|| anyhow!("Missing 'type' in requirement"))?;

    if req_type != "NonFungible" {
        return Err(anyhow!("Expected NonFungible requirement, got: {req_type}"));
    }

    let nf = &req["non_fungible"];

    let resource_address = nf["resource_address"]
        .as_str()
        .ok_or_else(|| anyhow!("Missing resource_address in NonFungible"))?;

    let local_id = &nf["local_id"];
    let simple_rep = local_id["simple_rep"]
        .as_str()
        .ok_or_else(|| anyhow!("Missing simple_rep in local_id"))?;

    // Extract the hex hash from simple_rep, e.g. "[a0c2219f...]" -> "a0c2219f..."
    let key_hash = simple_rep
        .trim_start_matches('[')
        .trim_end_matches(']')
        .to_string();

    // Determine key type from resource address.
    // Ed25519: resource_*_1nfxxxxxxxxxxed25sgxxxxxxxxx...
    // Secp256k1: resource_*_1nfxxxxxxxxxxsecpsgxxxxxxxxx...
    let key_type = if resource_address.contains("ed25sg") {
        "EddsaEd25519"
    } else if resource_address.contains("secpsg") {
        "EcdsaSecp256k1"
    } else {
        "Unknown"
    };

    Ok(SignerInfo {
        key_hash,
        key_type: key_type.to_string(),
        badge_resource: resource_address.to_string(),
        badge_local_id: simple_rep.to_string(),
    })
}

// --- Transaction submission/status response types ---

#[derive(Debug, Deserialize)]
struct SubmitTransactionResponse {
    duplicate: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TransactionStatusResponse {
    pub status: String,
    pub error_message: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Real response from Stokenet for a 3-of-4 multisig account.
    fn multisig_role_assignments_json() -> serde_json::Value {
        serde_json::json!({
            "type": "Protected",
            "access_rule": {
                "type": "ProofRule",
                "proof_rule": {
                    "type": "CountOf",
                    "count": 3,
                    "list": [
                        {
                            "type": "NonFungible",
                            "non_fungible": {
                                "local_id": {
                                    "id_type": "Bytes",
                                    "sbor_hex": "5cc0021da0c2219f58abcbc2ebd2da349acb10773ffbc37b6af91fa8df2486c9ea",
                                    "simple_rep": "[a0c2219f58abcbc2ebd2da349acb10773ffbc37b6af91fa8df2486c9ea]"
                                },
                                "resource_address": "resource_tdx_2_1nfxxxxxxxxxxed25sgxxxxxxxxx002236757237xxxxxxxxx3e2cpa"
                            }
                        },
                        {
                            "type": "NonFungible",
                            "non_fungible": {
                                "local_id": {
                                    "id_type": "Bytes",
                                    "sbor_hex": "5cc0021d3aadfdff1d2bfdcf3cd26c653b87f494bb6a990882b403cf0557293778",
                                    "simple_rep": "[3aadfdff1d2bfdcf3cd26c653b87f494bb6a990882b403cf0557293778]"
                                },
                                "resource_address": "resource_tdx_2_1nfxxxxxxxxxxed25sgxxxxxxxxx002236757237xxxxxxxxx3e2cpa"
                            }
                        },
                        {
                            "type": "NonFungible",
                            "non_fungible": {
                                "local_id": {
                                    "id_type": "Bytes",
                                    "sbor_hex": "5cc0021dce4a51a5ca01ea8e0e59b1c8abdb520edfb19a24571b5a747498cad627",
                                    "simple_rep": "[ce4a51a5ca01ea8e0e59b1c8abdb520edfb19a24571b5a747498cad627]"
                                },
                                "resource_address": "resource_tdx_2_1nfxxxxxxxxxxed25sgxxxxxxxxx002236757237xxxxxxxxx3e2cpa"
                            }
                        },
                        {
                            "type": "NonFungible",
                            "non_fungible": {
                                "local_id": {
                                    "id_type": "Bytes",
                                    "sbor_hex": "5cc0021d05c46c54fc86e5651ed504d4636e702fa39fbe7fa24d9dbe57212ab073",
                                    "simple_rep": "[05c46c54fc86e5651ed504d4636e702fa39fbe7fa24d9dbe57212ab073]"
                                },
                                "resource_address": "resource_tdx_2_1nfxxxxxxxxxxed25sgxxxxxxxxx002236757237xxxxxxxxx3e2cpa"
                            }
                        }
                    ]
                }
            }
        })
    }

    #[test]
    fn parse_3_of_4_multisig_access_rule() {
        let json = multisig_role_assignments_json();
        let result = parse_access_rule(&json).unwrap();

        assert_eq!(result.threshold, 3);
        assert_eq!(result.signers.len(), 4);

        assert_eq!(
            result.signers[0].key_hash,
            "a0c2219f58abcbc2ebd2da349acb10773ffbc37b6af91fa8df2486c9ea"
        );
        assert_eq!(result.signers[0].key_type, "EddsaEd25519");
        assert_eq!(
            result.signers[0].badge_resource,
            "resource_tdx_2_1nfxxxxxxxxxxed25sgxxxxxxxxx002236757237xxxxxxxxx3e2cpa"
        );
    }

    #[test]
    fn parse_single_signer_access_rule() {
        let json = serde_json::json!({
            "type": "Protected",
            "access_rule": {
                "type": "ProofRule",
                "proof_rule": {
                    "type": "Require",
                    "requirement": {
                        "type": "NonFungible",
                        "non_fungible": {
                            "local_id": {
                                "simple_rep": "[abcdef1234567890]"
                            },
                            "resource_address": "resource_tdx_2_1nfxxxxxxxxxxed25sgxxxxxxxxx002236757237xxxxxxxxx3e2cpa"
                        }
                    }
                }
            }
        });

        let result = parse_access_rule(&json).unwrap();
        assert_eq!(result.threshold, 1);
        assert_eq!(result.signers.len(), 1);
        assert_eq!(result.signers[0].key_hash, "abcdef1234567890");
    }

    #[test]
    fn parse_allow_all_access_rule() {
        let json = serde_json::json!({ "type": "AllowAll" });
        let result = parse_access_rule(&json).unwrap();
        assert_eq!(result.threshold, 0);
        assert_eq!(result.signers.len(), 0);
    }

    #[test]
    fn parse_deny_all_access_rule() {
        let json = serde_json::json!({ "type": "DenyAll" });
        let result = parse_access_rule(&json);
        assert!(result.is_err());
    }

    #[test]
    #[ignore]
    fn integration_read_access_rule_from_stokenet() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let client =
                GatewayClient::new("https://babylon-stokenet-gateway.radixdlt.com".to_string());
            let result = client
                .read_access_rule(
                    "account_tdx_2_1cx3u3xgr9anc9fk54dxzsz6k2n6lnadludkx4mx5re5erl8jt9lpnp",
                )
                .await
                .unwrap();

            assert_eq!(result.threshold, 3);
            assert_eq!(result.signers.len(), 4);
            println!("Access rule: {result:?}");
        });
    }

    #[test]
    #[ignore]
    fn integration_get_current_epoch() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let client =
                GatewayClient::new("https://babylon-stokenet-gateway.radixdlt.com".to_string());
            let epoch = client.get_current_epoch().await.unwrap();
            assert!(epoch > 0);
            println!("Current epoch: {epoch}");
        });
    }
}
