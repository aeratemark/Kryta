-- Orion Elek: what a talk opens and what each choice does, per map.
--
-- Real Lua, loaded by the world (game/Scripting/NpcScripts.rs) and run by scripts/runtime.lua.
-- Converted 2026-11-23 from pseudo_lua/presearing dialogs/orion_elek.lua, which came from ASURA
-- config/World/Vanilla/dialogs.json (tools/npc-content-from-captures.py over preforged's pre-Searing
-- captures). The README in this folder is the whole format.
--
-- gw(...) is the game's own text, as the capture carried it. A page's `when` waits for a quest state;
-- the quests arrive in the next round, so until then a quest page stays closed. A choice's `id` is
-- what the client sends back in 0x803B. Pages are numbered from 1.

return {
    npc = "Orion Elek",

    [161] = {
        pages = {
            {
                body = gw(10982, 63947, 59705, 24018, 266, 5682, 46601, 50502, 27148, 1),
                buttons = {
                    {
                        icon = 21,
                        label = gw(5673, 58870, 35349, 7652),
                        id = 8402692,
                        action = { kind = "quest", quest = 55, step = "credit", page = 3 },
                        when = { kind = "quest_active", quest = 55 },
                    },
                },
            },
            {
                body = gw(10982, 63947, 59705, 24018, 266, 5682, 46601, 50502, 27148, 1),
                buttons = {
                    {
                        icon = 15,
                        label = gw(10981, 56770, 59190, 3361),
                        id = 128,
                        action = { kind = "close" },
                    },
                },
            },
            { when = { kind = "answer" }, body = gw(5684, 38403, 63127, 737), buttons = {} },
        },
    },
}
