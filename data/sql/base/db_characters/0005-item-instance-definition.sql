-- A character's item row carries the whole item, not a reference to a template.
--
-- The load draws a character's items from these rows, and until now half of what it had to send was
-- read through `template_id` into the world database's `item_template`. That join is a second place
-- an item can go missing: a character whose row named a template the world no longer holds had its
-- items dropped from the load, and the client was then drawn a paper doll for items it had never been
-- given - `Assertion: ptr`, `Item/Cli/ItCliApi.cpp(1004)`, measured 2026-11-12 on the load of map 148.
--
-- The working implementation keeps every field on the row for the same reason
-- (`ASURA/crates/asura-store/src/state/inventory.rs`, `NewItem`: bag, slot, file id, kind, model,
-- dyes, materials, unk1, flags, value, quantity, profession, name and modifiers). The definition
-- columns are added here so this tree's rows say the same thing; `template_id` stays as the key the
-- row was made from, for the administration tool, and is no longer what the load reads.

ALTER TABLE "item_instance" ADD COLUMN IF NOT EXISTS "item_type" BIGINT NOT NULL DEFAULT 0;
ALTER TABLE "item_instance" ADD COLUMN IF NOT EXISTS "model_id"  BIGINT NOT NULL DEFAULT 0;
ALTER TABLE "item_instance" ADD COLUMN IF NOT EXISTS "materials" BIGINT NOT NULL DEFAULT 0;
ALTER TABLE "item_instance" ADD COLUMN IF NOT EXISTS "unk1"      BIGINT NOT NULL DEFAULT 0;
ALTER TABLE "item_instance" ADD COLUMN IF NOT EXISTS "flags"     BIGINT NOT NULL DEFAULT 0;
ALTER TABLE "item_instance" ADD COLUMN IF NOT EXISTS "value"     BIGINT NOT NULL DEFAULT 0;
ALTER TABLE "item_instance" ADD COLUMN IF NOT EXISTS "name"      TEXT   NOT NULL DEFAULT '';
ALTER TABLE "item_instance" ADD COLUMN IF NOT EXISTS "modifiers" TEXT   NOT NULL DEFAULT '';
