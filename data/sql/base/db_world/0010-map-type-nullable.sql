-- A map record the client can resolve but this world does not host carries no type.
--
-- The pack's records are of two shapes (`ASURA/crates/asura-config/src/world.rs:26-54`,
-- `catalogue::load`, and `ASURA/crates/asura-world/src/instance_table.rs:84-127`, `walk`):
--
--   * a **map** states a `map_type` and a `map_file_id`, which is what the world needs to put a client
--     in it - 145 of them in the Vanilla pack;
--   * an **instance-table-only record** states an `instance_value` and no type - 120 of them - and is
--     what a client can name in a `0x01A0` pair without this world being able to load it. The pack's
--     own catalogue reader skips those (`world.rs:42`); its instance-table walker keeps them
--     (`instance_table.rs:63-71`).
--
-- Both are rows here, so the column that only the first shape states becomes nullable. A row with no
-- type is a destination the client may resolve and this world cannot serve.

ALTER TABLE "map" ALTER COLUMN "type" DROP NOT NULL;

COMMENT ON COLUMN "map"."type" IS 'The client''s MapType: 0 outpost, 1 explorable, 11 character creation; NULL for a record the client can resolve but this world does not host (ASURA instance_table.rs:84-127)';
