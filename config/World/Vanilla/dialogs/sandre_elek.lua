-- Sandre Elek: what a talk opens and what each choice does, per map.
--
-- Real Lua, loaded by the world (game/Scripting/NpcScripts.rs) and run by scripts/runtime.lua.
-- Converted 2026-11-23 from pseudo_lua/presearing dialogs/sandre_elek.lua, which came from ASURA
-- config/World/Vanilla/dialogs.json (tools/npc-content-from-captures.py over preforged's pre-Searing
-- captures). The README in this folder is the whole format.
--
-- gw(...) is the game's own text, as the capture carried it. A page's `when` waits for a quest state;
-- the quests arrive in the next round, so until then a quest page stays closed. A choice's `id` is
-- what the client sends back in 0x803B. Pages are numbered from 1.

return {
    npc = "Sandre Elek",

    [148] = {
        pages = {
            {
                body = gw(10982, 63947, 59705, 24018, 266, 13171, 62152, 42429, 6305, 1),
                buttons = {
                    {
                        icon = 22,
                        label = gw(5673, 58870, 35349, 7652),
                        id = 8402693,
                        action = { kind = "close" },
                        when = { kind = "quest_active", quest = 55 },
                    },
                },
            },
            {
                when = { kind = "quest_reward_open", quest = 55 },
                body = gw(
                    5676, 51447, 36550, 13182, 266, 2985, 263, 76, 111, 103, 103, 101, 114, 1, 1, 2, 258, 2,
                    258, 2, 10984, 59348, 58828, 13938, 2, 10986, 35903, 46361, 26129, 257, 506, 2, 10991,
                    63120, 53358, 19539, 266, 9966, 43471, 46674, 918, 1, 267, 2692, 266, 2327, 1, 257, 259, 2,
                    258, 2, 2697, 266, 2638, 1, 267, 2275, 1, 257, 259, 258, 261, 2, 258, 2, 2708, 1
                ),
                buttons = {
                    {
                        icon = 23,
                        label = gw(10975, 59449, 46843, 6569),
                        id = 8402695,
                        action = { kind = "quest", quest = 55, step = "reward" },
                    },
                },
            },
            {
                when = { kind = "quest_offer_open", quest = 55 },
                body = gw(
                    5674, 48324, 33064, 24792, 266, 2985, 263, 76, 111, 103, 103, 101, 114, 1, 1, 2, 263, 2,
                    10984, 59348, 58828, 13938, 2, 10986, 35903, 46361, 26129, 257, 506, 2, 10991, 63120,
                    53358, 19539, 266, 9966, 43471, 46674, 918, 1, 267, 2692, 266, 2327, 1, 257, 259, 2, 258,
                    2, 2697, 266, 2638, 1, 267, 2275, 1, 257, 259, 258, 261, 2, 258, 2, 2708, 1, 2, 263
                ),
                buttons = {
                    {
                        icon = 16,
                        label = gw(5679, 54201, 34386, 25585),
                        id = 8402689,
                        action = { kind = "quest", quest = 55, step = "offer" },
                    },
                    {
                        icon = 17,
                        label = gw(5680, 63573, 37725, 27690),
                        id = 8402690,
                        action = { kind = "close" },
                    },
                },
            },
            {
                body = gw(10982, 63947, 59705, 24018, 266, 13171, 62152, 42429, 6305, 1),
                buttons = {
                    {
                        icon = 15,
                        label = gw(10981, 56770, 59190, 3361),
                        id = 128,
                        action = { kind = "close" },
                    },
                },
            },
        },
    },
}
