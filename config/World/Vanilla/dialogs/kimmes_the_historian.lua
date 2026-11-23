-- Kimmes the Historian: what a talk opens and what each choice does, per map.
--
-- Real Lua, loaded by the world (game/Scripting/NpcScripts.rs) and run by scripts/runtime.lua.
-- Converted 2026-11-23 from pseudo_lua/presearing dialogs/kimmes_the_historian.lua, which came from ASURA
-- config/World/Vanilla/dialogs.json (tools/npc-content-from-captures.py over preforged's pre-Searing
-- captures). The README in this folder is the whole format.
--
-- gw(...) is the game's own text, as the capture carried it. A page's `when` waits for a quest state;
-- the quests arrive in the next round, so until then a quest page stays closed. A choice's `id` is
-- what the client sends back in 0x803B. Pages are numbered from 1.

return {
    npc = "Kimmes the Historian",

    [148] = {
        pages = {
            {
                body = gw(33026, 29193, 34884, 38128, 2402),
                buttons = {
                    {
                        icon = 15,
                        label = gw(33026, 29199, 46921, 48272, 22792),
                        id = 132,
                        action = { kind = "close" },
                    },
                },
            },
        },
    },
}
