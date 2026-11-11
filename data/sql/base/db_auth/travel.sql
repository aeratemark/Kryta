-- The travel record: one row per hand-off from the login to a world.
--
-- A client asks for a map (`AUTH_CMSG_REQUEST_GAME_INSTANCE`, `0x8029`); the login answers with a
-- world's address (`AUTH_SMSG_GAME_SERVER_INFO`, `0x0009`) carrying a token pair, and the gameplay
-- connection that follows presents that pair in its version message. The row is how the world learns
-- the map and the district the login promised, without the two programs talking to each other
-- (`docs/port/06-world-plan.md`, "The three seams"). It is state, so it is a row.
--
-- The login inserts one when it answers `0x8029`; the world claims it once - reads it and deletes it
-- in the same statement - when the version message presents the same world, account and player token.
-- A row that is never claimed is a client that asked and did not arrive; nothing reads it again, and
-- the operator may prune by `created_at` (the reference has no such record and needs none: it keeps
-- the destination in the character's own row).
--
-- Every number is BIGINT because a query parameter is a 64-bit integer in this pool
-- (`docs/decisions/0007-character-schema.md`).

CREATE TABLE IF NOT EXISTS "travel" (
    "id"            BIGSERIAL PRIMARY KEY,
    -- The account, as the portal's own identifier in the wire's uuid text form - the same form
    -- `characters.account_uuid` holds, so the two programs agree on who is travelling.
    "account"       TEXT      NOT NULL,
    -- The world the login named in `0x0009`, as its server id.
    "world"         BIGINT    NOT NULL,
    -- The map the client asked for, and the district and language the request carried.
    "map_id"        BIGINT    NOT NULL,
    "district"      BIGINT    NOT NULL DEFAULT 0,
    "language"      BIGINT    NOT NULL DEFAULT 0,
    -- The number the login drew for this travel, which the version message carries back.
    "player_token"  BIGINT    NOT NULL,
    "created_at"    TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT now()
);

-- A claim looks a row up by exactly what the version message presents.
CREATE INDEX IF NOT EXISTS "travel_by_handoff"
    ON "travel" ("world", "account", "player_token");
