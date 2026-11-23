-- Lady Althea: what a talk opens and what each choice does, per map.
--
-- Real Lua, loaded by the world (game/Scripting/NpcScripts.rs) and run by scripts/runtime.lua.
-- Converted 2026-11-23 from pseudo_lua/presearing dialogs/lady_althea.lua, which came from ASURA
-- config/World/Vanilla/dialogs.json (tools/npc-content-from-captures.py over preforged's pre-Searing
-- captures). The README in this folder is the whole format.
--
-- gw(...) is the game's own text, as the capture carried it. A page's `when` waits for a quest state;
-- the quests arrive in the next round, so until then a quest page stays closed. A choice's `id` is
-- what the client sends back in 0x803B. Pages are numbered from 1.

return {
    npc = "Lady Althea",

    [146] = {
        pages = {
            {
                body = gw(10982, 63947, 59705, 24018, 266, 13309, 38511, 48752, 7052, 1),
                buttons = {
                    {
                        icon = 18,
                        label = gw(5801, 60141, 37705, 22812),
                        id = 8406787,
                        action = { kind = "page", page = 3 },
                        when = { kind = "quest_offer_open", quest = 71 },
                    },
                    {
                        icon = 11,
                        label = gw(13318, 56011, 62993, 5702),
                        id = 129,
                        action = { kind = "page", page = 5 },
                    },
                    {
                        icon = 21,
                        label = gw(5651, 61193, 50525, 23447),
                        id = 8403460,
                        action = { kind = "close" },
                        when = { kind = "quest_active", quest = 58 },
                    },
                    {
                        icon = 26,
                        label = gw(13322, 58674, 47839, 2701),
                        id = 127,
                        action = { kind = "close" },
                    },
                },
            },
            {
                when = { kind = "quest_reward_open", quest = 71 },
                body = gw(
                    5805, 62506, 50117, 29467, 266, 2985, 263, 76, 111, 103, 103, 101, 114, 1, 1, 2, 258, 2,
                    258, 2, 10984, 59348, 58828, 13938, 2, 10986, 35903, 46361, 26129, 257, 506
                ),
                buttons = {
                    {
                        icon = 23,
                        label = gw(10975, 59449, 46843, 6569),
                        id = 8406791,
                        action = { kind = "quest", quest = 71, step = "reward" },
                    },
                },
            },
            {
                when = { kind = "quest_offer_open", quest = 71 },
                body = gw(
                    5802, 48806, 54772, 32266, 266, 2985, 263, 76, 111, 103, 103, 101, 114, 1, 1, 2, 263, 2,
                    10984, 59348, 58828, 13938, 2, 10986, 35903, 46361, 26129, 257, 506, 2, 263
                ),
                buttons = {
                    {
                        icon = 16,
                        label = gw(5807, 65054, 47955, 20865),
                        id = 8406785,
                        action = { kind = "quest", quest = 71, step = "offer" },
                    },
                    {
                        icon = 17,
                        label = gw(5808, 42654, 58692, 23833),
                        id = 8406786,
                        action = { kind = "close" },
                    },
                },
            },
            {
                body = gw(13319, 52268, 57736, 22011),
                buttons = {
                    {
                        icon = 1,
                        label = gw(13320, 63963, 58274, 8273),
                        id = 132,
                        action = { kind = "page", page = 6 },
                    },
                    {
                        icon = 3,
                        label = gw(13321, 58517, 42017, 24769),
                        id = 128,
                        action = { kind = "close" },
                    },
                },
            },
            { when = { kind = "answer" }, body = gw(13316, 47205, 60067, 19735), buttons = {} },
            { when = { kind = "answer" }, body = gw(13323, 48712, 42639, 5916), buttons = {} },
        },
    },
}
