-- Duke Barradin: what a talk opens and what each choice does, per map.
--
-- Real Lua, loaded by the world (game/Scripting/NpcScripts.rs) and run by scripts/runtime.lua.
-- Converted 2026-11-23 from pseudo_lua/presearing dialogs/duke_barradin.lua, which came from ASURA
-- config/World/Vanilla/dialogs.json (tools/npc-content-from-captures.py over preforged's pre-Searing
-- captures). The README in this folder is the whole format.
--
-- gw(...) is the game's own text, as the capture carried it. A page's `when` waits for a quest state;
-- the quests arrive in the next round, so until then a quest page stays closed. A choice's `id` is
-- what the client sends back in 0x803B. Pages are numbered from 1.

return {
    npc = "Duke Barradin",

    [163] = {
        pages = {
            {
                body = gw(10982, 63947, 59705, 24018, 266, 13236, 55271, 54765, 30810, 1),
                buttons = {
                    {
                        icon = 11,
                        label = gw(13238, 41119, 49079, 21626),
                        id = 129,
                        action = { kind = "close" },
                    },
                },
            },
        },
    },
}
