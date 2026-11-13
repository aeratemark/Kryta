-- Where a character last stood: the position a load resumes at, and the map it belongs to.
--
-- Ported from `ASURA/crates/asura-store/src/state/character.rs:213-294` (`Position`, `position`,
-- `record_position`, `forget_position`), where the columns are `map_id`, `pos_x`, `pos_y` and
-- `pos_plane`. The owner's rules they serve:
--
--   * R14 "a load always places a character at its last outpost" - and, for a map that is not an
--     outpost, where the character left it (`ASURA/crates/asura-world/src/agent.rs:531-545`,
--     `Standing::resume`), which is the disconnect case;
--   * a **travel** does not preserve a position - only a disconnect does - so a travel that is not a
--     doorway crossing clears the saved position and the destination stands on its own spawn list
--     (`agent.rs:460-476`, R13).
--
-- `pos_plane IS NULL` is what "never saved" means: a character that has never been played has
-- `pos_x`/`pos_y` at their default of 0, which is a legal coordinate, so a caller must not read the
-- pair as a position without the plane (`character.rs:226-246`).

ALTER TABLE "characters" ADD COLUMN IF NOT EXISTS "map_id"    BIGINT NULL;
ALTER TABLE "characters" ADD COLUMN IF NOT EXISTS "pos_x"     REAL   NULL;
ALTER TABLE "characters" ADD COLUMN IF NOT EXISTS "pos_y"     REAL   NULL;
ALTER TABLE "characters" ADD COLUMN IF NOT EXISTS "pos_plane" BIGINT NULL;

COMMENT ON COLUMN "characters"."map_id" IS 'The map the saved position belongs to: a position is only usable on the map it was taken in (ASURA character.rs:213-224)';
COMMENT ON COLUMN "characters"."pos_plane" IS 'NULL means the character has never been saved; the position pair is then not a position (ASURA character.rs:226-246)';
