-- Devona: what a talk opens and what each choice does, per map.
--
-- Real Lua, loaded by the world (game/Scripting/NpcScripts.rs) and run by scripts/runtime.lua.
-- Converted 2026-11-23 from pseudo_lua/presearing dialogs/devona.lua, which came from ASURA
-- config/World/Vanilla/dialogs.json (tools/npc-content-from-captures.py over preforged's pre-Searing
-- captures). The README in this folder is the whole format.
--
-- gw(...) is the game's own text, as the capture carried it. A page's `when` waits for a quest state;
-- the quests arrive in the next round, so until then a quest page stays closed. A choice's `id` is
-- what the client sends back in 0x803B. Pages are numbered from 1.

return {
    npc = "Devona",

    [146] = {
        pages = {
            {
                body = gw(10982, 63947, 59705, 24018, 266, 13256, 61237, 45690, 30750, 1),
                buttons = {
                    {
                        icon = 18,
                        label = gw(5737, 60676, 52161, 11121),
                        id = 8404483,
                        action = { kind = "page", page = 4 },
                        when = { kind = "quest_offer_open", quest = 62 },
                    },
                    {
                        icon = 21,
                        label = gw(5632, 37838, 64636, 9418),
                        id = 8402438,
                        action = { kind = "page", page = 3 },
                        when = { kind = "quest_reward_open", quest = 54 },
                    },
                    {
                        icon = 21,
                        label = gw(5503, 37998, 48760, 23264),
                        id = 8399110,
                        action = { kind = "page", page = 2 },
                        when = { kind = "quest_reward_open", quest = 41 },
                    },
                    {
                        icon = 22,
                        label = gw(5737, 60676, 52161, 11121),
                        id = 8404485,
                        action = { kind = "close" },
                        when = { kind = "quest_active", quest = 62 },
                    },
                },
            },
            {
                when = { kind = "quest_reward_open", quest = 41 },
                body = gw(
                    5510, 43732, 58515, 8165, 266, 2985, 263, 76, 111, 103, 103, 101, 114, 1, 1, 2, 258, 2,
                    258, 2, 10984, 59348, 58828, 13938, 2, 10986, 35903, 46361, 26129, 257, 506, 2, 10991,
                    63120, 53358, 19539, 266, 9168, 34355, 39460, 22700, 1, 267, 2692, 266, 2327, 1, 257, 259,
                    1, 2, 10991, 63120, 53358, 19539, 266, 9808, 48724, 51282, 20020, 1, 267, 2694, 266, 2628,
                    1, 257, 260, 1
                ),
                buttons = {
                    {
                        icon = 23,
                        label = gw(10975, 59449, 46843, 6569),
                        id = 8399111,
                        action = { kind = "quest", quest = 41, step = "reward" },
                    },
                },
            },
            {
                when = { kind = "quest_reward_open", quest = 54 },
                body = gw(
                    5637, 54844, 36342, 8820, 266, 2985, 263, 76, 111, 103, 103, 101, 114, 1, 1, 2, 258, 2,
                    258, 2, 10984, 59348, 58828, 13938, 2, 10986, 35903, 46361, 26129, 257, 506
                ),
                buttons = {
                    {
                        icon = 23,
                        label = gw(10975, 59449, 46843, 6569),
                        id = 8402439,
                        action = { kind = "quest", quest = 54, step = "reward" },
                    },
                },
            },
            {
                when = { kind = "quest_offer_open", quest = 62 },
                body = gw(
                    5740, 37586, 38312, 27892, 266, 2985, 263, 76, 111, 103, 103, 101, 114, 1, 1, 2, 263, 2,
                    10984, 59348, 58828, 13938, 2, 10986, 35903, 46361, 26129, 257, 506, 2, 10991, 63120,
                    53358, 19539, 266, 9809, 33833, 54950, 12855, 1, 267, 2694, 266, 2628, 1, 257, 260, 1, 2,
                    263
                ),
                buttons = {
                    {
                        icon = 16,
                        label = gw(5738, 61927, 57527, 23335),
                        id = 8404481,
                        action = { kind = "quest", quest = 62, step = "offer" },
                    },
                    {
                        icon = 17,
                        label = gw(5739, 48341, 61582, 9692),
                        id = 8404482,
                        action = { kind = "close" },
                    },
                },
            },
        },
    },
}
