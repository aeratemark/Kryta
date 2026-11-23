-- Brother Mhenlo: what a talk opens and what each choice does, per map.
--
-- Real Lua, loaded by the world (game/Scripting/NpcScripts.rs) and run by scripts/runtime.lua.
-- Converted 2026-11-23 from pseudo_lua/presearing dialogs/brother_mhenlo.lua, which came from ASURA
-- config/World/Vanilla/dialogs.json (tools/npc-content-from-captures.py over preforged's pre-Searing
-- captures). The README in this folder is the whole format.
--
-- gw(...) is the game's own text, as the capture carried it. A page's `when` waits for a quest state;
-- the quests arrive in the next round, so until then a quest page stays closed. A choice's `id` is
-- what the client sends back in 0x803B. Pages are numbered from 1.

return {
    npc = "Brother Mhenlo",

    [164] = {
        pages = {
            {
                body = gw(10982, 63947, 59705, 24018, 266, 13357, 62001, 51039, 13145, 1),
                buttons = {
                    {
                        icon = 18,
                        label = gw(5834, 44514, 53426, 15224),
                        id = 8407555,
                        action = { kind = "page", page = 4 },
                        when = { kind = "quest_offer_open", quest = 74 },
                    },
                    {
                        icon = 11,
                        label = gw(13367, 37508, 37386, 15522),
                        id = 129,
                        action = { kind = "page", page = 7 },
                    },
                    {
                        icon = 18,
                        label = gw(6024, 62465, 37086, 7946),
                        id = 8410883,
                        action = { kind = "page", page = 5 },
                        when = { kind = "quest_offer_open", quest = 87 },
                    },
                    {
                        icon = 22,
                        label = gw(5834, 44514, 53426, 15224),
                        id = 8407557,
                        action = { kind = "close" },
                        when = { kind = "quest_active", quest = 74 },
                    },
                },
            },
            {
                when = { kind = "quest_reward_open", quest = 74 },
                body = gw(
                    5840, 34576, 61648, 10278, 266, 2985, 263, 76, 111, 103, 103, 101, 114, 1, 1, 2, 258, 2,
                    258, 2, 10984, 59348, 58828, 13938, 2, 10986, 35903, 46361, 26129, 257, 506
                ),
                buttons = {
                    {
                        icon = 23,
                        label = gw(10975, 59449, 46843, 6569),
                        id = 8407559,
                        action = { kind = "quest", quest = 74, step = "reward" },
                    },
                },
            },
            {
                when = { kind = "quest_reward_open", quest = 87 },
                body = gw(
                    6029, 46702, 58118, 30250, 266, 2985, 263, 76, 111, 103, 103, 101, 114, 1, 1, 2, 258, 2,
                    258, 2, 10984, 59348, 58828, 13938, 2, 10986, 35903, 46361, 26129, 257, 506
                ),
                buttons = {
                    {
                        icon = 23,
                        label = gw(10975, 59449, 46843, 6569),
                        id = 8410887,
                        action = { kind = "quest", quest = 87, step = "reward" },
                    },
                },
            },
            {
                when = { kind = "quest_offer_open", quest = 74 },
                body = gw(
                    5837, 56951, 65039, 5488, 266, 2985, 263, 76, 111, 103, 103, 101, 114, 1, 1, 2, 263, 2,
                    10984, 59348, 58828, 13938, 2, 10986, 35903, 46361, 26129, 257, 506, 2, 263
                ),
                buttons = {
                    {
                        icon = 16,
                        label = gw(5835, 54704, 61175, 20791),
                        id = 8407553,
                        action = { kind = "quest", quest = 74, step = "offer" },
                    },
                    {
                        icon = 17,
                        label = gw(5836, 56665, 45999, 20920),
                        id = 8407554,
                        action = { kind = "close" },
                    },
                },
            },
            {
                when = { kind = "quest_offer_open", quest = 87 },
                body = gw(
                    6027, 43425, 38554, 14652, 266, 2985, 263, 76, 111, 103, 103, 101, 114, 1, 1, 2, 263, 2,
                    10984, 59348, 58828, 13938, 2, 10986, 35903, 46361, 26129, 257, 506, 2, 263
                ),
                buttons = {
                    {
                        icon = 16,
                        label = gw(6025, 46895, 37480, 23462),
                        id = 8410881,
                        action = { kind = "quest", quest = 87, step = "offer" },
                    },
                    {
                        icon = 17,
                        label = gw(6026, 46856, 40958, 8193),
                        id = 8410882,
                        action = { kind = "close" },
                    },
                },
            },
            {
                body = gw(13368, 41663, 57167, 14198),
                buttons = {
                    {
                        icon = 5,
                        label = gw(13369, 62192, 42617, 27259),
                        id = 132,
                        action = { kind = "page", page = 8 },
                    },
                    {
                        icon = 3,
                        label = gw(13370, 49041, 54454, 21019),
                        id = 128,
                        action = { kind = "close" },
                    },
                },
            },
            { when = { kind = "answer" }, body = gw(13365, 42790, 57758, 15529), buttons = {} },
            { when = { kind = "answer" }, body = gw(13358, 33535, 48248, 19497), buttons = {} },
        },
    },
}
