-- Town Crier: what a talk opens and what each choice does, per map.
--
-- Real Lua, loaded by the world (game/Scripting/NpcScripts.rs) and run by scripts/runtime.lua.
-- Converted 2026-11-23 from pseudo_lua/presearing dialogs/town_crier.lua, which came from ASURA
-- config/World/Vanilla/dialogs.json (tools/npc-content-from-captures.py over preforged's pre-Searing
-- captures). The README in this folder is the whole format.
--
-- gw(...) is the game's own text, as the capture carried it. A page's `when` waits for a quest state;
-- the quests arrive in the next round, so until then a quest page stays closed. A choice's `id` is
-- what the client sends back in 0x803B. Pages are numbered from 1.

return {
    npc = "Town Crier",

    [148] = {
        pages = {
            {
                body = gw(10982, 63947, 59705, 24018, 266, 13173, 65041, 54639, 8597, 1),
                buttons = {
                    {
                        icon = 22,
                        label = gw(5891, 43118, 40603, 19586),
                        id = 8409093,
                        action = { kind = "close" },
                        when = { kind = "quest_active", quest = 80 },
                    },
                },
            },
            {
                when = { kind = "quest_offer_open", quest = 80 },
                body = gw(
                    5894, 63503, 44219, 20386, 266, 2985, 263, 76, 111, 103, 103, 101, 114, 1, 1, 2, 263, 2,
                    10984, 59348, 58828, 13938, 2, 10986, 35903, 46361, 26129, 257, 356, 2, 10988, 56007,
                    33198, 13442, 257, 266, 2, 263
                ),
                buttons = {
                    {
                        icon = 16,
                        label = gw(5892, 43591, 33851, 5198),
                        id = 8409089,
                        action = { kind = "quest", quest = 80, step = "offer" },
                    },
                    {
                        icon = 17,
                        label = gw(5893, 57119, 59239, 16699),
                        id = 8409090,
                        action = { kind = "close" },
                    },
                },
            },
            {
                body = gw(10982, 63947, 59705, 24018, 266, 13173, 65041, 54639, 8597, 1),
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
