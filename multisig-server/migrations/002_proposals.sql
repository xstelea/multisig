CREATE TYPE proposal_status AS ENUM (
    'created',
    'signing',
    'ready',
    'submitting',
    'committed',
    'failed',
    'expired',
    'invalid'
);

CREATE TABLE proposals (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    manifest_text TEXT NOT NULL,
    treasury_account TEXT,
    epoch_min BIGINT NOT NULL,
    epoch_max BIGINT NOT NULL,
    status proposal_status NOT NULL DEFAULT 'created',
    subintent_hash TEXT,
    intent_discriminator BIGINT NOT NULL,
    partial_transaction_bytes BYTEA,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    submitted_at TIMESTAMPTZ,
    tx_id TEXT
);

CREATE INDEX idx_proposals_status ON proposals (status);
CREATE INDEX idx_proposals_created_at ON proposals (created_at DESC);
