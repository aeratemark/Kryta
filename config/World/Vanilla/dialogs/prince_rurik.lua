-- Prince Rurik: what a talk opens and what each choice does, per map.
--
-- Real Lua, loaded by the world (game/Scripting/NpcScripts.rs) and run by scripts/runtime.lua.
-- Converted 2026-11-23 from pseudo_lua/presearing dialogs/prince_rurik.lua, which came from ASURA
-- config/World/Vanilla/dialogs.json (tools/npc-content-from-captures.py over preforged's pre-Searing
-- captures). The README in this folder is the whole format.
--
-- gw(...) is the game's own text, as the capture carried it. A page's `when` waits for a quest state;
-- the quests arrive in the next round, so until then a quest page stays closed. A choice's `id` is
-- what the client sends back in 0x803B. Pages are numbered from 1.

return {
    npc = "Prince Rurik",

    [146] = {
        pages = {
            {
                when = { kind = "quest_reward_open", quest = 46 },
                body = gw(
                    5561, 40090, 58435, 8451, 266, 2985, 263, 76, 111, 103, 103, 101, 114, 1, 1, 2, 258, 2,
                    258, 2, 10984, 59348, 58828, 13938, 2, 10986, 35903, 46361, 26129, 257, 456, 2, 10991,
                    63120, 53358, 19539, 266, 9997, 60200, 53520, 30681, 1, 267, 2697, 266, 2638, 1, 267, 2272,
                    1, 257, 262, 258, 265, 1
                ),
                buttons = {
                    {
                        icon = 23,
                        label = gw(10975, 59449, 46843, 6569),
                        id = 8400391,
                        action = { kind = "quest", quest = 46, step = "reward" },
                    },
                },
            },
            {
                body = gw(10982, 63947, 59705, 24018, 266, 13185, 59013, 48422, 14300, 266, 2302, 1, 1),
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
    [148] = {
        pages = {
            {
                body = gw(10982, 63947, 59705, 24018, 266, 13185, 59013, 48422, 14300, 266, 2302, 1, 1),
                buttons = {
                    {
                        icon = 22,
                        label = gw(5555, 56520, 61082, 22663),
                        id = 8400389,
                        action = { kind = "close" },
                        when = { kind = "quest_active", quest = 46 },
                    },
                },
            },
            {
                when = { kind = "quest_offer_open", quest = 46 },
                body = gw(
                    5557, 56408, 49767, 29461, 266, 2985, 263, 76, 111, 103, 103, 101, 114, 1, 1, 2, 263, 2,
                    10984, 59348, 58828, 13938, 2, 10986, 35903, 46361, 26129, 257, 456, 2, 10991, 63120,
                    53358, 19539, 266, 9997, 60200, 53520, 30681, 1, 267, 2697, 266, 2638, 1, 267, 2272, 1,
                    257, 262, 258, 265, 1, 2, 263
                ),
                buttons = {
                    {
                        icon = 16,
                        label = gw(5558, 56585, 65350, 24071),
                        id = 8400385,
                        action = { kind = "quest", quest = 46, step = "offer" },
                    },
                    {
                        icon = 17,
                        label = gw(5559, 50333, 50893, 1039),
                        id = 8400386,
                        action = { kind = "close" },
                    },
                },
            },
            {
                body = gw(10982, 63947, 59705, 24018, 266, 13185, 59013, 48422, 14300, 266, 2302, 1, 1),
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
