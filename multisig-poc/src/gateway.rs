//! Stokenet Gateway API client for transaction submission and status queries.

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

const STOKENET_GATEWAY: &str = "https://stokenet.radixdlt.com";

/// Client for interacting with the Radix Gateway API on Stokenet.
pub struct GatewayClient {
    client: reqwest::blocking::Client,
    base_url: String,
}

impl GatewayClient {
    /// Create a new gateway client for Stokenet.
    pub fn new() -> Self {
        Self {
            client: reqwest::blocking::Client::new(),
            base_url: STOKENET_GATEWAY.to_string(),
        }
    }

    /// Get current network status (epoch, state version).
    pub fn get_network_status(&self) -> Result<NetworkStatusResponse> {
        let response = self
            .client
            .post(format!("{}/status/gateway-status", self.base_url))
            .json(&serde_json::json!({}))
            .send()?;

        if !response.status().is_success() {
            let error_text = response.text()?;
            return Err(anyhow!("Gateway status failed: {}", error_text));
        }

        let status: NetworkStatusResponse = response.json()?;
        Ok(status)
    }

    /// Get current epoch from the network.
    pub fn get_current_epoch(&self) -> Result<u64> {
        let status = self.get_network_status()?;
        Ok(status.ledger_state.epoch)
    }

    /// Submit a notarized transaction to the network.
    pub fn submit_transaction(&self, compiled_tx_hex: &str) -> Result<SubmitResponse> {
        let response = self
            .client
            .post(format!("{}/transaction/submit", self.base_url))
            .json(&SubmitRequest {
                notarized_transaction_hex: compiled_tx_hex.to_string(),
            })
            .send()?;

        if !response.status().is_success() {
            let error_text = response.text()?;
            return Err(anyhow!("Submit failed: {}", error_text));
        }

        let result: SubmitResponse = response.json()?;
        Ok(result)
    }

    /// Get transaction status by intent hash.
    pub fn get_transaction_status(&self, intent_hash: &str) -> Result<TransactionStatusResponse> {
        let response = self
            .client
            .post(format!("{}/transaction/status", self.base_url))
            .json(&TransactionStatusRequest {
                intent_hash: intent_hash.to_string(),
            })
            .send()?;

        if !response.status().is_success() {
            let error_text = response.text()?;
            return Err(anyhow!("Status query failed: {}", error_text));
        }

        let status: TransactionStatusResponse = response.json()?;
        Ok(status)
    }

    /// Poll until transaction is committed or failed.
    /// Returns the final status string on success.
    pub fn wait_for_commit(&self, intent_hash: &str, max_attempts: u32) -> Result<String> {
        for attempt in 0..max_attempts {
            let status = self.get_transaction_status(intent_hash)?;

            match status.status.as_str() {
                "CommittedSuccess" => return Ok("CommittedSuccess".to_string()),
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
                "Pending" | "Unknown" => {
                    if attempt < max_attempts - 1 {
                        std::thread::sleep(std::time::Duration::from_secs(2));
                    }
                }
                other => {
                    return Err(anyhow!("Unexpected status: {}", other));
                }
            }
        }
        Err(anyhow!(
            "Timeout waiting for commit after {} attempts",
            max_attempts
        ))
    }
}

impl Default for GatewayClient {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Request/Response types for Gateway API
// ============================================================================

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

#[derive(Debug, Deserialize)]
pub struct NetworkStatusResponse {
    pub ledger_state: LedgerState,
}

#[derive(Debug, Deserialize)]
pub struct LedgerState {
    pub epoch: u64,
    pub state_version: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore] // Requires network access
    fn test_gateway_connectivity() {
        let client = GatewayClient::new();
        let status = client.get_network_status().unwrap();
        assert!(status.ledger_state.epoch > 0);
        println!("Connected to Stokenet at epoch {}", status.ledger_state.epoch);
    }
}
