-- Vassar: what a talk opens and what each choice does, per map.
--
-- Real Lua, loaded by the world (game/Scripting/NpcScripts.rs) and run by scripts/runtime.lua.
-- Converted 2026-11-23 from pseudo_lua/presearing dialogs/vassar.lua, which came from ASURA
-- config/World/Vanilla/dialogs.json (tools/npc-content-from-captures.py over preforged's pre-Searing
-- captures). The README in this folder is the whole format.
--
-- gw(...) is the game's own text, as the capture carried it. A page's `when` waits for a quest state;
-- the quests arrive in the next round, so until then a quest page stays closed. A choice's `id` is
-- what the client sends back in 0x803B. Pages are numbered from 1.

return {
    npc = "Vassar",

    [165] = {
        pages = {
            {
                body = gw(10982, 63947, 59705, 24018, 266, 13008, 57951, 63283, 5979, 1),
                buttons = {
                    {
                        icon = 21,
                        label = gw(6007, 34027, 54662, 5611),
                        id = 8408836,
                        action = { kind = "quest", quest = 79, step = "credit", page = 5 },
                        when = { kind = "quest_active", quest = 79 },
                    },
                    {
                        icon = 11,
                        label = gw(13017, 48411, 45425, 28781),
                        id = 129,
                        action = { kind = "close" },
                    },
                    {
                        icon = 18,
                        label = gw(5593, 56108, 61390, 28904),
                        id = 8401155,
                        action = { kind = "page", page = 3 },
                        when = { kind = "quest_offer_open", quest = 49 },
                    },
                },
            },
            {
                when = { kind = "quest_reward_open", quest = 49 },
                body = gw(
                    5596, 62865, 60847, 16759, 266, 2985, 263, 76, 111, 103, 103, 101, 114, 1, 1, 2, 258, 2,
                    258, 2, 10984, 59348, 58828, 13938, 2, 10986, 35903, 46361, 26129, 257, 506
                ),
                buttons = {
                    {
                        icon = 23,
                        label = gw(10975, 59449, 46843, 6569),
                        id = 8401159,
                        action = { kind = "quest", quest = 49, step = "reward" },
                    },
                },
            },
            {
                when = { kind = "quest_offer_open", quest = 49 },
                body = gw(
                    5594, 36945, 56956, 23296, 266, 2985, 263, 76, 111, 103, 103, 101, 114, 1, 1, 2, 263, 2,
                    10984, 59348, 58828, 13938, 2, 10986, 35903, 46361, 26129, 257, 506, 2, 263
                ),
                buttons = {
                    {
                        icon = 16,
                        label = gw(5598, 42603, 52960, 28562),
                        id = 8401153,
                        action = { kind = "quest", quest = 49, step = "offer" },
                    },
                    {
                        icon = 17,
                        label = gw(5599, 43440, 35005, 2856),
                        id = 8401154,
                        action = { kind = "close" },
                    },
                },
            },
            {
                body = gw(10982, 63947, 59705, 24018, 266, 13008, 57951, 63283, 5979, 1),
                buttons = {
                    {
                        icon = 11,
                        label = gw(13017, 48411, 45425, 28781),
                        id = 129,
                        action = { kind = "close" },
                    },
                    {
                        icon = 15,
                        label = gw(10981, 56770, 59190, 3361),
                        id = 128,
                        action = { kind = "close" },
                    },
                },
            },
            { when = { kind = "answer" }, body = gw(6018, 53199, 53156, 22604), buttons = {} },
        },
    },
}
