-- The world schema: a map is a definition, and what a definition holds.
--
-- A map is game data, so it is rows (`docs/port/04-configuration.md`, "three kinds of data"). What a
-- map's row holds is decided by the messages that read it, not by another server's schema:
--
--   `map.id`       the client's own map id - the one a travel request names and a load echoes (`0x019A`)
--   `map.type`     the client's `MapType`: 0 outpost, 1 explorable, 11 character creation
--   `map.file_id`  the `Gw.dat` file id the client loads the map from (`0x0196`)
--   `map.name`     what the map is called, for a log line and for the administration tool
--   `map.party_cap` the cap this map applies, or NULL for the world's own (R15)
--
-- The spawn and arrival tables are the entry rules the owner stated: every entry into an outpost takes
-- a random point of that map's own list and lands scattered around it (R13), and a portal crossing
-- takes the destination's arrival entry for the map the character came from (R21).

CREATE TABLE IF NOT EXISTS "map" (
  "id"        INTEGER PRIMARY KEY,
  "type"      INTEGER NOT NULL,
  "file_id"   BIGINT  NOT NULL,
  "name"      VARCHAR(128) NOT NULL,
  "party_cap" INTEGER NULL
);

COMMENT ON TABLE "map" IS 'The maps this world can put a client in: one row per map definition';
COMMENT ON COLUMN "map"."id" IS 'The client''s map id, as a travel request and a load name it';
COMMENT ON COLUMN "map"."type" IS 'The client''s MapType: 0 outpost, 1 explorable, 11 character creation';
COMMENT ON COLUMN "map"."file_id" IS 'The Gw.dat file id the client loads the map from';
COMMENT ON COLUMN "map"."party_cap" IS 'The cap this map applies, or NULL for the world''s own (R15)';

CREATE TABLE IF NOT EXISTS "map_spawn" (
  "map_id" INTEGER  NOT NULL REFERENCES "map" ("id") ON DELETE CASCADE,
  "idx"    SMALLINT NOT NULL,
  "x"      REAL     NOT NULL,
  "y"      REAL     NOT NULL,
  "plane"  SMALLINT NOT NULL DEFAULT 0,
  PRIMARY KEY ("map_id", "idx")
);

COMMENT ON TABLE "map_spawn" IS 'Where a character may stand on entry: every entry picks one at random and scatters around it (R13)';

CREATE TABLE IF NOT EXISTS "map_arrival" (
  "map_id"   INTEGER  NOT NULL REFERENCES "map" ("id") ON DELETE CASCADE,
  "from_map" INTEGER  NOT NULL,
  "x"        REAL     NOT NULL,
  "y"        REAL     NOT NULL,
  "plane"    SMALLINT NOT NULL DEFAULT 0,
  PRIMARY KEY ("map_id", "from_map")
);

COMMENT ON TABLE "map_arrival" IS 'Where a portal crossing puts a character: the destination''s entry for the map it came from (R21)';
