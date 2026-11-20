-- Content is not seeded here. A map record, its entry points and its doorways are the world
-- pack's (`config/World/Vanilla/maps/<campaign>/<area>/<kind>/<map id>/`), read at start-up by
-- `game/src/Maps/Pack.rs`; a database row is for what a server writes and for nothing else
-- (`docs/decisions/0014-the-world-content-is-a-pack.md`, `docs/port/04-configuration.md`).

-- A map's doorways: the wall a character crosses, and the facing an arrival is created with.
--
-- Ported from `ASURA/crates/asura-world/src/portals.rs` (503 lines), where these are one
-- `portals.json` per map folder. Here they are rows, because a doorway is the world's content
-- (`docs/port/04-configuration.md`, "three kinds of data"): `map_arch` is the `arches` list and
-- `map_arrival`, which this schema already had for R21, is the `arrivals` list, gaining the facing the
-- file states as `angle`.
--
-- The owner's rule the shape follows (`portals.rs:19-26`): "the portal is an invisible wall we cross
-- ... should be a plane". An arch is therefore a segment - two endpoints - and a step crosses it only
-- when the step passes through it. The older point shape (`x`/`y` alone) is written as an arch whose
-- two endpoints are equal, and it uses the box `World.PointBoxHalfExtent`.

CREATE TABLE IF NOT EXISTS "map_arch" (
  "map_id" INTEGER  NOT NULL REFERENCES "map" ("id") ON DELETE CASCADE,
  "idx"    INTEGER  NOT NULL,
  "to_map" INTEGER  NULL,
  "from_x" REAL     NOT NULL,
  "from_y" REAL     NOT NULL,
  "to_x"   REAL     NOT NULL,
  "to_y"   REAL     NOT NULL,
  PRIMARY KEY ("map_id", "idx")
);

COMMENT ON TABLE "map_arch" IS 'A doorway as a wall: the segment a step must pass through, and the map the crossing leads to (ASURA portals.rs, Arch)';
COMMENT ON COLUMN "map_arch"."to_map" IS 'The map the crossing leads to; NULL names none, and a crossing that names none does not fire';
COMMENT ON COLUMN "map_arch"."from_x" IS 'The wall''s first endpoint; equal to the to_* pair when the doorway is a point';

ALTER TABLE "map_arrival" ADD COLUMN IF NOT EXISTS "angle" REAL NULL;

COMMENT ON COLUMN "map_arrival"."angle" IS 'The facing, in radians, the arriving agent is created with (ASURA portals.rs, Arrival::angle); NULL leaves the default (1, 0)';

-- Ascalon City (pre-Searing) is still the one map with a doorway, and this is ASURA's own measured row
-- for it (`ASURA/config/World/Vanilla/maps/Prophecies/Ascalon (pre-Searing)/Outposts/148/portals.json`):
-- one wall leading to Lakeside County (146), and the arrival a character coming from 146 stands at.

