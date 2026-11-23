-- The Purveyor: what a talk opens and what each choice does, per map.
--
-- Real Lua, loaded by the world (game/Scripting/NpcScripts.rs) and run by scripts/runtime.lua.
-- Converted 2026-11-23 from pseudo_lua/presearing dialogs/the_purveyor.lua, which came from ASURA
-- config/World/Vanilla/dialogs.json (tools/npc-content-from-captures.py over preforged's pre-Searing
-- captures). The README in this folder is the whole format.
--
-- gw(...) is the game's own text, as the capture carried it. A page's `when` waits for a quest state;
-- the quests arrive in the next round, so until then a quest page stays closed. A choice's `id` is
-- what the client sends back in 0x803B. Pages are numbered from 1.

return {
    npc = "The Purveyor",

    [148] = {
        pages = {
            {
                body = gw(33026, 15998, 55221, 56292, 18930),
                buttons = {
                    {
                        icon = 28,
                        label = gw(33026, 15175),
                        id = 140,
                        action = { kind = "window", window = 5 },
                        skill = 0,
                    },
                    {
                        icon = 11,
                        label = gw(33026, 16053, 50075, 39151, 2035),
                        id = 135,
                        action = { kind = "unanswered" },
                    },
                    {
                        icon = 11,
                        label = gw(33026, 15999, 45222, 34459, 7559),
                        id = 141,
                        action = { kind = "unanswered" },
                    },
                    {
                        icon = 11,
                        label = gw(33026, 16023, 64181, 49740, 10724),
                        id = 142,
                        action = { kind = "unanswered" },
                    },
                    {
                        icon = 11,
                        label = gw(33026, 16002, 56342, 45921, 14533),
                        id = 143,
                        action = { kind = "unanswered" },
                    },
                    {
                        icon = 24,
                        label = gw(33026, 16050, 38968, 52451, 31423),
                        id = 139,
                        action = { kind = "merchant" },
                    },
                    {
                        icon = 15,
                        label = gw(33026, 16011, 63858, 38457, 27270),
                        id = 133,
                        action = { kind = "close" },
                    },
                },
            },
        },
    },
}
