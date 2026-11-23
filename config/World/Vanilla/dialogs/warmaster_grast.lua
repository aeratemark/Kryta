-- Warmaster Grast: what a talk opens and what each choice does, per map.
--
-- Real Lua, loaded by the world (game/Scripting/NpcScripts.rs) and run by scripts/runtime.lua.
-- Converted 2026-11-23 from pseudo_lua/presearing dialogs/warmaster_grast.lua, which came from ASURA
-- config/World/Vanilla/dialogs.json (tools/npc-content-from-captures.py over preforged's pre-Searing
-- captures). The README in this folder is the whole format.
--
-- gw(...) is the game's own text, as the capture carried it. A page's `when` waits for a quest state;
-- the quests arrive in the next round, so until then a quest page stays closed. A choice's `id` is
-- what the client sends back in 0x803B. Pages are numbered from 1.

return {
    npc = "Warmaster Grast",

    [160] = {
        pages = {
            {
                body = gw(10982, 63947, 59705, 24018, 266, 13373, 51377, 43464, 28564, 1),
                buttons = {
                    {
                        icon = 18,
                        label = gw(5845, 49588, 54426, 12167),
                        id = 8407811,
                        action = { kind = "page", page = 3 },
                        when = { kind = "quest_offer_open", quest = 75 },
                    },
                    {
                        icon = 11,
                        label = gw(13383, 41289, 55240, 15909),
                        id = 129,
                        action = { kind = "page", page = 5 },
                    },
                },
            },
            {
                when = { kind = "quest_reward_open", quest = 75 },
                body = gw(
                    5848, 64773, 50685, 14204, 266, 2985, 263, 76, 111, 103, 103, 101, 114, 1, 1, 2, 258, 2,
                    258, 2, 10984, 59348, 58828, 13938, 2, 10986, 35903, 46361, 26129, 257, 506
                ),
                buttons = {
                    {
                        icon = 23,
                        label = gw(10975, 59449, 46843, 6569),
                        id = 8407815,
                        action = { kind = "quest", quest = 75, step = "reward" },
                    },
                },
            },
            {
                when = { kind = "quest_offer_open", quest = 75 },
                body = gw(
                    5846, 53780, 36848, 2472, 266, 2985, 263, 76, 111, 103, 103, 101, 114, 1, 1, 2, 263, 2,
                    10984, 59348, 58828, 13938, 2, 10986, 35903, 46361, 26129, 257, 506, 2, 263
                ),
                buttons = {
                    {
                        icon = 16,
                        label = gw(5851, 54039, 39201, 10079),
                        id = 8407809,
                        action = { kind = "quest", quest = 75, step = "offer" },
                    },
                    {
                        icon = 17,
                        label = gw(5852, 61760, 64160, 26176),
                        id = 8407810,
                        action = { kind = "close" },
                    },
                },
            },
            {
                body = gw(13384, 64625, 46717, 3907),
                buttons = {
                    {
                        icon = 6,
                        label = gw(13385, 65076, 36442, 27290),
                        id = 132,
                        action = { kind = "page", page = 6 },
                    },
                    {
                        icon = 3,
                        label = gw(13386, 63443, 33190, 17627),
                        id = 128,
                        action = { kind = "close" },
                    },
                },
            },
            { when = { kind = "answer" }, body = gw(13381, 58930, 40507, 16734), buttons = {} },
            { when = { kind = "answer" }, body = gw(13374, 56005, 33530, 3207), buttons = {} },
        },
    },
}
