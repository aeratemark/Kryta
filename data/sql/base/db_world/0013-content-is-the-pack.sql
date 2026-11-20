-- The content tables hold no rows, and this is the migration that says so once.
--
-- A map record, its entry points, its doorways and an item's definition are the world pack's files
-- (`config/World/Vanilla/`), read at start-up (`game/src/Maps/Pack.rs`, `docs/decisions/0014-...`).
-- A database row is for what a server writes while it runs, never for what a world is
-- (`AGENTS.md` rule 7).
--
-- The rows these tables carry - in an older database, or in a fresh one seeded by the migrations
-- before this one - are the import that used to be the source. They are removed here once, and
-- nothing writes them again. `map_spawn`, `map_arch` and `map_arrival` go with `map` by their own
-- cascade (`0001-world.sql`, `0008-map-doorways.sql`). Instance rows are state and stay; an instance
-- naming a map this world no longer hosts is disposed by the world's own sweep.

DELETE FROM "item_template";
DELETE FROM "map";

INSERT INTO schema_version (version) VALUES (13) ON CONFLICT (version) DO NOTHING;
