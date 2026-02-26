-- Add per-proposal multisig account address and drop unused treasury_account column.

ALTER TABLE proposals ADD COLUMN multisig_account TEXT NOT NULL DEFAULT '';

ALTER TABLE proposals DROP COLUMN treasury_account;
