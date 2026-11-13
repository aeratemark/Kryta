-- A travel's landing: where the destination puts the arriving character, when the trigger knew.
--
-- A doorway crossing hands over that doorway's own landing point and facing, and the connection the
-- client then opens reads them back to stand on - and face along
-- (`ASURA/crates/asura-world/src/travel.rs:359-362`, `446-468`). Every other travel states none, and
-- an outpost picks a spawn point of its own list instead (R13).
--
-- The columns are NULL together or filled together: a row that states half a point is not a landing,
-- and the reader treats it as none.

ALTER TABLE "travel" ADD COLUMN IF NOT EXISTS "landing_x"     REAL   NULL;
ALTER TABLE "travel" ADD COLUMN IF NOT EXISTS "landing_y"     REAL   NULL;
ALTER TABLE "travel" ADD COLUMN IF NOT EXISTS "landing_plane" BIGINT NULL;
ALTER TABLE "travel" ADD COLUMN IF NOT EXISTS "landing_from"  BIGINT NULL;

COMMENT ON COLUMN "travel"."landing_x" IS 'The point a doorway crossing lands on, or NULL when no landing was stated';
COMMENT ON COLUMN "travel"."landing_from" IS 'The map the character crossed from; the receiving map states the arrival by it (R21)';
