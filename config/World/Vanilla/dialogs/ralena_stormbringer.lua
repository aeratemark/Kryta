-- Ralena Stormbringer: what a talk opens and what each choice does, per map.
--
-- Real Lua, loaded by the world (game/Scripting/NpcScripts.rs) and run by scripts/runtime.lua.
-- Converted 2026-11-23 from pseudo_lua/presearing dialogs/ralena_stormbringer.lua, which came from ASURA
-- config/World/Vanilla/dialogs.json (tools/npc-content-from-captures.py over preforged's pre-Searing
-- captures). The README in this folder is the whole format.
--
-- gw(...) is the game's own text, as the capture carried it. A page's `when` waits for a quest state;
-- the quests arrive in the next round, so until then a quest page stays closed. A choice's `id` is
-- what the client sends back in 0x803B. Pages are numbered from 1.

return {
    npc = "Ralena Stormbringer",

    [165] = {
        pages = {
            {
                body = gw(10982, 63947, 59705, 24018, 266, 13041, 40303, 35507, 4053, 1),
                buttons = {
                    {
                        icon = 18,
                        label = gw(5477, 51966, 63968, 11667),
                        id = 8398083,
                        action = { kind = "page", page = 2 },
                        when = { kind = "quest_offer_open", quest = 37 },
                    },
                    {
                        icon = 11,
                        label = gw(13050, 59696, 38326, 22421),
                        id = 129,
                        action = { kind = "close" },
                    },
                    {
                        icon = 21,
                        label = gw(5673, 58870, 35349, 7652),
                        id = 8402692,
                        action = { kind = "page", page = 3 },
                        when = { kind = "quest_active", quest = 55 },
                    },
                    {
                        icon = 22,
                        label = gw(5477, 51966, 63968, 11667),
                        id = 8398085,
                        action = { kind = "close" },
                        when = { kind = "quest_active", quest = 37 },
                    },
                    {
                        icon = 21,
                        label = gw(6007, 34027, 54662, 5611),
                        id = 8408836,
                        action = { kind = "quest", quest = 79, step = "credit", page = 4 },
                        when = { kind = "quest_active", quest = 79 },
                    },
                },
            },
            {
                when = { kind = "quest_offer_open", quest = 37 },
                body = gw(
                    5478, 63058, 50298, 5696, 266, 2985, 263, 76, 111, 103, 103, 101, 114, 1, 1, 2, 263, 2,
                    10984, 59348, 58828, 13938, 2, 10986, 35903, 46361, 26129, 257, 506, 2, 263
                ),
                buttons = {
                    {
                        icon = 16,
                        label = gw(5482, 40184, 54501, 12153),
                        id = 8398081,
                        action = { kind = "quest", quest = 37, step = "offer" },
                    },
                    {
                        icon = 17,
                        label = gw(5483, 33463, 36443, 29861),
                        id = 8398082,
                        action = { kind = "close" },
                    },
                },
            },
            { when = { kind = "answer" }, body = gw(5686, 57983, 55926, 16940), buttons = {} },
            { when = { kind = "answer" }, body = gw(6020, 37663, 37758, 27917), buttons = {} },
        },
    },
}
