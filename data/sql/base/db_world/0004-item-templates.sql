-- Item templates: what an item *is*, once, however many of it exist.
--
-- A character's own items are rows too (`item_instance`, arriving with what a character holds); this
-- table is the definition they point at - the `file_id` the client draws, the kind, the dyes, the
-- value and the name. It is game data: the world's content, not a setting and not a player's
-- (`docs/port/04-configuration.md`, "Three kinds of data").
--
-- The creation screen's record lists name these definitions (`config/World/Vanilla/creation/inventory.json`,
-- `config/World/Vanilla/creation/equipment.json`) and the world's start-up seeds this table from them, once per definition;
-- a template repeated in another profession's set is the same item, so the key is what the client
-- draws: the file id and the dyes.
--
-- `name` and `modifiers` are the client's own values (UTF-16 code units and dwords) as
-- comma-separated decimal text: a template is read back into the same items the record lists carry,
-- whatever a query returns for them.

CREATE TABLE IF NOT EXISTS "item_template" (
    "id"         BIGSERIAL PRIMARY KEY,
    "file_id"    BIGINT NOT NULL,
    "item_type"  BIGINT NOT NULL DEFAULT 0,
    "dye_tint"   BIGINT NOT NULL DEFAULT 0,
    "dye_colors" BIGINT NOT NULL DEFAULT 0,
    "materials"  BIGINT NOT NULL DEFAULT 0,
    "unk1"       BIGINT NOT NULL DEFAULT 0,
    "flags"      BIGINT NOT NULL DEFAULT 0,
    "value"      BIGINT NOT NULL DEFAULT 0,
    "model_id"   BIGINT NOT NULL DEFAULT 0,
    "quantity"   BIGINT NOT NULL DEFAULT 1,
    "profession" BIGINT NOT NULL DEFAULT 0,
    "name"       TEXT   NOT NULL DEFAULT '',
    "modifiers"  TEXT   NOT NULL DEFAULT '',
    CONSTRAINT "idx_item_template_definition" UNIQUE ("file_id", "dye_tint", "dye_colors")
);
