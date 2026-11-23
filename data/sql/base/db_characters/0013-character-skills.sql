-- character_skills - a character's skills: the ids it has learned, the eight slots of its bar, and its
-- attribute ranks.
--
-- Ported from ASURA_testfork's `crates/asura-db/migrations/0034_character_skills.sql`, with this tree's
-- key (`character_uuid`, as `character_exploration`) and without its schema_version row.
--
-- Ids and ranks, and nothing a skill *is*. What skill 281 costs, what it does and which profession it
-- belongs to is content - `config/World/Vanilla/skills.json` and the Lua script it names - and the same for
-- every character. The bar is eight (skill, copy) pairs kept as two parallel eight-element arrays, the
-- way the client's `0x00DA` carries it. Ranks are one entry per attribute index, 0..50, the order of
-- `attributes.json`. A character with no row has learned nothing, has an empty bar and no ranks - a new
-- character, not an error.
--
-- Operational state, not content: this is a schema file and writes no row (AGENTS.md, rule 7).

CREATE TABLE IF NOT EXISTS "character_skills" (
  "character_uuid" VARCHAR(36) PRIMARY KEY,
  "learned"        INTEGER[]   NOT NULL DEFAULT '{}',
  "bar_skills"     INTEGER[]   NOT NULL DEFAULT '{0,0,0,0,0,0,0,0}'
                               CHECK (cardinality("bar_skills") = 8),
  "bar_copies"     INTEGER[]   NOT NULL DEFAULT '{0,0,0,0,0,0,0,0}'
                               CHECK (cardinality("bar_copies") = 8),
  "ranks"          SMALLINT[]  NOT NULL DEFAULT '{}',
  "updated_at"     TIMESTAMPTZ NOT NULL DEFAULT now()
);
