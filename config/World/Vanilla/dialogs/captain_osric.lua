-- Captain Osric: what a talk opens and what each choice does, per map.
--
-- Real Lua, loaded by the world (game/Scripting/NpcScripts.rs) and run by scripts/runtime.lua.
-- Converted 2026-11-23 from pseudo_lua/presearing dialogs/captain_osric.lua, which came from ASURA
-- config/World/Vanilla/dialogs.json (tools/npc-content-from-captures.py over preforged's pre-Searing
-- captures). The README in this folder is the whole format.
--
-- gw(...) is the game's own text, as the capture carried it. A page's `when` waits for a quest state;
-- the quests arrive in the next round, so until then a quest page stays closed. A choice's `id` is
-- what the client sends back in 0x803B. Pages are numbered from 1.

return {
    npc = "Captain Osric",

    [146] = {
        pages = {
            {
                body = gw(10982, 63947, 59705, 24018, 266, 5641, 56271, 33468, 11991, 1),
                buttons = {
                    {
                        icon = 21,
                        label = gw(5651, 61193, 50525, 23447),
                        id = 8403460,
                        action = { kind = "page", page = 5 },
                        when = { kind = "quest_active", quest = 58 },
                    },
                },
            },
            {
                when = { kind = "quest_reward_open", quest = 58 },
                body = gw(
                    5654, 64642, 45395, 532, 266, 9551, 56040, 63292, 28367, 1, 2, 258, 2, 258, 2, 10984,
                    59348, 58828, 13938, 2, 10986, 35903, 46361, 26129, 257, 506, 2, 10991, 63120, 53358,
                    19539, 266, 9172, 39937, 46198, 932, 1, 267, 2692, 266, 2327, 1, 257, 259, 1
                ),
                buttons = {
                    {
                        icon = 23,
                        label = gw(10975, 59449, 46843, 6569),
                        id = 8403463,
                        action = { kind = "quest", quest = 58, step = "reward" },
                    },
                },
            },
            {
                when = { kind = "quest_offer_open", quest = 58 },
                body = gw(
                    5652, 48606, 38832, 30403, 266, 2985, 263, 76, 111, 103, 103, 101, 114, 1, 1, 2, 263, 2,
                    10984, 59348, 58828, 13938, 2, 10986, 35903, 46361, 26129, 257, 506, 2, 10991, 63120,
                    53358, 19539, 266, 9172, 39937, 46198, 932, 1, 267, 2692, 266, 2327, 1, 257, 259, 1, 2,
                    263
                ),
                buttons = {
                    {
                        icon = 16,
                        label = gw(5659, 40249, 44265, 15953),
                        id = 8403457,
                        action = { kind = "quest", quest = 58, step = "offer" },
                    },
                    {
                        icon = 17,
                        label = gw(5660, 40921, 46607, 16016),
                        id = 8403458,
                        action = { kind = "close" },
                    },
                },
            },
            {
                body = gw(10982, 63947, 59705, 24018, 266, 5641, 56271, 33468, 11991, 1),
                buttons = {
                    {
                        icon = 15,
                        label = gw(10981, 56770, 59190, 3361),
                        id = 128,
                        action = { kind = "close" },
                    },
                },
            },
            {
                when = { kind = "answer" },
                body = gw(
                    5653, 56312, 43220, 21134, 2, 258, 2, 5665, 63551, 54042, 15834, 257, 388, 266, 9549,
                    61858, 55452, 5933, 1, 2, 5665, 63551, 54042, 15834, 257, 389, 266, 9550, 58301, 44681,
                    19425, 1, 2, 5665, 63551, 54042, 15834, 257, 390, 266, 9551, 56040, 63292, 28367, 1
                ),
                buttons = {},
            },
        },
    },
}
