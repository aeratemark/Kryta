-- Namar: what a talk opens and what each choice does, per map.
--
-- Real Lua, loaded by the world (game/Scripting/NpcScripts.rs) and run by scripts/runtime.lua.
-- Converted 2026-11-23 from pseudo_lua/presearing dialogs/namar.lua, which came from ASURA
-- config/World/Vanilla/dialogs.json (tools/npc-content-from-captures.py over preforged's pre-Searing
-- captures). The README in this folder is the whole format.
--
-- gw(...) is the game's own text, as the capture carried it. A page's `when` waits for a quest state;
-- the quests arrive in the next round, so until then a quest page stays closed. A choice's `id` is
-- what the client sends back in 0x803B. Pages are numbered from 1.

return {
    npc = "Namar",

    [148] = {
        pages = {
            {
                body = gw(10982, 63947, 59705, 24018, 266, 13181, 64045, 56986, 16111, 1),
                buttons = {
                    {
                        icon = 22,
                        label = gw(5703, 39622, 57907, 6159),
                        id = 8406533,
                        action = { kind = "close" },
                        when = { kind = "quest_active", quest = 70 },
                    },
                },
            },
            {
                when = { kind = "quest_offer_open", quest = 70 },
                body = gw(
                    5704, 64550, 52340, 8339, 266, 2985, 263, 76, 111, 103, 103, 101, 114, 1, 1, 2, 263, 2,
                    10984, 59348, 58828, 13938, 2, 10986, 35903, 46361, 26129, 257, 756, 2, 10991, 63120,
                    53358, 19539, 266, 9173, 43855, 36894, 9784, 1, 267, 2692, 266, 2327, 1, 257, 259, 1, 2,
                    263
                ),
                buttons = {
                    {
                        icon = 16,
                        label = gw(5709, 63832, 42738, 11661),
                        id = 8406529,
                        action = { kind = "quest", quest = 70, step = "offer" },
                    },
                    {
                        icon = 17,
                        label = gw(5710, 41072, 64919, 10183),
                        id = 8406530,
                        action = { kind = "close" },
                    },
                },
            },
        },
    },
}
