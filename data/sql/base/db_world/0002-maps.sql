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

INSERT INTO "map" ("id", "type", "file_id", "name", "party_cap") VALUES
  (148, 0, 112509, 'Ascalon City (pre-Searing)', NULL),
  (0,   11, 0,     'Character creation', NULL)
ON CONFLICT ("id") DO UPDATE
  SET "type" = EXCLUDED."type", "file_id" = EXCLUDED."file_id", "name" = EXCLUDED."name";

-- 0x0001B97D is the file id that frame carried for map 148; the point is the position it carried.
INSERT INTO "map_spawn" ("map_id", "idx", "x", "y", "plane") VALUES
  (148, 0, 9541.87, 4796.46, 0)
ON CONFLICT ("map_id", "idx") DO UPDATE
  SET "x" = EXCLUDED."x", "y" = EXCLUDED."y", "plane" = EXCLUDED."plane";
