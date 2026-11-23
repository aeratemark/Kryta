-- Ivor Trueshot: what a talk opens and what each choice does, per map.
--
-- Real Lua, loaded by the world (game/Scripting/NpcScripts.rs) and run by scripts/runtime.lua.
-- Converted 2026-11-23 from pseudo_lua/presearing dialogs/ivor_trueshot.lua, which came from ASURA
-- config/World/Vanilla/dialogs.json (tools/npc-content-from-captures.py over preforged's pre-Searing
-- captures). The README in this folder is the whole format.
--
-- gw(...) is the game's own text, as the capture carried it. A page's `when` waits for a quest state;
-- the quests arrive in the next round, so until then a quest page stays closed. A choice's `id` is
-- what the client sends back in 0x803B. Pages are numbered from 1.

return {
    npc = "Ivor Trueshot",

    [162] = {
        pages = {
            {
                body = gw(10982, 63947, 59705, 24018, 266, 13074, 60779, 53038, 17545, 1),
                buttons = {
                    {
                        icon = 18,
                        label = gw(5521, 34677, 57313, 9517),
                        id = 8399363,
                        action = { kind = "page", page = 3 },
                        when = { kind = "quest_offer_open", quest = 42 },
                    },
                    {
                        icon = 11,
                        label = gw(13083, 34851, 47142, 29890),
                        id = 129,
                        action = { kind = "close" },
                    },
                },
            },
            {
                when = { kind = "quest_reward_open", quest = 42 },
                body = gw(
                    5525, 49217, 56119, 22725, 266, 2985, 263, 76, 111, 103, 103, 101, 114, 1, 1, 2, 258, 2,
                    258, 2, 10984, 59348, 58828, 13938, 2, 10986, 35903, 46361, 26129, 257, 506
                ),
                buttons = {
                    {
                        icon = 23,
                        label = gw(10975, 59449, 46843, 6569),
                        id = 8399367,
                        action = { kind = "quest", quest = 42, step = "reward" },
                    },
                },
            },
            {
                when = { kind = "quest_offer_open", quest = 42 },
                body = gw(
                    5522, 59955, 47473, 30037, 266, 2985, 263, 76, 111, 103, 103, 101, 114, 1, 1, 2, 263, 2,
                    10984, 59348, 58828, 13938, 2, 10986, 35903, 46361, 26129, 257, 506, 2, 263
                ),
                buttons = {
                    {
                        icon = 16,
                        label = gw(5528, 35207, 62195, 29743),
                        id = 8399361,
                        action = { kind = "quest", quest = 42, step = "offer" },
                    },
                    {
                        icon = 17,
                        label = gw(5529, 50865, 34812, 32014),
                        id = 8399362,
                        action = { kind = "close" },
                    },
                },
            },
        },
    },
}
