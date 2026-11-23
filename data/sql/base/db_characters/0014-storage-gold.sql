-- gold_storage - the gold a character keeps in Xunlai storage.
--
-- Ported from ASURA/crates/asura-db/migrations/0035_storage_gold.sql: a balance, 0 to 1,000,000 (the
-- client's own cap), changed only at a Xunlai window (GAME_CMSG_ITEM_CHANGE_GOLD, 0x807C) in the same
-- transaction as the carried gold beside it. The load sends it as 0x0141 after the carried gold's
-- 0x0140, when it is not 0 (game/Creation/mod.rs, load_item_stream_purse).
--
-- Operational state, not content: this is a schema file and writes no row (AGENTS.md, rule 7).

ALTER TABLE "characters" ADD COLUMN IF NOT EXISTS "gold_storage" BIGINT NOT NULL DEFAULT 0
  CHECK ("gold_storage" >= 0);
