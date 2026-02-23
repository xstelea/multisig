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
    pub created_at: DateTime<Utc>,
    pub submitted_at: Option<DateTime<Utc>>,
    pub tx_id: Option<String>,
}

pub struct CreateProposal {
    pub manifest_text: String,
    pub treasury_account: Option<String>,
    pub epoch_min: i64,
    pub epoch_max: i64,
    pub subintent_hash: String,
    pub intent_discriminator: i64,
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
            INSERT INTO proposals (manifest_text, treasury_account, epoch_min, epoch_max, subintent_hash, intent_discriminator, partial_transaction_bytes)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING id, manifest_text, treasury_account, epoch_min, epoch_max,
                      status, subintent_hash, intent_discriminator, created_at, submitted_at, tx_id
            "#,
        )
        .bind(&input.manifest_text)
        .bind(&input.treasury_account)
        .bind(input.epoch_min)
        .bind(input.epoch_max)
        .bind(&input.subintent_hash)
        .bind(input.intent_discriminator)
        .bind(&input.partial_transaction_bytes)
        .fetch_one(&self.pool)
        .await?;

        Ok(row)
    }

    pub async fn get(&self, id: Uuid) -> Result<Option<Proposal>> {
        let row = sqlx::query_as::<_, Proposal>(
            r#"
            SELECT id, manifest_text, treasury_account, epoch_min, epoch_max,
                   status, subintent_hash, intent_discriminator, created_at, submitted_at, tx_id
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
                   status, subintent_hash, intent_discriminator, created_at, submitted_at, tx_id
            FROM proposals
            ORDER BY created_at DESC
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
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
