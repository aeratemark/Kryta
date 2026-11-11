-- What a character holds: one row per item, with where it sits.
--
-- An item's definition is a row of `kryta_world.item_template` - what the item *is*; this table is
-- what a *character* has, and it lives in the character database beside the character itself
-- (`docs/decisions/0007-character-schema.md`). The row names its definition by what the client
-- draws (the file id and the dyes, the template's own key) and carries what is the item's own: the
-- runtime id the client keys it by, the bag and slot it sits in, its stack and profession.
--
-- A new character's rows are written at the creation confirm, from the creation record list's
-- starting set; the load then draws the character's own rows rather than the template of a set. The
-- bags themselves are the client's inventory model (`data/inventory.json`, a setting), not rows.

CREATE TABLE IF NOT EXISTS "item_instance" (
    "id"             BIGSERIAL PRIMARY KEY,
    "character_uuid" TEXT   NOT NULL,
    -- The definition: a `kryta_world.item_template` row by its own key.
    "file_id"        BIGINT NOT NULL,
    "dye_tint"       BIGINT NOT NULL DEFAULT 0,
    "dye_colors"     BIGINT NOT NULL DEFAULT 0,
    -- The item itself: its runtime id, where it sits, and what a set named for it.
    "item_id"        BIGINT NOT NULL,
    "bag_id"         BIGINT NOT NULL DEFAULT 0,
    "slot"           BIGINT NOT NULL DEFAULT 0,
    "quantity"       BIGINT NOT NULL DEFAULT 1,
    "profession"     BIGINT NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS "item_instance_by_character"
    ON "item_instance" ("character_uuid");
