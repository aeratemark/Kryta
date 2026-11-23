-- Meerak the Scribe: what a talk opens and what each choice does, per map.
--
-- Real Lua, loaded by the world (game/Scripting/NpcScripts.rs) and run by scripts/runtime.lua.
-- Converted 2026-11-23 from pseudo_lua/presearing dialogs/meerak_the_scribe.lua, which came from ASURA
-- config/World/Vanilla/dialogs.json (tools/npc-content-from-captures.py over preforged's pre-Searing
-- captures). The README in this folder is the whole format.
--
-- gw(...) is the game's own text, as the capture carried it. A page's `when` waits for a quest state;
-- the quests arrive in the next round, so until then a quest page stays closed. A choice's `id` is
-- what the client sends back in 0x803B. Pages are numbered from 1.

return {
    npc = "Meerak the Scribe",

    [164] = {
        pages = {
            {
                body = gw(10982, 63947, 59705, 24018, 266, 13302, 50066, 39284, 12642, 1),
                buttons = {
                    {
                        icon = 21,
                        label = gw(5737, 60676, 52161, 11121),
                        id = 8404484,
                        action = { kind = "quest", quest = 62, step = "credit", page = 2 },
                        when = { kind = "quest_active", quest = 62 },
                    },
                },
            },
            { when = { kind = "answer" }, body = gw(5741, 53432, 37383, 1667), buttons = {} },
        },
    },
}
