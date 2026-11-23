-- Verata the Necromancer: what a talk opens and what each choice does, per map.
--
-- Real Lua, loaded by the world (game/Scripting/NpcScripts.rs) and run by scripts/runtime.lua.
-- Converted 2026-11-23 from pseudo_lua/presearing dialogs/verata_the_necromancer.lua, which came from ASURA
-- config/World/Vanilla/dialogs.json (tools/npc-content-from-captures.py over preforged's pre-Searing
-- captures). The README in this folder is the whole format.
--
-- gw(...) is the game's own text, as the capture carried it. A page's `when` waits for a quest state;
-- the quests arrive in the next round, so until then a quest page stays closed. A choice's `id` is
-- what the client sends back in 0x803B. Pages are numbered from 1.

return {
    npc = "Verata the Necromancer",

    [146] = {
        pages = {
            {
                body = gw(10982, 63947, 59705, 24018, 266, 5917, 52144, 35131, 17048, 1),
                buttons = {
                    {
                        icon = 22,
                        label = gw(5909, 59866, 50308, 20843),
                        id = 8409605,
                        action = { kind = "page", page = 5 },
                        when = { kind = "quest_active", quest = 82 },
                    },
                },
            },
            {
                when = { kind = "quest_reward_open", quest = 82 },
                body = gw(
                    5914, 57212, 63938, 19471, 266, 2985, 263, 76, 111, 103, 103, 101, 114, 1, 1, 2, 258, 2,
                    258, 2, 10984, 59348, 58828, 13938, 2, 10986, 35903, 46361, 26129, 257, 756, 2, 10994,
                    51111, 34011, 18705, 266, 25382, 1, 2, 10994, 51111, 34011, 18705, 266, 25286, 1
                ),
                buttons = {
                    {
                        icon = 23,
                        label = gw(10975, 59449, 46843, 6569),
                        id = 8409607,
                        action = { kind = "quest", quest = 82, step = "reward" },
                    },
                },
            },
            {
                when = { kind = "quest_reward_open", quest = 218 },
                body = gw(
                    5356, 45093, 54228, 9857, 266, 2985, 263, 76, 111, 103, 103, 101, 114, 1, 1, 2, 258, 2,
                    258, 2, 10984, 59348, 58828, 13938, 2, 10986, 35903, 46361, 26129, 257, 506
                ),
                buttons = {
                    {
                        icon = 23,
                        label = gw(10975, 59449, 46843, 6569),
                        id = 8444423,
                        action = { kind = "quest", quest = 218, step = "reward" },
                    },
                },
            },
            {
                when = { kind = "quest_offer_open", quest = 82 },
                body = gw(
                    5912, 41541, 40860, 18521, 266, 2985, 263, 76, 111, 103, 103, 101, 114, 1, 1, 2, 263, 2,
                    10984, 59348, 58828, 13938, 2, 10986, 35903, 46361, 26129, 257, 756, 2, 10994, 51111,
                    34011, 18705, 266, 25382, 1, 2, 10994, 51111, 34011, 18705, 266, 25286, 1, 2, 263
                ),
                buttons = {
                    {
                        icon = 16,
                        label = gw(5910, 40305, 62583, 30200),
                        id = 8409601,
                        action = { kind = "quest", quest = 82, step = "offer" },
                    },
                    {
                        icon = 17,
                        label = gw(5911, 39617, 59597, 32335),
                        id = 8409602,
                        action = { kind = "close" },
                    },
                },
            },
            { when = { kind = "answer" }, body = gw(5913, 35593, 38685, 5060), buttons = {} },
        },
    },
}
