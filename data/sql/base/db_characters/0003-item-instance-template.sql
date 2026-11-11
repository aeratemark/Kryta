-- A character's item row names its template by id.
--
-- 0002-item-instance.sql keyed the definition by what the client draws (file id and dyes); two
-- definitions can share those and differ elsewhere, so the row is pointed at the template it was
-- written from (`kryta_world.item_template.id`). A row written before this column existed keeps 0,
-- and a load resolves it by the key columns it already carries.

ALTER TABLE "item_instance" ADD COLUMN IF NOT EXISTS "template_id" BIGINT NOT NULL DEFAULT 0;
