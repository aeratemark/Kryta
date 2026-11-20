-- Content is not seeded here. A map record, its entry points and its doorways are the world
-- pack's (`config/World/Vanilla/maps/<campaign>/<area>/<kind>/<map id>/`), read at start-up by
-- `game/src/Maps/Pack.rs`; a database row is for what a server writes and for nothing else
-- (`docs/decisions/0014-the-world-content-is-a-pack.md`, `docs/port/04-configuration.md`).

-- The maps this world hosts, and where a character stands when it enters one.
--
-- Every number here is measured, and each row says where from. A map is data, so it is seeded by a SQL
-- file the updater applies, the same way a schema change is; an operator adds a map the same way, or
-- through the administration tool once its content page lands.
--
-- What is here today is one outpost - Ascalon City (pre-Searing), map 148 - because one map is what a
-- character can be put into while the load sequence is proven. Its file id and its spawn point are the
-- values a working server sent for that map in
-- `recordings/asura/feature/travel/first-crossing-20261031.jsonl` seq 125.

-- 0x0001B97D is the file id that frame carried for map 148; the point is the position it carried.
