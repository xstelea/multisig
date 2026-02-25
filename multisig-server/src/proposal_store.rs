use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "proposal_status", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum ProposalStatus {
    Created,
    Signing,
    Ready,
    Submitting,
    Committed,
    Failed,
    Expired,
    Invalid,
}

impl ProposalStatus {
    /// Returns true if transitioning from `self` to `to` is valid.
    pub fn can_transition_to(&self, to: ProposalStatus) -> bool {
        matches!(
            (self, to),
            (ProposalStatus::Created, ProposalStatus::Signing)
                | (ProposalStatus::Signing, ProposalStatus::Ready)
                | (ProposalStatus::Ready, ProposalStatus::Submitting)
                | (ProposalStatus::Submitting, ProposalStatus::Committed)
                | (ProposalStatus::Submitting, ProposalStatus::Failed)
                | (ProposalStatus::Created, ProposalStatus::Expired)
                | (ProposalStatus::Signing, ProposalStatus::Expired)
                | (ProposalStatus::Ready, ProposalStatus::Expired)
                | (ProposalStatus::Created, ProposalStatus::Invalid)
                | (ProposalStatus::Signing, ProposalStatus::Invalid)
                | (ProposalStatus::Ready, ProposalStatus::Invalid)
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Proposal {
    pub id: Uuid,
    pub manifest_text: String,
    pub treasury_account: Option<String>,
    pub epoch_min: i64,
    pub epoch_max: i64,
    pub status: ProposalStatus,
    pub subintent_hash: Option<String>,
    pub intent_discriminator: i64,
    pub min_proposer_timestamp: i64,
    pub max_proposer_timestamp: i64,
    pub created_at: DateTime<Utc>,
    pub submitted_at: Option<DateTime<Utc>>,
    pub tx_id: Option<String>,
    pub invalid_reason: Option<String>,
}

pub struct CreateProposal {
    pub manifest_text: String,
    pub treasury_account: Option<String>,
    pub epoch_min: i64,
    pub epoch_max: i64,
    pub subintent_hash: String,
    pub intent_discriminator: i64,
    pub min_proposer_timestamp: i64,
    pub max_proposer_timestamp: i64,
    pub partial_transaction_bytes: Vec<u8>,
}

pub struct ProposalStore {
    pool: PgPool,
}

impl ProposalStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create(&self, input: CreateProposal) -> Result<Proposal> {
        let row = sqlx::query_as::<_, Proposal>(
            r#"
            INSERT INTO proposals (manifest_text, treasury_account, epoch_min, epoch_max, subintent_hash, intent_discriminator, min_proposer_timestamp, max_proposer_timestamp, partial_transaction_bytes)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            RETURNING id, manifest_text, treasury_account, epoch_min, epoch_max,
                      status, subintent_hash, intent_discriminator, min_proposer_timestamp, max_proposer_timestamp,
                      created_at, submitted_at, tx_id, invalid_reason
            "#,
        )
        .bind(&input.manifest_text)
        .bind(&input.treasury_account)
        .bind(input.epoch_min)
        .bind(input.epoch_max)
        .bind(&input.subintent_hash)
        .bind(input.intent_discriminator)
        .bind(input.min_proposer_timestamp)
        .bind(input.max_proposer_timestamp)
        .bind(&input.partial_transaction_bytes)
        .fetch_one(&self.pool)
        .await?;

        Ok(row)
    }

    pub async fn get(&self, id: Uuid) -> Result<Option<Proposal>> {
        let row = sqlx::query_as::<_, Proposal>(
            r#"
            SELECT id, manifest_text, treasury_account, epoch_min, epoch_max,
                   status, subintent_hash, intent_discriminator, min_proposer_timestamp, max_proposer_timestamp,
                   created_at, submitted_at, tx_id, invalid_reason
            FROM proposals
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row)
    }

    pub async fn list(&self) -> Result<Vec<Proposal>> {
        let rows = sqlx::query_as::<_, Proposal>(
            r#"
            SELECT id, manifest_text, treasury_account, epoch_min, epoch_max,
                   status, subintent_hash, intent_discriminator, min_proposer_timestamp, max_proposer_timestamp,
                   created_at, submitted_at, tx_id, invalid_reason
            FROM proposals
            ORDER BY created_at DESC
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }

    /// Get the raw partial transaction bytes for a proposal.
    pub async fn get_partial_transaction_bytes(&self, id: Uuid) -> Result<Vec<u8>> {
        let row: (Vec<u8>,) =
            sqlx::query_as("SELECT partial_transaction_bytes FROM proposals WHERE id = $1")
                .bind(id)
                .fetch_optional(&self.pool)
                .await?
                .ok_or_else(|| anyhow!("Proposal {id} not found"))?;

        Ok(row.0)
    }

    /// Update the tx_id and submitted_at fields after submission.
    pub async fn update_tx_id(&self, id: Uuid, tx_id: &str) -> Result<()> {
        let result =
            sqlx::query("UPDATE proposals SET tx_id = $1, submitted_at = NOW() WHERE id = $2")
                .bind(tx_id)
                .bind(id)
                .execute(&self.pool)
                .await?;

        if result.rows_affected() == 0 {
            return Err(anyhow!("Proposal {id} not found"));
        }

        Ok(())
    }

    /// Record a submission attempt for audit trail.
    pub async fn record_submission_attempt(
        &self,
        proposal_id: Uuid,
        fee_payer_account: &str,
        tx_hash: Option<&str>,
        status: &str,
        error_message: Option<&str>,
    ) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO submission_attempts (proposal_id, fee_payer_account, tx_hash, status, error_message)
            VALUES ($1, $2, $3, $4, $5)
            "#,
        )
        .bind(proposal_id)
        .bind(fee_payer_account)
        .bind(tx_hash)
        .bind(status)
        .bind(error_message)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// List proposals in active states (Created, Signing, Ready) for validity monitoring.
    pub async fn list_active(&self) -> Result<Vec<Proposal>> {
        let rows = sqlx::query_as::<_, Proposal>(
            r#"
            SELECT id, manifest_text, treasury_account, epoch_min, epoch_max,
                   status, subintent_hash, intent_discriminator, min_proposer_timestamp, max_proposer_timestamp,
                   created_at, submitted_at, tx_id, invalid_reason
            FROM proposals
            WHERE status IN ('created', 'signing', 'ready')
            ORDER BY created_at ASC
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }

    /// Mark a proposal as expired (epoch window passed).
    pub async fn mark_expired(&self, id: Uuid) -> Result<()> {
        let result = sqlx::query(
            "UPDATE proposals SET status = 'expired', invalid_reason = 'Proposal epoch window has passed' WHERE id = $1 AND status IN ('created', 'signing', 'ready')",
        )
        .bind(id)
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(anyhow!(
                "Proposal {id} not found or not in an active status"
            ));
        }

        Ok(())
    }

    /// Mark a proposal as invalid with a reason (e.g. access rule changed).
    pub async fn mark_invalid(&self, id: Uuid, reason: &str) -> Result<()> {
        let result = sqlx::query(
            "UPDATE proposals SET status = 'invalid', invalid_reason = $1 WHERE id = $2 AND status IN ('created', 'signing', 'ready')",
        )
        .bind(reason)
        .bind(id)
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(anyhow!(
                "Proposal {id} not found or not in an active status"
            ));
        }

        Ok(())
    }

    /// Flag a signature as invalid (signer removed from access rule).
    pub async fn invalidate_signature(
        &self,
        proposal_id: Uuid,
        signer_key_hash: &str,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE signatures SET is_valid = FALSE WHERE proposal_id = $1 AND signer_key_hash = $2",
        )
        .bind(proposal_id)
        .bind(signer_key_hash)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Get (key_hash, is_valid) pairs for all signatures on a proposal.
    pub async fn get_signature_key_hashes(&self, proposal_id: Uuid) -> Result<Vec<(String, bool)>> {
        let rows: Vec<(String, bool)> = sqlx::query_as(
            "SELECT signer_key_hash, is_valid FROM signatures WHERE proposal_id = $1",
        )
        .bind(proposal_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }

    /// Count valid signatures for a proposal.
    pub async fn count_valid_signatures(&self, proposal_id: Uuid) -> Result<i64> {
        let row: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM signatures WHERE proposal_id = $1 AND is_valid = TRUE",
        )
        .bind(proposal_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(row.0)
    }

    pub async fn transition_status(
        &self,
        id: Uuid,
        from: ProposalStatus,
        to: ProposalStatus,
    ) -> Result<()> {
        if !from.can_transition_to(to) {
            return Err(anyhow!("Invalid status transition: {from:?} → {to:?}"));
        }

        let result = sqlx::query("UPDATE proposals SET status = $1 WHERE id = $2 AND status = $3")
            .bind(to)
            .bind(id)
            .bind(from)
            .execute(&self.pool)
            .await?;

        if result.rows_affected() == 0 {
            return Err(anyhow!("Proposal {id} not found or not in {from:?} status"));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_state_transitions() {
        assert!(ProposalStatus::Created.can_transition_to(ProposalStatus::Signing));
        assert!(ProposalStatus::Signing.can_transition_to(ProposalStatus::Ready));
        assert!(ProposalStatus::Ready.can_transition_to(ProposalStatus::Submitting));
        assert!(ProposalStatus::Submitting.can_transition_to(ProposalStatus::Committed));
        assert!(ProposalStatus::Submitting.can_transition_to(ProposalStatus::Failed));
    }

    #[test]
    fn expiry_transitions() {
        assert!(ProposalStatus::Created.can_transition_to(ProposalStatus::Expired));
        assert!(ProposalStatus::Signing.can_transition_to(ProposalStatus::Expired));
        assert!(ProposalStatus::Ready.can_transition_to(ProposalStatus::Expired));
    }

    #[test]
    fn invalid_transitions() {
        assert!(ProposalStatus::Created.can_transition_to(ProposalStatus::Invalid));
        assert!(ProposalStatus::Signing.can_transition_to(ProposalStatus::Invalid));
        assert!(ProposalStatus::Ready.can_transition_to(ProposalStatus::Invalid));
    }

    #[test]
    fn rejects_invalid_transitions() {
        assert!(!ProposalStatus::Created.can_transition_to(ProposalStatus::Ready));
        assert!(!ProposalStatus::Created.can_transition_to(ProposalStatus::Committed));
        assert!(!ProposalStatus::Committed.can_transition_to(ProposalStatus::Created));
        assert!(!ProposalStatus::Failed.can_transition_to(ProposalStatus::Signing));
        assert!(!ProposalStatus::Expired.can_transition_to(ProposalStatus::Created));
        assert!(!ProposalStatus::Submitting.can_transition_to(ProposalStatus::Signing));
    }
}
