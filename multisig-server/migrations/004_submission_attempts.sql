-- Submission attempts for audit trail (multiple attempts possible per proposal)
CREATE TABLE submission_attempts (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    proposal_id UUID NOT NULL REFERENCES proposals(id),
    fee_payer_account TEXT NOT NULL,
    tx_hash TEXT,
    status TEXT NOT NULL DEFAULT 'pending',
    error_message TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_submission_attempts_proposal ON submission_attempts(proposal_id);
