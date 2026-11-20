-- account.active - whether this account may begin a new login.
--
-- An operator may need to keep an account and its characters while preventing future authentication.
-- An inactive row is deliberately resolved the same way as an unknown login name, using the existing
-- portal refusal path; no new client packet or refusal is introduced. Existing connections are not
-- interrupted by this column because session termination is a separate world-operation subject.
--
-- This is an additive update rather than an edit to account.sql: the updater records each applied file
-- and refuses a changed one.

ALTER TABLE "account" ADD COLUMN IF NOT EXISTS "active" BOOLEAN NOT NULL DEFAULT TRUE;

COMMENT ON COLUMN "account"."active" IS 'Whether the account may start a new portal login';
