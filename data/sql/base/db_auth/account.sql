-- account - who a login may be.
--
-- This is the login database's account table, and accounts are rows: they are the server's state,
-- changed by playing, not a configuration an operator shares. A JSON file held them for a while
-- because there was no schema when the portal was written; that was a stopgap, and this table is
-- what replaces it (docs/decisions/0006-accounts-are-rows.md).
--
-- Reference: data/sql/base/db_auth/account.sql. The reference's columns are WoW's: a salt, a
-- verifier and a session key, which is how its client proves a password without sending one. This
-- game's client does not do that - it carries an identity the portal issued (docs/decisions/
-- 0003-portal-host.md) - so what a row holds here is that identity: the login name the client
-- types, and the pair of identifiers its login message presents.
--
-- Kryta's own rows are inserted by the operator, not by this file: an account names a person.

CREATE TABLE IF NOT EXISTS "account" (
  "id"       SERIAL PRIMARY KEY,
  "username" VARCHAR(255) NOT NULL,
  "user_id"  VARCHAR(36)  NOT NULL DEFAULT '',
  "token"    VARCHAR(36)  NOT NULL DEFAULT '',
  "joindate" TIMESTAMP    NOT NULL DEFAULT CURRENT_TIMESTAMP,
  CONSTRAINT "idx_username" UNIQUE ("username")
);

COMMENT ON TABLE "account" IS 'Account System';

-- The pair the client presents is what a login is checked against, so both are indexed together
-- with the name: a lookup takes all three and a mismatch is the client's error, not the server's.
COMMENT ON COLUMN "account"."username" IS 'The login name the client types';
COMMENT ON COLUMN "account"."user_id" IS 'The portal identifier the client carries in its login message';
COMMENT ON COLUMN "account"."token" IS 'The token the portal issued for this session';
