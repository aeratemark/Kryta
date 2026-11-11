-- The instance table: one row per running copy of a map.
--
-- A map is a definition (`map`); an instance is one running copy of it - an outpost's district, an
-- explorable's party copy (`docs/game-rules.md`, R1-R4). The row is what outlives a connection:
-- when a copy opens, which district number it holds and in which language, and when it last became
-- empty - the moment R37's 15-minute hold counts from and R40's disposal acts on.
--
-- Every number is BIGINT because a query parameter is a 64-bit integer in this pool
-- (`docs/decisions/0007-character-schema.md`).

CREATE TABLE IF NOT EXISTS "instance" (
    "id"          BIGSERIAL PRIMARY KEY,
    "map_id"      BIGINT    NOT NULL,
    "district"    BIGINT    NOT NULL,
    "language"    BIGINT    NOT NULL DEFAULT 0,
    -- Which kind of copy this is: 0 outpost, 1 explorable (the map's own type).
    "kind"        BIGINT    NOT NULL DEFAULT 0,
    "opened_at"   TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT now(),
    -- When the last member left; NULL while somebody is inside. R37's hold is measured from here.
    "empty_since" TIMESTAMP WITH TIME ZONE NULL
);

CREATE INDEX IF NOT EXISTS "instance_by_map"
    ON "instance" ("map_id", "district");
