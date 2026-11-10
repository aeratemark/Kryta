-- account.eula - the version of the EULA this account has accepted, or 0 for none.
--
-- The client accepts the EULA once per account (`0x8026`, three bytes, one of them the version it
-- accepted) and then stops asking, because the login's account-info message reports what this column
-- holds. Nothing else records it: the accept handler stores the version and answers with nothing.
--
-- Reference: `AuthSrv_HandleAcceptEula` records the version on the account row, and the reference's
-- own schema carries the column for it. A version rather than a flag, because "accepted version 26"
-- and "accepted version 40" are different states and the number is what the client sent.
--
-- Added as its own file rather than by editing `account.sql`: the updater records a file by name and
-- sha1 and refuses a file that changed, so a schema grows by a new file.

ALTER TABLE "account" ADD COLUMN IF NOT EXISTS "eula" INTEGER NOT NULL DEFAULT 0;

COMMENT ON COLUMN "account"."eula" IS 'The EULA version this account accepted, or 0 for none';
