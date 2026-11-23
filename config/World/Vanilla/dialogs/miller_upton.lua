-- Miller Upton: what a talk opens and what each choice does, per map.
--
-- Real Lua, loaded by the world (game/Scripting/NpcScripts.rs) and run by scripts/runtime.lua.
-- Converted 2026-11-23 from pseudo_lua/presearing dialogs/miller_upton.lua, which came from ASURA
-- config/World/Vanilla/dialogs.json (tools/npc-content-from-captures.py over preforged's pre-Searing
-- captures). The README in this folder is the whole format.
--
-- gw(...) is the game's own text, as the capture carried it. A page's `when` waits for a quest state;
-- the quests arrive in the next round, so until then a quest page stays closed. A choice's `id` is
-- what the client sends back in 0x803B. Pages are numbered from 1.

return {
    npc = "Miller Upton",

    [146] = {
        pages = {
            {
                body = gw(10982, 63947, 59705, 24018, 266, 13295, 64710, 47231, 22126, 1),
                buttons = {
                    {
                        icon = 21,
                        label = gw(5703, 39622, 57907, 6159),
                        id = 8406532,
                        action = { kind = "page", page = 4 },
                        when = { kind = "quest_active", quest = 70 },
                    },
                },
            },
            {
                when = { kind = "quest_reward_open", quest = 70 },
                body = gw(
                    5706, 41035, 52223, 30240, 266, 2985, 263, 76, 111, 103, 103, 101, 114, 1, 1, 2, 258, 2,
                    258, 2, 10984, 59348, 58828, 13938, 2, 10986, 35903, 46361, 26129, 257, 756, 2, 10991,
                    63120, 53358, 19539, 266, 9173, 43855, 36894, 9784, 1, 267, 2692, 266, 2327, 1, 257, 259,
                    1
                ),
                buttons = {
                    {
                        icon = 23,
                        label = gw(10975, 59449, 46843, 6569),
                        id = 8406535,
                        action = { kind = "quest", quest = 70, step = "reward" },
                    },
                },
            },
            {
                body = gw(10982, 63947, 59705, 24018, 266, 13295, 64710, 47231, 22126, 1),
                buttons = {
                    {
                        icon = 15,
                        label = gw(10981, 56770, 59190, 3361),
                        id = 128,
                        action = { kind = "close" },
                    },
                },
            },
            { when = { kind = "answer" }, body = gw(5711, 64666, 65247, 20726), buttons = {} },
        },
    },
}
