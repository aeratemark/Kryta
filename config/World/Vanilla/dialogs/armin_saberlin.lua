-- Armin Saberlin: what a talk opens and what each choice does, per map.
--
-- Real Lua, loaded by the world (game/Scripting/NpcScripts.rs) and run by scripts/runtime.lua.
-- Converted 2026-11-23 from pseudo_lua/presearing dialogs/armin_saberlin.lua, which came from ASURA
-- config/World/Vanilla/dialogs.json (tools/npc-content-from-captures.py over preforged's pre-Searing
-- captures). The README in this folder is the whole format.
--
-- gw(...) is the game's own text, as the capture carried it. A page's `when` waits for a quest state;
-- the quests arrive in the next round, so until then a quest page stays closed. A choice's `id` is
-- what the client sends back in 0x803B. Pages are numbered from 1.

return {
    npc = "Armin Saberlin",

    [148] = {
        pages = {
            {
                body = gw(10982, 63947, 59705, 24018, 266, 13210, 39754, 35343, 9640, 1),
                buttons = {
                    {
                        icon = 22,
                        label = gw(6007, 34027, 54662, 5611),
                        id = 8408837,
                        action = { kind = "page", page = 7 },
                        when = { kind = "quest_active", quest = 79 },
                    },
                    {
                        icon = 11,
                        label = gw(13212, 59116, 52615, 9584),
                        id = 129,
                        action = { kind = "close" },
                    },
                },
            },
            {
                when = { kind = "quest_reward_open", quest = 62 },
                body = gw(
                    5743, 45961, 43002, 24497, 266, 2985, 263, 76, 111, 103, 103, 101, 114, 1, 1, 2, 258, 2,
                    258, 2, 10984, 59348, 58828, 13938, 2, 10986, 35903, 46361, 26129, 257, 506, 2, 10991,
                    63120, 53358, 19539, 266, 9809, 33833, 54950, 12855, 1, 267, 2694, 266, 2628, 1, 257, 260,
                    1
                ),
                buttons = {
                    {
                        icon = 23,
                        label = gw(10975, 59449, 46843, 6569),
                        id = 8404487,
                        action = { kind = "quest", quest = 62, step = "reward" },
                    },
                },
            },
            {
                when = { kind = "quest_reward_open", quest = 79 },
                body = gw(
                    6009, 41895, 46598, 23366, 266, 2985, 263, 76, 111, 103, 103, 101, 114, 1, 1, 2, 258, 2,
                    258, 2, 10984, 59348, 58828, 13938, 2, 10986, 35903, 46361, 26129, 257, 506
                ),
                buttons = {
                    {
                        icon = 23,
                        label = gw(10975, 59449, 46843, 6569),
                        id = 8408839,
                        action = { kind = "quest", quest = 79, step = "reward" },
                    },
                },
            },
            {
                when = { kind = "quest_offer_open", quest = 79 },
                body = gw(
                    6008, 36366, 54757, 6776, 266, 2985, 263, 76, 111, 103, 103, 101, 114, 1, 1, 2, 263, 2,
                    10984, 59348, 58828, 13938, 2, 10986, 35903, 46361, 26129, 257, 506, 2, 263
                ),
                buttons = {
                    {
                        icon = 16,
                        label = gw(6016, 65230, 48555, 29852),
                        id = 8408833,
                        action = { kind = "quest", quest = 79, step = "offer" },
                    },
                    {
                        icon = 17,
                        label = gw(6017, 37632, 40804, 28488),
                        id = 8408834,
                        action = { kind = "close" },
                    },
                },
            },
            {
                when = { kind = "quest_offer_open", quest = 89 },
                body = gw(
                    6041, 40818, 63895, 21667, 266, 2985, 263, 76, 111, 103, 103, 101, 114, 1, 1, 2, 263, 2,
                    10984, 59348, 58828, 13938, 2, 10986, 35903, 46361, 26129, 257, 356, 2, 263
                ),
                buttons = {
                    {
                        icon = 16,
                        label = gw(6042, 62660, 39191, 6042),
                        id = 8411393,
                        action = { kind = "quest", quest = 89, step = "offer" },
                    },
                    {
                        icon = 17,
                        label = gw(6043, 54496, 57513, 22472),
                        id = 8411394,
                        action = { kind = "close" },
                    },
                },
            },
            {
                body = gw(10982, 63947, 59705, 24018, 266, 13210, 39754, 35343, 9640, 1),
                buttons = {
                    {
                        icon = 15,
                        label = gw(10981, 56770, 59190, 3361),
                        id = 128,
                        action = { kind = "close" },
                    },
                },
            },
            { when = { kind = "answer" }, body = gw(13211, 39437, 62941, 27968), buttons = {} },
        },
    },
}
