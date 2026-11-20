-- No data is seeded here. A map record, its entry points and its doorways are the world pack's
-- (`config/World/Vanilla/maps/<campaign>/<area>/<kind>/<map id>/`), read at start-up by
-- `game/src/Maps/Pack.rs`; a database row is for what a server writes and for nothing else
-- (`AGENTS.md` rule 7, `docs/port/04-configuration.md`). What the file carried as rows was moved
-- to the pack before it was removed from here.

-- The maps this world hosts, and where a character stands when it enters one: the tables only.
--
-- A map is not data here. It is the pack''s (`config/World/Vanilla/maps/**`), and this file
-- creates the columns that used to hold it.
--

-- What is here today is one outpost - Ascalon City (pre-Searing), map 148 - because one map is what a
-- character can be put into while the load sequence is proven. Its file id and its spawn point are the
-- values a working server sent for that map in
-- `recordings/asura/feature/travel/first-crossing-20261031.jsonl` seq 125.


-- 0x0001B97D is the file id that frame carried for map 148; the point is the position it carried.
