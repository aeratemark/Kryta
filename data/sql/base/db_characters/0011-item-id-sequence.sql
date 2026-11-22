-- The id a new item is given, for an item the server creates rather than one the creation content
-- names.
--
-- ASURA allocates every owned item's id from `item_id_seq` (`asura-db/src/inventory.rs:1028-1032`), and
-- so does this tree now: one counter, above every id the creation content assigns. The content
-- renumbers each character's starting items from 1, and its largest set is smaller than a thousand,
-- so a new item's id cannot collide with a starting row, a granted one, or a ground item that has not
-- been picked up yet.
CREATE SEQUENCE IF NOT EXISTS "item_id_seq" AS BIGINT START WITH 1000;
