-- Haversdan: what a talk opens and what each choice does, per map.
--
-- Real Lua, loaded by the world (game/Scripting/NpcScripts.rs) and run by scripts/runtime.lua.
-- Converted 2026-11-23 from pseudo_lua/presearing dialogs/haversdan.lua, which came from ASURA
-- config/World/Vanilla/dialogs.json (tools/npc-content-from-captures.py over preforged's pre-Searing
-- captures). The README in this folder is the whole format.
--
-- gw(...) is the game's own text, as the capture carried it. A page's `when` waits for a quest state;
-- the quests arrive in the next round, so until then a quest page stays closed. A choice's `id` is
-- what the client sends back in 0x803B. Pages are numbered from 1.

return {
    npc = "Haversdan",

    [146] = {
        pages = {
            {
                body = gw(10982, 63947, 59705, 24018, 266, 13290, 46076, 63363, 30213, 1),
                buttons = {
                    {
                        icon = 22,
                        label = gw(5632, 37838, 64636, 9418),
                        id = 8402437,
                        action = { kind = "close" },
                        when = { kind = "quest_active", quest = 54 },
                    },
                },
            },
            {
                when = { kind = "quest_offer_open", quest = 54 },
                body = gw(
                    5635, 33958, 47493, 16501, 266, 2302, 1, 2, 263, 2, 10984, 59348, 58828, 13938, 2, 10986,
                    35903, 46361, 26129, 257, 506, 2, 263
                ),
                buttons = {
                    {
                        icon = 16,
                        label = gw(5633, 58686, 41778, 24823),
                        id = 8402433,
                        action = { kind = "quest", quest = 54, step = "offer" },
                    },
                    {
                        icon = 17,
                        label = gw(5634, 36687, 40870, 28248),
                        id = 8402434,
                        action = { kind = "close" },
                    },
                },
            },
            {
                when = { kind = "quest_offer_open", quest = 65 },
                body = gw(
                    5761, 44431, 34935, 29107, 266, 2985, 263, 76, 111, 103, 103, 101, 114, 1, 1, 2, 263, 2,
                    10984, 59348, 58828, 13938, 2, 10986, 35903, 46361, 26129, 257, 506, 2, 263
                ),
                buttons = {
                    {
                        icon = 16,
                        label = gw(5765, 61731, 36959, 26986),
                        id = 8405249,
                        action = { kind = "quest", quest = 65, step = "offer" },
                    },
                    {
                        icon = 17,
                        label = gw(5766, 61527, 37124, 8422),
                        id = 8405250,
                        action = { kind = "close" },
                    },
                },
            },
        },
    },
}
