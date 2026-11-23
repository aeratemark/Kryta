-- Elementalist Aziure: what a talk opens and what each choice does, per map.
--
-- Real Lua, loaded by the world (game/Scripting/NpcScripts.rs) and run by scripts/runtime.lua.
-- Converted 2026-11-23 from pseudo_lua/presearing dialogs/elementalist_aziure.lua, which came from ASURA
-- config/World/Vanilla/dialogs.json (tools/npc-content-from-captures.py over preforged's pre-Searing
-- captures). The README in this folder is the whole format.
--
-- gw(...) is the game's own text, as the capture carried it. A page's `when` waits for a quest state;
-- the quests arrive in the next round, so until then a quest page stays closed. A choice's `id` is
-- what the client sends back in 0x803B. Pages are numbered from 1.

return {
    npc = "Elementalist Aziure",

    [161] = {
        pages = {
            {
                body = gw(10982, 63947, 59705, 24018, 266, 13341, 45418, 36369, 26396, 1),
                buttons = {
                    {
                        icon = 18,
                        label = gw(5824, 37750, 58859, 11074),
                        id = 8407299,
                        action = { kind = "page", page = 5 },
                        when = { kind = "quest_offer_open", quest = 73 },
                    },
                    {
                        icon = 21,
                        label = gw(5760, 33110, 40494, 32593),
                        id = 8405254,
                        action = { kind = "page", page = 3 },
                        when = { kind = "quest_reward_open", quest = 65 },
                    },
                    {
                        icon = 21,
                        label = gw(5477, 51966, 63968, 11667),
                        id = 8398086,
                        action = { kind = "page", page = 2 },
                        when = { kind = "quest_reward_open", quest = 37 },
                    },
                    {
                        icon = 11,
                        label = gw(13351, 57225, 36404, 27092),
                        id = 129,
                        action = { kind = "close" },
                    },
                    {
                        icon = 21,
                        label = gw(5824, 37750, 58859, 11074),
                        id = 8407302,
                        action = { kind = "page", page = 4 },
                        when = { kind = "quest_reward_open", quest = 73 },
                    },
                },
            },
            {
                when = { kind = "quest_reward_open", quest = 37 },
                body = gw(
                    5480, 46542, 43778, 8083, 266, 2985, 263, 76, 111, 103, 103, 101, 114, 1, 1, 2, 258, 2,
                    258, 2, 10984, 59348, 58828, 13938, 2, 10986, 35903, 46361, 26129, 257, 506
                ),
                buttons = {
                    {
                        icon = 23,
                        label = gw(10975, 59449, 46843, 6569),
                        id = 8398087,
                        action = { kind = "quest", quest = 37, step = "reward" },
                    },
                },
            },
            {
                when = { kind = "quest_reward_open", quest = 65 },
                body = gw(
                    5763, 47668, 59868, 9003, 266, 2985, 263, 76, 111, 103, 103, 101, 114, 1, 1, 2, 258, 2,
                    258, 2, 10984, 59348, 58828, 13938, 2, 10986, 35903, 46361, 26129, 257, 506
                ),
                buttons = {
                    {
                        icon = 23,
                        label = gw(10975, 59449, 46843, 6569),
                        id = 8405255,
                        action = { kind = "quest", quest = 65, step = "reward" },
                    },
                },
            },
            {
                when = { kind = "quest_reward_open", quest = 73 },
                body = gw(
                    5830, 39358, 49566, 18023, 266, 2985, 263, 76, 111, 103, 103, 101, 114, 1, 1, 2, 258, 2,
                    258, 2, 10984, 59348, 58828, 13938, 2, 10986, 35903, 46361, 26129, 257, 506
                ),
                buttons = {
                    {
                        icon = 23,
                        label = gw(10975, 59449, 46843, 6569),
                        id = 8407303,
                        action = { kind = "quest", quest = 73, step = "reward" },
                    },
                },
            },
            {
                when = { kind = "quest_offer_open", quest = 73 },
                body = gw(
                    5827, 54996, 52127, 6304, 266, 2985, 263, 76, 111, 103, 103, 101, 114, 1, 1, 2, 263, 2,
                    10984, 59348, 58828, 13938, 2, 10986, 35903, 46361, 26129, 257, 506, 2, 263
                ),
                buttons = {
                    {
                        icon = 16,
                        label = gw(5825, 49723, 48240, 19312),
                        id = 8407297,
                        action = { kind = "quest", quest = 73, step = "offer" },
                    },
                    {
                        icon = 17,
                        label = gw(5826, 35579, 44567, 29336),
                        id = 8407298,
                        action = { kind = "close" },
                    },
                },
            },
            {
                body = gw(10982, 63947, 59705, 24018, 266, 13341, 45418, 36369, 26396, 1),
                buttons = {
                    {
                        icon = 11,
                        label = gw(13351, 57225, 36404, 27092),
                        id = 129,
                        action = { kind = "close" },
                    },
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
