-- What a character has unlocked: the maps it may travel to.
--
-- R12: unlocks gate travel - a character may only travel to maps it has unlocked, and it starts with
-- one. A scripted or forced travel unlocks at arrival. The unlock is the character's own state, so it
-- is a row beside the character (`docs/port/04-configuration.md`, "three kinds of data"); the gate
-- itself is applied when a travel is answered, which is the same path the redirect arrives with.
--
-- A null `unlocked_at` is what a row looks like when something other than the character's own
-- progress put it there; the timestamp is for the operator.

CREATE TABLE IF NOT EXISTS "character_unlock" (
    "character_uuid" TEXT   NOT NULL,
    "map_id"         BIGINT NOT NULL,
    "unlocked_at"    TIMESTAMP WITH TIME ZONE NULL DEFAULT now(),
    PRIMARY KEY ("character_uuid", "map_id")
);
