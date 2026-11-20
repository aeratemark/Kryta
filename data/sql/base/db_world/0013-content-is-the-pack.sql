-- The content tables hold no rows, and this is the migration that says so once.
--
-- A map record, its entry points, its doorways and an item's definition are the world pack's files
-- (`config/World/Vanilla/`), read at start-up (`game/src/Maps/Pack.rs`,
-- `docs/decisions/0014-the-world-content-is-a-pack.md`). A database row is for what a server writes
-- while it runs, never for what a world is (`AGENTS.md` rule 7).
--
-- The schema files before this one no longer write those rows, so on a fresh database this file finds
-- nothing to remove and says so by doing nothing. On a database that applied the older files it clears
-- what they left. `map_spawn`, `map_arch` and `map_arrival` go with `map` by their own cascade
-- (`0001-world.sql`, `0008-map-doorways.sql`). Instance rows are state and stay.

DELETE FROM "item_template";
DELETE FROM "map";
