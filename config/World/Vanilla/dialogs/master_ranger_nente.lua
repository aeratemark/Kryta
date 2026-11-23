-- Master Ranger Nente: what a talk opens and what each choice does, per map.
--
-- Real Lua, loaded by the world (game/Scripting/NpcScripts.rs) and run by scripts/runtime.lua.
-- Converted 2026-11-23 from pseudo_lua/presearing dialogs/master_ranger_nente.lua, which came from ASURA
-- config/World/Vanilla/dialogs.json (tools/npc-content-from-captures.py over preforged's pre-Searing
-- captures). The README in this folder is the whole format.
--
-- gw(...) is the game's own text, as the capture carried it. A page's `when` waits for a quest state;
-- the quests arrive in the next round, so until then a quest page stays closed. A choice's `id` is
-- what the client sends back in 0x803B. Pages are numbered from 1.

return {
    npc = "Master Ranger Nente",

    [162] = {
        pages = {
            {
                body = gw(10982, 63947, 59705, 24018, 266, 13389, 57930, 44337, 8721, 1),
                buttons = {
                    {
                        icon = 18,
                        label = gw(5856, 39789, 48804, 13222),
                        id = 8408067,
                        action = { kind = "page", page = 3 },
                        when = { kind = "quest_offer_open", quest = 76 },
                    },
                    {
                        icon = 11,
                        label = gw(13399, 39317, 54623, 12619),
                        id = 129,
                        action = { kind = "page", page = 5 },
                    },
                },
            },
            {
                when = { kind = "quest_reward_open", quest = 76 },
                body = gw(
                    5862, 49886, 38008, 29184, 266, 2985, 263, 76, 111, 103, 103, 101, 114, 1, 1, 2, 258, 2,
                    258, 2, 10984, 59348, 58828, 13938, 2, 10986, 35903, 46361, 26129, 257, 506
                ),
                buttons = {
                    {
                        icon = 23,
                        label = gw(10975, 59449, 46843, 6569),
                        id = 8408071,
                        action = { kind = "quest", quest = 76, step = "reward" },
                    },
                },
            },
            {
                when = { kind = "quest_offer_open", quest = 76 },
                body = gw(
                    5859, 57515, 46327, 1573, 266, 2985, 263, 76, 111, 103, 103, 101, 114, 1, 1, 2, 263, 2,
                    10984, 59348, 58828, 13938, 2, 10986, 35903, 46361, 26129, 257, 506, 2, 263
                ),
                buttons = {
                    {
                        icon = 16,
                        label = gw(5857, 53010, 47505, 14892),
                        id = 8408065,
                        action = { kind = "quest", quest = 76, step = "offer" },
                    },
                    {
                        icon = 17,
                        label = gw(5858, 39776, 36051, 2252),
                        id = 8408066,
                        action = { kind = "close" },
                    },
                },
            },
            {
                body = gw(10982, 63947, 59705, 24018, 266, 13389, 57930, 44337, 8721, 1),
                buttons = {
                    {
                        icon = 11,
                        label = gw(13399, 39317, 54623, 12619),
                        id = 129,
                        action = { kind = "page", page = 5 },
                    },
                    {
                        icon = 15,
                        label = gw(10981, 56770, 59190, 3361),
                        id = 128,
                        action = { kind = "close" },
                    },
                },
            },
            { when = { kind = "answer" }, body = gw(13397, 35399, 62838, 4891), buttons = {} },
        },
    },
}
