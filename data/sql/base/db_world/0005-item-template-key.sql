-- The item template's key is the whole definition, not just what the client draws.
--
-- 0004-item-templates.sql keyed a template on (file id, dye tint, dye colours), which is what the
-- client draws - but two definitions can share those and differ elsewhere (a name, a modifier list),
-- and one of them was then lost. With a character's item rows naming their template by id
-- (db_characters/0003-item-instance-template.sql), the table can hold both: the key is the whole
-- definition, and an exact repeat is still the same item.

ALTER TABLE "item_template" DROP CONSTRAINT IF EXISTS "idx_item_template_definition";
ALTER TABLE "item_template" ADD CONSTRAINT "idx_item_template_definition" UNIQUE (
    "file_id", "item_type", "dye_tint", "dye_colors", "materials", "unk1", "flags", "value",
    "model_id", "quantity", "profession", "name", "modifiers"
);
