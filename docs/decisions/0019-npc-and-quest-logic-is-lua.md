# 0019 - What an NPC says, and what a quest does, is Lua

## The reference

ASURA reads its dialogs from `config/World/Vanilla/dialogs.json` (`crates/asura-world/src/dialog.rs`)
and its quests from `quests.json` (`quest.rs`, with the rules in `asura-rules`), and decides in Rust
which page a talk opens, which choices a character sees and what a choice does
(`vendor/mod.rs:218-298`, `:723-900`).

## This tree

The owner's decision (2026-11-23): the game-logic half of `pseudo_lua/runtime.lua` runs in the server
as real Lua, so quests and NPCs can be scripted. Its other half - entry bursts, sight, the tick,
behaviour trees, the burrow clock - is **not** run: the server already does those in Rust, ported from
ASURA and proven in the live client, and two engines must not drive the same bodies.

## The decision

* `config/World/<pack>/scripts/runtime.lua` decides what happens: the page a talk opens, the choices a
  character is shown, what a choice does, the page that follows a quest step, and (next round) the
  quest stages. It builds no message.
* `config/World/<pack>/dialogs/<npc>.lua` holds one NPC's pages, per map: the same pages, conditions,
  choices and actions as ASURA's `dialogs.json`, with the same checks at load. A condition may be a
  function, and a dialog may carry `on_talk` / `on_choose` hooks.
* `game/Scripting/NpcScripts.rs` is the host: sandboxed (`table`, `string`, `math`), an instruction
  budget per call, every file read once at start-up, any fault refusing the start.
* Every frame, its fields and its order stay ASURA's: `game/Server/Packets/DialogPackets.rs` is a 1:1
  port of `asura-protocol/src/game/dialog.rs`, and `game/Handlers/NpcHandler.rs` ports `answer_talk`'s
  dialog branch and `handle_send_dialog`.

So the split is decision 0016's: **the script decides what happens; the ported Rust decides how it
reaches the client.**

## Text

A text is a custom string (a Lua string, sent as the literal form `0x0108 0x0107 <UTF-16> 0x0001`,
which preforged's `dialogue::open` sends and ASURA's `dialog.rs:10-12` documents), a real GW string
(`gw(...)`, the captured units), or a hacked-up GW string (`mix(...)`, parts joined, experimental).
The literal form in a dialog body has not yet been seen in this client; the first custom line opened
in-game is its proof. When kryta-CS can install strings into `Gw.dat`, custom strings become string
ids without the content changing.

## What it is not

It is not a protocol change: no message, field or order differs from ASURA's. It is not a second
engine: nothing in the runtime moves, creates or removes a body. A quest's persistence (the rows ASURA
writes) stays Rust's when the quest round ports it.
