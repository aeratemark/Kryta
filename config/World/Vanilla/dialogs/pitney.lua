-- Pitney: what a talk opens and what each choice does, per map.
--
-- Real Lua, loaded by the world (game/Scripting/NpcScripts.rs) and run by scripts/runtime.lua.
-- Converted 2026-11-23 from pseudo_lua/presearing dialogs/pitney.lua, which came from ASURA
-- config/World/Vanilla/dialogs.json (tools/npc-content-from-captures.py over preforged's pre-Searing
-- captures). The README in this folder is the whole format.
--
-- gw(...) is the game's own text, as the capture carried it. A page's `when` waits for a quest state;
-- the quests arrive in the next round, so until then a quest page stays closed. A choice's `id` is
-- what the client sends back in 0x803B. Pages are numbered from 1.

return {
    npc = "Pitney",

    [146] = {
        pages = {
            {
                body = gw(10982, 63947, 59705, 24018, 266, 13252, 59974, 49989, 29864, 1),
                buttons = {
                    {
                        icon = 22,
                        label = gw(5618, 37507, 35109, 13131),
                        id = 8401925,
                        action = { kind = "close" },
                        when = { kind = "quest_active", quest = 52 },
                    },
                    {
                        icon = 21,
                        label = gw(5618, 37507, 35109, 13131),
                        id = 8401924,
                        action = { kind = "page", page = 4 },
                        when = { kind = "quest_active", quest = 52 },
                    },
                },
            },
            {
                when = { kind = "quest_reward_open", quest = 52 },
                body = gw(
                    5624, 59077, 51008, 31321, 266, 2985, 263, 76, 111, 103, 103, 101, 114, 1, 1, 2, 258, 2,
                    258, 2, 10984, 59348, 58828, 13938, 2, 10986, 35903, 46361, 26129, 257, 506, 2, 10991,
                    63120, 53358, 19539, 266, 9766, 34654, 41479, 27277, 1, 267, 2697, 266, 2638, 1, 267, 2276,
                    1, 257, 259, 258, 261, 1
                ),
                buttons = {
                    {
                        icon = 23,
                        label = gw(10975, 59449, 46843, 6569),
                        id = 8401927,
                        action = { kind = "quest", quest = 52, step = "reward" },
                    },
                },
            },
            {
                when = { kind = "quest_offer_open", quest = 52 },
                body = gw(
                    5619, 47276, 55642, 31900, 266, 2985, 263, 76, 111, 103, 103, 101, 114, 1, 1, 2, 263, 2,
                    10984, 59348, 58828, 13938, 2, 10986, 35903, 46361, 26129, 257, 506, 2, 10991, 63120,
                    53358, 19539, 266, 9766, 34654, 41479, 27277, 1, 267, 2697, 266, 2638, 1, 267, 2276, 1,
                    257, 259, 258, 261, 1, 2, 263
                ),
                buttons = {
                    {
                        icon = 16,
                        label = gw(5620, 46385, 51930, 4879),
                        id = 8401921,
                        action = { kind = "quest", quest = 52, step = "offer" },
                    },
                    {
                        icon = 17,
                        label = gw(5621, 54819, 42307, 18920),
                        id = 8401922,
                        action = { kind = "close" },
                    },
                },
            },
            { when = { kind = "answer" }, body = gw(5625, 63098, 35394, 8828), buttons = {} },
        },
    },
}
