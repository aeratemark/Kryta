-- Gate Guard Torin: what a talk opens and what each choice does, per map.
--
-- Real Lua, loaded by the world (game/Scripting/NpcScripts.rs) and run by scripts/runtime.lua.
-- Converted 2026-11-23 from pseudo_lua/presearing dialogs/gate_guard_torin.lua, which came from ASURA
-- config/World/Vanilla/dialogs.json (tools/npc-content-from-captures.py over preforged's pre-Searing
-- captures). The README in this folder is the whole format.
--
-- gw(...) is the game's own text, as the capture carried it. A page's `when` waits for a quest state;
-- the quests arrive in the next round, so until then a quest page stays closed. A choice's `id` is
-- what the client sends back in 0x803B. Pages are numbered from 1.

return {
    npc = "Gate Guard Torin",

    [146] = {
        pages = {
            {
                body = gw(10982, 63947, 59705, 24018, 266, 13288, 53572, 47777, 10130, 1),
                buttons = {
                    {
                        icon = 22,
                        label = gw(5788, 60009, 46286, 25229),
                        id = 8406277,
                        action = { kind = "close" },
                        when = { kind = "quest_active", quest = 69 },
                    },
                },
            },
            {
                when = { kind = "quest_reward_open", quest = 69 },
                body = gw(
                    5797, 55668, 53249, 27098, 266, 2985, 263, 76, 111, 103, 103, 101, 114, 1, 1, 2, 258, 2,
                    258, 2, 10984, 59348, 58828, 13938, 2, 10986, 35903, 46361, 26129, 257, 506, 2, 10991,
                    63120, 53358, 19539, 266, 8761, 38295, 41115, 1827, 1, 267, 2697, 266, 2638, 1, 267, 2271,
                    1, 257, 263, 258, 266, 2, 258, 2, 33026, 4647, 266, 33026, 4650, 1, 1, 2, 10991, 63120,
                    53358, 19539, 266, 9767, 48911, 58770, 26386, 1, 267, 2697, 266, 2638, 1, 267, 2278, 1,
                    257, 259, 258, 261, 1
                ),
                buttons = {
                    {
                        icon = 23,
                        label = gw(10975, 59449, 46843, 6569),
                        id = 8406279,
                        action = { kind = "quest", quest = 69, step = "reward" },
                    },
                },
            },
            {
                when = { kind = "quest_offer_open", quest = 69 },
                body = gw(
                    5792, 37947, 43808, 15834, 266, 2985, 263, 76, 111, 103, 103, 101, 114, 1, 1, 2, 263, 2,
                    10984, 59348, 58828, 13938, 2, 10986, 35903, 46361, 26129, 257, 506, 2, 10991, 63120,
                    53358, 19539, 266, 8761, 38295, 41115, 1827, 1, 267, 2697, 266, 2638, 1, 267, 2271, 1, 257,
                    263, 258, 266, 2, 258, 2, 33026, 4647, 266, 33026, 4650, 1, 1, 2, 10991, 63120, 53358,
                    19539, 266, 9767, 48911, 58770, 26386, 1, 267, 2697, 266, 2638, 1, 267, 2278, 1, 257, 259,
                    258, 261, 1, 2, 263
                ),
                buttons = {
                    {
                        icon = 16,
                        label = gw(5794, 37151, 56040, 4313),
                        id = 8406273,
                        action = { kind = "quest", quest = 69, step = "offer" },
                    },
                    {
                        icon = 17,
                        label = gw(5795, 51542, 57296, 9546),
                        id = 8406274,
                        action = { kind = "close" },
                    },
                },
            },
        },
    },
}
