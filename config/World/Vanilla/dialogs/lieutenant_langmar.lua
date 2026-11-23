-- Lieutenant Langmar: what a talk opens and what each choice does, per map.
--
-- Real Lua, loaded by the world (game/Scripting/NpcScripts.rs) and run by scripts/runtime.lua.
-- Converted 2026-11-23 from pseudo_lua/presearing dialogs/lieutenant_langmar.lua, which came from ASURA
-- config/World/Vanilla/dialogs.json (tools/npc-content-from-captures.py over preforged's pre-Searing
-- captures). The README in this folder is the whole format.
--
-- gw(...) is the game's own text, as the capture carried it. A page's `when` waits for a quest state;
-- the quests arrive in the next round, so until then a quest page stays closed. A choice's `id` is
-- what the client sends back in 0x803B. Pages are numbered from 1.

return {
    npc = "Lieutenant Langmar",

    [148] = {
        pages = {
            {
                when = { kind = "quest_reward_open", quest = 1185 },
                body = gw(
                    33026, 28703, 39950, 63660, 13897, 266, 2985, 263, 67, 116, 114, 108, 32, 65, 108, 116, 32,
                    77, 101, 111, 119, 1, 1, 2, 258, 2, 258, 2, 10984, 59348, 58828, 13938, 2, 10986, 35903,
                    46361, 26129, 257, 1256, 2, 10988, 56007, 33198, 13442, 257, 306
                ),
                buttons = {
                    {
                        icon = 23,
                        label = gw(10975, 59449, 46843, 6569),
                        id = 8691975,
                        action = { kind = "quest", quest = 1185, step = "reward" },
                    },
                },
            },
            {
                when = { kind = "quest_offer_open", quest = 1185 },
                body = gw(
                    33026, 28699, 34269, 62262, 20755, 266, 2985, 263, 67, 116, 114, 108, 32, 65, 108, 116, 32,
                    77, 101, 111, 119, 1, 1, 2, 263, 2, 10984, 59348, 58828, 13938, 2, 10986, 35903, 46361,
                    26129, 257, 1256, 2, 10988, 56007, 33198, 13442, 257, 306, 2, 263
                ),
                buttons = {
                    {
                        icon = 16,
                        label = gw(33026, 28700, 39216, 57257, 20447),
                        id = 8691969,
                        action = { kind = "quest", quest = 1185, step = "offer" },
                    },
                    {
                        icon = 17,
                        label = gw(33026, 28701, 52109, 35724, 17549),
                        id = 8691970,
                        action = { kind = "close" },
                    },
                },
            },
        },
    },
}
