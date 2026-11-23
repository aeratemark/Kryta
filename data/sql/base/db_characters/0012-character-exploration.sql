-- character_exploration - what one character has uncovered of one continent's world map.
--
-- The client keeps one bit per 32-unit block, per continent. The bits are a character's own state:
-- they are sent as the 0x008B/0x008A pair when it loads a map on that continent, and one 0x008C mark
-- is sent when a block is newly revealed (game/Maps/Cartography.rs, Preforged's exploration.md with
-- the modern wire from Rurik's fogrle.py). The grid dimensions are content, not a column: a row is
-- only readable against the continent's grid from config/World/Vanilla/cartography.json.
--
-- A character that has never explored a continent has no row, which is an all-fogged grid. The
-- primary key is the pair (character, continent): one grid per character per continent, replaced as
-- it grows. `bits` is the raw little-endian words, not the compressed stream, because the server
-- marks bits between saves and the stream is an encoding of this same state.
--
-- Operational state, not content: this is a schema file and writes no row (AGENTS.md, rule 7).

CREATE TABLE IF NOT EXISTS "character_exploration" (
  "character_uuid" VARCHAR(36) NOT NULL,
  "continent"      SMALLINT    NOT NULL,
  "bits"           BYTEA       NOT NULL,
  "updated_at"     TIMESTAMPTZ NOT NULL DEFAULT now(),
  PRIMARY KEY ("character_uuid", "continent")
);
