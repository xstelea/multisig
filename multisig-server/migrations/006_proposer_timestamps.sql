ALTER TABLE proposals
    ADD COLUMN min_proposer_timestamp BIGINT NOT NULL DEFAULT 0,
    ADD COLUMN max_proposer_timestamp BIGINT NOT NULL DEFAULT 0;
