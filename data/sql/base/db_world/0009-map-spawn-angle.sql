-- A spawn point's facing: the direction the agent is created facing on entry.
--
-- `ASURA/crates/asura-world/src/agent.rs:460-476` (`Standing::at_spawn`): a character that enters an
-- outpost stands on a random point of that map's own list, scattered inside `World.SpawnScatter` of it,
-- and **faces the direction that point states** - the vector `GAME_SMSG_WORLD_CREATE_AGENT` carries. The
-- pack states it per point (`config/World/<pack>/maps/.../<map>/spawns.json`, `angle`), so it is a
-- column here rather than a constant.
--
-- NULL keeps the old answer, the neutral +y, for a row that states no angle.

ALTER TABLE "map_spawn" ADD COLUMN IF NOT EXISTS "angle" REAL NULL;

COMMENT ON COLUMN "map_spawn"."angle" IS 'The facing, in radians, a character entering at this point is created with; NULL leaves the neutral +y (ASURA agent.rs:460-476)';

-- Ascalon City (pre-Searing)'s own list, as the pack states it
-- (`ASURA/config/World/Vanilla/maps/Prophecies/Ascalon (pre-Searing)/Outposts/148/spawns.json`): two
-- points, each with the facing a character entering there is created with. R13 picks one at random and
-- scatters inside World.SpawnScatter of it.
INSERT INTO "map_spawn" ("map_id", "idx", "x", "y", "plane", "angle") VALUES
  (148, 0, 9497.82, 4835.33, 0, -0.72),
  (148, 1, 9686.74, 7979.27, 0, -2.01)
ON CONFLICT ("map_id", "idx") DO UPDATE
  SET "x"     = EXCLUDED."x",
      "y"     = EXCLUDED."y",
      "plane" = EXCLUDED."plane",
      "angle" = EXCLUDED."angle";