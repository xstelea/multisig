-- Signatures collected for proposals
CREATE TABLE signatures (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    proposal_id UUID NOT NULL REFERENCES proposals(id),
    signer_public_key TEXT NOT NULL,
    signer_key_hash TEXT NOT NULL,
    signature_bytes BYTEA NOT NULL,
    signed_partial_transaction_hex TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- One signature per signer per proposal
    UNIQUE (proposal_id, signer_key_hash)
);

CREATE INDEX idx_signatures_proposal_id ON signatures (proposal_id);
