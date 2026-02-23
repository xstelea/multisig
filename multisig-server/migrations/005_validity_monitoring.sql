-- Add invalid_reason to proposals for tracking why a proposal was invalidated
ALTER TABLE proposals ADD COLUMN invalid_reason TEXT;

-- Add is_valid flag to signatures for flagging individual signatures as invalid
ALTER TABLE signatures ADD COLUMN is_valid BOOLEAN NOT NULL DEFAULT TRUE;
