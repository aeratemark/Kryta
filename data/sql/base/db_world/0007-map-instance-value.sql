-- The instance table a client resolves its destinations from: one value per map, sent in `0x01A0`.
--
-- A client cannot ask to travel to a map it does not know exists, and `0x01A0` is how it learns: a
-- batch of `{instance, value}` pairs. The value is the map's own, and it comes from the wire: map 148's
-- `0xBB14F539` is what a live load of it carried - `recordings/client/feature/maps/pre-searing-load-3.jsonl`
-- seq 751, whose first pair reads `94 00 00 00 | 39 f5 14 bb` - and the same pair is the working
-- implementation's own assertion (`ASURA/crates/asura-world/src/instance_table.rs:166`).
--
-- The column is nullable and a map without a value is **left out of the table**, with the log saying
-- so: a wrong value is worse than a destination the client cannot resolve yet
-- (`ASURA/crates/asura-world/src/instance_table.rs:12-13`).

ALTER TABLE "map" ADD COLUMN IF NOT EXISTS "instance_value" BIGINT;

UPDATE "map" SET "instance_value" = 3138712889 WHERE "id" = 148;
