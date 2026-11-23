-- Necromancer Munne: what a talk opens and what each choice does, per map.
--
-- Real Lua, loaded by the world (game/Scripting/NpcScripts.rs) and run by scripts/runtime.lua.
-- Converted 2026-11-23 from pseudo_lua/presearing dialogs/necromancer_munne.lua, which came from ASURA
-- config/World/Vanilla/dialogs.json (tools/npc-content-from-captures.py over preforged's pre-Searing
-- captures). The README in this folder is the whole format.
--
-- gw(...) is the game's own text, as the capture carried it. A page's `when` waits for a quest state;
-- the quests arrive in the next round, so until then a quest page stays closed. A choice's `id` is
-- what the client sends back in 0x803B. Pages are numbered from 1.

return {
    npc = "Necromancer Munne",

    [145] = {
        pages = {
            {
                body = gw(10982, 63947, 59705, 24018, 266, 13338, 55511, 56494, 24437, 1),
                buttons = {
                    {
                        icon = 18,
                        label = gw(5566, 53956, 61573, 10077),
                        id = 8398339,
                        action = { kind = "page", page = 4 },
                        when = { kind = "quest_offer_open", quest = 38 },
                    },
                    {
                        icon = 18,
                        label = gw(5811, 55789, 46416, 22465),
                        id = 8407043,
                        action = { kind = "page", page = 5 },
                        when = { kind = "quest_offer_open", quest = 72 },
                    },
                    {
                        icon = 11,
                        label = gw(13326, 46916, 52898, 7373),
                        id = 129,
                        action = { kind = "close" },
                    },
                    {
                        icon = 22,
                        label = gw(5566, 53956, 61573, 10077),
                        id = 8398341,
                        action = { kind = "close" },
                        when = { kind = "quest_active", quest = 38 },
                    },
                },
            },
            {
                when = { kind = "quest_reward_open", quest = 38 },
                body = gw(
                    5569, 59215, 44596, 30260, 266, 2985, 263, 76, 111, 103, 103, 101, 114, 1, 1, 2, 258, 2,
                    258, 2, 10984, 59348, 58828, 13938, 2, 10986, 35903, 46361, 26129, 257, 506, 2, 10991,
                    63120, 53358, 19539, 266, 9167, 39512, 53500, 17693, 1, 267, 2692, 266, 2327, 1, 257, 259,
                    1
                ),
                buttons = {
                    {
                        icon = 23,
                        label = gw(10975, 59449, 46843, 6569),
                        id = 8398343,
                        action = { kind = "quest", quest = 38, step = "reward" },
                    },
                },
            },
            {
                when = { kind = "quest_reward_open", quest = 72 },
                body = gw(
                    5815, 60194, 50354, 4665, 266, 2985, 263, 76, 111, 103, 103, 101, 114, 1, 1, 2, 258, 2,
                    258, 2, 10984, 59348, 58828, 13938, 2, 10986, 35903, 46361, 26129, 257, 506
                ),
                buttons = {
                    {
                        icon = 23,
                        label = gw(10975, 59449, 46843, 6569),
                        id = 8407047,
                        action = { kind = "quest", quest = 72, step = "reward" },
                    },
                },
            },
            {
                when = { kind = "quest_offer_open", quest = 38 },
                body = gw(
                    5567, 37557, 33330, 10356, 266, 2985, 263, 76, 111, 103, 103, 101, 114, 1, 1, 2, 263, 2,
                    10984, 59348, 58828, 13938, 2, 10986, 35903, 46361, 26129, 257, 506, 2, 10991, 63120,
                    53358, 19539, 266, 9167, 39512, 53500, 17693, 1, 267, 2692, 266, 2327, 1, 257, 259, 1, 2,
                    263
                ),
                buttons = {
                    {
                        icon = 16,
                        label = gw(5572, 39889, 53795, 21520),
                        id = 8398337,
                        action = { kind = "quest", quest = 38, step = "offer" },
                    },
                    {
                        icon = 17,
                        label = gw(5573, 34177, 51083, 31869),
                        id = 8398338,
                        action = { kind = "close" },
                    },
                },
            },
            {
                when = { kind = "quest_offer_open", quest = 72 },
                body = gw(
                    5812, 56866, 58508, 14568, 266, 2985, 263, 76, 111, 103, 103, 101, 114, 1, 1, 2, 263, 2,
                    10984, 59348, 58828, 13938, 2, 10986, 35903, 46361, 26129, 257, 506, 2, 263
                ),
                buttons = {
                    {
                        icon = 16,
                        label = gw(5820, 46741, 57579, 30022),
                        id = 8407041,
                        action = { kind = "quest", quest = 72, step = "offer" },
                    },
                    {
                        icon = 17,
                        label = gw(5821, 34071, 47957, 8192),
                        id = 8407042,
                        action = { kind = "close" },
                    },
                },
            },
            {
                body = gw(13335, 50342, 40322, 20269),
                buttons = {
                    {
                        icon = 2,
                        label = gw(13336, 38130, 52205, 30895),
                        id = 132,
                        action = { kind = "page", page = 7 },
                    },
                    { icon = 3, label = gw(13337, 51827, 65275, 2927), id = 128, action = { kind = "close" } },
                },
            },
            { when = { kind = "answer" }, body = gw(13339, 54156, 52230, 21025), buttons = {} },
        },
    },
}
