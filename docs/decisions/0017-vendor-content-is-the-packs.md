# 0017 - What an NPC offers is the pack's content; the trade is ASURA's

**Date:** 2026-11-23. **Decided by:** the owner.

## The decision

Merchants, collectors and Xunlai storage are split in two:

- **Everything the client sees is ASURA's, 1:1.** The walk to an NPC (`game/Movement/Approach.rs`,
  ASURA `approach.rs`), the window frames, the trade rules, the gold messages and the handlers
  (`game/Handlers/NpcHandler.rs`, `game/Trader/Rules.rs`, `game/Server/Packets/VendorPackets.rs`, ASURA
  `vendor/` and `asura-protocol/src/game/vendor.rs`).
- **What each NPC offers is content**: which window an NPC opens, the stock, the collector offers and
  the limits, held as `game/Trader/Content.rs`'s `Vendors` (ASURA `vendor/content.rs`).

Today that content is ASURA's own `config/World/Vanilla/vendors.json`, unchanged, read by
`game/Maps/VendorPack.rs`. The owner's direction is that it becomes Lua, as the skills did
(`0016-skills-are-lua-scripts.md`): a script fills the same `Vendors`. Nothing a client sees depends
on which source filled it.

## Why it is not simply ASURA

ASURA reads `vendors.json` and has no Lua for vendors. Moving the *source* of the content to Lua is a
deviation of arrangement only: the protocol and every handler step stay ported.

## What follows

- The Lua source replaces `VendorPack.rs` and keeps `Vendors::check`'s refusals.
- Prices stay ASURA's rule (`value x scale x 2` to buy, `value` to sell) unless the owner decides a
  script may set them; that would be a second decision.
